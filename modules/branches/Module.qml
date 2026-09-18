import QtQuick
import qs.Commons
import "../../lib/Format.js" as Format
import "../../lib/GitSummary.js" as GitSummary
import "../../lib/Highlight.js" as Highlight
import "../../theme"

FocusScope {
  id: module

  property var context: null

  readonly property string title: "Branches"
  readonly property var shortcuts: [
    {
      title: "Branches",
      items: [
        { shortcut: "Enter", text: "Switch to branch, open worktree, or fold a branch with one" },
        { shortcut: "o", text: "Open the worktree a branch is checked out in" },
        { shortcut: "s", text: "Cycle sort" },
        { shortcut: "f", text: "Filter" },
        { shortcut: "Shift+R", text: "Rescan" }
      ]
    }
  ].concat(files && files.keybindings ? [files.keybindings.treeShortcuts] : [])
  readonly property var metricOptions: context ? context.metrics.options(["off", { key: "kind", label: "Kind" }, "status", "updated", { key: "author", label: "Author" }, "summary"]) : []
  readonly property var view: viewLoader.item
  readonly property var files: context ? context.service("files") : null
  readonly property string anchorPath: files ? String(files.contextPath || files.rootPath || "") : ""
  readonly property bool active: !!context && context.bladeOpen !== false && context.collapsed !== true
  readonly property color paneBackground: Qt.lighter(Color.background, 1.035)

  property var document: null
  property string loadError: ""
  property string switchError: ""
  property bool busy: false
  property bool switching: false
  property int generation: 0
  property string requestId: ""
  property string query: ""
  property bool caseSensitive: false
  property bool regex: false

  readonly property var branches: document && Array.isArray(document.branches) ? document.branches : []
  readonly property var worktrees: document && Array.isArray(document.worktrees) ? document.worktrees : []
  readonly property var linkedWorktrees: worktrees.filter(function(tree) { return tree.main !== true })
  readonly property var summaryFields: files && files.gitSummaryFields !== undefined ? files.gitSummaryFields : undefined
  readonly property var items: buildItems(worktrees, branches, summaryFields)
  property var knownFolders: ({})
  readonly property var currentBranch: items.find(function(entry) { return entry.kind !== "worktree" && entry.current }) || null
  readonly property string repositoryStatus: currentBranch ? currentBranch.name + " " + currentBranch.metrics.status : ""
  readonly property string error: loadError || switchError
  readonly property string status: {
    if (switching) return "Switching…"
    if (error !== "") return error
    if (busy && !document) return "Loading…"
    if (tree.item && tree.item.searching) return tree.item.visibleItems.length + " of " + items.length
    return branches.length + " branches, " + linkedWorktrees.length + " worktrees"
  }

  function takeFocus(part) {
    if (tree.item) tree.item.forceActiveFocus()
  }

  function cancelRefresh() {
    refreshDelay.stop()
    if (requestId && files) files.cancelBackendRequest(requestId, generation)
    requestId = ""
    generation++
    busy = false
  }

  function refresh() {
    cancelRefresh()
    if (files && active && anchorPath !== "") refreshDelay.restart()
  }

  function requestPlaces() {
    var serial = ++generation
    busy = true
    requestId = files.backendRequest("git-places", ["--path", anchorPath], serial, function(response) {
      if (serial !== module.generation) return
      module.requestId = ""
      module.busy = false
      if (!response || response.ok !== true) {
        module.loadError = response && response.error ? String(response.error) : "branch list is unavailable"
        module.document = null
        return
      }
      module.loadError = ""
      if (!module.document || module.document.root !== response.root) module.knownFolders = ({})
      module.document = response
    })
  }

  function switchTo(entry) {
    if (!files || switching || !entry || entry.current) return
    var root = document && document.root ? String(document.root) : anchorPath
    switching = true
    files.backendRequest("git-switch", ["--path", root, "--branch", String(entry.name)], 0, function(response) {
      module.switching = false
      if (!response || response.ok !== true) {
        module.switchError = response && response.error ? String(response.error) : "branch switch failed"
        return
      }
      module.switchError = ""
      if (typeof files.requestVisibleGitMetadataRefresh === "function") files.requestVisibleGitMetadataRefresh("branch-switch")
      if (typeof files.refreshTree === "function") files.refreshTree()
      module.refresh()
    })
  }

  function openWorktree(path) {
    if (!files || !path) return
    var index = files.indexOfTreePath(String(path))
    if (index >= 0) files.selectModelIndex(files.treeModel, index, "replace")
    else files.navigateToLocation(String(path), context.screen, "browse")
  }

  function selectCheckout(entry) {
    if (!entry) return
    if (entry.kind === "worktree") openWorktree(entry.path)
    else if (entry.kind !== "remote" && entry.worktree) openWorktree(entry.worktree)
  }

  function activate(entry) {
    if (!entry) return
    if (entry.kind === "worktree" || entry.worktree) selectCheckout(entry)
    else switchTo(entry)
  }

  function leafName(path) {
    var text = String(path || "").replace(/\/+$/, "")
    return text.slice(text.lastIndexOf("/") + 1)
  }

  function changeCounts(tree) {
    var keys = ["modified", "added", "untracked", "deleted", "renamed", "copied", "type_changed", "conflicted"]
    var result = ({})
    for (var i = 0; i < keys.length; i++) result[keys[i]] = Number(tree[keys[i]]) || 0
    return result
  }

  function describe(summary, fields) {
    return GitSummary.describe(JSON.stringify(Object.assign({ ok: true, ahead: null, behind: null, upstream: "" }, summary)), fields)
  }

  function worktreeSummary(tree, fields) {
    var described = describe(changeCounts(tree), fields)
    var text = described.text || "clean"
    return { text: tree.locked ? text + " locked" : text, tokens: described.tokens }
  }

  function branchSummary(branch, checkout, fields) {
    var summary = checkout ? changeCounts(checkout) : ({})
    if (branch.upstream && !branch.gone) {
      summary.upstream = String(branch.upstream)
      summary.ahead = Number(branch.ahead) || 0
      summary.behind = Number(branch.behind) || 0
    }
    var described = describe(summary, fields)
    var fallback = branch.kind === "remote" ? "remote" : (branch.gone ? "gone" : (!branch.upstream ? (checkout ? "clean" : "no upstream") : "clean"))
    return { text: described.text || fallback, tokens: described.tokens }
  }

  function tokenColor(marker) {
    var status = files && typeof files.gitStatusColor === "function" ? files.gitStatusColor : function() { return Color.muted }
    if (marker === "ahead") return Color.accent
    if (marker === "behind") return status("M")
    return marker ? status(marker) : Color.muted
  }

  function markupFor(tokens) {
    if (!Array.isArray(tokens) || tokens.length === 0) return ""
    return tokens.map(function(part) {
      return '<font color="' + tokenColor(part.marker) + '">' + Highlight.escapeHtml(part.text) + '</font>'
    }).join(" ")
  }

  function statusMarkup(entry, key) {
    return key === "status" && entry && Array.isArray(entry.tokens) ? markupFor(entry.tokens) : ""
  }

  function updatedText(at) {
    var seconds = Number(at)
    return isFinite(seconds) && seconds > 0 ? Format.isoDate(new Date(seconds * 1000)) : ""
  }

  function worktreeItem(tree, groups, fields) {
    var path = String(tree.path || "")
    var summary = worktreeSummary(tree, fields)
    return { id: "worktree:" + path, kind: "worktree", name: leafName(path), path: path,
             detail: tree.branch ? path : "detached at " + String(tree.head || ""),
             current: tree.current === true, groups: groups, tokens: summary.tokens,
             metrics: { kind: "worktree", status: summary.text, updated: updatedText(tree.at), author: "" } }
  }

  function buildItems(worktreeRows, branchRows, fields) {
    var checkouts = ({})
    var result = []
    for (var w = 0; w < worktreeRows.length; w++) checkouts[String(worktreeRows[w].path || "")] = worktreeRows[w]
    var claimed = ({})
    for (var b = 0; b < branchRows.length; b++) {
      var branch = branchRows[b]
      var checkout = branch.worktree ? checkouts[branch.worktree] : null
      var linked = checkout && checkout.main !== true ? worktreeItem(checkout, [], fields) : null
      if (linked) claimed[linked.path] = true
      var summary = branchSummary(branch, checkout, fields)
      result.push({ id: "branch:" + String(branch.name), kind: String(branch.kind || "local"), name: String(branch.name || ""),
                    detail: String(branch.subject || ""), current: branch.current === true, worktree: String(branch.worktree || ""),
                    remote: String(branch.remote || ""), upstream: String(branch.upstream || ""), author: String(branch.author || ""),
                    groups: branch.kind === "remote" ? ["Branches", "Remote"] : ["Branches"], linked: linked, tokens: summary.tokens,
                    metrics: { kind: String(branch.kind || "local"), status: summary.text,
                               updated: updatedText(branch.at), author: String(branch.author || "") } })
    }
    for (var t = 0; t < worktreeRows.length; t++) {
      var tree = worktreeRows[t]
      if (tree.main === true || claimed[String(tree.path || "")]) continue
      result.push(worktreeItem(tree, ["Worktrees"], fields))
    }
    return result
  }

  function expansionKeyFor(entry) {
    return entry && entry.linked ? String(entry.id) : ""
  }

  function revealNewFolders() {
    if (!tree.item) return
    var next = Object.assign({}, tree.item.expandedFolders)
    var known = Object.assign({}, knownFolders)
    for (var i = 0; i < items.length; i++) {
      var key = expansionKeyFor(items[i])
      if (key === "" || known[key]) continue
      known[key] = true
      next[key] = true
    }
    knownFolders = known
    tree.item.expandedFolders = next
  }

  function glyphFor(entry) {
    if (entry.kind === "worktree") return "󰉖"
    return entry.kind === "remote" ? "󰅡" : ""
  }

  function searchText(entry) {
    return [entry.name, entry.detail, entry.author, entry.upstream, entry.kind, entry.path].join(" ")
  }

  function handleKey(event) {
    if (event.modifiers !== Qt.NoModifier || event.key !== Qt.Key_O || !tree.item) return false
    var row = tree.item.rowAt(tree.item.currentIndex)
    if (!row || row.kind !== "leaf" || row.item.kind === "worktree" || !row.item.worktree) return false
    openWorktree(row.item.worktree)
    return true
  }

  onActiveChanged: refresh()
  onAnchorPathChanged: { document = null; switchError = ""; refresh() }
  onItemsChanged: revealNewFolders()
  Component.onCompleted: refresh()
  Component.onDestruction: cancelRefresh()

  Timer {
    id: refreshDelay
    interval: 120
    onTriggered: module.requestPlaces()
  }

  Connections {
    target: module.files
    ignoreUnknownSignals: true
    function onGitMetadataRefreshCountChanged() { module.refresh() }
  }

  Loader {
    id: viewLoader
    source: module.context ? module.context.ui.url("PaneView") : ""
    onLoaded: {
      item.defaultMetric = "status"
      item.options = module.metricOptions
      item.context = Qt.binding(function() { return module.context })
    }
  }

  Rectangle {
    anchors.fill: parent
    color: module.paneBackground
  }

  Loader {
    id: header
    anchors.top: search.bottom
    anchors.topMargin: search.height > 0 ? Style.space(4) : 0
    anchors.left: parent.left
    anchors.right: parent.right
    source: module.context ? module.context.ui.url("PaneHeader") : ""
    onLoaded: {
      item.context = module.context
      item.title = "BRANCHES"
      item.tabIndex = Qt.binding(function() { return module.context.tabIndex })
      item.reservedLeft = Qt.binding(function() { return module.context.cornerReserveLeft })
      item.reservedRight = Qt.binding(function() { return module.context.cornerReserveRight })
      item.highlighted = Qt.binding(function() { return module.activeFocus })
      item.view = Qt.binding(function() { return module.view })
      item.status = Qt.binding(function() { return module.error !== "" || module.switching || (tree.item && tree.item.searching) || !module.currentBranch ? module.status : module.repositoryStatus })
      item.statusGlyph = Qt.binding(function() { return module.currentBranch && !module.switching && module.error === "" ? "" : "" })
      item.statusColor = Qt.binding(function() { return module.error !== "" ? Color.urgent : Color.muted })
    }
  }

  Loader {
    id: search
    anchors.top: parent.top
    anchors.topMargin: height > 0 ? Style.space(6) : 0
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.leftMargin: Style.space(7)
    anchors.rightMargin: Style.space(7)
    active: module.active
    height: item && item.visible ? Style.space(32) : 0
    source: module.context ? module.context.ui.url("PaneSearchField") : ""
    onLoaded: {
      item.context = Qt.binding(function() { return module.context })
      item.prompt = "Filter branches…"
      item.text = Qt.binding(function() { return module.query })
      item.showOptions = true
      item.caseSensitive = module.caseSensitive
      item.regex = module.regex
      item.optionsToggled.connect(function(nextCase, nextRegex) {
        module.caseSensitive = nextCase
        module.regex = nextRegex
      })
      item.textChanged.connect(function() { module.query = item.text })
      item.dismissed.connect(function() { module.closeSearch() })
      item.advanced.connect(function() { module.takeFocus("") })
    }
  }

  function openSearch() {
    if (search.item) search.item.reveal()
  }

  function closeSearch() {
    query = ""
    takeFocus("")
  }

  function openFilter() {
    if (header.item) header.item.openFilter()
  }

  Loader {
    id: tree
    anchors.top: header.bottom
    anchors.topMargin: Style.space(4)
    anchors.bottom: parent.bottom
    anchors.left: parent.left
    anchors.right: parent.right
    active: module.active
    visible: active
    source: module.context ? module.context.ui.url("ArtifactTree") : ""
    onLoaded: {
      item.context = Qt.binding(function() { return module.context })
      item.items = Qt.binding(function() { return module.items })
      item.query = Qt.binding(function() { return module.query })
      item.caseSensitive = Qt.binding(function() { return module.caseSensitive })
      item.regex = Qt.binding(function() { return module.regex })
      item.view = Qt.binding(function() { return module.view })
      item.surfaceColor = Qt.binding(function() { return module.paneBackground })
      item.fileActionsFor = function(entry) { return false }
      item.expandableItems = true
      item.childMetrics = true
      item.expansionKey = function(entry) { return module.expansionKeyFor(entry) }
      item.childrenFor = function(entry) { return entry && entry.linked ? [entry.linked] : [] }
      item.metricMarkup = function(entry, key) { return module.statusMarkup(entry, key) }
      item.groupsFor = function(entry) { return entry.groups }
      item.leafGlyph = function(entry) { return module.glyphFor(entry) }
      item.leafGlyphColor = function(entry) { return entry && entry.kind !== "remote" ? Color.accent : Color.muted }
      item.leafLabel = function(entry) { return entry.name + (entry.current ? " 󰄬" : "") }
      item.rowMark = function(entry) { return "" }
      item.searchText = function(entry) { return module.searchText(entry) }
      item.filterKeys = ["kind", "remote"]
      item.searchFields = function(entry) { return ({ kind: entry.kind, remote: entry.remote }) }
      item.keyHandler = function(event) { return module.handleKey(event) }
      item.selectionChanged.connect(function(entry) { module.selectCheckout(entry) })
      item.activated.connect(function(entry) { module.activate(entry) })
      item.revealed.connect(function(entry) { if (entry.worktree) module.openWorktree(entry.worktree) })
      item.searchRequested.connect(function() { module.openSearch() })
      item.filterRequested.connect(function() { module.openFilter() })
      item.focusNextRequested.connect(function() { module.context.focusNext() })
      item.focusPreviousRequested.connect(function() { module.context.focusPrevious() })
      item.dismissRequested.connect(function() { module.context.closeBlade() })
      module.revealNewFolders()
    }
  }

  Keys.onPressed: function(event) {
    if (event.key !== Qt.Key_R || !(event.modifiers & Qt.ShiftModifier) || (event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier))) return
    module.refresh()
    event.accepted = true
  }

  Text {
    textFormat: Text.PlainText
    anchors.centerIn: parent
    width: Math.max(0, parent.width - Style.space(40))
    visible: module.active && (module.items.length === 0 || module.loadError !== "")
    horizontalAlignment: Text.AlignHCenter
    wrapMode: Text.WordWrap
    text: module.loadError !== "" ? module.loadError : (module.busy ? "Loading branches…" : (module.query ? "No match" : "No branches found"))
    color: module.loadError !== "" ? Color.urgent : Color.muted
    font.family: Style.font.family
    font.pixelSize: Typography.body
  }
}
