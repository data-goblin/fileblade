import QtQuick
import Quickshell
import "../ui" as PluginUi
import "../lib/DropFocusPolicy.js" as DropFocusPolicy
import "../lib/WheelGeometry.js" as WheelGeometry

Item {
  id: wheelController

  required property var service

  visible: false

  property string modifierName: "space"
  readonly property var modifierTable: ({
    space: { flag: 0, key: Qt.Key_Space, label: "Space" },
    alt: { flag: Qt.AltModifier, key: Qt.Key_Alt, label: "Alt" },
    ctrl: { flag: Qt.ControlModifier, key: Qt.Key_Control, label: "Ctrl" },
    shift: { flag: Qt.ShiftModifier, key: Qt.Key_Shift, label: "Shift" },
    meta: { flag: Qt.MetaModifier, key: Qt.Key_Meta, label: "Super" }
  })
  readonly property var modifierSpec: modifierTable[modifierName] || modifierTable.space
  readonly property int modifierFlag: modifierSpec.flag
  readonly property int modifierKey: modifierSpec.key

  property bool dragActive: false
  property Item dragSource: null
  property bool dragDocked: true
  property bool dragOutside: false
  property bool dragConsumed: false
  property var dragScreen: null
  property var dragPaths: []
  property var dragEntries: []
  property var dragSpec: null
  property real pointerX: 0
  property real pointerY: 0
  property int dragModifiers: 0
  property bool modifierHeld: false
  readonly property string pathForm: (dragModifiers & Qt.ShiftModifier) ? "absolute" : (dragModifiers & Qt.ControlModifier) ? "relative" : ""
  property string toast: ""
  property bool toastError: false

  property bool wheelOpen: false
  property bool wheelFromDrag: false
  property bool keyboardFocusReleased: false
  property bool loading: false
  property var pendingRelease: null
  property var wheelScreen: null
  property real wheelX: 0
  property real wheelY: 0
  property var context: null
  property var ringItems: []
  property int highlighted: -1
  property int parentIndex: -1
  property var outerItems: []
  property int outerHighlighted: -1
  property bool outerFocus: false
  property int subParentIndex: -1
  property var subItems: []
  property int subHighlighted: -1
  property bool subFocus: false
  property string diagnostics: ""
  property string error: ""
  property string status: ""
  property string pasteRequestId: ""
  property int pasteGeneration: 0
  property string contextRequestId: ""
  property int contextGeneration: 0
  property string runRequestId: ""
  property int runGeneration: 0

  readonly property int count: dragActive ? dragPaths.length : (context && context.files ? Number(context.files.count) || 0 : 0)
  readonly property string targetLabel: context && context.target ? String(context.target.label || "") : ""
  readonly property string ringTitle: targetLabel
  readonly property var parentItem: parentIndex >= 0 ? ringItems[parentIndex] || null : null
  readonly property real hubRadius: 26
  readonly property real gapRadius: 5
  readonly property real outerRadius: Math.max(86, 58 + 5 * Math.max(ringItems.length, 4))
  readonly property real bandWidth: 46
  readonly property real childInnerRadius: outerRadius + gapRadius
  readonly property real childOuterRadius: childInnerRadius + bandWidth
  readonly property real subInnerRadius: childOuterRadius + gapRadius
  readonly property real subOuterRadius: subInnerRadius + bandWidth
  readonly property real subStep: WheelGeometry.childStep(subItems.length)
  readonly property real subParentAngle: subParentIndex >= 0 ? childAngle(subParentIndex) : 0
  readonly property real extent: subItems.length > 0 ? subOuterRadius : outerItems.length > 0 ? childOuterRadius : outerRadius
  readonly property real childStep: WheelGeometry.childStep(outerItems.length)
  readonly property real parentAngle: parentIndex >= 0 ? wedgeAngle(parentIndex) : 0

  signal openedAt(var targetScreen, real x, real y)

  function fileUrlsToPaths(paths) {
    return Array.isArray(paths) ? paths.map(function(value) { return String(value) }) : []
  }

  function entrySnapshots(entries) {
    var rows = Array.isArray(entries) ? entries : []
    var result = []
    for (var i = 0; i < rows.length && i < 3; i++) {
      var entry = rows[i] || {}
      result.push({ name: String(entry.name || ""), isDir: !!entry.isDir, isSymlink: !!entry.isSymlink, isGitRepo: !!entry.isGitRepo, glyph: String(entry.glyph || "") })
    }
    return result
  }

  function beginDrag(paths, entries, targetScreen, docked, x, y, spec, source) {
    dragPaths = fileUrlsToPaths(paths)
    if (dragPaths.length === 0) return false
    dragSource = source || null
    dragEntries = entrySnapshots(entries)
    dragSpec = spec && typeof spec === "object" ? spec : null
    dragScreen = targetScreen || null
    dragDocked = !!docked
    dragOutside = false
    dragConsumed = false
    dragModifiers = 0
    modifierHeld = false
    pointerX = Number(x) || 0
    pointerY = Number(y) || 0
    service.bladeHost.pressActive = true
    dragActive = true
    return true
  }

  function updateDrag(x, y, outside, modifiers) {
    if (!dragActive) return
    pointerX = Number(x) || 0
    pointerY = Number(y) || 0
    dragOutside = !!outside
    dragModifiers = Number(modifiers) || 0
    if (wheelOpen) {
      if (wheelFromDrag) hover(pointerX, pointerY)
      return
    }
    if (dragDocked && dragOutside && (modifierHeld || (modifierFlag !== 0 && (Number(modifiers) & modifierFlag)))) openWheel(dragScreen, pointerX, pointerY, true)
  }

  function modifierPressed() {
    if (!dragActive || wheelOpen || !dragDocked || !dragOutside) return false
    openWheel(dragScreen, pointerX, pointerY, true)
    return true
  }

  function handleDragKeyRelease(event) { return dragKeys.handleRelease(event) }

  function handleDragKey(event) { return dragKeys.handlePress(event) }

  function endDrag(x, y, outside) {
    if (!dragActive) return false
    dragKeys.cancelRelease()
    if (isFinite(Number(x)) && isFinite(Number(y))) {
      pointerX = Number(x)
      pointerY = Number(y)
    }
    if (outside !== undefined) dragOutside = !!outside
    dragSource = null
    var paths = dragPaths
    var wasOutside = dragOutside
    dragActive = false
    service.bladeHost.pressActive = false
    if (dragConsumed) {
      dragConsumed = false
      return true
    }
    if (wheelOpen && wheelFromDrag) {
      wheelFromDrag = false
      if (loading) {
        pendingRelease = { x: pointerX, y: pointerY, generation: contextGeneration, deadline: Date.now() + 800 }
        releaseWait.restart()
        return true
      }
      if (withinHub(pointerX, pointerY)) resetHighlight()
      else if (!activateAt(pointerX, pointerY)) close()
      return true
    }
    if (!wasOutside || pathForm === "") return false
    dragPaths = paths
    pasteAt(dragScreen, pointerX, pointerY, pathForm)
    return true
  }

  function pasteAt(targetScreen, x, y, form) {
    wheelScreen = targetScreen || service.referenceScreen(null)
    wheelX = Number(x) || 0
    wheelY = Number(y) || 0
    var arguments = ["--form", form]
    if (dragDocked) arguments = arguments.concat(["--x", String(Math.round(wheelX + screenOffsetX())), "--y", String(Math.round(wheelY + screenOffsetY()))])
    for (var i = 0; i < dragPaths.length; i++) arguments.push("--path", dragPaths[i])
    arguments.push("--blade-title", String(service.bladeHost.windowTitle("left")), "--blade-title", String(service.bladeHost.windowTitle("right")))
    if (pasteRequestId) service.cancelBackendRequest(pasteRequestId, pasteGeneration)
    service.yieldFocusForExternalLaunch()
    pasteGeneration++
    var requestGeneration = pasteGeneration
    pasteRequestId = service.backendRequest("drop-paste", arguments, requestGeneration, function(result) {
      if (requestGeneration !== wheelController.pasteGeneration) return
      wheelController.pasteRequestId = ""
      wheelController.finishPaste(result)
    })
  }

  function finishPaste(result) {
    result = result || { ok: false, error: "Paste failed" }
    if (!result.ok) showToast(String(result.error || "Paste failed"), true)
  }

  function showToast(text, failed) {
    toast = String(text || "")
    toastError = !!failed
    toastTimer.interval = failed ? 4000 : 1800
    toastTimer.restart()
  }

  function cancelDrag() {
    if (!dragActive) return
    dragActive = false
    service.bladeHost.pressActive = false
    dragKeys.cancelRelease()
    if (dragSource) dragSource.Drag.cancel()
    dragSource = null
    if (wheelOpen && wheelFromDrag) close()
    dragPaths = []
    dragEntries = []
    if (!wheelOpen) context = null
  }

  function screenAt(x, y) {
    for (var i = 0; i < Quickshell.screens.length; i++) {
      var candidate = Quickshell.screens[i]
      var left = Number(candidate.x) || 0
      var top = Number(candidate.y) || 0
      if (x >= left && x < left + candidate.width && y >= top && y < top + candidate.height) return candidate
    }
    return null
  }

  function openForSelection(targetScreen, x, y) {
    dragPaths = service.selectedPaths.slice()
    if (dragPaths.length === 0) return false
    dragEntries = entrySnapshots(service.selectedEntries)
    dragSpec = null
    dragDocked = true
    openWheel(targetScreen, x, y, false)
    return true
  }

  function openWheel(targetScreen, x, y, fromDrag) {
    if (wheelOpen) close()
    wheelScreen = targetScreen || service.referenceScreen(null)
    wheelX = Number(x) || 0
    wheelY = Number(y) || 0
    wheelFromDrag = !!fromDrag
    ringItems = []
    resetHighlight()
    error = ""
    status = ""
    context = null
    diagnostics = ""
    keyboardFocusReleased = false
    wheelOpen = true
    requestContext()
    openedAt(wheelScreen, wheelX, wheelY)
  }

  function resetHighlight() {
    highlighted = -1
    parentIndex = -1
    outerItems = []
    outerHighlighted = -1
    outerFocus = false
    resetSub()
  }

  function resetSub() {
    subParentIndex = -1
    subItems = []
    subHighlighted = -1
    subFocus = false
  }

  function screenOffsetX() { return wheelScreen ? Number(wheelScreen.x) || 0 : 0 }
  function screenOffsetY() { return wheelScreen ? Number(wheelScreen.y) || 0 : 0 }

  function requestContext() {
    if (contextRequestId) {
      service.cancelBackendRequest(contextRequestId, contextGeneration)
      contextGeneration++
      contextRequestId = ""
    }
    loading = true
    var arguments = []
    if (dragDocked) arguments = arguments.concat(["--x", String(Math.round(wheelX + screenOffsetX())), "--y", String(Math.round(wheelY + screenOffsetY()))])
    for (var i = 0; i < dragPaths.length; i++) arguments.push("--path", dragPaths[i])
    arguments.push("--blade-title", String(service.bladeHost.windowTitle("left")), "--blade-title", String(service.bladeHost.windowTitle("right")))
    contextGeneration++
    var requestGeneration = contextGeneration
    contextRequestId = service.backendRequest("drop-context", arguments, requestGeneration, function(result) {
      if (requestGeneration !== wheelController.contextGeneration) return
      wheelController.contextRequestId = ""
      if (wheelController.wheelOpen) wheelController.applyContext(result)
    })
  }

  function applyContext(result) {
    loading = false
    if (!result || !result.ok) {
      clearPendingRelease()
      error = result && result.error ? String(result.error) : "Unable to resolve the drop target"
      context = result || null
      ringItems = mergeActions(result && Array.isArray(result.actions) ? result.actions : [])
      return
    }
    context = result
    diagnostics = Array.isArray(result.diagnostics) ? result.diagnostics.join(" · ") : ""
    if (!dragDocked && result.at) {
      wheelX = Number(result.at.x) - screenOffsetX()
      wheelY = Number(result.at.y) - screenOffsetY()
    }
    ringItems = mergeActions(Array.isArray(result.actions) ? result.actions : [])
    if (wheelFromDrag) hover(pointerX, pointerY)
    if (pendingRelease) {
      var release = pendingRelease
      clearPendingRelease()
      if (release.generation !== contextGeneration) return
      if (Date.now() >= release.deadline) {
        status = "Choose an action to continue"
        return
      }
      if (withinHub(release.x, release.y)) resetHighlight()
      else if (!activateAt(release.x, release.y)) close()
    }
  }

  function clearPendingRelease() {
    releaseWait.stop()
    pendingRelease = null
  }

  function customActions() {
    var rows = dragSpec && Array.isArray(dragSpec.actions) ? dragSpec.actions : []
    var result = []
    for (var i = 0; i < rows.length && i < 12; i++) {
      var row = rows[i]
      if (!row || typeof row !== "object" || !row.label) continue
      var placements = []
      var choices = Array.isArray(row.placements) ? row.placements : []
      for (var j = 0; j < choices.length && j < 12; j++) {
        var choice = choices[j]
        if (!choice || !choice.label) continue
        placements.push({ id: String(choice.id || choice.label), label: String(choice.label), glyph: String(choice.glyph || ""), key: String(choice.key || "").toLowerCase(), description: String(choice.description || ""), run: typeof choice.run === "function" ? choice.run : null })
      }
      result.push({ id: String(row.id || row.label), label: String(row.label), glyph: String(row.glyph || ""), key: String(row.key || "").toLowerCase(), description: String(row.description || ""), placements: assignKeys(placements), run: typeof row.run === "function" ? row.run : null, custom: true })
    }
    return result
  }

  function mergeActions(defaults) {
    var custom = customActions()
    if (custom.length === 0) return defaults
    var rows = dragSpec.includeDefaults ? custom.concat(defaults) : custom
    return assignKeys(rows)
  }

  function assignKeys(rows) {
    var used = {}
    var i
    for (i = 0; i < rows.length; i++) if (rows[i].key && !used[rows[i].key]) used[rows[i].key] = true; else rows[i].key = ""
    for (i = 0; i < rows.length; i++) {
      if (rows[i].key) continue
      var candidates = String(rows[i].label || "").toLowerCase().replace(/[^a-z0-9]/g, "") + "abcdefghijklmnopqrstuvwxyz0123456789"
      for (var c = 0; c < candidates.length; c++) {
        var letter = candidates.charAt(c)
        if (!used[letter]) {
          used[letter] = true
          rows[i].key = letter
          break
        }
      }
    }
    return rows
  }

  function childrenOf(item) {
    return item && Array.isArray(item.placements) ? item.placements : []
  }

  function hasChildren(index) {
    return childrenOf(ringItems[index]).length > 0
  }

  function withinHub(x, y) { return wheelOpen && Math.hypot(Number(x) - wheelX, Number(y) - wheelY) < hubRadius }

  function pointAt(x, y) {
    if (!wheelOpen) return { ring: "none", index: -1 }
    return WheelGeometry.pointAt({
      x: wheelX,
      y: wheelY,
      hubRadius: hubRadius,
      outerRadius: outerRadius,
      childOuterRadius: childOuterRadius,
      count: ringItems.length,
      outerCount: outerItems.length,
      parentAngle: parentAngle,
      subCount: subItems.length,
      subOuterRadius: subOuterRadius,
      subParentAngle: subParentAngle
    }, x, y)
  }

  function hover(x, y) {
    var hit = pointAt(x, y)
    if (hit.ring === "sub") {
      subHighlighted = hit.index
      return
    }
    if (hit.ring === "outer") {
      if (hit.index === outerHighlighted && subFocus) return
      setOuterHighlighted(hit.index)
      return
    }
    if (hit.ring === "inner" && hit.index === highlighted && outerFocus) return
    outerHighlighted = -1
    resetSub()
    setHighlighted(hit.index)
  }

  function setHighlighted(index) {
    if (index !== highlighted) {
      highlighted = index
      outerFocus = false
      outerHighlighted = -1
      resetSub()
    }
    var children = index >= 0 && ringItems[index].enabled !== false ? childrenOf(ringItems[index]) : []
    parentIndex = children.length > 0 ? index : -1
    outerItems = children
  }

  function setOuterHighlighted(index) {
    if (index !== outerHighlighted) resetSub()
    outerHighlighted = index
    var children = index >= 0 ? childrenOf(outerItems[index]) : []
    subParentIndex = children.length > 0 ? index : -1
    subItems = children
  }

  function subAngle(index) {
    return WheelGeometry.childAngle(subParentAngle, subItems.length, index)
  }

  function wedgeAngle(index) {
    return WheelGeometry.wedgeAngle(ringItems.length, index)
  }

  function childAngle(index) {
    return WheelGeometry.childAngle(parentAngle, outerItems.length, index)
  }

  function activateAt(x, y) {
    clearPendingRelease()
    var hit = pointAt(x, y)
    if (hit.ring === "sub") return activateSub(hit.index)
    if (hit.ring === "outer") return activateChild(hit.index)
    if (hit.ring === "inner") return activate(hit.index)
    resetHighlight()
    return false
  }

  function enterOuter() {
    if (outerItems.length === 0) return false
    outerFocus = true
    if (outerHighlighted < 0) setOuterHighlighted(0)
    return true
  }

  function accept() {
    if (subFocus && subHighlighted >= 0) return activateSub(subHighlighted)
    if (outerFocus && outerHighlighted >= 0) return activateChild(outerHighlighted)
    if (highlighted >= 0) return activate(highlighted)
    return false
  }

  function activate(index) {
    clearPendingRelease()
    var item = ringItems[index]
    if (!item || item.enabled === false) return false
    setHighlighted(index)
    if (outerItems.length > 0) return enterOuter()
    if (item.id === "open-with") {
      error = "No registered application for this file type"
      return false
    }
    if (item.custom) return runCustom(item.run, null)
    return runItem(item, null)
  }

  function activateChild(index) {
    clearPendingRelease()
    var parent = parentItem
    var item = outerItems[index]
    if (!parent || !item) return false
    setOuterHighlighted(index)
    if (subItems.length > 0) {
      outerFocus = true
      subFocus = true
      if (subHighlighted < 0) subHighlighted = 0
      return true
    }
    return runItem(item, parent)
  }

  function activateSub(index) {
    clearPendingRelease()
    var item = subItems[index]
    if (!item) return false
    subHighlighted = index
    return runItem(item, parentItem)
  }

  function runItem(item, parent) {
    if (item.enabled === false || (parent && parent.enabled === false)) return false
    if (Array.isArray(item.command_route)) return run("configured", JSON.stringify(item.command_route), "")
    if (item.builtin_action) return run(String(item.builtin_action), String(item.builtin_placement || ""), String(item.desktop_id || ""))
    if (parent && parent.custom) return runCustom(item.run || parent.run, item)
    if (parent && parent.id === "open-with") return run("application", "", String(item.desktop_id || ""))
    return run(String(parent ? parent.id : item.id), parent ? String(item.id) : "", "")
  }

  function runCustom(callback, placement) {
    if (typeof callback !== "function") {
      error = "This action has no handler"
      return false
    }
    var paths = context && context.files && Array.isArray(context.files.paths) ? context.files.paths : dragPaths
    var details = {
      target: context ? context.target || null : null,
      files: context ? context.files || null : null,
      placement: placement ? String(placement.id || "") : "",
      screen: wheelScreen,
      x: wheelX + screenOffsetX(),
      y: wheelY + screenOffsetY()
    }
    dragConsumed = dragActive
    var outcome
    try { outcome = callback(paths, details) }
    catch (exception) { outcome = String(exception) }
    if (outcome === false || (typeof outcome === "string" && outcome !== "")) {
      error = outcome === false ? "The action did not run" : outcome
      return false
    }
    close()
    return true
  }

  function back() {
    clearPendingRelease()
    if (subFocus || subItems.length > 0) {
      resetSub()
      outerFocus = true
      error = ""
      return
    }
    if (outerFocus) {
      outerFocus = false
      outerHighlighted = -1
      resetSub()
      error = ""
      return
    }
    close()
  }

  function activateKey(text, repeated) {
    var wanted = String(text || "").toLowerCase()
    if (!wanted) return false
    var i
    if (subFocus || subHighlighted >= 0)
      for (i = 0; i < subItems.length; i++)
        if (String(subItems[i].key || "") === wanted) {
          if (!repeated) activateSub(i)
          return true
        }
    if (outerFocus || outerHighlighted >= 0)
      for (i = 0; i < outerItems.length; i++)
        if (String(outerItems[i].key || "") === wanted) {
          if (!repeated) activateChild(i)
          return true
        }
    for (i = 0; i < ringItems.length; i++)
      if (String(ringItems[i].key || "") === wanted) {
        if (!repeated) activate(i)
        return true
      }
    return false
  }

  function moveHighlight(delta) {
    if (subFocus && subItems.length > 0) {
      var count = subItems.length
      subHighlighted = subHighlighted < 0 ? (delta > 0 ? 0 : count - 1) : (subHighlighted + delta + count) % count
      return
    }
    if (outerFocus && outerItems.length > 0) {
      var children = outerItems.length
      setOuterHighlighted(outerHighlighted < 0 ? (delta > 0 ? 0 : children - 1) : (outerHighlighted + delta + children) % children)
      return
    }
    var count = ringItems.length
    if (count === 0) return
    setHighlighted(highlighted < 0 ? (delta > 0 ? 0 : count - 1) : (highlighted + delta + count) % count)
  }

  function run(actionId, placement, desktopId) {
    if (runRequestId || !context) return false
    var paths = context.files && Array.isArray(context.files.paths) ? context.files.paths : dragPaths
    var files = context.files && Array.isArray(context.files.files) ? context.files.files : []
    dragConsumed = dragActive
    if (actionId === "open") {
      var directories = context.files && Array.isArray(context.files.directories) ? context.files.directories : []
      if (directories.length > 0) service.navigateToLocation(String(directories[directories.length - 1]), null, "browse")
      if (files.length === 0) { close(); return true }
      paths = files
    }
    var arguments = ["--action", actionId, "--placement", placement, "--target", JSON.stringify(context.target || {})]
    if (desktopId) arguments.push("--desktop-id", desktopId)
    for (var i = 0; i < paths.length; i++) arguments.push("--path", paths[i])
    status = "Running…"
    error = ""
    var fileCount = files.length
    if (DropFocusPolicy.transfersFocus(actionId, fileCount)) service.yieldFocusForExternalLaunch()
    runGeneration++
    var requestGeneration = runGeneration
    runRequestId = service.backendRequest("drop-run", arguments, requestGeneration, function(result) {
      if (requestGeneration !== wheelController.runGeneration) return
      wheelController.runRequestId = ""
      wheelController.finishRun(result)
    }, null, 30000)
    return true
  }

  function finishRun(result) {
    result = result || { ok: false, error: "Drop action failed" }
    status = ""
    if (result.ok) {
      close()
      return
    }
    keyboardFocusReleased = false
    error = String(result.error || "Drop action failed")
  }

  function close() {
    wheelOpen = false
    wheelFromDrag = false
    keyboardFocusReleased = false
    loading = false
    clearPendingRelease()
    dragKeys.cancelRelease()
    runGeneration++
    runRequestId = ""
    if (contextRequestId) {
      service.cancelBackendRequest(contextRequestId, contextGeneration)
      contextGeneration++
      contextRequestId = ""
    }
    ringItems = []
    resetHighlight()
    status = ""
  }

  DropWheelDragKeys {
    id: dragKeys
    controller: wheelController
  }

  Timer {
    id: releaseWait
    interval: 800
    onTriggered: {
      wheelController.pendingRelease = null
      if (wheelController.wheelOpen) wheelController.status = "Choose an action to continue"
    }
  }

  Timer {
    id: toastTimer
    onTriggered: wheelController.toast = ""
  }

  property int idleCloseMs: 30000
  readonly property string idleActivity: [highlighted, outerHighlighted, subHighlighted, outerFocus, subFocus, loading, runRequestId, ringItems.length].join(":")
  onIdleActivityChanged: if (wheelOpen) idleClose.restart()
  onWheelOpenChanged: {
    if (wheelOpen) idleClose.restart()
    else idleClose.stop()
  }

  Timer {
    id: idleClose
    interval: wheelController.idleCloseMs
    onTriggered: {
      if (!wheelController.wheelOpen) return
      if (wheelController.runRequestId !== "") {
        restart()
        return
      }
      console.warn("FileBlade: drop wheel closed after " + wheelController.idleCloseMs + " ms without input")
      wheelController.close()
    }
  }

  Variants {
    model: Quickshell.screens

    delegate: Component {
      PluginUi.DropWheel {
        required property var modelData
        controller: wheelController
        screen: modelData
      }
    }
  }

}
