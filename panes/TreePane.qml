import QtQuick
import QtQuick.Controls
import qs.Commons
import qs.Ui
import "../ui" as PluginUi
import "../lib/KeyRouter.js" as KeyRouter
import "../lib/ScrollMarks.js" as ScrollMarks
import "../theme"

FocusScope {
  id: root

  required property var controller
  required property var hostWindow
  property alias actionKeys: actionKeyGuard
  PluginUi.ActionKeyGuard { id: actionKeyGuard; active: root.activeFocus; shared: root.hostWindow ? root.hostWindow.actionKeys : null }
  PluginUi.TreeKeys {
    id: treeKeys
    active: root.focusEnabled && (treeList.activeFocus || searchList.activeFocus || recentList.activeFocus)
    scope: treeList.activeFocus ? "files-tree" : "files-list"
    plan: controller.keybindings.plan
  }
  Keys.onReleased: function(event) { actionKeyGuard.release(event) }
  TapHandler {
    acceptedButtons: Qt.BackButton | Qt.ForwardButton
    gesturePolicy: TapHandler.ReleaseWithinBounds
    enabled: root.focusEnabled && root.activeFocus
    onTapped: function(eventPoint, button) {
      if (button === Qt.BackButton) root.runBrowserAction("back")
      else if (button === Qt.ForwardButton) root.runBrowserAction("forward")
    }
  }
  property var context: null
  property bool focusEnabled: true
  property bool locationEditing: false
  readonly property string editorMode: root.visualMode ? "VISUAL" : (searchField.activeFocus || locationField.activeFocus ? "INSERT" : "NORMAL")

  Binding { target: root.controller; property: "editorMode"; value: root.editorMode }
  property var gitMarks: []
  readonly property var activeList: controller.trashMode
    ? trashView.list
    : (controller.drivesMode ? drivesView.list : (controller.recentMode ? recentList : (controller.searching ? searchList : treeList)))
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

  function targetScreen() {
    return hostWindow ? hostWindow.screen : null
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
    var next = Math.max(0, Math.min(view.count - 1, index))
    view.currentIndex = next
    treeAnchor.clear()
    view.positionViewAtIndex(next, ListView.Contain)
    var row = modelRow(next, treeMode, view)
    if (row && row.kind === "More") {
      controller.loadMoreChildren(String(row.path).slice(0, -5))
      return
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
    var rowHeight = view.currentItem ? view.currentItem.height : Style.space(30)
    var rows = Math.max(1, Math.floor(view.height / Math.max(1, rowHeight) * 0.8))
    moveCurrent(view, treeMode, direction * rows, extend, false)
  }

  function activateIndex(view, treeMode, index, enterFolder) {
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
    var row = modelRow(Math.max(0, view.currentIndex), true, view)
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
    var flat = controller.recentMode || controller.searching
    var view = controller.recentMode ? recentList : (controller.searching ? searchList : treeList)
    if (view.count > 0 && view.currentIndex < 0) selectIndex(view, !flat, 0)
    view.forceActiveFocus()
  }

  function focusSearch() {
    locationEditing = false
    searchField.reveal()
  }

  function toggleDeepSearch() {
    if (controller.quickNavActive) controller.stopQuickNav()
    controller.toggleSearchDeep()
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
    var index = view && view.currentIndex >= 0 ? view.currentIndex : 0
    var delegate = view && view.itemAtIndex ? view.itemAtIndex(index) : null
    if (delegate && delegate.openMenu) {
      delegate.openMenu(mode || "actions", 0, 0, true)
      return
    }
    var edge = context ? String(context.edge || "left") : "left"
    var x = edge === "right" ? originX() + Style.space(2) : root.width - Style.space(8) + originX()
    controller.openActionMenu(mode || "actions", targetScreen(), x, Style.space(70), undefined, { edge: edge, keyboard: true })
  }

  function runBrowserAction(action) {
    var actions = {
      location: function() { controller.focusLocation(targetScreen()) },
      hidden: function() { controller.toggleHidden() },
      back: function() { controller.goBack(targetScreen()) },
      forward: function() { controller.goForward(targetScreen()) },
      up: function() { controller.goUp() },
      home: function() { controller.goHome() },
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
    if (!treeMode) controller.loadAllSearchRows()
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
        if (!treeMode) controller.loadAllSearchRows()
        selectIndex(view, treeMode, view.count - 1, event.modifiers & Qt.ShiftModifier ? "range" : "replace")
      },
      collapse: function() { foldCurrent(view, false) },
      expand: function() { foldCurrent(view, true) },
      "expand-recursive": function() { var row = modelRow(view.currentIndex, treeMode, view); if (row) controller.setBranchExpanded(row.path, true) },
      "collapse-recursive": function() { var row = modelRow(view.currentIndex, treeMode, view); if (row) controller.setBranchExpanded(row.path, false) },
      "expand-all": function() { controller.setBranchExpanded(controller.rootPath, true) },
      "collapse-all": function() { selectIndex(view, true, 0, "replace"); controller.setBranchExpanded(controller.rootPath, false) },
      "open-with": function() { openMenuForCurrent(view, "open-with") },
      activate: function() { activateIndex(view, treeMode, view.currentIndex < 0 ? 0 : view.currentIndex) },
      open: function() { activateIndex(view, treeMode, view.currentIndex < 0 ? 0 : view.currentIndex, true) },
      editor: function() { controller.openInEditor(controller.selectedPath) },
      "toggle-selection": function() { controller.selectModelIndex(view.model, view.currentIndex < 0 ? 0 : view.currentIndex, "toggle") },
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
        if (!treeMode) controller.loadAllSearchRows()
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
    navigationActions: [
      { key: "back", glyph: "", title: "Back", enabled: controller.canGoBack,
        actions: [{ button: "left", text: "Back" }, { shortcut: "Alt+←" }], context: [{ glyph: "󰉋", text: controller.backDestination }] },
      { key: "forward", glyph: "", title: "Forward", enabled: controller.canGoForward,
        actions: [{ button: "left", text: "Forward" }, { shortcut: "Alt+→" }], context: [{ glyph: "󰉋", text: controller.forwardDestination }] },
      { key: "up", glyph: "", title: "Up", enabled: controller.canGoUp,
        actions: [{ button: "left", text: "Up" }, { shortcut: "Alt+↑" }], context: [{ glyph: "󰉋", text: controller.parentDirectory(controller.rootPath) }] },
      { key: "home", glyph: "", title: "Home", enabled: controller.rootPath !== controller.home,
        actions: [{ button: "left", text: "Home" }, { shortcut: "Alt+Home" }], context: [{ glyph: "󰉋", text: controller.home }] },
      { key: "recent", glyph: "󰋚", title: "Recent", active: controller.recentMode,
        actions: [{ button: "left", text: "Open" }], context: root.recentNavigationContext() },
      { key: "drives", glyph: "󰋊", title: "Drives", active: controller.drivesMode,
        actions: [{ button: "left", text: "Open" }], context: root.drivesNavigationContext() },
      { key: "desktop-trash", glyph: controller.trashCount > 0 ? "󰩹" : "󰩺", title: "Trash", active: controller.trashMode,
        actions: [{ button: "left", text: "Open" }], context: root.trashNavigationContext() }
    ]
    onColumnsCommitted: controller.setPriorityColumns(columns)
    onSortsChanged: root.pushTreeOrder()
    onFilterChanged: root.pushTreeOrder()
    onNavigationTriggered: function(key) {
      if (!root.runBrowserAction(key)) return
      Qt.callLater(root.focusTree)
    }
  }

  function pushTreeOrder() {
    controller.setTreeOrder(filesView.sorts, filesView.filter)
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

  DrivesPanel {
    id: drivesSection
    anchors.top: favoritesSection.bottom
    anchors.left: parent.left
    anchors.right: parent.right
    controller: root.controller
    pane: root
  }

  PluginUi.PaneSearchField {
    id: searchField
    service: root.controller
    anchors.top: drivesSection.bottom
    anchors.topMargin: visible ? Style.space(6) : 0
    anchors.left: parent.left
    anchors.right: parent.right
    text: controller.quickNavActive ? "" : controller.searchQuery
    enabled: !controller.trashMode && !controller.recentMode && !controller.drivesMode
    readonly property string quickNavKeys: controller.keybindings.label("quicknav")
    prompt: controller.searchListActive ? "Filter " + controller.searchListTitle + "..."
      : (quickNavKeys === "Unbound" ? "Search..." : "Search... (" + quickNavKeys + " to quick nav)")
    showOptions: true
    showDeepOption: true
    caseSensitive: controller.searchCaseSensitive
    regex: controller.searchRegex

    onTextEdited: controller.searchQuery = text
    onCleared: controller.searchQuery = ""
    onAdvanced: root.focusTree()
    onOptionsToggled: function(nextCase, nextRegex) { controller.setSearchOptions(nextCase, nextRegex) }
    deep: controller.searchDeepActive
    onDeepToggled: controller.toggleSearchDeep()
    onHistoryStepped: function(delta) {
      var recalled = controller.recallSearch(delta)
      if (recalled === null) return
      browsingHistory = recalled !== ""
      text = recalled
      controller.searchQuery = recalled
      cursorPosition = text.length
    }

    onAccepted: {
      if (controller.searching && searchList.count > 0)
        root.activateIndex(searchList, false, searchList.currentIndex < 0 ? 0 : searchList.currentIndex)
    }

    onDismissed: {
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
        if (controller.quickNavActive) return
        if (searchField.text !== controller.searchQuery) searchField.text = controller.searchQuery
      }
    }
  }

  Item {
    id: navigationBar
    anchors.top: searchField.bottom
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

    Rectangle {
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.bottom: parent.bottom
      height: 1
      color: Util.alpha(Color.bar.text, 0.10)
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
    visible: controller.searching || controller.locationValidationError !== "" || controller.treeExpansionError !== "" || controller.keybindings.error !== ""
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

  ListView {
    id: treeList
    reuseItems: true
    anchors.top: statusText.bottom
    anchors.topMargin: controller.searching ? 0 : Style.space(7)
    anchors.bottom: parent.bottom
    anchors.left: parent.left
    anchors.right: parent.right
    visible: !controller.searching && !controller.trashMode && !controller.recentMode && !controller.drivesMode
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    model: controller.treeModel
    currentIndex: -1
    onMovementStarted: treeAnchor.clear()
    onFlickStarted: treeAnchor.clear()

    delegate: BrowserRow {
      controller: root.controller
      pane: root
      treeMode: true
      ownerView: treeList
    }

    Keys.priority: Keys.BeforeItem
    Keys.onPressed: function(event) { root.handleListKey(event, treeList, true) }
    Keys.onReleased: function(event) { if (controller.dropWheel.handleDragKeyRelease(event)) event.accepted = true }
  }

  ListView {
    id: searchList
    reuseItems: true
    anchors.fill: treeList
    visible: controller.searching && !controller.trashMode && !controller.recentMode && !controller.drivesMode
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    model: controller.searchModel
    currentIndex: -1
    onCurrentIndexChanged: if (currentIndex >= 0 && currentIndex >= count - 8) controller.loadMoreSearchRows()
    onContentYChanged: if (contentHeight - contentY - height < Style.space(42) * 6) controller.loadMoreSearchRows()

    delegate: BrowserRow {
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
    visible: !!root.activeList
    surfaceColor: Color.bar.background
    z: 24
  }

  PluginUi.MarkedScrollBar {
    id: scrollRuler
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
    var collected = ScrollMarks.collect(controller.treeModel, scrollRuler.slots)
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
    visible: !controller.trashMode && controller.searching && !controller.searchBusy && controller.searchModel.count === 0 && !controller.searchError
    text: "No matching files or folders"
    color: Color.muted
    horizontalAlignment: Text.AlignHCenter
    wrapMode: Text.WordWrap
    font.family: Style.font.family
    font.pixelSize: Typography.body
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
