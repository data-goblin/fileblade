import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import "../theme"

Item {
  id: host

  property var shell: null
  property var pluginRegistry: null
  property var catalogProviders: []
  property var providerErrors: ({})
  property string pluginDir: ""
  property var config: ({})
  property var services: ({})
  readonly property bool welcomePending: !!(services && services.files) && String(services.files.welcomeState || "") === ""
  property var updates: null
  property var legacyDefaults: ({})

  readonly property string home: Quickshell.env("HOME") || "/"
  readonly property string configHome: Quickshell.env("XDG_CONFIG_HOME") || (home + "/.config")
  readonly property string configDir: configHome + "/omarchy/fileblade"
  readonly property string layoutPath: configDir + "/blades.json"
  readonly property var edges: ["left", "right"]
  readonly property int minimumWidth: 280
  readonly property int minimumSlotHeight: Style.space(116)
  readonly property int collapsedSlotHeight: Style.space(32)
  readonly property int tabBarHeight: Style.space(32)
  readonly property int slotHandleSize: Style.space(6)
  readonly property int tabEdgeZone: Style.space(10)
  readonly property int edgeOpenZone: Style.space(56)
  readonly property int dragThreshold: 6
  readonly property int moduleContractVersion: 3

  property alias registry: registry
  property alias layout: persisted.layout
  property alias monitorMode: persisted.monitorMode
  property alias monitorLock: persisted.monitorLock
  property alias animateBlades: persisted.animateBlades
  property alias fontScale: persisted.fontScale
  property bool layoutReady: false
  property bool pressActive: false
  property bool pointerHeld: false
  property alias focusedEdge: focusController.focusedEdge
  property alias focusedScreen: focusController.focusedScreen
  readonly property alias focusRevision: focusController.focusRevision
  property alias activeSlots: focusController.activeSlots
  property alias settingsOpen: focusController.settingsOpen
  property alias settingsEdge: focusController.settingsEdge
  property int frameWidth: 2
  property bool animationsExplicit: false
  readonly property string cliPath: pluginDir + "/fileblade"
  readonly property var service: services ? services.files : null
  property alias restoreFocusAddress: focusController.restoreFocusAddress
  property alias restoreFocusClass: focusController.restoreFocusClass
  property alias externalFocusHandoff: focusController.externalFocusHandoff
  property alias focusRestoreCount: focusController.focusRestoreCount
  property alias lastFocusRestore: focusController.lastFocusRestore
  property string lastSavedLayoutText: ""
  property string lastWrittenLayoutText: ""
  property var pendingOpenEdges: null
  property int layoutRevision: 0
  property var windowAddresses: ({ left: "", right: "" })
  property string lastWindowPlacement: ""
  property string pendingPlacementEdge: ""
  property bool layoutReadQueued: false
  property bool layoutWritable: false
  property bool layoutRereadPending: false
  property alias lastFocusDirection: focusController.lastFocusDirection
  property alias focusDirectionCount: focusController.focusDirectionCount
  property string placementRequestId: ""
  property int placementGeneration: 0
  property string windowFocusRequestId: ""
  property int windowFocusGeneration: 0
  property int focusEpoch: 0
  property string windowDispatchRequestId: ""
  property int windowDispatchGeneration: 0
  property string layoutReadRequestId: ""
  property int layoutReadGeneration: 0
  property string layoutWriteRequestId: ""
  property string queuedLayoutDocument: ""
  property int layoutWriteGeneration: 0

  readonly property var leftBlade: bladeFor("left")
  readonly property var rightBlade: bladeFor("right")
  readonly property bool anyOpen: leftBlade.open || rightBlade.open
  signal bladeFocusRequested(string edge, var targetScreen, int slotIndex, string part)
  signal bladeFocusReleased(string edge)
  signal layoutApplied()
  signal bladeOpened(string edge)

  BladeRegistry {
    id: registry
    socketId: "data-goblin.fileblade/blade"
    pluginDir: host.pluginDir
    userModulesDir: host.configDir + "/modules"
    pluginRegistry: host.pluginRegistry
    catalogProviders: host.catalogProviders
    service: host.services ? host.services.files : null
    contractVersion: host.moduleContractVersion
  }

  PersistentProperties {
    id: persisted
    reloadableId: "kurt-filetree-blades"
    property bool hydrated: false
    property var layout: ({
      left: { open: false, width: 380, mode: "docked", slots: [] },
      right: { open: false, width: 360, mode: "docked", slots: [] }
    })
    property string monitorMode: "active"
    property string monitorLock: ""
    property bool animateBlades: true
    property real fontScale: 1.0
  }

  Binding {
    target: Typography
    property: "scale"
    value: host.fontScale
  }

  BladeFocusController {
    id: focusController
    host: host
  }

  BladeLayout {
    id: bladeLayout
    host: host
  }

  BladeDragController {
    id: dragController
    host: host
  }

  BladeTabs {
    id: tabController
    host: host
  }

  BladeModuleDirs {
    id: moduleDirs
    host: host
    service: host.service
  }
  property alias dirs: moduleDirs

  function detachTab(next, edge, slotIndex, tabIndex) { return tabController.detachTab(next, edge, slotIndex, tabIndex) }
  function moveTabInto(sourceEdge, sourceIndex, tabIndex, targetEdge, targetSlotIndex, insertAt) { return tabController.moveTabInto(sourceEdge, sourceIndex, tabIndex, targetEdge, targetSlotIndex, insertAt) }
  function sendTabAcross(edge, slotIndex, tabIndex, targetScreen) { return tabController.sendTabAcross(edge, slotIndex, tabIndex, targetScreen) }
  function setSlotTab(edge, slotIndex, tabIndex) { return tabController.setSlotTab(edge, slotIndex, tabIndex) }
  function cycleSlotTab(edge, slotIndex, delta) { return tabController.cycleSlotTab(edge, slotIndex, delta) }
  function removeTab(edge, slotIndex, tabIndex) { return tabController.removeTab(edge, slotIndex, tabIndex) }
  function addTab(edge, slotIndex, moduleId, state) { return tabController.addTab(edge, slotIndex, moduleId, state) }

  function tabSeedState(moduleId) { return tabController.tabSeedState(moduleId) }
  function tabTitle(edge, slotIndex, tabIndex) { return tabController.tabTitle(edge, slotIndex, tabIndex) }

  property alias dragActive: dragController.dragActive
  property alias dragEdge: dragController.dragEdge
  property alias dragIndex: dragController.dragIndex
  property alias dragSlotId: dragController.dragSlotId
  property alias dragTitle: dragController.dragTitle
  property alias dragGlyph: dragController.dragGlyph
  property alias dragScope: dragController.dragScope
  property alias dragScreen: dragController.dragScreen
  property alias dragScreenX: dragController.dragScreenX
  property alias dragScreenY: dragController.dragScreenY
  property alias dragLocalY: dragController.dragLocalY
  property alias dragStackHeight: dragController.dragStackHeight
  property alias dropEdge: dragController.dropEdge
  property alias dropIndex: dragController.dropIndex
  property alias dragTab: dragController.dragTab
  property alias dropTabSlot: dragController.dropTabSlot
  property alias dropTabBand: dragController.dropTabBand
  property alias dropTabIndex: dragController.dropTabIndex
  property alias dropNoop: dragController.dropNoop
  property alias dropLabel: dragController.dropLabel
  property alias dragAutoOpened: dragController.dragAutoOpened
  function beginSlotDrag(edge, slotIndex, targetScreen, scope, screenX, screenY, localY, stackHeight, tabIndex) {
    return dragController.beginSlotDrag(edge, slotIndex, targetScreen, scope, screenX, screenY, localY, stackHeight, tabIndex)
  }
  function updateSlotDrag(screenX, screenY, localY) { dragController.updateSlotDrag(screenX, screenY, localY) }
  function endSlotDrag(commit) { return dragController.endSlotDrag(commit) }
  function setDropTabIndex(index) { dragController.setDropTabIndex(index) }
  function dropIndexAt(edge, localY, stackHeight) { return dragController.dropIndexAt(edge, localY, stackHeight) }
  function tabDropSlotAt(edge, localY, stackHeight) { return dragController.tabDropSlotAt(edge, localY, stackHeight) }

  function normalizeEdge(value) { return bladeLayout.normalizeEdge(value) }
  function normalizeMonitorMode(value) { return bladeLayout.normalizeMonitorMode(value) }
  function normalizeMode(value) { return bladeLayout.normalizeMode(value) }
  function clampNumber(value, fallback, minimum, maximum) { return bladeLayout.clampNumber(value, fallback, minimum, maximum) }
  function emptyBlade(edge) { return bladeLayout.emptyBlade(edge) }
  function slotIdFor(module, index, taken) { return bladeLayout.slotIdFor(module, index, taken) }
  function normalizeSlot(raw, index, taken) { return bladeLayout.normalizeSlot(raw, index, taken) }
  function normalizeBlade(raw, edge) { return bladeLayout.normalizeBlade(raw, edge) }
  function normalizeLayout(raw) { return bladeLayout.normalizeLayout(raw) }
  function cloneLayout(value) { return bladeLayout.cloneLayout(value) }
  function normalizePlacement(value) { return bladeLayout.normalizePlacement(value) }
  function legacyLayout(legacy) { return bladeLayout.legacyLayout(legacy) }
  function defaultLayout() { return bladeLayout.defaultLayout() }
  function resetLayout() {
    fontScale = Typography.clamp(config.fontScale)
    applyLayout(defaultLayout(), config.monitorMode, true, config.animateBlades)
    return true
  }
  function revertDefaults() {
    if (service && typeof service.resetSettings === "function") service.resetSettings()
    return resetLayout()
  }
  function bladeFor(edge) { return bladeLayout.bladeFor(edge) }
  function isOpen(edge) { return bladeLayout.isOpen(edge) }
  function bladeMode(edge) { return bladeLayout.bladeMode(edge) }
  function isWindowMode(edge) { return bladeLayout.isWindowMode(edge) }
  function isDockedOpen(edge) { return bladeLayout.isDockedOpen(edge) }
  function bladeState(edge) { return bladeLayout.bladeState(edge) }
  function windowTitle(edge) { return bladeLayout.windowTitle(edge) }
  function windowAddress(edge) { return bladeLayout.windowAddress(edge) }
  function bladeWidth(edge) { return bladeLayout.bladeWidth(edge) }
  function slots(edge) { return bladeLayout.slots(edge) }
  function slotAt(edge, index) { return bladeLayout.slotAt(edge, index) }
  function slotCollapsed(edge, index) { return bladeLayout.slotCollapsed(edge, index) }
  function findModule(moduleId) { return bladeLayout.findModule(moduleId) }
  function findSlotId(edge, slotId) { return bladeLayout.findSlotId(edge, slotId) }
  function normalizedFractions(edge) { return bladeLayout.normalizedFractions(edge) }
  function slotGeometry(edge, stackHeight) { return bladeLayout.slotGeometry(edge, stackHeight) }
  function slotTops(edge, stackHeight) { return bladeLayout.slotTops(edge, stackHeight) }
  function slotTitle(edge, index) { return bladeLayout.slotTitle(edge, index) }
  function moduleTitle(moduleId) { return bladeLayout.moduleTitle(moduleId) }
  function slotTabs(edge, index) { return bladeLayout.slotTabs(edge, index) }
  function slotActiveTab(edge, index) { return bladeLayout.slotActiveTab(edge, index) }
  function slotModuleAt(edge, index, tabIndex) { return bladeLayout.slotModuleAt(edge, index, tabIndex) }
  function propertiesPlacement() { return bladeLayout.propertiesPlacement() }
  function layoutDocument() { return bladeLayout.layoutDocument() }
  function activeSlot(edge) { return bladeLayout.activeSlot(edge) }
  function panelActiveFor(panelScreen, edge) { return bladeLayout.panelActiveFor(panelScreen, edge) }

  function screenAlive(candidate) {
    if (!candidate) return true
    for (var i = 0; i < Quickshell.screens.length; i++) if (Quickshell.screens[i] === candidate) return true
    return false
  }

  function validateScreenOwners() {
    if (!screenAlive(focusController.focusedScreen)) focusController.dropOwnership()
    if (dragActive && !screenAlive(dragController.dragScreen)) endSlotDrag(false)
    if (!service) return
    if (service.actionMenuOpen && !screenAlive(service.actionMenuScreen)) service.closeActionMenu()
    if (service.dropWheelOpen && !screenAlive(service.dropWheel.wheelScreen)) service.dropWheel.close()
      if (service.dropWheel.dragActive && !screenAlive(service.dropWheel.dragScreen)) service.dropWheel.cancelDrag()
  }

  Connections {
    target: Quickshell
    function onScreensChanged() { host.validateScreenOwners() }
  }
  function screenNamed(name) { return bladeLayout.screenNamed(name) }
  function preferredScreen(edge) { return bladeLayout.preferredScreen(edge) }
  function referenceScreen(candidate, edge) { return bladeLayout.referenceScreen(candidate, edge) }
  function bladeScreenName(edge) { return bladeLayout.isOpen(edge) ? bladeLayout.openedOnFor(edge) : "" }
  readonly property string focusedMonitorName: bladeLayout.focusedMonitorName
  readonly property var screenNames: bladeLayout.screenNames
  function configuredEdges() { return bladeLayout.configuredEdges() }
  function validIndex(index, length) { return bladeLayout.validIndex(index, length) }
  function validMoveTab(tabIndex, length) { return bladeLayout.validMoveTab(tabIndex, length) }
  function wholeSlotMove(tabIndex, length) { return bladeLayout.wholeSlotMove(tabIndex, length) }
  function targetSlotIndex(value, length) { return bladeLayout.targetSlotIndex(value, length) }

  function replaceLayout(next, persist) {
    layout = normalizeLayout(next)
    layoutRevision++
    if (persist !== false) scheduleSave()
  }

  function updateBlade(edge, mutator, persist) {
    var target = normalizeEdge(edge)
    var next = cloneLayout(layout)
    if (!next[target]) next[target] = emptyBlade(target)
    mutator(next[target], next)
    replaceLayout(next, persist)
  }

  function setOpen(edge, value, persist) {
    var desired = !!value
    var target = normalizeEdge(edge)
    if (isOpen(target) === desired) return desired
    if (!desired && focusedEdge === target) restoreWorkspaceFocus()
    if (desired) bladeLayout.noteOpened(target)
    updateBlade(target, function(blade) { blade.open = desired }, persist)
    if (desired) bladeOpened(target)
    if (!desired) bladeLayout.noteClosed(target)
    if (!desired && focusedEdge === target) focusedEdge = ""
    if (!desired && settingsOpen && settingsEdge === target) settingsOpen = false
    return desired
  }

  function setAllOpen(value) {
    var desired = !!value
    var next = cloneLayout(layout)
    var changed = false
    var targets = desired ? configuredEdges() : edges
    for (var i = 0; i < targets.length; i++) {
      var edge = targets[i]
      if (!next[edge]) next[edge] = emptyBlade(edge)
      if (!!next[edge].open !== desired) {
        next[edge].open = desired
        changed = true
        if (desired) bladeOpened(edge)
      }
    }
    if (!desired) {
      if (focusedEdge) restoreWorkspaceFocus()
      focusedEdge = ""
      settingsOpen = false
    }
    if (changed) replaceLayout(next, true)
    return desired
  }

  function toggleAll() {
    return setAllOpen(!anyOpen)
  }

  function toggleOpen(edge) {
    return setOpen(edge, !isOpen(edge), true)
  }

  function setMode(edge, value) {
    var target = normalizeEdge(edge)
    var desired = normalizeMode(value)
    if (bladeMode(target) === desired) return desired
    if (focusedEdge === target) focusedEdge = ""
    updateBlade(target, function(blade) { blade.mode = desired }, true)
    if (desired === "docked") setWindowAddress(target, "")
    return desired
  }

  function undock(edge) {
    var target = normalizeEdge(edge)
    setMode(target, "window")
    setOpen(target, true, true)
    return "window"
  }

  function dock(edge) {
    return setMode(edge, "docked")
  }

  function redock(edge, targetScreen) {
    var target = normalizeEdge(edge)
    var epoch = focusEpoch
    dock(target)
    Qt.callLater(function() { if (epoch === host.focusEpoch) host.focusBlade(target, targetScreen || null, -1, "", true) })
    return "docked"
  }

  function toggleDock(edge) {
    return isWindowMode(edge) ? dock(edge) : undock(edge)
  }

  function setWindowAddress(edge, address) {
    var next = ({})
    var keys = Object.keys(windowAddresses)
    for (var i = 0; i < keys.length; i++) next[keys[i]] = windowAddresses[keys[i]]
    next[normalizeEdge(edge)] = String(address || "")
    windowAddresses = next
  }

  function maximumWidth(screenWidth) {
    return Math.max(320, Math.min(1600, Number(screenWidth || 0) * 0.72))
  }

  function setWidth(edge, value, screenWidth, persist) {
    var target = normalizeEdge(edge)
    var next = Math.round(Math.max(minimumWidth, Math.min(maximumWidth(screenWidth), Number(value) || bladeWidth(target))))
    if (next === bladeWidth(target)) return next
    updateBlade(target, function(blade) { blade.width = next }, persist)
    return next
  }

  function setSlots(edge, list) {
    var target = normalizeEdge(edge)
    var incoming = Array.isArray(list) ? list : []
    updateBlade(target, function(blade) { blade.slots = incoming }, true)
    return slots(target).length
  }

  function setSlotModule(edge, index, moduleId) {
    var target = normalizeEdge(edge)
    var module = String(moduleId || "").trim()
    if (!module || !registry.module(module)) return false
    var slotIndex = Number(index)
    if (!validIndex(slotIndex, slots(target).length)) return false
    if (slotModuleAt(target, slotIndex, -1) === module) return true
    if (registry.module(module).singleton && findModule(module)) return false
    updateBlade(target, function(blade) {
      var slot = blade.slots[slotIndex]
      slot.modules[Math.max(0, Math.min(slot.modules.length - 1, Number(slot.active) || 0))] = { module: module, state: {} }
    }, true)
    return true
  }

  function newSlot(module, fraction) {
    return { modules: [{ module: module, state: {} }], active: 0, collapsed: false, fraction: fraction === undefined ? -1 : fraction }
  }

  function setSlotCollapsed(edge, index, value) { return bladeLayout.setSlotCollapsed(edge, index, value) }
  function toggleSlotCollapsed(edge, index) { return bladeLayout.toggleSlotCollapsed(edge, index) }

  function addSlot(edge, moduleId, index) {
    var target = normalizeEdge(edge)
    var module = String(moduleId || "").trim()
    if (!module || !registry.module(module)) return false
    var entry = registry.module(module)
    if (entry.singleton && findModule(module)) return false
    updateBlade(target, function(blade) {
      var slot = newSlot(module)
      var position = Number(index)
      if (!isFinite(position) || Math.floor(position) !== position || position < 0 || position > blade.slots.length) blade.slots.push(slot)
      else blade.slots.splice(position, 0, slot)
    }, true)
    return true
  }

  function removeSlot(edge, index) {
    var target = normalizeEdge(edge)
    var slotIndex = Number(index)
    if (!validIndex(slotIndex, slots(target).length)) return false
    updateBlade(target, function(blade) { blade.slots.splice(slotIndex, 1) }, true)
    return true
  }

  function moveSlot(edge, index, delta) {
    var target = normalizeEdge(edge)
    var list = slots(target)
    var from = Number(index)
    var to = from + Number(delta)
    if (!validIndex(from, list.length) || !validIndex(to, list.length)) return false
    updateBlade(target, function(blade) {
      var moved = blade.slots.splice(from, 1)[0]
      blade.slots.splice(to, 0, moved)
    }, true)
    return true
  }

  function moveModule(moduleId, edge, index) {
    var location = findModule(moduleId)
    var target = normalizeEdge(edge)
    var next = cloneLayout(layout)
    var slot = location
      ? detachTab(next, location.edge, location.index, location.tab)
      : newSlot(String(moduleId))
    if (!registry.module(slot.modules[0].module)) return false
    if (!next[target]) next[target] = emptyBlade(target)
    var position = Number(index)
    if (!isFinite(position) || Math.floor(position) !== position || position < 0 || position > next[target].slots.length) next[target].slots.push(slot)
    else next[target].slots.splice(position, 0, slot)
    replaceLayout(next, true)
    return true
  }

  function moveSlotTo(sourceEdge, sourceIndex, targetEdge, targetIndex, tabIndex) {
    var source = normalizeEdge(sourceEdge)
    var target = normalizeEdge(targetEdge)
    var from = Number(sourceIndex)
    var list = slots(source)
    if (!validIndex(from, list.length)) return false
    if (!validMoveTab(tabIndex, list[from].modules.length)) return false
    var next = cloneLayout(layout)
    if (!next[target]) next[target] = emptyBlade(target)
    var whole = wholeSlotMove(tabIndex, next[source].slots[from].modules.length)
    var position = targetSlotIndex(targetIndex, next[target].slots.length)
    if (source === target && whole && position > from) position--
    if (source === target && whole && position === from) return false
    var moved = detachTab(next, source, from, tabIndex)
    position = Math.min(position, next[target].slots.length)
    next[target].slots.splice(position, 0, moved)
    if (next[source].slots.length === 0) next[source].open = false
    replaceLayout(next, true)
    return true
  }

  function setSlotFractions(edge, fractions) {
    var target = normalizeEdge(edge)
    if (!Array.isArray(fractions)) return
    updateBlade(target, function(blade) {
      for (var i = 0; i < blade.slots.length && i < fractions.length; i++) {
        var value = Number(fractions[i])
        blade.slots[i].fraction = isFinite(value) && value > 0 && value < 1 ? Math.round(value * 1000) / 1000 : -1
      }
    }, true)
  }

  function slotState(edge, slotId) {
    var index = findSlotId(edge, slotId)
    var slot = index >= 0 ? slots(edge)[index] : null
    if (!slot || !Array.isArray(slot.modules) || slot.modules.length === 0) return ({})
    var active = Math.max(0, Math.min(slot.modules.length - 1, Number(slot.active) || 0))
    return slot.modules[active].state || ({})
  }

  function setSlotStateValue(edge, slotId, key, value) {
    return bladeLayout.setSlotStateValue(edge, slotId, key, value)
  }

  function setTabStateValue(edge, slotIndex, tabIndex, key, value) {
    return bladeLayout.setTabStateValue(edge, slotIndex, tabIndex, key, value)
  }

  function detachModule(next, moduleId) {
    for (var edgeIndex = 0; edgeIndex < edges.length; edgeIndex++) {
      var edge = edges[edgeIndex]
      if (!next[edge]) next[edge] = emptyBlade(edge)
      for (var slotIndex = 0; slotIndex < next[edge].slots.length; slotIndex++) {
        var tabs = next[edge].slots[slotIndex].modules
        for (var tabIndex = 0; tabIndex < tabs.length; tabIndex++)
          if (String(tabs[tabIndex].module) === moduleId) return detachTab(next, edge, slotIndex, tabIndex)
      }
    }
    return null
  }

  function placeClassicSlots(next, placement, filesSlot, propertiesSlot) {
    if (placement === "right") {
      next.left.slots.unshift(filesSlot)
      propertiesSlot.fraction = -1
      next.right.slots.unshift(propertiesSlot)
      next.right.open = next.left.open
      return
    }
    next.left.slots.unshift(filesSlot)
    if (placement !== "none") next.left.slots.splice(placement === "above" ? 0 : 1, 0, propertiesSlot)
  }

  function closeEmptyBlades(next) {
    for (var i = 0; i < edges.length; i++)
      if (next[edges[i]].slots.length === 0) next[edges[i]].open = false
  }

  function applyPlacement(value) {
    var placement = normalizePlacement(value)
    var next = cloneLayout(layout)
    var propertiesSlot = detachModule(next, "properties") || newSlot("properties", 0.34)
    var filesSlot = detachModule(next, "files") || newSlot("files")
    if (propertiesSlot.fraction === undefined || propertiesSlot.fraction < 0) propertiesSlot.fraction = 0.34
    filesSlot.fraction = -1
    placeClassicSlots(next, placement, filesSlot, propertiesSlot)
    closeEmptyBlades(next)
    replaceLayout(next, true)
    return placement
  }

  function setMonitorMode(value, lock) {
    var mode = normalizeMonitorMode(value)
    var wanted = String(lock || "")
    if (mode === "locked" && wanted === "" && String(value || "").toLowerCase() === "primary") wanted = bladeLayout.primaryScreenName
    if (mode === "locked" && wanted === "") wanted = monitorLock
    if (mode === "locked" && !screenNamed(wanted)) return "unknown-monitor"
    monitorMode = mode
    monitorLock = mode === "locked" ? wanted : ""
    scheduleSave()
    return monitorMode
  }

  function setAnimateBlades(value) {
    animateBlades = !!value
    animationsExplicit = true
    scheduleSave()
    return animateBlades
  }

  function setFontScale(value) {
    fontScale = Typography.clamp(value)
    scheduleSave()
    return fontScale
  }

  function focusBlade(edge, targetScreen, slotIndex, part, openIfClosed) { return focusController.focusBlade(edge, targetScreen, slotIndex, part, openIfClosed) }
  function focusSlot(edge, slotIndex, targetScreen, part) { return focusController.focusSlot(edge, slotIndex, targetScreen, part) }
  function focusModule(moduleId, targetScreen, part) { return focusController.focusModule(moduleId, targetScreen, part) }
  function toggleBladeFocus(edge, targetScreen) { return focusController.toggleBladeFocus(edge, targetScreen) }
  function focusMatches(edge, screen, revision) { return focusController.focusMatches(edge, screen, revision) }
  function releaseFocus(edge, screen, revision) { return focusController.releaseFocus(edge, screen, revision) }
  function yieldFocus() {
    focusEpoch++
    if (windowFocusRequestId && service)
      service.cancelBackendRequest(windowFocusRequestId, windowFocusGeneration)
    windowFocusGeneration++
    windowFocusRequestId = ""
    if (placementRequestId && service)
      service.cancelBackendRequest(placementRequestId, placementGeneration)
    placementGeneration++
    placementRequestId = ""
    pendingPlacementEdge = ""
    return focusController.yieldFocus()
  }
  function bladePointerEntered(edge) { focusController.bladePointerEntered(edge) }
  function bladePointerExited(edge, screen) { focusController.bladePointerExited(edge, screen) }
  function actionMenuPointerExited() { focusController.actionMenuPointerExited() }
  function orderedSlots() { return focusController.orderedSlots() }
  function focusRelativeSlot(edge, slotIndex, delta, targetScreen) { return focusController.focusRelativeSlot(edge, slotIndex, delta, targetScreen) }
  function focusExpandedNeighbor(edge, index, targetScreen) { return focusController.focusExpandedNeighbor(edge, index, targetScreen) }
  function reportFocus(edge, focused, screen) { focusController.reportFocus(edge, focused, screen) }
  function focusDirection(direction) { return focusController.focusDirection(direction) }
  function rememberWorkspaceFocus() { focusController.rememberWorkspaceFocus() }
  function restoreWorkspaceFocus() { return focusController.restoreWorkspaceFocus() }
  function isBladeWindowTitle(title) { return focusController.isBladeWindowTitle(title) }
  function reportSlotFocus(edge, slotIndex, focused) { focusController.reportSlotFocus(edge, slotIndex, focused) }
  function setSettingsOpen(value, edge) { focusController.setSettingsOpen(value, edge) }
  function toggleSettings(edge) { return focusController.toggleSettings(edge) }

  function placeBladeWindow(edge) {
    var target = normalizeEdge(edge)
    if (!pluginDir || !service) return
    if (placementRequestId) {
      pendingPlacementEdge = target
      return
    }
    var arguments = [
      "--title", windowTitle(target),
      "--edge", target,
      "--width", String(bladeWidth(target)),
      "--timeout", "4"
    ]
    placementGeneration++
    var requestGeneration = placementGeneration
    placementRequestId = service.backendRequest("place-blade-window", arguments, requestGeneration, function(parsed) {
      if (requestGeneration !== host.placementGeneration) return
      host.placementRequestId = ""
      if (parsed && parsed.ok) {
        host.setWindowAddress(parsed.edge, parsed.address)
        host.lastWindowPlacement = String(parsed.edge) + " " + String(parsed.address) + " at " + JSON.stringify(parsed.at) + " size " + JSON.stringify(parsed.size)
      } else {
        host.lastWindowPlacement = String(parsed && parsed.error || "no response")
      }
      var pending = host.pendingPlacementEdge
      host.pendingPlacementEdge = ""
      if (pending !== "") host.placeBladeWindow(pending)
    })
  }

  function focusBladeWindow(edge) {
    var target = normalizeEdge(edge)
    var address = windowAddress(target)
    if (!address || !pluginDir || !service) return false
    if (windowFocusRequestId) service.cancelBackendRequest(windowFocusRequestId, windowFocusGeneration)
    windowFocusGeneration++
    var requestGeneration = windowFocusGeneration
    windowFocusRequestId = service.backendRequest("focus-window", ["--address", address], requestGeneration, function(response) {
      if (requestGeneration !== host.windowFocusGeneration) return
      host.windowFocusRequestId = ""
    })
    return true
  }

  function swapBladeEdges() {
    var next = cloneLayout(layout)
    var keep = next.left
    next.left = next.right
    next.right = keep
    var width = next.left.width
    next.left.width = next.right.width
    next.right.width = width
    replaceLayout(next, true)
  }

  function windowClose() {
    if (focusedEdge !== "" && !isWindowMode(focusedEdge)) {
      setOpen(focusedEdge, false, true)
      return "blade-closed"
    }
    dispatchWindow(["--action", "close"])
    return "dispatched"
  }

  function windowToggle() {
    if (focusedEdge !== "") {
      var edge = focusedEdge
      if (isWindowMode(edge)) {
        redock(edge, null)
        return "blade-docked"
      }
      undock(edge)
      return "blade-undocked"
    }
    dispatchWindow(["--action", "float"])
    return "dispatched"
  }

  function windowResize(deltaX, deltaY) {
    var dx = Number(deltaX) || 0
    var dy = Number(deltaY) || 0
    if (focusedEdge !== "" && !isWindowMode(focusedEdge)) {
      if (dx === 0) return dy === 0 ? "none" : resizeSlotVertical(focusedEdge, dy)
      var edge = focusedEdge
      var reference = referenceScreen(focusedScreen)
      var screenWidth = reference ? reference.width : 0
      var change = edge === "right" ? -dx : dx
      return "blade-" + String(setWidth(edge, bladeWidth(edge) + change, screenWidth, true))
    }
    dispatchWindow(["--action", "resize", "--x", String(dx), "--y", String(dy)])
    return "dispatched"
  }

  function windowSwap(direction) {
    var raw = String(direction || "").toLowerCase().charAt(0)
    if (raw === "u" || raw === "d") return windowSwapVertical(raw)
    var side = raw === "r" ? "r" : "l"
    if (focusedEdge !== "" && !isWindowMode(focusedEdge)) {
      var inward = (focusedEdge === "left" && side === "r") || (focusedEdge === "right" && side === "l")
      if (!inward) return "none"
      var origin = focusedEdge
      var target = origin === "left" ? "right" : "left"
      swapBladeEdges()
      focusBlade(target, null, -1, "", true)
      if (!isOpen(origin) || slots(origin).length === 0) setOpen(origin, false, true)
      return "blade-swapped"
    }
    dispatchWindow(["--action", "swap", "--direction", side])
    return "dispatched"
  }
  function windowSwapVertical(side) {
    if (focusedEdge === "" || isWindowMode(focusedEdge)) {
      dispatchWindow(["--action", "swap", "--direction", side])
      return "dispatched"
    }
    var edge = focusedEdge
    var from = activeSlot(edge)
    var target = from + (side === "d" ? 1 : -1)
    if (target < 0 || target >= slots(edge).length) return "none"
    if (!moveSlotTo(edge, from, edge, side === "d" ? from + 2 : from - 1)) return "none"
    focusBlade(edge, null, target, "", false)
    return "blade-slot-swapped"
  }
  function resizeSlotVertical(edge, dy) {
    var list = slots(edge)
    var index = activeSlot(edge)
    if (list.length < 2) return "none"
    var referenceScreenItem = referenceScreen(focusedScreen)
    var screenHeight = referenceScreenItem ? referenceScreenItem.height : 1440
    var stackHeight = Math.max(200, screenHeight - Style.bar.sizeHorizontal)
    var geometry = slotGeometry(edge, stackHeight)
    var usable = geometry.expandedSpace
    var expandedWeight = geometry.expandedWeight
    if (usable <= 0 || expandedWeight <= 0) return "none"
    var fractions = normalizedFractions(edge).slice()
    var boundary = index >= list.length - 1 ? index - 1 : index
    if (slotCollapsed(edge, boundary) || slotCollapsed(edge, boundary + 1)) return "none"
    var delta = (Number(dy) || 0) / usable * expandedWeight
    var upperDelta = boundary === index ? delta : -delta
    var minimum = Math.min(expandedWeight * 0.45, minimumSlotHeight / usable * expandedWeight)
    var upper = fractions[boundary] + upperDelta
    var lower = fractions[boundary + 1] - upperDelta
    if (upper < minimum) {
      lower -= minimum - upper
      upper = minimum
    }
    if (lower < minimum) {
      upper -= minimum - lower
      lower = minimum
    }
    if (upper < minimum || lower < minimum) return "none"
    fractions[boundary] = upper
    fractions[boundary + 1] = lower
    setSlotFractions(edge, fractions)
    return "blade-slot-resized"
  }

  function dispatchWindow(arguments) {
    if (!pluginDir || !service) return
    if (windowDispatchRequestId) service.cancelBackendRequest(windowDispatchRequestId, windowDispatchGeneration)
    windowDispatchGeneration++
    var requestGeneration = windowDispatchGeneration
    windowDispatchRequestId = service.backendRequest("window-dispatch", arguments, requestGeneration, function(response) {
      if (requestGeneration !== host.windowDispatchGeneration) return
      host.windowDispatchRequestId = ""
    })
  }

  function bootstrap(legacy) {
    legacyDefaults = legacy && typeof legacy === "object" ? legacy : ({})
    if (persisted.hydrated) {
      fontScale = Typography.clamp(fontScale)
      layoutReady = true
      layoutApplied()
      return
    }
    requestLayoutRead()
  }

  function applyLayout(next, desiredMonitorMode, persist, desiredAnimations) {
    var normalized = normalizeLayout(next)
    var configured = typeof desiredAnimations === "boolean"
      ? desiredAnimations
      : (typeof config.animateBlades === "boolean" ? config.animateBlades : null)
    if (configured !== null) {
      animateBlades = configured
      animationsExplicit = true
    }
    var desiredOpen = ({})
    for (var i = 0; i < edges.length; i++) {
      desiredOpen[edges[i]] = !!normalized[edges[i]].open
      normalized[edges[i]].open = false
    }
    monitorMode = normalizeMonitorMode(desiredMonitorMode || monitorMode)
    if (monitorMode === "locked" && monitorLock === "") monitorLock = bladeLayout.primaryScreenName
    replaceLayout(normalized, false)
    layoutReady = true
    persisted.hydrated = true
    pendingOpenEdges = desiredOpen
    hydratedOpenTimer.restart()
    if (persist) scheduleSave()
    layoutApplied()
  }

  function applyLayoutText(text, seed) {
    var raw = String(text || "").trim()
    if (raw && raw === lastSavedLayoutText.trim()) return
    var parsed = null
    try { parsed = raw ? JSON.parse(raw) : null } catch (e) { parsed = null }
    if (!parsed || typeof parsed !== "object") {
      layoutWritable = !!seed
      if (!layoutReady) applyLayout(defaultLayout(), config.monitorMode, layoutWritable, config.animateBlades)
      return
    }
    layoutWritable = true
    if (layoutReady) return applyLiveLayout(parsed)
    if (typeof parsed.monitorLock === "string") monitorLock = parsed.monitorLock
    if (typeof parsed.fontScale === "number") fontScale = Typography.clamp(parsed.fontScale)
    applyLayout(parsed, parsed.monitorMode || config.monitorMode, false, parsed.animations)
  }
  function applyLayoutResponse(response) {
    if (response && response.ok) {
      layoutRereadPending = false
      return applyLayoutText(response.text, !String(response.text || "").trim())
    }
    layoutWritable = !!(response && response.missing)
    layoutRereadPending = !layoutWritable
    if (!layoutWritable) console.warn("data-goblin.fileblade: preserving unreadable blade layout: " + String(response && response.error || "read failed"))
    if (!layoutReady) applyLayout(defaultLayout(), config.monitorMode, layoutWritable, config.animateBlades)
  }
  function applyLiveLayout(parsed) {
    var incoming = normalizeLayout(parsed)
    var desiredMonitorMode = normalizeMonitorMode(parsed.monitorMode || monitorMode)
    var desiredMonitorLock = typeof parsed.monitorLock === "string" ? parsed.monitorLock : monitorLock
    var desiredAnimations = typeof parsed.animations === "boolean" ? parsed.animations : animateBlades
    var desiredFontScale = typeof parsed.fontScale === "number" ? Typography.clamp(parsed.fontScale) : fontScale
    var unchanged = JSON.stringify(incoming) === JSON.stringify(normalizeLayout(layout))
      && desiredMonitorMode === monitorMode && desiredMonitorLock === monitorLock && desiredAnimations === animateBlades
      && desiredFontScale === fontScale
    if (unchanged) return
    monitorMode = desiredMonitorMode
    monitorLock = desiredMonitorLock
    animateBlades = desiredAnimations
    fontScale = desiredFontScale
    if (typeof parsed.animations === "boolean") animationsExplicit = true
    replaceLayout(incoming, false)
    layoutApplied()
  }

  function scheduleSave() {
    if (layoutReady && layoutWritable) saveTimer.restart()
  }
  function save() {
    if (!layoutWritable) return
    var text = bladeLayout.serialized(layoutDocument(), 2)
    if (!text || bladeLayout.utf8Length(text + "\n") > bladeLayout.maximumLayoutBytes) {
      console.warn("data-goblin.fileblade: refusing to overwrite blade layout above " + bladeLayout.maximumLayoutBytes + " bytes")
      return
    }
    text += "\n"
    lastSavedLayoutText = text
    writeLayout(text)
  }
  function writeLayout(text) {
    if (layoutWriteRequestId) {
      queuedLayoutDocument = text
      return
    }
    layoutWriteGeneration++
    var requestGeneration = layoutWriteGeneration
    layoutWriteRequestId = service.backendRequest("layout-write", ["--document", text], requestGeneration, function(response) {
      if (requestGeneration !== host.layoutWriteGeneration) return
      host.layoutWriteRequestId = ""
      if (response && response.ok) {
        host.lastWrittenLayoutText = text
        layoutFileSignal.reload()
      }
      var queued = host.queuedLayoutDocument
      host.queuedLayoutDocument = ""
      if (queued) Qt.callLater(function() { host.writeLayout(queued) })
    })
  }
  function requestLayoutRead() {
    if (layoutReadRequestId) {
      layoutReadQueued = true
      return
    }
    layoutReadQueued = false
    layoutReadGeneration++
    var requestGeneration = layoutReadGeneration
    layoutReadRequestId = service.backendRequest("layout-read", [], requestGeneration, function(response) {
      if (requestGeneration !== host.layoutReadGeneration) return
      host.layoutReadRequestId = ""
      host.applyLayoutResponse(response)
      if (host.layoutReadQueued) Qt.callLater(host.requestLayoutRead)
    })
  }
  function probeAppearance() {
    if (!service) return
    service.backendRequest("hypr-option", ["--name", "general:border_size"], "border", function(response) {
      var value = Number(response && response["int"])
      if (isFinite(value) && value >= 0) host.frameWidth = Math.round(value)
    })
    service.backendRequest("hypr-option", ["--name", "animations:enabled"], "animations", function(response) {
      if (!host.animationsExplicit && response && response["bool"] === false) host.animateBlades = false
    })
  }
  Connections {
    target: host.service
    function onBackendReadyChanged() {
      if (host.service.backendReady && host.layoutRereadPending) host.requestLayoutRead()
    }
  }

  FileView {
    id: layoutFileSignal
    preload: false
    path: host.layoutPath
    watchChanges: true
    atomicWrites: true
    printErrors: false
    onFileChanged: { reload(); host.requestLayoutRead() }
  }

  Timer {
    id: saveTimer
    interval: 160
    onTriggered: host.save()
  }

  Timer {
    id: hydratedOpenTimer
    interval: 80
    onTriggered: {
      var desired = host.pendingOpenEdges
      host.pendingOpenEdges = null
      if (!desired) return
      var next = host.cloneLayout(host.layout)
      for (var i = 0; i < host.edges.length; i++) {
        var edge = host.edges[i]
        if (next[edge]) next[edge].open = !!desired[edge]
      }
      host.replaceLayout(next, false)
    }
  }

  Component.onCompleted: {
    probeAppearance()
  }

}
