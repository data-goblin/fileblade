import QtQuick
import QtQuick.Controls
import qs.Commons
import qs.Ui
import "../ui" as PluginUi
import "../lib/KeyRouter.js" as KeyRouter
import "../lib/PathText.js" as PathText
import "../lib/ScrollMarks.js" as ScrollMarks
import "../modules/files/MediaModel.js" as MediaModel
import "../modules/files/ViewChrome.js" as ViewChrome
import "../lib/FileIcons.js" as FileIcons
import "../lib/FooterFields.js" as FooterFields
import "../lib/GitSummary.js" as GitSummary
import "../lib/ToolbarFields.js" as ToolbarFields
import "../theme"

FocusScope {
  id: root

  required property var controller
  property string branchError: ""
  required property var hostWindow
  property alias actionKeys: actionKeyGuard
  PluginUi.ActionKeyGuard { id: actionKeyGuard; active: root.activeFocus; shared: root.hostWindow ? root.hostWindow.actionKeys : null }
  PluginUi.TreeKeys {
    id: treeKeys
    active: root.focusEnabled && (treeList.activeFocus || searchList.activeFocus || recentList.activeFocus || (mediaView && mediaView.contentActiveFocus))
    scope: treeList.activeFocus ? "files-tree" : "files-list"
    plan: controller.keybindings.plan
  }
  Keys.onReleased: function(event) { actionKeyGuard.release(event) }
  property var context: null
  property bool focusEnabled: true
  property bool locationEditing: false
  property bool mediaStateReady: false
  property bool mediaMode: false
  property bool mediaRecursive: false
  property string mediaQuery: ""
  property bool mediaQueryReady: false
  property int mediaSizeStep: 2
  property bool mediaShowEmptyPeriods: false
  property var densityAnchor: null
  property bool treeOrderUpdating: false
  property bool summaryInTree: true
  readonly property bool contextAbove: !summaryInTree && !controller.trashMode && !controller.drivesMode && !controller.recentMode
  readonly property bool contextRootExpanded: !contextAbove || controller.treeModel.count === 0 || controller.treeModel.get(0).expanded
  onContextRootExpandedChanged: Qt.callLater(root.ensureContextRoot)
  onSummaryInTreeChanged: { persistMedia(); Qt.callLater(root.ensureContextRoot) }
  readonly property var densityPresets: [0.8, 0.9, 1, 1.1, 1.2]
  readonly property var densitySizes: ["XS", "S", "M", "L", "XL"]
  property var toolbarButtons: ToolbarFields.normalizeFields()
  readonly property bool volumesConfigurable: !!context
  property bool volumesInTree: !volumesConfigurable
  readonly property bool capacityBar: !context || !context.slotState || context.slotState.capacityBar !== false
  readonly property bool capacityDemanded: capacityBar && root.visible && (!context || (context.bladeOpen !== false && !context.collapsed && !context.retired))
  readonly property bool capacityShown: capacityDemanded && controller.capacity.status === "ready"
  onCapacityDemandedChanged: syncCapacityDemand()
  Component.onDestruction: controller.capacity.detach(root)

  function setCapacityBar(value) {
    return !!context && !!context.state && context.state.set("capacityBar", value === true)
  }

  function syncCapacityDemand() {
    if (capacityDemanded) controller.capacity.attach(root)
    else controller.capacity.detach(root)
  }
  property real ordinaryDensityValue: 1
  readonly property real ordinaryDensity: ordinaryDensityValue > 0 ? ordinaryDensityValue : 1
  readonly property int ordinaryDensityStep: root.nearestDensityStep(ordinaryDensity)
  property var mediaLocationDescriptor: null
  property real ordinaryContentY: 0
  readonly property bool mediaActive: mediaMode && !controller.pickerActive && toolbarButtons.indexOf("media") >= 0 && !controller.trashMode && !controller.drivesMode && !controller.recentMode && !PathText.isRemote(controller.rootPath)
  property var folderCountData: ({ loaded: 0, total: 0, known: false })
  readonly property var folderCount: folderCountData
  readonly property bool folderCountReady: ViewChrome.folderReady(controller.treeModel, controller.rootPath)
  onFolderCountReadyChanged: Qt.callLater(root.refreshFolderCount)
  readonly property var mediaView: mediaLoader.item
  readonly property var mediaProvider: mediaView ? mediaView.provider : null
  readonly property var mediaMatches: mediaView ? mediaView.matches : ({ rows: [], invalid: false })

  function persistMedia() {
    if (!mediaStateReady || !context || !context.state) return
    context.state.set("mediaMode", mediaMode)
    context.state.set("mediaRecursive", mediaRecursive)
    context.state.set("mediaQuery", mediaQuery)
    context.state.set("mediaQueryReady", mediaQueryReady)
    context.state.set("mediaSizeStep", mediaSizeStep)
    context.state.set("mediaShowEmptyPeriods", mediaShowEmptyPeriods)
    context.state.set("ordinaryDensityPercent", Math.round(ordinaryDensityValue * 100))
    context.state.set("rootRowInTree", summaryInTree)
    context.state.set("toolbarButtons", toolbarButtons)
    context.state.set("volumesInTree", volumesInTree)
  }

  function setToolbarButtons(value) {
    toolbarButtons = ToolbarFields.normalizeFields(value)
    return toolbarButtons
  }

  function ensureContextRoot() {
    if (!contextAbove || !treeList.visible || !focusEnabled) return
    controller.setDirectoryExpanded(controller.rootPath, true)
    if (treeList.currentIndex === 0) treeList.currentIndex = -1
  }

  function collapseTree(view) {
    if (!contextAbove) {
      selectIndex(view, true, 0, "replace")
      controller.setBranchExpanded(controller.rootPath, false)
      return
    }
    for (var i = controller.treeModel.count - 1; i > 0; i--) {
      var entry = controller.treeModel.get(i)
      if (entry.depth === 1 && entry.isDir) controller.setBranchExpanded(entry.path, false)
    }
    selectIndex(view, true, 1, "replace")
  }

  function refreshFolderCount() {
    if (mediaActive || controller.trashMode || controller.drivesMode || controller.recentMode || PathText.isRemote(controller.rootPath)) return
    folderCountData = ViewChrome.folderCount(controller.treeModel, controller.rootPath,
      controller.treeStructureRevision, controller.treeRowsRevision)
  }

  function invalidateFolderCount() {
    folderCountData = ({ loaded: 0, total: 0, known: false })
    Qt.callLater(root.refreshFolderCount)
  }

  onMediaActiveChanged: Qt.callLater(root.refreshFolderCount)

  Connections {
    target: root.controller
    function onTreeStructureRevisionChanged() { Qt.callLater(root.refreshFolderCount) }
    function onTreeRowsRevisionChanged() { Qt.callLater(root.refreshFolderCount) }
    function onTreeLoadingChanged() { Qt.callLater(root.refreshFolderCount) }
    function onRecentModeChanged() { Qt.callLater(root.refreshFolderCount) }
    function onTrashModeChanged() { Qt.callLater(root.refreshFolderCount) }
    function onDrivesModeChanged() { Qt.callLater(root.refreshFolderCount) }
    function onRootPathChanged() { root.invalidateFolderCount() }
    function onTreeFilterChanged() { root.invalidateFolderCount() }
    function onShowHiddenChanged() { root.invalidateFolderCount() }
    Component.onCompleted: Qt.callLater(root.refreshFolderCount)
  }

  function changeDensity(step) {
    var view = root.activeList
    if (!densityAnchor || densityAnchor.view !== view || densityAnchor.rootPath !== controller.rootPath) {
      var first = view.indexAt(1, view.contentY + 1)
      var item = first >= 0 ? view.itemAtIndex(first) : null
      densityAnchor = {
        view: view, rootPath: controller.rootPath,
        path: first >= 0 ? String(view.model.get(first).path || "") : "",
        fraction: item && item.height > 0 ? Math.max(0, Math.min(1, (view.contentY - item.y) / item.height)) : 0
      }
    }
    var index = Math.max(0, Math.min(root.densityPresets.length - 1, Math.round(Number(step))))
    ordinaryDensityValue = root.densityPresets[index]
    root.persistMedia()
    Qt.callLater(root.restoreDensityAnchor)
  }

  function restoreDensityAnchor() {
    var saved = densityAnchor
    densityAnchor = null
    if (!saved || root.mediaActive || root.activeList !== saved.view || controller.rootPath !== saved.rootPath || !saved.path) return
    var view = saved.view
    view.forceLayout()
    for (var i = 0; i < view.count; i++) {
      if (String(view.model.get(i).path) !== saved.path) continue
      view.positionViewAtIndex(i, ListView.Beginning)
      var item = view.itemAtIndex(i)
      var offset = item ? Math.min(Math.max(0, item.height - 1), saved.fraction * item.height) : 0
      view.contentY = Math.min(view.originY + Math.max(0, view.contentHeight - view.height), view.contentY + offset)
      break
    }
  }

  function toggleMedia() {
    if (!mediaMode) {
      ordinaryContentY = controller.searching ? searchList.contentY : treeList.contentY
      if (!mediaQueryReady) mediaQuery = controller.searchQuery
      mediaQueryReady = true
      var eligible = controller.selectedEntries.filter(function(entry) {
        return !!MediaModel.kind(entry) && (root.mediaRecursive
          ? String(entry.path).indexOf(controller.rootPath === "/" ? "/" : controller.rootPath + "/") === 0
          : controller.parentDirectory(entry.path) === controller.rootPath)
      }).map(MediaModel.record)
      eligible = MediaModel.matching(eligible, mediaQuery,
        { caseSensitive: controller.searchCaseSensitive, regex: controller.searchRegex }, controller.treeFilter).rows
      var primary = eligible.filter(function(entry) { return entry.path === controller.selectedPath })[0]
      controller.applySelection(eligible, primary || null, controller.selectionAnchorPath)
    }
    mediaMode = !mediaMode
    visualMode = false
    persistMedia()
    Qt.callLater(function() {
      if (!root.mediaMode) {
        var ordinary = controller.searching ? searchList : treeList
        ordinary.contentY = root.ordinaryContentY
      }
      root.focusTree()
    })
  }

  function reconcileMediaSelection() {
    if (!focusEnabled || !mediaActive || !mediaProvider || mediaProvider.busy) return
    var kept = MediaModel.retained(controller.selectedEntries, mediaMatches.rows)
    if (kept.length !== controller.selectedEntries.length) {
      var primary = kept.filter(function(entry) { return entry.path === controller.selectedPath })[0]
      controller.applySelection(kept, primary || null, controller.selectionAnchorPath)
    }
  }

  function mediaAllows(action) {
    var capability = ({ copy: "read", cut: "rename", paste: "write", rename: "rename", trash: "trash",
      "new-folder": "mkdir", "new-file": "write", open: "read", activate: "read", "open-with": "read", editor: "read" })[action]
    if (action === "actions" && mediaLocationDescriptor)
      return ["read", "write", "rename", "trash"].every(function(key) { return MediaModel.allows(root.mediaLocationDescriptor, key) })
    return !capability || MediaModel.allows(mediaLocationDescriptor, capability)
  }

  onMediaRecursiveChanged: persistMedia()
  onMediaQueryChanged: { persistMedia(); if (mediaProvider && !mediaProvider.busy) reconcileMediaSelection() }
  onMediaSizeStepChanged: persistMedia()
  onMediaShowEmptyPeriodsChanged: persistMedia()
  onOrdinaryDensityStepChanged: persistMedia()
  onToolbarButtonsChanged: persistMedia()
  onVolumesInTreeChanged: persistMedia()
  onMediaMatchesChanged: if (mediaProvider && !mediaProvider.busy) reconcileMediaSelection()

  Component.onCompleted: {
    if (!context || !context.state) return
    mediaQuery = String(context.state.get("mediaQuery", ""))
    mediaSizeStep = Math.max(0, Math.min(4, Number(context.state.get("mediaSizeStep", 2))))
    mediaShowEmptyPeriods = context.state.get("mediaShowEmptyPeriods", false) === true
    ordinaryDensityValue = root.restoredDensity(context.state.get("ordinaryDensityPercent", 100))
    toolbarButtons = ToolbarFields.normalizeFields(context.state.get("toolbarButtons", undefined))
    volumesInTree = context.state.get("volumesInTree", controller.showSystemVolumes === true) === true
    summaryInTree = context.state.get("rootRowInTree", true) === true
    mediaRecursive = context.state.get("mediaRecursive", false) === true
    mediaMode = context.state.get("mediaMode", false) === true
    mediaQueryReady = context.state.get("mediaQueryReady", mediaMode) === true
    mediaStateReady = true
    Qt.callLater(root.ensureContextRoot)
  }
  readonly property string editorMode: root.visualMode ? "VISUAL" : (searchField.activeFocus || locationField.activeFocus ? "INSERT" : "NORMAL")

  Binding { target: root.controller; property: "editorMode"; value: root.editorMode }
  property var gitMarks: []
  readonly property var activeList: controller.trashMode
    ? trashView.list
    : (controller.drivesMode ? drivesView.list : (controller.recentMode ? recentList : (mediaActive && mediaView ? mediaView.flickable : (controller.searching ? searchList : treeList))))
  readonly property bool settingsActive: context
    ? (context.host.settingsOpen && context.host.settingsEdge === context.edge)
    : controller.settingsOpen
  readonly property real gitColumnInset: controller.gitEnabled ? Style.space(7) : 0
  readonly property real gitDetailSlotWidth: Style.space(38)
  readonly property real gitDetailSpacing: Style.space(2)
  readonly property real gitColumnWidth: !controller.gitEnabled
    ? 0
    : (controller.gitStatusDetails.length > 0
      ? controller.gitStatusDetails.length * gitDetailSlotWidth
          + (controller.gitStatusDetails.length - 1) * gitDetailSpacing
      : Style.space(18))
  readonly property real columnAdderReserve: browserHeader.addSlotWidth

  property int footerOffset: 0
  FontMetrics {
    id: footerFont
    font.family: Style.font.family
    font.pixelSize: Typography.caption
  }
  readonly property var footerLayout: {
    var parts = root.footerParts()
    var budget = Math.max(0, footerRow.width)
    return FooterFields.window(parts, root.footerOffset, function(text) {
      return footerFont.advanceWidth(text)
    }, budget, footerFont.advanceWidth(footerSeparator.text))
  }

  function footerFieldText(key) {
    if (key === "count") {
      return root.mediaActive && mediaView
        ? (mediaView.count === mediaProvider.rows.length ? mediaView.count : mediaView.count + "/" + mediaProvider.rows.length) + " media"
        : (controller.searching ? controller.searchResultCount + " matches"
          : (controller.recentMode ? recentList.count + " recent"
            : (root.folderCount.known ? root.folderCount.total + (Object.keys(controller.treeFilter).length ? " matching" : " items") : "Count unavailable")))
    }
    if (key === "selected") return controller.selectedCount ? controller.selectedCount + " selected" : ""
    if (key === "scope") {
      if (root.mediaActive) return root.mediaRecursive ? "recursive" : "folder"
      if (controller.searching) return "search"
      if (controller.recentMode) return "history"
      return "folder"
    }
    if (key === "activity") {
      if (root.mediaActive && mediaProvider) {
        if (mediaProvider.busy) return "loading"
        if (mediaProvider.limited) return "partial"
        return ""
      }
      if (controller.searching) return controller.searchBusy ? "searching" : ""
      return controller.treeLoading ? "loading" : ""
    }
    if (key === "loaded") {
      return !root.mediaActive && !controller.searching && !controller.recentMode
        && root.folderCount.loaded < root.folderCount.total ? root.folderCount.loaded + " loaded" : ""
    }
    return ""
  }

  function footerParts() {
    var keys = FooterFields.normalizeFields(controller.footerFields)
    var parts = []
    for (var i = 0; i < keys.length; i++) {
      var text = String(root.footerFieldText(keys[i]) || "")
      if (text !== "") parts.push(text)
    }
    var failure = root.branchError || (root.mediaActive && mediaProvider && mediaProvider.error ? String(mediaProvider.error) : "")
    if (failure) parts.push(failure)
    return parts
  }

  function nearestDensityStep(value) {
    var nearest = 0
    for (var i = 1; i < root.densityPresets.length; i++)
      if (Math.abs(root.densityPresets[i] - value) < Math.abs(root.densityPresets[nearest] - value)) nearest = i
    return nearest
  }

  function clampDensity(value) {
    var number = Number(value)
    if (!isFinite(number) || number <= 0) return 0
    if (number > 10) number = number / 100
    return Math.max(0.6, Math.min(1.8, number))
  }

  function restoredDensity(value) {
    var saved = root.clampDensity(value)
    return saved > 0 ? root.densityPresets[root.nearestDensityStep(saved)] : 1
  }

  function applyDensityText(text) {
    var typed = root.clampDensity(String(text).replace("%", "").trim())
    if (typed <= 0) return
    root.ordinaryDensityValue = typed
    root.persistMedia()
  }

  function targetScreen() {
    return hostWindow ? hostWindow.screen : null
  }

  function openBranchPicker(sceneX, sceneY) {
    if (!controller.gitEnabled || branchPicker.busy) return
    var point = root.mapFromItem(null, Number(sceneX) || 0, Number(sceneY) || 0)
    branchPicker.x = Math.max(Style.space(4), Math.min(point.x, root.width - branchPicker.menuWidth - Style.space(4)))
    branchPicker.y = Math.max(Style.space(4), point.y + Style.space(2))
    branchPicker.load()
  }

  function originX() {
    return context ? Number(context.surfaceOriginX) || 0 : 0
  }

  function focusNext() {
    if (context) context.focusNext()
    else controller.focusProperties(targetScreen())
  }

  function focusPrevious() {
    if (context) context.focusPrevious()
    else controller.focusProperties(targetScreen())
  }

  function toggleSettings() {
    if (context) context.toggleSettings()
    else controller.toggleSettings("left")
  }

  function closeSettings() {
    if (context) context.host.setSettingsOpen(false, context.edge)
    else controller.setSettingsOpen(false)
  }

  function showTrash(confirmEmpty) {
    if (!controller.trashMode)
      controller.navigateToLocation(controller.trashResource, targetScreen(), "browse")
    Qt.callLater(function() {
      root.focusTree()
      if (confirmEmpty) trashView.confirmEmpty()
    })
  }

  function showRecent() {
    if (!controller.recentMode)
      controller.navigateToLocation(controller.recentResource, targetScreen(), "browse")
    Qt.callLater(root.focusTree)
  }

  function showDrives() {
    if (!controller.drivesMode)
      controller.navigateToLocation(controller.drivesResource, targetScreen(), "browse")
    Qt.callLater(root.focusTree)
  }

  function drivesNavigationContext() {
    var count = controller.drivesController.volumeCount
    return [{ glyph: "󰋊", text: count + (count === 1 ? " volume" : " volumes") }]
  }

  function recentNavigationContext() {
    var count = controller.recentModel.count
    return [{ glyph: "󰈔", text: controller.recentMode ? count + (count === 1 ? " file" : " files") : "Recently opened files" }]
  }

  function trashNavigationContext() {
    return [
      { glyph: "󰉋", text: controller.trashCount + (controller.trashCount === 1 ? " item" : " items") },
      { glyph: "󰔛", text: "Last cleared: " + controller.trashLastClearedText },
      { glyph: "󰃭", text: "Next automatic clear: " + controller.trashNextCleanupText }
    ]
  }

  function preferredColumnWidth(key) {
    var widths = { modified: 108, created: 108, repo: 90, branch: 90, worktree: 90, type: 72, size: 66 }
    return Style.space(widths[key] || 66)
  }

  function setGitStatusDetail(key, enabled) {
    var next = controller.gitStatusDetails.slice()
    var index = next.indexOf(key)
    if (enabled && index < 0) next.push(key)
    else if (!enabled && index >= 0) next.splice(index, 1)
    controller.setGitStatusDetails(next)
  }

  function priorityColumnWidth(rowWidth, key) {
    var keys = controller.priorityColumns
    var total = 0
    for (var i = 0; i < keys.length; i++) total += preferredColumnWidth(keys[i])
    var budget = Math.max(Style.space(60), rowWidth * (keys.length > 1 ? 0.48 : 0.36))
    var scale = total > 0 ? Math.min(1, budget / total) : 1
    return Math.max(Style.space(44), Math.round(preferredColumnWidth(key) * scale))
  }

  function matchesScreen(target) {
    return !target || target === targetScreen()
  }

  property bool visualMode: false

  function modelRow(index, treeMode, view) {
    var model = view ? view.model : (treeMode ? controller.treeModel : controller.searchModel)
    return index >= 0 && index < model.count ? model.get(index) : null
  }

  property bool revealPending: false

  function restoreTreeCursor() {
    var index = controller.indexOfTreePath(controller.selectedPath)
    if (contextAbove && index === 0) { treeList.currentIndex = -1; return }
    if (index < 0) return
    treeList.currentIndex = index
    if (!root.revealPending) return
    root.revealPending = false
    if (treeList.visible && !treeAnchor.pending) treeList.positionViewAtIndex(index, ListView.Contain)
  }

  function selectionMode(modifiers) {
    var shift = !!(modifiers & Qt.ShiftModifier)
    var control = !!(modifiers & Qt.ControlModifier)
    if (shift && control) return "add-range"
    if (shift) return "range"
    if (control) return "toggle"
    return "replace"
  }

  function selectIndex(view, treeMode, index, mode) {
    if (!view || view.count === 0) return
    var first = treeMode && contextAbove ? 1 : 0
    if (view.count <= first) return
    var next = Math.max(first, Math.min(view.count - 1, index))
    view.currentIndex = next
    treeAnchor.clear()
    view.positionViewAtIndex(next, ListView.Contain)
    var row = modelRow(next, treeMode, view)
    if (row && row.kind === "More") {
      controller.loadMoreChildren(String(row.path).slice(0, -5))
      return
    }
    if (mode === "keep") return
    if (first) {
      var anchor = controller.selectionAnchorPath === controller.rootPath ? view.model.get(first).path : controller.selectionAnchorPath
      if (controller.isSelected(controller.rootPath)) {
        var kept = controller.selectedEntries.filter(function(entry) { return entry.path !== controller.rootPath })
        var primary = kept.filter(function(entry) { return entry.path === controller.selectedPath })[0]
        controller.applySelection(kept, primary || kept[0] || null, anchor)
      } else controller.selectionAnchorPath = anchor
    }
    controller.selectModelIndex(view.model, next, mode || "replace")
  }

  function moveCurrent(view, treeMode, delta, extend, additive) {
    if (!view || view.count === 0) return
    var current = view.currentIndex
    if (current < 0) current = delta < 0 ? view.count : -1
    selectIndex(view, treeMode, current + delta, extend ? (additive ? "add-range" : "range") : "replace")
  }

  function movePage(view, treeMode, direction, extend) {
    if (mediaView && view === mediaView) {
      moveCurrent(view, false, direction * mediaView.columns * Math.max(1, Math.floor(mediaView.height / (mediaView.cell + mediaView.labelHeight))), extend, false)
      return
    }
    var rowHeight = view.currentItem ? view.currentItem.height : Style.space(30)
    var rows = Math.max(1, Math.floor(view.height / Math.max(1, rowHeight) * 0.8))
    moveCurrent(view, treeMode, direction * rows, extend, false)
  }

  function activateIndex(view, treeMode, index, enterFolder) {
    if (treeMode && contextAbove) index = Math.max(1, index)
    if (mediaView && view === mediaView) {
      reconcileMediaSelection()
      if (!mediaAllows("open")) return
    }
    var row = modelRow(index, treeMode, view)
    if (!row || row.gitDeleted) return
    if (row.kind === "More") {
      controller.loadMoreChildren(String(row.path).slice(0, -5))
      return
    }
    if (!controller.isSelected(row.path)) selectIndex(view, treeMode, index, "replace")
    if (row.kind === "Match") {
      controller.openAtLine(row.path, Number(row.relative.slice(row.relative.lastIndexOf(":") + 1)))
      return
    }
    if (row.isDir) {
      if (treeMode && !enterFolder) controller.toggleDirectory(index)
      else controller.navigateToLocation(row.path, targetScreen(), "browse")
    } else {
      if (!controller.activatePickerEntry(row.path, false, row.name)) controller.openDefault(row.path, targetScreen(), false)
    }
  }

  function foldCurrent(view, expanded) {
    var row = modelRow(Math.max(view === treeList && contextAbove ? 1 : 0, view.currentIndex), true, view)
    if (row && row.isDir && !row.gitDeleted) controller.setDirectoryExpanded(row.path, expanded)
  }

  function trashPrompt(paths) {
    var entries = Array.isArray(controller.selectedEntries) ? controller.selectedEntries : []
    var names = paths.map(function(path) { return String(path).split("/").filter(Boolean).pop() || String(path) })
    var shown = names.slice(0, 4).join(", ") + (names.length > 4 ? ", +" + (names.length - 4) + " more" : "")
    var repository = entries.some(function(entry) { return !!entry && !!entry.isGitRepo && paths.indexOf(String(entry.path)) >= 0 })
    var head = paths.length === 1 ? "Move to Trash?" : "Move " + paths.length + " items to Trash?"
    return head + "\n" + shown + (repository ? "\nGit repository" : "")
  }

  function focusTree() {
    locationEditing = false
    if (controller.trashMode) {
      trashView.focusList()
      return
    }
    if (controller.drivesMode) {
      drivesView.focusList()
      return
    }
    if (mediaActive && mediaView) {
      mediaView.currentIndex = mediaView.indexOfPath(controller.selectedPath)
      mediaView.forceActiveFocus()
      return
    }
    var flat = controller.recentMode || controller.searching
    var view = controller.recentMode ? recentList : (controller.searching ? searchList : treeList)
    if (view.count > 0 && view.currentIndex < 0 && (!controller.selectedPath || controller.selectedPath === controller.rootPath)) selectIndex(view, !flat, 0)
    view.forceActiveFocus()
  }

  function focusSearch() {
    locationEditing = false
    searchField.reveal()
  }

  function toggleDeepSearch() {
    if (controller.quickNavActive) controller.stopQuickNav()
    if (mediaActive) mediaRecursive = !mediaRecursive
    else controller.toggleSearchDeep()
    focusSearch()
  }

  function focusLocation() {
    locationEditing = true
    locationField.text = controller.rootPath
    controller.clearLocationValidationError()
    Qt.callLater(function() {
      if (!root.locationEditing) return
      locationField.forceActiveFocus()
      locationField.selectAll()
    })
  }

  function openMenuForCurrent(view, mode) {
    var first = view === treeList && contextAbove ? 1 : 0
    var empty = !view || view.count <= first
    var creating = mode === "new-file" || mode === "new-folder"
    if (empty && !creating) return
    var index = Math.max(first, view ? view.currentIndex : first)
    var delegate = view && view.itemAtIndex ? view.itemAtIndex(index) : null
    if (delegate && delegate.openMenu) {
      delegate.openMenu(mode || "actions", 0, 0, true)
      return
    }
    var edge = context ? String(context.edge || "left") : "left"
    var x = edge === "right" ? originX() + Style.space(2) : root.width - Style.space(8) + originX()
    var row = !empty ? modelRow(index, view === treeList, view) : null
    if (row && !controller.isSelected(row.path)) selectIndex(view, view === treeList, index, "replace")
    var destination = empty && creating ? [{ path: controller.rootPath, isDir: true }] : undefined
    controller.openActionMenu(mode || "actions", targetScreen(), x, Style.space(70), destination, { edge: edge, keyboard: true })
  }

  function runBrowserAction(action) {
    var actions = {
      media: function() { root.toggleMedia() },
      location: function() { controller.focusLocation(targetScreen()) },
      hidden: function() { controller.toggleHidden() },
      back: function() { controller.goBack(targetScreen()) },
      forward: function() { controller.goForward(targetScreen()) },
      up: function() { controller.goUp() },
      home: function() { controller.goHome() },
      screenshots: function() { if (controller.screenshotsPath) controller.setRootPath(controller.screenshotsPath) },
      recent: function() { root.showRecent() },
      drives: function() { root.showDrives() },
      "desktop-trash": function() { root.showTrash(false) }
    }
    var handler = actions[action]
    if (!handler) return false
    handler()
    return true
  }

  function handleBrowserShortcut(event) {
    return runBrowserAction(KeyRouter.browserAction(event))
  }

  function handleFieldShortcut(event) {
    if (KeyRouter.listModeAction(event, false) === "deep") {
      toggleDeepSearch()
      return true
    }
    if (event.key === Qt.Key_Backspace) return false
    return handleBrowserShortcut(event)
  }

  function selectAll(view, treeMode) {
    if (view.count === 0) return
    selectIndex(view, treeMode, 0, "replace")
    if (!treeMode && view !== mediaView) controller.loadAllSearchRows()
    selectIndex(view, treeMode, view.count - 1, "range")
  }

  function dismissList() {
    if (controller.actionMenuOpen) controller.closeActionMenu()
    else if (controller.pickerActive) controller.cancelPicker()
    else if (controller.quickNavActive || controller.searching) {
      controller.stopQuickNav()
      Qt.callLater(focusTree)
    } else if (root.settingsActive) closeSettings()
    else controller.setOpen(false)
  }

  function runListAction(action, event, view, treeMode) {
    if (runBrowserAction(action)) return true
    if (mediaView && view === mediaView) {
      reconcileMediaSelection()
      if (!mediaAllows(action)) return true
      if (action === "refresh") { mediaProvider.reload(true); return true }
      if (action === "dismiss" && mediaQuery !== "") { mediaQuery = ""; return true }
      if (["next", "previous", "next-extend", "previous-extend", "expand", "collapse"].indexOf(action) >= 0) {
        var delta = action === "expand" ? 1 : (action === "collapse" ? -1 : (action.indexOf("previous") === 0 ? -mediaView.columns : mediaView.columns))
        moveCurrent(view, false, delta, root.visualMode || action.indexOf("-extend") >= 0 || !!(event.modifiers & Qt.ShiftModifier), !!(event.modifiers & Qt.ControlModifier))
        return true
      }
      if (["expand-recursive", "collapse-recursive", "expand-all", "collapse-all", "layout"].indexOf(action) >= 0) return true
    }
    var handlers = {
      quicknav: function() { controller.startQuickNav(targetScreen(), "folders") },
      picker: function() { controller.startQuickNav(targetScreen(), "files") },
      search: function() {
        if (controller.quickNavActive) controller.stopQuickNav()
        focusSearch()
      },
      help: function() { if (hostWindow) hostWindow.shortcutsOpen = true },
      layout: function() { controller.setSearchLayout(!controller.searchTreeLayout) },
      deep: function() { toggleDeepSearch() },
      "focus-next": function() { focusNext() },
      "focus-previous": function() { focusPrevious() },
      next: function() { moveCurrent(view, treeMode, 1, !!(event.modifiers & Qt.ShiftModifier), !!(event.modifiers & Qt.ControlModifier)) },
      previous: function() { moveCurrent(view, treeMode, -1, !!(event.modifiers & Qt.ShiftModifier), !!(event.modifiers & Qt.ControlModifier)) },
      "page-next": function() { movePage(view, treeMode, 1, false) },
      "page-previous": function() { movePage(view, treeMode, -1, false) },
      first: function() { selectIndex(view, treeMode, 0, event.modifiers & Qt.ShiftModifier ? "range" : "replace") },
      last: function() {
        if (!treeMode && view !== mediaView) controller.loadAllSearchRows()
        selectIndex(view, treeMode, view.count - 1, event.modifiers & Qt.ShiftModifier ? "range" : "replace")
      },
      collapse: function() { foldCurrent(view, false) },
      expand: function() { foldCurrent(view, true) },
      "expand-recursive": function() { var row = modelRow(view.currentIndex, treeMode, view); if (row) controller.setBranchExpanded(row.path, true) },
      "collapse-recursive": function() { var row = modelRow(view.currentIndex, treeMode, view); if (row) controller.setBranchExpanded(row.path, false) },
      "expand-all": function() { controller.setBranchExpanded(controller.rootPath, true) },
      "collapse-all": function() { collapseTree(view) },
      "open-with": function() { openMenuForCurrent(view, "open-with") },
      activate: function() { activateIndex(view, treeMode, view.currentIndex < 0 ? 0 : view.currentIndex) },
      open: function() { activateIndex(view, treeMode, view.currentIndex < 0 ? 0 : view.currentIndex, true) },
      editor: function() { controller.openInEditor(controller.selectedPath) },
      "toggle-selection": function() { selectIndex(view, treeMode, view.currentIndex < 0 ? 0 : view.currentIndex, "toggle") },
      "select-all": function() { selectAll(view, treeMode) },
      copy: function() { controller.copySelection(false) },
      cut: function() { controller.copySelection(true) },
      paste: function() { controller.pasteInto(controller.selectionDestination()) },
      undo: function() { controller.history.undoOperation() },
      redo: function() { controller.history.redoOperation() },
      "skip-refused": function() { controller.history.skipRefused() },
      "new-folder": function() { openMenuForCurrent(view, "new-folder") },
      "new-file": function() { openMenuForCurrent(view, "new-file") },
      rename: function() { openMenuForCurrent(view, "rename") },
      trash: function() { controller.requestTrash() },
      actions: function() { openMenuForCurrent(view, "actions") },
      refresh: function() { controller.refreshTree() },
      dismiss: function() { dismissList() },
      close: function() { controller.setOpen(false) },
      visual: function() { toggleVisual(view, treeMode) },
      "visual-exit": function() { exitVisual(view, treeMode) },
      "next-extend": function() { moveCurrent(view, treeMode, 1, true, false) },
      "previous-extend": function() { moveCurrent(view, treeMode, -1, true, false) },
      "page-next-extend": function() { movePage(view, treeMode, 1, true) },
      "page-previous-extend": function() { movePage(view, treeMode, -1, true) },
      "first-extend": function() { selectIndex(view, treeMode, 0, "range") },
      "last-extend": function() {
        if (!treeMode && view !== mediaView) controller.loadAllSearchRows()
        selectIndex(view, treeMode, view.count - 1, "range")
      }
    }
    var handler = handlers[action]
    if (!handler) return false
    handler()
    if (root.visualMode && ["copy", "cut", "trash", "actions", "activate", "open", "paste", "search", "quicknav", "picker", "deep"].indexOf(action) >= 0)
      root.visualMode = false
    return true
  }

  function toggleVisual(view, treeMode) {
    if (root.visualMode) {
      exitVisual(view, treeMode)
      return
    }
    if (!view || view.count === 0) return
    if (view.currentIndex < 0 || controller.selectedCount === 0) selectIndex(view, treeMode, Math.max(0, view.currentIndex), "replace")
    root.visualMode = true
  }

  function exitVisual(view, treeMode) {
    root.visualMode = false
    if (view && view.count > 0) selectIndex(view, treeMode, view.currentIndex < 0 ? 0 : view.currentIndex, "replace")
  }

  function handleListKey(event, view, treeMode) {
    if (controller.dropWheel.handleDragKey(event)) {
      treeKeys.reset()
      event.accepted = true
      return
    }
    var repeated = actionKeyGuard.isRepeat(event)
    var state = {
      count: controller.selectedCount,
      deleted: controller.selectionHasDeleted,
      directory: controller.selectedCount === 1 && !!controller.selectedEntries[0].isDir,
      visual: root.visualMode
    }
    var action = treeKeys.action(event, repeated, KeyRouter.listAction(event, treeMode, state))
    if (action === "" && view !== mediaView && !controller.trashMode && !controller.drivesMode
        && !(event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier))
        && [Qt.Key_Plus, Qt.Key_Equal, Qt.Key_Minus, Qt.Key_Underscore].indexOf(event.key) >= 0) {
      root.changeDensity(root.ordinaryDensityStep + (event.key === Qt.Key_Minus || event.key === Qt.Key_Underscore ? -1 : 1))
      event.accepted = true
      return
    }
    if (action.indexOf("key-") === 0) { event.accepted = true; return }
    if (root.visualMode && ["next", "previous", "first", "last", "page-next", "page-previous"].indexOf(action) >= 0) action += "-extend"
    if (KeyRouter.ignoresAutoRepeat(action, event.key) && repeated) {
      event.accepted = true
      return
    }
    if (!runListAction(action, event, view, treeMode)) return
    event.accepted = true
  }

  Connections {
    target: controller

    function onTreeStructureRevisionChanged() { root.restoreTreeCursor() }
    function onSelectedPathChanged() { root.revealPending = true; root.restoreTreeCursor() }
    function onRootPathChanged() { treeKeys.reset() }
    function onFilesystemRefreshCountChanged() { if (root.mediaActive) mediaRefresh.restart() }
    function onLocationValidationFinished(targetScreen, success, path, error, monitor) {
      if (!root.focusEnabled || !root.matchesScreen(targetScreen)) return
      var requested = String(monitor || "")
      root.locationEditing = !success
      Qt.callLater(function() {
        if (!root.focusEnabled || requested !== root.controller.bladeHost.focusedMonitorName) return
        if (success) {
          root.focusTree()
          return
        }
        locationField.forceActiveFocus()
        locationField.selectAll()
      })
    }
  }

  PluginUi.PaneView {
    id: filesView
    persist: false
    pinnedSortKeys: ["git"]
    options: controller.priorityPropertyChoices
    columns: controller.priorityColumns
    sorts: controller.treeSort
    filter: controller.treeFilter
    navigationActions: {
      var live = {
        "back": { enabled: controller.canGoBack,
          actions: [{ button: "left", text: "Back" }, { shortcut: "Alt+←" }], context: [{ glyph: "󰉋", text: controller.backDestination }] },
        "forward": { enabled: controller.canGoForward,
          actions: [{ button: "left", text: "Forward" }, { shortcut: "Alt+→" }], context: [{ glyph: "󰉋", text: controller.forwardDestination }] },
        "up": { enabled: controller.canGoUp,
          actions: [{ button: "left", text: "Up" }, { shortcut: "Alt+↑" }], context: [{ glyph: "󰉋", text: controller.parentDirectory(controller.rootPath) }] },
        "home": { enabled: controller.rootPath !== controller.home,
          actions: [{ button: "left", text: "Home" }, { shortcut: "Alt+Home" }], context: [{ glyph: "󰉋", text: controller.home }] },
        "screenshots": { enabled: controller.screenshotsPath !== "" && controller.rootPath !== controller.screenshotsPath,
          actions: [{ button: "left", text: "Screenshots" }],
          context: [{ glyph: "󰉋", text: controller.screenshotsPath }] },
        "recent": { active: controller.recentMode,
          actions: [{ button: "left", text: "Open" }], context: root.recentNavigationContext() },
        "media": { title: root.mediaMode ? "Show files" : "Show media", active: root.mediaMode,
          enabled: !controller.trashMode && !controller.drivesMode && !controller.recentMode,
          actions: [{ button: "left", text: "Switch content mode" }] },
        "drives": { active: controller.drivesMode,
          actions: [{ button: "left", text: "Open" }], context: root.drivesNavigationContext() },
        "desktop-trash": Object.assign(controller.trashCount > 0 ? { glyph: "󰩹" } : {}, { active: controller.trashMode,
          actions: [{ button: "left", text: "Open" }], context: root.trashNavigationContext() })
      }
      return ToolbarFields.choices
        .filter(function(choice) { return root.toolbarButtons.indexOf(choice.key) >= 0 })
        .map(function(choice) { return Object.assign({ key: choice.key, glyph: choice.glyph, title: choice.label }, live[choice.key]) })
    }
    onColumnsCommitted: controller.setPriorityColumns(columns)
    onSortsChanged: root.pushTreeOrder()
    onFilterChanged: root.pushTreeOrder()
    onNavigationTriggered: function(key) {
      if (!root.runBrowserAction(key)) return
      Qt.callLater(root.focusTree)
    }
  }

  function pushTreeOrder() {
    if (treeOrderUpdating) return
    if (JSON.stringify(filesView.sorts) === JSON.stringify(controller.treeSort) && JSON.stringify(filesView.filter) === JSON.stringify(controller.treeFilter)) return
    if (mediaView) mediaView.rememberAnchor()
    treeOrderUpdating = true
    try { controller.setTreeOrder(filesView.sorts, filesView.filter) }
    finally { treeOrderUpdating = false }
  }

  Connections {
    target: controller
    function onPriorityColumnsChanged() { filesView.columns = controller.priorityColumns }
    function onTreeSortChanged() { filesView.sorts = controller.treeSort }
    function onTreeFilterChanged() { filesView.filter = controller.treeFilter }
  }

  PluginUi.PaneHeader {
    id: header
    anchors.top: parent.top
    preferredHeight: header.integrated ? 0 : Style.space(34)
    context: root.context
    title: "FILEBLADE"
    reservedLeft: root.context ? root.context.cornerReserveLeft : 0
    reservedRight: root.context ? root.context.cornerReserveRight : 0
    status: treeKeys.hint !== "" ? treeKeys.hint : (controller.modeBadge === "header" ? root.editorMode : "")
    statusGlyph: treeKeys.hint !== "" ? "" : (controller.modeBadge === "header" ? "\ue6ae" : "")
    statusColor: treeKeys.hint !== "" ? Color.muted : controller.editorModeColor
    highlighted: root.activeFocus || root.visualMode || (root.context && root.context.dragging)
    Row {
      anchors.verticalCenter: parent ? parent.verticalCenter : undefined
      spacing: Style.space(5)
      visible: !header.integrated
      PluginUi.PaneCorner {
        glyph: "󰑐"
        tip: controller.trashMode ? "Refresh Trash" : "Refresh"
        tipActions: [{ button: "left", text: "Refresh" }, { shortcut: controller.trashMode ? "R" : "Shift+R" }]
        onActivated: controller.trashMode ? controller.refreshTrash() : controller.refreshTree()
      }

      PluginUi.PaneCorner {
        glyph: "󰒓"
        tip: "Settings"
        tipActions: [{ button: "left", text: "Open settings" }, { shortcut: "," }]
        active: root.settingsActive
        onActivated: root.toggleSettings()
      }
    }
  }

  FavoritesPanel {
    id: favoritesSection
    anchors.top: header.bottom
    anchors.left: parent.left
    anchors.right: parent.right
    controller: root.controller
    pane: root
  }

  Item {
    id: drivesSection
    anchors.top: favoritesSection.bottom
    anchors.left: parent.left
    anchors.right: parent.right
    height: root.volumesInTree ? volumes.height : 0
    visible: height > 0

    DrivesPanel {
      id: volumes
      anchors.left: parent.left
      anchors.right: parent.right
      controller: root.controller
      pane: root
    }
  }

  PluginUi.PaneSearchField {
    id: searchField
    service: root.controller
    anchors.top: drivesSection.bottom
    anchors.topMargin: visible ? Style.space(4) : 0
    anchors.left: parent.left
    anchors.right: parent.right
    text: root.mediaActive ? root.mediaQuery : (controller.quickNavActive ? "" : controller.searchQuery)
    enabled: !controller.trashMode && !controller.recentMode && !controller.drivesMode
    readonly property string quickNavKeys: controller.keybindings.label("quicknav")
    prompt: controller.searchListActive ? "Filter " + controller.searchListTitle + "..."
      : (quickNavKeys === "Unbound" ? "Search..." : "Search... (" + quickNavKeys + " to quick nav)")
    showOptions: true
    showDeepOption: true
    caseSensitive: controller.searchCaseSensitive
    regex: controller.searchRegex

    onTextEdited: { if (root.mediaActive) root.mediaQuery = text; else controller.searchQuery = text }
    onCleared: { if (root.mediaActive) root.mediaQuery = ""; else controller.searchQuery = "" }
    onAdvanced: root.focusTree()
    onOptionsToggled: function(nextCase, nextRegex) { controller.setSearchOptions(nextCase, nextRegex) }
    deep: root.mediaActive ? root.mediaRecursive : controller.searchDeepActive
    onDeepToggled: { if (root.mediaActive) root.mediaRecursive = !root.mediaRecursive; else controller.toggleSearchDeep() }
    onHistoryStepped: function(delta) {
      var recalled = controller.recallSearch(delta)
      if (recalled === null) return
      browsingHistory = recalled !== ""
      text = recalled
      if (root.mediaActive) root.mediaQuery = recalled
      else controller.searchQuery = recalled
      cursorPosition = text.length
    }

    onAccepted: {
      if (root.mediaActive) { root.focusTree(); return }
      if (controller.searching && searchList.count > 0)
        root.activateIndex(searchList, false, searchList.currentIndex < 0 ? 0 : searchList.currentIndex)
    }

    onDismissed: {
      if (root.mediaActive) { root.mediaQuery = ""; root.focusTree(); return }
      if (text !== "") {
        text = ""
        controller.searchQuery = ""
      }
      controller.stopList()
      root.focusTree()
    }

    Keys.priority: Keys.BeforeItem

    Keys.onTabPressed: function(event) {
      var modified = event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier | Qt.ShiftModifier)
      if (!modified) root.focusNext()
      event.accepted = !modified
    }

    Keys.onBacktabPressed: function(event) {
      var modified = event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier)
      if (!modified) root.focusPrevious()
      event.accepted = !modified
    }

    Keys.onPressed: function(event) {
      if (root.handleFieldShortcut(event)) event.accepted = true
    }

    Connections {
      target: controller
      function onSearchQueryChanged() {
        if (root.mediaActive || controller.quickNavActive) return
        if (searchField.text !== controller.searchQuery) searchField.text = controller.searchQuery
      }
    }
  }

  PluginUi.PaneRow {
    id: contextSummary
    anchors.top: searchField.bottom
    anchors.topMargin: searchField.visible ? Style.space(4) : 0
    anchors.left: parent.left
    anchors.right: parent.right
    readonly property var entry: visible && controller.treeModel.count > 0 && controller.treeModel.get(0).path === controller.rootPath ? controller.treeModel.get(0) : null
    readonly property var summary: entry && entry.isGitRepo && controller.gitEnabled && controller.gitSummaryFields.length
      ? GitSummary.describe(entry.gitSummary, controller.gitSummaryFields) : ({})
    visible: root.contextAbove
    height: visible ? Style.space(30) : 0
    glyph: FileIcons.entryIcon(entry ? entry.name : "", true, entry ? entry.isSymlink : false, false,
      entry ? entry.isGitRepo : false, entry ? entry.path === controller.home : false)
    glyphColor: entry && entry.error ? Color.urgent : Color.accent
    label: entry && entry.path !== controller.home ? entry.name : controller.rootName(controller.rootPath)
    detail: summary.identity || ""
    badge: entry && entry.error ? "Unavailable" : (summary.text || "")
    columnWidths: badge ? [Math.min(width * 0.38, Style.space(150))] : []
    emphasized: true
    Accessible.role: Accessible.StaticText
    Accessible.name: [label, detail, badge].filter(Boolean).join(" ")
    HoverHandler { id: contextSummaryHover }
    PanelToolTip {
      visible: contextSummaryHover.hovered && text !== ""
      text: contextSummary.summary.tooltip || ""
    }
  }

  Item {
    id: navigationBar
    anchors.top: contextSummary.bottom
    anchors.topMargin: Style.space(4)
    anchors.left: parent.left
    anchors.right: parent.right
    height: Style.space(28)

    PluginUi.PaneHeader {
      id: browserHeader
      anchors.fill: parent
      visible: !root.locationEditing
      showIdentity: false
      extendAddGuide: false
      preferredHeight: navigationBar.height
      view: filesView
      widthFor: function(key) { return root.priorityColumnWidth(root.width, key) }

      PluginUi.MetricPicker {
        visible: controller.gitEnabled
        view: filesView
        pinnedKey: "git"
        pinnedGlyph: "󰊢"
        detailOptions: controller.gitStatusDetailChoices
        detailValues: controller.gitStatusDetails
        triggerWidth: root.gitColumnWidth
        onDetailToggled: function(key, enabled) { root.setGitStatusDetail(key, enabled) }
      }
      PluginUi.PaneCorner { visible: controller.trashMode; enabled: controller.trashCount > 0 && !controller.trashOperationBusy; opacity: enabled ? 1 : 0.32; glyph: "󰩹"; tip: "Empty Trash"; tipActions: [{ button: "left", text: "Empty the trash" }, { shortcut: "Shift+E" }]; onActivated: trashView.confirmEmpty() }
    }
    TextField {
      id: locationField
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.leftMargin: Style.space(7)
      anchors.rightMargin: Style.space(7)
      anchors.verticalCenter: parent.verticalCenter
      height: Style.space(26)
      visible: root.locationEditing
      readOnly: controller.locationValidationBusy
      selectByMouse: true
      leftPadding: Style.space(25)
      rightPadding: controller.locationValidationBusy ? Style.space(66) : Style.space(8)
      color: Color.bar.text
      selectionColor: Util.alpha(Color.accent, 0.38)
      selectedTextColor: Color.bar.text
      placeholderText: "/path/to/folder"
      placeholderTextColor: Color.muted
      font.family: Style.font.family
      font.pixelSize: Typography.bodySmall

      background: Rectangle {
        color: Util.alpha(Color.bar.text, locationField.activeFocus ? 0.10 : 0.06)
        radius: Math.min(Style.cornerRadius, Style.space(4))
        border.width: 1
        border.color: controller.locationValidationError
          ? Color.urgent
          : (locationField.activeFocus ? Color.accent : Util.alpha(Color.bar.text, 0.18))
      }

      Text {
        textFormat: Text.PlainText
        anchors.left: parent.left
        anchors.leftMargin: Style.space(8)
        anchors.verticalCenter: parent.verticalCenter
        text: ""
        color: controller.locationValidationError ? Color.urgent : Color.muted
        font.family: Style.font.family
        font.pixelSize: Typography.caption
      }

      Text {
        textFormat: Text.PlainText
        anchors.right: parent.right
        anchors.rightMargin: Style.space(8)
        anchors.verticalCenter: parent.verticalCenter
        visible: controller.locationValidationBusy
        text: "checking…"
        color: Color.muted
        font.family: Style.font.family
        font.pixelSize: Typography.caption
      }

      onTextEdited: controller.clearLocationValidationError()
      onAccepted: {
        if (!controller.locationValidationBusy)
          controller.navigateToLocation(text, root.targetScreen())
      }

      Keys.onEscapePressed: function(event) {
        controller.cancelLocationValidation()
        root.locationEditing = false
        Qt.callLater(root.focusTree)
        event.accepted = true
      }
      Keys.priority: Keys.BeforeItem
      Keys.onPressed: function(event) {
        if (root.handleFieldShortcut(event)) event.accepted = true
      }
    }

    PluginUi.CapacityBar {
      objectName: "capacityBar"
      fraction: root.capacityShown ? controller.capacity.fraction : -1
      fillColor: Util.alpha(controller.themedFolderColor("blue", ViewChrome.CAPACITY_FALLBACK_BLUE), 0.85)
      tipTitle: root.capacityShown ? controller.capacity.tip.title : ""
      tipContext: root.capacityShown ? controller.capacity.tip.context : []
    }
  }

  Text {
    textFormat: Text.PlainText
    id: statusText
    anchors.top: navigationBar.bottom
    anchors.topMargin: visible ? Style.space(5) : 0
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.leftMargin: Style.space(9)
    anchors.rightMargin: Style.space(9)
    visible: (!root.mediaActive && controller.searching) || controller.locationValidationError !== "" || controller.treeExpansionError !== "" || controller.keybindings.error !== ""
      || controller.rootRecoveryNotice !== "" || controller.backendStalled || controller.backendVersionSkew
      || (!controller.backendReady && controller.backendError !== "")
    height: visible ? Style.space(20) : 0
    verticalAlignment: Text.AlignVCenter
    text: {
      if (controller.backendStalled) return "Backend keeps exiting: " + controller.backendError
      if (!controller.backendReady && controller.backendError) return controller.backendError
      if (controller.backendVersionSkew) return "Backend mismatch; update or reinstall FileBlade and restart the shell (binary " + controller.backendVersion + ", plugin " + (controller.manifest && controller.manifest.version ? controller.manifest.version : "") + ")"
      if (controller.locationValidationError) return controller.locationValidationError
      if (controller.treeExpansionError) return controller.treeExpansionError
      if (controller.keybindings.error) return "Keybindings: " + controller.keybindings.error
      if (controller.rootRecoveryNotice) return controller.rootRecoveryNotice
      var spinner = controller.searchSpinner ? controller.searchSpinner + "  " : ""
      if (controller.searchBusy && controller.searchResultCount === 0) return spinner + "Looking…"
      if (controller.searchError) return controller.searchError
      if (controller.searchBackend === "tree" && controller.searching) return controller.searchIndexed + " of " + controller.searchWalked + " shown  ·  fzf for the whole root"
      var summary = spinner + controller.searchResultCount + " result" + (controller.searchResultCount === 1 ? "" : "s") + (controller.searchListActive ? " — " + controller.searchListTitle : "")
      if (controller.searchFilterSummary) summary += "  ·  " + controller.searchFilterSummary
      return summary
    }
    MouseArea {
      anchors.fill: parent
      enabled: controller.backendStalled
      cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
      onClicked: controller.retryBackend()
    }
    color: controller.locationValidationError || controller.searchError || controller.backendStalled || controller.backendVersionSkew
      || (!controller.backendReady && controller.backendError)
      ? Color.urgent
      : (controller.rootRecoveryNotice ? Color.accent : Color.muted)
    elide: Text.ElideRight
    font.family: Style.font.family
    font.pixelSize: Typography.caption
  }

  Loader {
    id: mediaLoader
    anchors.fill: treeList
    active: root.mediaActive
    Component.onCompleted: setSource("../modules/files/MediaContent.qml", {
      controller: Qt.binding(function() { return root.controller }), pane: root
    })
  }

  Timer { id: mediaRefresh; interval: 120; onTriggered: if (mediaProvider) mediaProvider.reload(true) }

  Rectangle {
    id: mediaFooter
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.bottom: parent.bottom
    height: visible ? Style.space(28) : 0
    visible: !controller.trashMode && !controller.drivesMode
    color: Color.bar.background
    TextMetrics {
      id: footerSeparator
      font.family: Style.font.family
      font.pixelSize: Typography.caption
      text: "    "
    }

    Row {
      id: footerRow
      anchors.left: parent.left
      anchors.leftMargin: Style.space(8)
      anchors.right: mediaSize.left
      anchors.rightMargin: Style.space(4)
      anchors.verticalCenter: parent.verticalCenter
      spacing: footerSeparator.width
      Text {
        id: footerRewind
        objectName: "footerRewind"
        textFormat: Text.PlainText
        visible: root.footerLayout.back
        text: "‹"
        color: rewindPointer.containsMouse ? Color.accent : Color.muted
        font.family: Style.font.family
        font.pixelSize: Typography.caption
        font.weight: Font.Bold
        MouseArea {
          id: rewindPointer
          anchors.fill: parent
          anchors.margins: -Style.space(4)
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onClicked: root.footerOffset = FooterFields.retreat(root.footerParts(), root.footerLayout.start, root.footerLayout.shown.length)
        }
      }
      Text {
        id: footerCount
        textFormat: Text.PlainText
        text: root.footerLayout.shown.length > 0 ? root.footerLayout.shown[0] : ""
        color: Color.muted
        elide: Text.ElideRight
        font.family: Style.font.family
        font.pixelSize: Typography.caption
      }
      Text {
        id: footerDetail
        width: Math.max(0, parent.width - footerCount.width - parent.spacing
          - (footerPager.visible ? footerPager.width + parent.spacing : 0)
          - (footerRewind.visible ? footerRewind.width + parent.spacing : 0)
          )
        textFormat: Text.PlainText
        text: root.footerLayout.shown.slice(1).join(footerSeparator.text)
        color: root.branchError || (mediaProvider && mediaProvider.error) ? Color.urgent : Color.muted
        elide: Text.ElideRight
        font.family: Style.font.family
        font.pixelSize: Typography.caption
      }
      Text {
        id: footerPager
        objectName: "footerPager"
        textFormat: Text.PlainText
        visible: root.footerLayout.more
        text: "›"
        color: pagerPointer.containsMouse ? Color.accent : Color.muted
        font.family: Style.font.family
        font.pixelSize: Typography.caption
        font.weight: Font.Bold
        MouseArea {
          id: pagerPointer
          anchors.fill: parent
          anchors.margins: -Style.space(4)
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onClicked: root.footerOffset = FooterFields.advance(root.footerParts(), root.footerLayout.start, root.footerLayout.shown.length)
        }
      }
    }
    PluginUi.DensitySlider {
      id: mediaSize
      anchors.right: parent.right
      anchors.verticalCenter: parent.verticalCenter
      step: root.mediaActive ? root.mediaSizeStep : root.ordinaryDensityStep
      label: root.mediaActive ? "Preview size" : "Row density"
      labels: root.densitySizes
      editableValue: !root.mediaActive
      editText: root.mediaActive ? "" : String(Math.round(root.ordinaryDensity * 100))
      onValueEntered: function(text) { root.applyDensityText(text) }
      onStepRequested: function(step) {
        if (root.mediaActive) { if (mediaView) mediaView.rememberAnchor(); root.mediaSizeStep = step }
        else root.changeDensity(step)
      }
      onKeyPressed: function(event) { root.handleListKey(event, mediaView || root.activeList, root.activeList === treeList) }
    }
  }

  ListView {
    id: treeList
    reuseItems: true
    anchors.top: statusText.bottom
    anchors.topMargin: controller.searching ? 0 : Style.space(7)
    anchors.bottom: mediaFooter.top
    anchors.left: parent.left
    anchors.right: parent.right
    visible: !root.mediaActive && !controller.searching && !controller.trashMode && !controller.recentMode && !controller.drivesMode
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    model: controller.treeModel
    currentIndex: -1
    onMovementStarted: treeAnchor.clear()
    onFlickStarted: treeAnchor.clear()
    onVisibleChanged: if (visible) Qt.callLater(root.ensureContextRoot)

    delegate: BrowserRow {
      id: treeRow
      function choose(modifiers) {
        root.selectIndex(treeList, true, index, root.selectionMode(modifiers || Qt.NoModifier))
        treeList.forceActiveFocus()
      }
      visible: !root.contextAbove || index !== 0
      Binding on height { when: root.contextAbove && treeRow.index === 0; value: 0 }
      density: root.ordinaryDensity
      controller: root.controller
      pane: root
      treeMode: true
      ownerView: treeList
      onBranchActivated: function(sceneX, sceneY) { root.openBranchPicker(sceneX, sceneY) }
    }

    Keys.priority: Keys.BeforeItem
    Keys.onPressed: function(event) { root.handleListKey(event, treeList, true) }
    Keys.onReleased: function(event) { if (controller.dropWheel.handleDragKeyRelease(event)) event.accepted = true }
  }

  ListView {
    id: searchList
    reuseItems: true
    anchors.fill: treeList
    visible: !root.mediaActive && controller.searching && !controller.trashMode && !controller.recentMode && !controller.drivesMode
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    model: controller.searchModel
    currentIndex: -1
    onCurrentIndexChanged: if (currentIndex >= 0 && currentIndex >= count - 8) controller.loadMoreSearchRows()
    onContentYChanged: if (contentHeight - contentY - height < Style.space(42) * 6) controller.loadMoreSearchRows()

    delegate: BrowserRow {
      density: root.ordinaryDensity
      controller: root.controller
      pane: root
      treeMode: false
      showPath: controller.searchDeepActive && !controller.searchTreeLayout
      ownerView: searchList
    }

    Keys.priority: Keys.BeforeItem
    Keys.onPressed: function(event) { root.handleListKey(event, searchList, false) }
    Keys.onReleased: function(event) { if (controller.dropWheel.handleDragKeyRelease(event)) event.accepted = true }
  }

  ListView {
    id: recentList
    reuseItems: true
    anchors.fill: treeList
    visible: controller.recentMode
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    model: controller.recentModel
    currentIndex: -1

    delegate: BrowserRow {
      density: root.ordinaryDensity
      controller: root.controller
      pane: root
      treeMode: false
      ownerView: recentList
    }

    Keys.priority: Keys.BeforeItem
    Keys.onPressed: function(event) { root.handleListKey(event, recentList, false) }
    Keys.onReleased: function(event) { if (controller.dropWheel.handleDragKeyRelease(event)) event.accepted = true }
  }

  Text {
    textFormat: Text.PlainText
    anchors.centerIn: treeList
    width: Math.max(0, parent.width - Style.space(32))
    visible: controller.recentMode && !controller.recentBusy && controller.recentModel.count === 0
    text: controller.recentError || "No recently opened files yet"
    color: controller.recentError ? Color.urgent : Color.muted
    horizontalAlignment: Text.AlignHCenter
    wrapMode: Text.WordWrap
    font.family: Style.font.family
    font.pixelSize: Typography.body
  }

  TrashView {
    id: trashView
    actionKeys: root.actionKeys
    anchors.fill: treeList
    visible: controller.trashMode
    controller: root.controller
    pane: root
  }

  DrivesView {
    id: drivesView
    anchors.fill: treeList
    visible: controller.drivesMode
    controller: root.controller
    pane: root
  }

  Rectangle {
    x: Math.round(browserHeader.addGuideX - width / 2)
    anchors.top: navigationBar.bottom
    anchors.bottom: parent.bottom
    width: Style.space(2)
    visible: browserHeader.addGuideVisible
    color: Util.alpha(Color.accent, 0.7)
    z: 25
  }

  PluginUi.ScrollEdgeFade {
    anchors.left: treeList.left
    anchors.right: treeList.right
    anchors.top: treeList.top
    anchors.bottom: treeList.bottom
    flickable: root.activeList
    visible: !!root.activeList && !root.mediaActive
    surfaceColor: Color.bar.background
    z: 24
  }

  PluginUi.OptionPopup {
    id: branchPicker
    property bool busy: false
    property string repositoryPath: ""
    menuWidth: Style.space(260)
    heading: "Switch branch"
    prompt: "Find branch…"

    function load() {
      busy = true
      repositoryPath = controller.rootPath
      controller.backendRequest("git-branches", ["--path", repositoryPath], 0, function(response) {
        branchPicker.busy = false
        if (!response || response.ok !== true) {
          root.branchError = response && response.error ? String(response.error) : "branch list is unavailable"
          return
        }
        var names = response.branches || []
        var entries = [{ key: "__expand", label: "Expand into Branches", glyph: "󰘬" }, { kind: "separator" }]
        for (var i = 0; i < names.length; i++)
          entries.push({ key: String(names[i]), label: String(names[i]), checked: String(names[i]) === String(response.current) })
        branchPicker.rows = entries
        if (entries.length > 0) branchPicker.open()
      })
    }

    onPicked: function(key) {
      if (String(key) === "__expand") {
        controller.openBranches(root.targetScreen())
        return
      }
      branchPicker.busy = true
      controller.backendRequest("git-switch", ["--path", branchPicker.repositoryPath, "--branch", String(key)], 0, function(response) {
        branchPicker.busy = false
        if (!response || response.ok !== true) {
          root.branchError = response && response.error ? String(response.error) : "branch switch failed"
          return
        }
        root.branchError = ""
        controller.requestVisibleGitMetadataRefresh("branch-switch")
        controller.refreshTree()
      })
    }
  }

  PluginUi.MarkedScrollBar {
    id: scrollRuler
    visible: !root.mediaActive && scrollRuler.scrollable
    anchors.right: treeList.right
    anchors.rightMargin: Style.space(2)
    anchors.top: treeList.top
    anchors.bottom: treeList.bottom
    flickable: root.activeList
    marks: root.activeList === treeList ? root.gitMarks : []
    z: 26
    onHeightChanged: markRefresh.restart()
  }

  PluginUi.ListAnchor {
    id: treeAnchor
    view: treeList
    loading: controller.treeLoading
    indexOf: function(path) { return controller.indexOfTreePath(path) }
    keyAt: function(index) {
      return index >= 0 && index < controller.treeModel.count ? String(controller.treeModel.get(index).path || "") : ""
    }
    parentOf: function(path) { return controller.parentDirectory(path) }
    onLoadingChanged: if (!loading && pending) Qt.callLater(restore)
  }

  Timer {
    id: markRefresh
    interval: 40
    onTriggered: root.refreshGitMarks()
  }

  function refreshGitMarks() {
    if (!controller.gitEnabled || !controller.scrollMarks) {
      root.gitMarks = []
      return
    }
    var collected = ScrollMarks.collect(controller.treeModel, scrollRuler.slots, function(row) {
      if (!row || row.gitIgnored) return ""
      if (GitSummary.summaryRow(row.isGitRepo, row.depth, row.path, controller.rootPath, controller.gitSummaryFields)) return ""
      return String(row.gitStatus || "")
    })
    var marks = []
    for (var i = 0; i < collected.length; i++)
      marks.push({ fraction: collected[i].fraction, color: controller.gitStatusColor(collected[i].status) })
    root.gitMarks = marks
  }

  Connections {
    target: root.controller
    ignoreUnknownSignals: true
    function onTreeRowsReplacing() { if (treeList.visible) treeAnchor.capture() }
    function onTreeStructureRevisionChanged() {
      markRefresh.restart()
      if (treeAnchor.pending) Qt.callLater(treeAnchor.restore)
    }
    function onTreeRowsRevisionChanged() {
      markRefresh.restart()
      if (treeAnchor.pending) Qt.callLater(treeAnchor.restore)
    }
    function onGitMetadataRefreshCountChanged() { markRefresh.restart() }
    function onGitEnabledChanged() { markRefresh.restart() }
    function onScrollMarksChanged() { markRefresh.restart() }
  }

  Text {
    textFormat: Text.PlainText
    anchors.centerIn: treeList
    width: Math.max(0, parent.width - Style.space(32))
    visible: !root.mediaActive && !controller.trashMode && controller.searching && !controller.searchBusy && controller.searchModel.count === 0 && !controller.searchError
    text: "No matching files or folders"
    color: Color.muted
    horizontalAlignment: Text.AlignHCenter
    wrapMode: Text.WordWrap
    font.family: Style.font.family
    font.pixelSize: Typography.body
  }

  PluginUi.ActionDialog {
    anchors.fill: parent
    z: 96
    actionKeys: root.actionKeys
    collisionController: root.controller.history
    paneVisible: root.visible && !!root.context && root.context.bladeOpen
  }

  PluginUi.ActionDialog {
    id: trashDialog
    actionKeys: root.actionKeys
    anchors.fill: parent
    z: 95
    onChosen: function(key) {
      trashConfirmation.resolve(key)
      root.focusTree()
    }
    onCanceled: {
      trashConfirmation.resolve("cancel")
      root.focusTree()
    }
  }

  PluginUi.TrashConfirmationBinding {
    id: trashConfirmation
    controller: root.controller
    dialog: trashDialog
    paneVisible: root.visible && !!root.context && root.context.bladeOpen
    choices: [{ key: "cancel", label: "Cancel" }, { key: "trash", label: "Move to Trash", danger: true }]
    promptFor: function(paths) { return root.trashPrompt(paths) }
  }
}
