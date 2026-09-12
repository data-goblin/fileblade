import QtQuick
import "../lib/PathText.js" as PathText
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
  readonly property string modifierLabel: modifierSpec.label

  property bool systemDragOut: false
  property bool dragActive: false
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
  readonly property string pathForm: (dragModifiers & Qt.ShiftModifier) ? "absolute" : (dragModifiers & Qt.ControlModifier) ? "relative" : ""
  property string toast: ""
  property bool toastError: false

  property bool wheelOpen: false
  property bool wheelFromDrag: false
  property bool keyboardFocusReleased: false
  property bool loading: false
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
  readonly property real extent: outerItems.length > 0 ? childOuterRadius : outerRadius
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

  function beginDrag(paths, entries, targetScreen, docked, x, y, spec) {
    dragPaths = fileUrlsToPaths(paths)
    if (dragPaths.length === 0) return false
    dragEntries = entrySnapshots(entries)
    dragSpec = spec && typeof spec === "object" ? spec : null
    dragScreen = targetScreen || null
    dragDocked = !!docked
    dragOutside = false
    dragConsumed = false
    dragModifiers = 0
    pointerX = Number(x) || 0
    pointerY = Number(y) || 0
    service.bladeHost.pressActive = true
    dragActive = true
    return true
  }

  function relativePreview(path) {
    var root = String(service.projectRoot || "")
    return root && PathText.within(path, root) ? PathText.relative(path, root) : PathText.name(path)
  }

  function previewPath(form) {
    if (dragPaths.length === 0) return ""
    var first = String(dragPaths[0])
    var shown = form === "relative" ? relativePreview(first) : form === "name" ? PathText.name(first) : first
    return dragPaths.length > 1 ? shown + "  +" + (dragPaths.length - 1) : shown
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
    if (modifierFlag !== 0 && dragDocked && dragOutside && (Number(modifiers) & modifierFlag)) openWheel(dragScreen, pointerX, pointerY, true)
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
    if (wheelOpen && wheelFromDrag) close()
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
    keyboardFocusReleased = false
    wheelOpen = true
    requestContext()
    openedAt(wheelScreen, wheelX, wheelY)
  }

  function openAfterDrag() {
    if (wheelOpen || dragPaths.length === 0) return false
    openWheel(dragScreen, pointerX, pointerY, false)
    return true
  }

  function resetHighlight() {
    highlighted = -1
    parentIndex = -1
    outerItems = []
    outerHighlighted = -1
    outerFocus = false
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
      error = result && result.error ? String(result.error) : "Unable to resolve the drop target"
      context = result || null
      ringItems = mergeActions(result && Array.isArray(result.actions) ? result.actions : [])
      return
    }
    context = result
    if (!dragDocked && result.at) {
      wheelX = Number(result.at.x) - screenOffsetX()
      wheelY = Number(result.at.y) - screenOffsetY()
    }
    ringItems = mergeActions(Array.isArray(result.actions) ? result.actions : [])
    if (wheelFromDrag) hover(pointerX, pointerY)
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
      parentAngle: parentAngle
    }, x, y)
  }

  function hover(x, y) {
    var hit = pointAt(x, y)
    if (hit.ring === "outer") {
      outerHighlighted = hit.index
      return
    }
    if (hit.ring === "inner" && hit.index === highlighted && outerFocus) return
    outerHighlighted = -1
    setHighlighted(hit.index)
  }

  function setHighlighted(index) {
    if (index !== highlighted) {
      highlighted = index
      outerFocus = false
      outerHighlighted = -1
    }
    var children = index >= 0 ? childrenOf(ringItems[index]) : []
    parentIndex = children.length > 0 ? index : -1
    outerItems = children
  }

  function wedgeAngle(index) {
    return WheelGeometry.wedgeAngle(ringItems.length, index)
  }

  function childAngle(index) {
    return WheelGeometry.childAngle(parentAngle, outerItems.length, index)
  }

  function activateAt(x, y) {
    var hit = pointAt(x, y)
    if (hit.ring === "outer") return activateChild(hit.index)
    if (hit.ring === "inner") return activate(hit.index)
    resetHighlight()
    return false
  }

  function enterOuter() {
    if (outerItems.length === 0) return false
    outerFocus = true
    if (outerHighlighted < 0) outerHighlighted = 0
    return true
  }

  function accept() {
    if (outerFocus && outerHighlighted >= 0) return activateChild(outerHighlighted)
    if (highlighted >= 0) return activate(highlighted)
    return false
  }

  function activate(index) {
    var item = ringItems[index]
    if (!item) return false
    setHighlighted(index)
    if (outerItems.length > 0) return enterOuter()
    if (item.id === "open-with") {
      error = "No registered application for this file type"
      return false
    }
    if (item.custom) return runCustom(item.run, null)
    return run(String(item.id), "", "")
  }

  function activateChild(index) {
    var parent = parentItem
    var item = outerItems[index]
    if (!parent || !item) return false
    outerHighlighted = index
    if (parent.custom) return runCustom(item.run || parent.run, item)
    if (parent.id === "open-with") return run("application", "", String(item.desktop_id || ""))
    return run(String(parent.id), String(item.id), "")
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
    if (outerFocus) {
      outerFocus = false
      outerHighlighted = -1
      error = ""
      return
    }
    close()
  }

  function activateKey(text, repeated) {
    var wanted = String(text || "").toLowerCase()
    if (!wanted) return false
    var i
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
    if (outerFocus && outerItems.length > 0) {
      var children = outerItems.length
      outerHighlighted = outerHighlighted < 0 ? (delta > 0 ? 0 : children - 1) : (outerHighlighted + delta + children) % children
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
    dragKeys.cancelRelease()
    runGeneration++
    runRequestId = ""
    if (contextRequestId) {
      service.cancelBackendRequest(contextRequestId, contextGeneration)
      contextGeneration++
      contextRequestId = ""
    }
    wheelOpen = false
    wheelFromDrag = false
    keyboardFocusReleased = false
    loading = false
    ringItems = []
    resetHighlight()
    status = ""
  }

  DropWheelDragKeys {
    id: dragKeys
    controller: wheelController
  }

  Timer {
    id: toastTimer
    onTriggered: wheelController.toast = ""
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
