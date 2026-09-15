import QtQuick
import Quickshell.Hyprland

Item {
  id: controller

  required property var host
  property string focusedEdge: ""
  property var focusedScreen: null
  property int focusRevision: 0
  property var activeSlots: ({ left: 0, right: 0 })
  property bool settingsOpen: false
  property string settingsEdge: "left"
  property string restoreFocusAddress: ""
  property string restoreFocusClass: ""
  property int focusRestoreCount: 0
  property string lastFocusRestore: ""
  property string lastFocusDirection: ""
  property int focusDirectionCount: 0
  property string lastBladeEdge: "left"
  property bool windowsDimmed: false
  property bool dimPending: false
  property bool disposing: false
  readonly property var edges: host.edges
  readonly property string pluginDir: host.pluginDir
  readonly property var service: host.services ? host.services.files : null
  readonly property bool menuOpen: !!(service && service.actionMenuOpen)
  readonly property bool pointerBusy: host.dragActive || host.pressActive || host.pointerHeld || menuOpen
  property string dimRequestId: ""
  property int dimGeneration: 0
  property bool hoverExitPending: false
  property string hoverExitEdge: ""
  property string hoverTargetRequestId: ""
  property int hoverTargetGeneration: 0
  property bool menuBoundaryPending: false
  property string menuBoundaryRequestId: ""
  property int menuBoundaryGeneration: 0
  property string directionRequestId: ""
  property int directionGeneration: 0
  property string activeWindowRequestId: ""
  property int activeWindowGeneration: 0
  property string restoreRequestId: ""
  property int restoreGeneration: 0
  property string emptyFocusRequestId: ""
  property int emptyFocusGeneration: 0
  property bool externalFocusHandoff: false
  readonly property var activeWindow: Hyprland.activeToplevel
  readonly property bool fullscreenHidesFocusedBlade: focusedEdge !== "" && !isWindowMode(focusedEdge)
    && !!activeWindow && Number(activeWindow.lastIpcObject.fullscreen) === 2
    && !!activeWindow.monitor && !!focusedScreen && activeWindow.monitor.name === focusedScreen.name

  onFullscreenHidesFocusedBladeChanged: {
    if (fullscreenHidesFocusedBlade) releaseFocus(focusedEdge)
  }

  function normalizeEdge(value) { return host.normalizeEdge(value) }
  function isOpen(edge) { return host.isOpen(edge) }
  function setOpen(edge, value, persist) { host.setOpen(edge, value, persist) }
  function activeSlot(edge) { return host.activeSlot(edge) }
  function isWindowMode(edge) { return host.isWindowMode(edge) }
  function focusBladeWindow(edge) { return host.focusBladeWindow(edge) }
  function findModule(moduleId) { return host.findModule(moduleId) }
  function slots(edge) { return host.slots(edge) }
  function bladeState(edge) { return host.bladeState(edge) }
  function windowTitle(edge) { return host.windowTitle(edge) }

  function focusBlade(edge, targetScreen, slotIndex, part, openIfClosed) {
    externalFocusHandoff = false
    externalFocusHandoffTimer.stop()
    var target = normalizeEdge(edge)
    var screen = targetScreen || null
    if (!isWindowMode(target) && screen && !isOpen(target) && !host.panelActiveFor(screen, target)) {
      var invocation = host.preferredScreen(target)
      if (!invocation || invocation !== screen) return false
    }
    if (!isWindowMode(target) && screen && isOpen(target) && !host.panelActiveFor(screen, target)) return false
    if (!screen) screen = (focusedScreen && host.panelActiveFor(focusedScreen, target)) ? focusedScreen : host.preferredScreen(target)
    if (!isWindowMode(target) && !screen) return false
    if (!isOpen(target)) {
      if (openIfClosed !== true) return false
      setOpen(target, true, true)
    }
    emptyWorkspaceFocusTimer.stop()
    if (emptyFocusRequestId && service) service.cancelBackendRequest(emptyFocusRequestId, emptyFocusGeneration)
    emptyFocusGeneration++
    emptyFocusRequestId = ""
    if (directionRequestId && service) service.cancelBackendRequest(directionRequestId, directionGeneration)
    directionGeneration++
    directionRequestId = ""
    if (restoreRequestId && service) service.cancelBackendRequest(restoreRequestId, restoreGeneration)
    restoreGeneration++
    restoreRequestId = ""
    if (focusedEdge === "") rememberWorkspaceFocus()
    focusedScreen = screen
    focusRevision++
    cancelHoverExit()
    focusedEdge = target
    lastBladeEdge = target
    var index = Number(slotIndex)
    if (!isFinite(index) || index < 0) index = activeSlot(target)
    if (host.slotCollapsed(target, index)) host.setSlotCollapsed(target, index, false)
    if (isWindowMode(target)) focusBladeWindow(target)
    host.bladeFocusRequested(target, focusedScreen, index, String(part || ""))
    return true
  }

  function focusSlot(edge, slotIndex, targetScreen, part) {
    return focusBlade(edge, targetScreen, slotIndex, part, false)
  }

  function focusModule(moduleId, targetScreen, part) {
    var location = findModule(moduleId)
    if (!location) return false
    var activeTab = host.slotActiveTab(location.edge, location.index)
    if (host.slotModuleAt(location.edge, location.index, activeTab) === String(moduleId)) location.tab = activeTab
    host.setSlotTab(location.edge, location.index, location.tab)
    return focusBlade(location.edge, targetScreen, location.index, part, false)
  }

  function toggleBladeFocus(edge, targetScreen) {
    var target = normalizeEdge(edge)
    if (isOpen(target)) {
      setOpen(target, false, true)
      return "closed"
    }
    return focusBlade(target, targetScreen, -1, "", true) ? "opened" : "no-screen"
  }

  function focusMatches(edge, screen, revision) {
    return focusedEdge === edge && focusedScreen === screen && focusRevision === revision
  }

  function releaseFocus(edge, screen, revision) {
    var target = edge ? normalizeEdge(edge) : focusedEdge
    if (target === "") return false
    if (screen && !focusMatches(target, screen, revision)) return false
    host.bladeFocusReleased(target)
    if (focusedEdge === target) focusedEdge = ""
    return true
  }

  function yieldFocus() {
    var wasFocused = focusedEdge !== ""
    externalFocusHandoff = true
    externalFocusHandoffTimer.restart()
    emptyWorkspaceFocusTimer.stop()
    if (emptyFocusRequestId && service)
      service.cancelBackendRequest(emptyFocusRequestId, emptyFocusGeneration)
    emptyFocusGeneration++
    emptyFocusRequestId = ""
    if (directionRequestId && service)
      service.cancelBackendRequest(directionRequestId, directionGeneration)
    directionGeneration++
    directionRequestId = ""
    if (activeWindowRequestId && service)
      service.cancelBackendRequest(activeWindowRequestId, activeWindowGeneration)
    activeWindowGeneration++
    activeWindowRequestId = ""
    if (restoreRequestId && service)
      service.cancelBackendRequest(restoreRequestId, restoreGeneration)
    restoreGeneration++
    restoreRequestId = ""
    cancelHoverExit()
    cancelMenuBoundary()
    for (var i = 0; i < edges.length; i++) host.bladeFocusReleased(edges[i])
    focusedEdge = ""
    restoreFocusAddress = ""
    restoreFocusClass = ""
    return wasFocused
  }

  function orderedSlots() {
    var result = []
    for (var edgeIndex = 0; edgeIndex < edges.length; edgeIndex++) {
      var edge = edges[edgeIndex]
      if (!isOpen(edge)) continue
      var count = slots(edge).length
      for (var i = 0; i < count; i++) result.push({ edge: edge, index: i })
    }
    return result
  }

  function focusRelativeSlot(edge, slotIndex, delta, targetScreen) {
    var ordered = orderedSlots()
    if (ordered.length === 0) return false
    var target = normalizeEdge(edge)
    var current = -1
    for (var i = 0; i < ordered.length; i++)
      if (ordered[i].edge === target && ordered[i].index === Number(slotIndex)) current = i
    var step = Number(delta) < 0 ? -1 : 1
    var next = current < 0 ? (step > 0 ? 0 : ordered.length - 1) : (current + step + ordered.length) % ordered.length
    return focusBlade(ordered[next].edge, targetScreen, ordered[next].index, "", false)
  }

  function focusExpandedNeighbor(edge, index, targetScreen) {
    var target = normalizeEdge(edge)
    var list = slots(target)
    for (var offset = 1; offset < list.length; offset++) {
      var candidate = (Number(index) + offset) % list.length
      if (!host.slotCollapsed(target, candidate)) return focusBlade(target, targetScreen, candidate, "", false)
    }
    releaseFocus(target)
    restoreWorkspaceFocus()
    return false
  }

  function reportFocus(edge, focused, screen) {
    var target = normalizeEdge(edge)
    if (!isWindowMode(target)) return
    if (focused) {
      if (focusedEdge === "") rememberWorkspaceFocus()
      focusedEdge = target
      if (screen) focusedScreen = screen
    } else if (focusedEdge === target) {
      focusedEdge = ""
    }
  }

  onFocusedEdgeChanged: {
    applyWindowDim()
    if (focusedEdge === "") {
      focusRevision++
      focusedScreen = null
      cancelHoverExit()
    }
  }

  onPointerBusyChanged: if (!pointerBusy && hoverExitPending) hoverExitTimer.restart()
  onMenuOpenChanged: if (!menuOpen) cancelMenuBoundary()

  function applyWindowDim() {
    var desired = focusedEdge !== "" && !isWindowMode(focusedEdge)
    if (desired === windowsDimmed) return
    windowsDimmed = desired
    runWindowDim(desired)
  }

  function runWindowDim(desired) {
    if (disposing || !pluginDir || !service) return
    if (dimRequestId) {
      dimPending = true
      return
    }
    var arguments = [
      "--state", desired ? "on" : "off",
      "--exclude-title", windowTitle("left"),
      "--exclude-title", windowTitle("right")
    ]
    dimGeneration++
    var requestGeneration = dimGeneration
    dimRequestId = service.backendRequest("dim-windows", arguments, requestGeneration, function(response) {
      if (requestGeneration !== controller.dimGeneration) return
      controller.dimRequestId = ""
      if (!controller.dimPending) return
      controller.dimPending = false
      controller.runWindowDim(controller.windowsDimmed)
    })
  }

  Component.onCompleted: {
    runWindowDim(false)
    emptyWorkspaceFocusTimer.restart()
  }

  Component.onDestruction: {
    disposing = true
    dimPending = false
    dimGeneration++
    if (dimRequestId && service) service.cancelBackendRequest(dimRequestId, dimGeneration - 1)
    if (emptyFocusRequestId && service) service.cancelBackendRequest(emptyFocusRequestId, emptyFocusGeneration)
    if (hoverTargetRequestId && service) service.cancelBackendRequest(hoverTargetRequestId, hoverTargetGeneration)
    if (menuBoundaryRequestId && service) service.cancelBackendRequest(menuBoundaryRequestId, menuBoundaryGeneration)
  }

  function emptyWorkspaceEdge() {
    var preferred = normalizeEdge(lastBladeEdge)
    if (isOpen(preferred) && !isWindowMode(preferred)) return preferred
    if (isOpen("left") && !isWindowMode("left")) return "left"
    if (isOpen("right") && !isWindowMode("right")) return "right"
    return ""
  }

  function scheduleEmptyWorkspaceFocus() {
    if (!externalFocusHandoff && focusedEdge === "" && emptyWorkspaceEdge() !== "") emptyWorkspaceFocusTimer.restart()
  }

  function focusEmptyWorkspace() {
    var edge = emptyWorkspaceEdge()
    if (!edge || focusedEdge !== "" || !service || emptyFocusRequestId) return
    var arguments = [
      "--direction", edge === "right" ? "r" : "l",
      "--left", bladeState("left"),
      "--right", bladeState("right"),
      "--empty-only",
      "--left-monitor", host.bladeScreenName("left"),
      "--right-monitor", host.bladeScreenName("right"),
      "--blade-title", windowTitle("left"),
      "--blade-title", windowTitle("right")
    ]
    emptyFocusGeneration++
    var requestGeneration = emptyFocusGeneration
    emptyFocusRequestId = service.backendRequest("focus-direction", arguments, requestGeneration, function(parsed) {
      if (requestGeneration !== controller.emptyFocusGeneration) return
      controller.emptyFocusRequestId = ""
      if (controller.focusedEdge === "" && parsed && parsed.action === "focus-blade")
        controller.focusBlade(parsed.edge, host.screenNamed(parsed.monitor), -1, "", false)
    })
  }

  function handleHyprlandEvent(event) {
    var name = String(event && event.name ? event.name : "")
    if (name === "fullscreen") Hyprland.refreshToplevels()
    if (name === "openwindow" || name === "openwindowv2") {
      externalFocusHandoff = false
      externalFocusHandoffTimer.stop()
    }
    if (name === "closewindow" || name === "workspace" || name === "workspacev2" || name === "focusedmon")
      scheduleEmptyWorkspaceFocus()
  }

  Connections {
    target: host
    function onMonitorModeChanged() { controller.reconcileOwnership() }
    function onMonitorLockChanged() { controller.reconcileOwnership() }
    function onFocusedMonitorNameChanged() { controller.reconcileOwnership() }
  }

  function reconcileOwnership() {
    if (focusedEdge === "" || isWindowMode(focusedEdge) || host.panelActiveFor(focusedScreen, focusedEdge)) return
    if (service && Array.isArray(service.pendingTrashPaths) && service.pendingTrashPaths.length > 0)
      service.resolveTrashConfirmation(false)
    dropOwnership()
  }

  function dropOwnership() {
    var target = focusedEdge
    cancelHoverExit()
    if (hoverTargetRequestId && service) service.cancelBackendRequest(hoverTargetRequestId, hoverTargetGeneration)
    hoverTargetGeneration++
    hoverTargetRequestId = ""
    emptyWorkspaceFocusTimer.stop()
    if (emptyFocusRequestId && service) service.cancelBackendRequest(emptyFocusRequestId, emptyFocusGeneration)
    emptyFocusGeneration++
    emptyFocusRequestId = ""
    if (directionRequestId && service) service.cancelBackendRequest(directionRequestId, directionGeneration)
    directionGeneration++
    directionRequestId = ""
    if (restoreRequestId && service) service.cancelBackendRequest(restoreRequestId, restoreGeneration)
    restoreGeneration++
    restoreRequestId = ""
    focusRevision++
    host.bladeFocusReleased(target)
    focusedEdge = ""
    focusedScreen = null
  }

  Timer {
    id: externalFocusHandoffTimer
    interval: 20000
    onTriggered: { controller.externalFocusHandoff = false; controller.scheduleEmptyWorkspaceFocus() }
  }

  Timer {
    id: emptyWorkspaceFocusTimer
    interval: 120
    onTriggered: controller.focusEmptyWorkspace()
  }

  Connections {
    target: Hyprland
    function onRawEvent(event) { controller.handleHyprlandEvent(event) }
  }

  Connections {
    target: host
    function onLayoutApplied() { controller.scheduleEmptyWorkspaceFocus() }
  }

  BladePointerFocusWatch {
    controller: controller
  }

  function cancelHoverExit() {
    hoverExitPending = false
    hoverExitEdge = ""
    hoverExitTimer.stop()
    if (hoverTargetRequestId && service)
      service.cancelBackendRequest(hoverTargetRequestId, hoverTargetGeneration)
    hoverTargetGeneration++
    hoverTargetRequestId = ""
  }

  function bladePointerEntered(edge) {
    var target = normalizeEdge(edge)
    if (hoverExitEdge === target || focusedEdge === target) cancelHoverExit()
  }

  function bladePointerExited(edge, screen) {
    var target = normalizeEdge(edge)
    if (focusedEdge !== target || focusedScreen !== screen || isWindowMode(target)) return
    hoverExitPending = true
    hoverExitEdge = target
    if (!pointerBusy) hoverExitTimer.restart()
  }

  function resolveHoverExit() {
    var target = hoverExitEdge
    if (!hoverExitPending || pointerBusy || focusedEdge !== target || isWindowMode(target) || !service)
      return
    hoverExitPending = false
    var arguments = [
      "--blade-title", windowTitle("left"),
      "--blade-title", windowTitle("right")
    ]
    hoverTargetGeneration++
    var requestGeneration = hoverTargetGeneration
    hoverTargetRequestId = service.backendRequest("hover-target", arguments, requestGeneration, function(parsed) {
      if (requestGeneration !== controller.hoverTargetGeneration) return
      controller.hoverTargetRequestId = ""
      if (controller.pointerBusy) {
        controller.hoverExitPending = true
        controller.hoverExitEdge = target
        return
      }
      if (!parsed || parsed.ok === false || controller.focusedEdge !== target) return
      controller.releaseFocus(target)
      if (parsed.action === "hover-focus") {
        controller.restoreFocusAddress = String(parsed.address || "")
        controller.restoreFocusClass = ""
      }
      controller.restoreWorkspaceFocus()
    })
  }

  function cancelMenuBoundary() {
    menuBoundaryPending = false
    if (menuBoundaryRequestId && service)
      service.cancelBackendRequest(menuBoundaryRequestId, menuBoundaryGeneration)
    menuBoundaryGeneration++
    menuBoundaryRequestId = ""
  }

  function actionMenuPointerExited() {
    if (!menuOpen || !service) return
    if (menuBoundaryRequestId) {
      menuBoundaryPending = true
      return
    }
    var arguments = [
      "--blade-title", windowTitle("left"),
      "--blade-title", windowTitle("right")
    ]
    menuBoundaryGeneration++
    var requestGeneration = menuBoundaryGeneration
    menuBoundaryRequestId = service.backendRequest("hover-target", arguments, requestGeneration, function(parsed) {
      if (requestGeneration !== controller.menuBoundaryGeneration) return
      controller.menuBoundaryRequestId = ""
      if (parsed && parsed.ok && controller.menuOpen
          && !controller.actionMenuLayerContains(parsed.x, parsed.y))
        controller.service.closeActionMenu()
      else if (controller.menuBoundaryPending && controller.menuOpen) {
        controller.menuBoundaryPending = false
        controller.actionMenuPointerExited()
      }
    })
  }

  function actionMenuLayerContains(x, y) {
    if (!service || !service.actionMenuScreen) return true
    var screen = service.actionMenuScreen
    var px = Number(x)
    var py = Number(y)
    var left = Number(screen.x) || 0
    var top = Number(screen.y) || 0
    var width = Number(screen.width) || 0
    var height = Number(screen.height) || 0
    var layerWidth = host.maximumWidth(width)
    if (!isFinite(px) || !isFinite(py) || py < top || py >= top + height) return false
    return service.actionMenuOpenLeft
      ? px >= left + width - layerWidth && px < left + width
      : px >= left && px < left + layerWidth
  }

  Timer {
    id: hoverExitTimer
    interval: 16
    onTriggered: controller.resolveHoverExit()
  }

  function horizontalDirection(direction) {
    var raw = String(direction || "").toLowerCase().charAt(0)
    if (raw === "r" || raw === "u" || raw === "d") return raw
    return "l"
  }

  function releaseTowardWorkspace(wanted) {
    if (focusedEdge === "" || isWindowMode(focusedEdge)) return ""
    var towardWorkspace = focusedEdge === "left" ? wanted === "r" : wanted === "l"
    if (!towardWorkspace) return "blocked"
    var source = focusedEdge
    releaseFocus(source)
    return source
  }

  function focusDirection(direction) {
    var wanted = horizontalDirection(direction)
    if (!pluginDir || !service) return "no-plugin-dir"
    if (wanted === "u" || wanted === "d") return focusVertical(wanted)
    var fromBlade = releaseTowardWorkspace(wanted)
    if (fromBlade === "blocked") {
      lastFocusDirection = "none: nothing beyond the " + focusedEdge + " blade"
      return "none"
    }
    if (directionRequestId) {
      service.cancelBackendRequest(directionRequestId, directionGeneration)
      directionGeneration++
      directionRequestId = ""
    }
    var arguments = [
      "--direction", wanted,
      "--left", bladeState("left"),
      "--right", bladeState("right"),
      "--from-blade", fromBlade,
      "--left-monitor", host.bladeScreenName("left"),
      "--right-monitor", host.bladeScreenName("right"),
      "--blade-title", windowTitle("left"),
      "--blade-title", windowTitle("right")
    ]
    directionGeneration++
    var requestGeneration = directionGeneration
    directionRequestId = service.backendRequest("focus-direction", arguments, requestGeneration, function(parsed) {
      if (requestGeneration !== controller.directionGeneration) return
      controller.directionRequestId = ""
      if (!parsed) {
        host.lastFocusDirection = "unreadable response"
        return
      }
      host.lastFocusDirection = String(parsed.action || "") + (parsed.reason ? ": " + parsed.reason : "") + (parsed.error ? ": " + parsed.error : "")
      if (parsed.action === "focus-blade") host.focusBlade(parsed.edge, host.screenNamed(parsed.monitor), -1, "", false)
    })
    focusDirectionCount++
    return fromBlade !== "" ? "released" : "routing"
  }

  function focusVertical(wanted) {
    if (focusedEdge === "" || isWindowMode(focusedEdge)) {
      host.dispatchWindow(["--action", "focus", "--direction", wanted])
      lastFocusDirection = "dispatched: " + wanted
      return "dispatched"
    }
    var edge = focusedEdge
    var next = activeSlot(edge) + (wanted === "d" ? 1 : -1)
    if (next < 0 || next >= slots(edge).length) {
      lastFocusDirection = "none: no slot " + (wanted === "d" ? "below" : "above")
      return "none"
    }
    focusBlade(edge, null, next, "", false)
    return "slot-focused"
  }

  function rememberWorkspaceFocus() {
    if (!pluginDir || !service || activeWindowRequestId) return
    activeWindowGeneration++
    var requestGeneration = activeWindowGeneration
    activeWindowRequestId = service.backendRequest("active-window", [], requestGeneration, function(parsed) {
      if (requestGeneration !== controller.activeWindowGeneration) return
      controller.activeWindowRequestId = ""
      if (parsed && parsed.ok && parsed.address && !host.isBladeWindowTitle(parsed.title)) {
        host.restoreFocusAddress = String(parsed.address)
        host.restoreFocusClass = String(parsed["class"] || "")
      }
    })
  }

  function restoreWorkspaceFocus() {
    var address = restoreFocusAddress
    restoreFocusAddress = ""
    restoreFocusClass = ""
    if (!address || !pluginDir || !service) return false
    if (restoreRequestId) service.cancelBackendRequest(restoreRequestId, restoreGeneration)
    restoreGeneration++
    var requestGeneration = restoreGeneration
    restoreRequestId = service.backendRequest("focus-window", ["--address", address], requestGeneration, function(parsed) {
      if (requestGeneration !== controller.restoreGeneration) return
      controller.restoreRequestId = ""
      host.focusRestoreCount++
      host.lastFocusRestore = parsed && parsed.focused
        ? "focused " + String(parsed.address || "")
        : String(parsed && (parsed.reason || parsed.error) || "no response")
    })
    return true
  }

  function isBladeWindowTitle(title) {
    var value = String(title || "")
    return value === windowTitle("left") || value === windowTitle("right")
  }

  function reportSlotFocus(edge, slotIndex, focused) {
    if (!focused) return
    var target = normalizeEdge(edge)
    var next = ({})
    var keys = Object.keys(activeSlots)
    for (var i = 0; i < keys.length; i++) next[keys[i]] = activeSlots[keys[i]]
    next[target] = Math.max(0, Number(slotIndex) || 0)
    activeSlots = next
  }

  function setSettingsOpen(value, edge) {
    settingsOpen = !!value
    if (edge) settingsEdge = normalizeEdge(edge)
  }

  function toggleSettings(edge) {
    var target = normalizeEdge(edge || settingsEdge)
    if (settingsOpen && settingsEdge === target) settingsOpen = false
    else setSettingsOpen(true, target)
    return settingsOpen
  }

}
