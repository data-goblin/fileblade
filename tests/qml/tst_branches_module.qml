import QtQuick
import QtTest
import "../../lib/PathText.js" as Paths
import "../../lib/Format.js" as Format
import "../../lib/KeyBindings.js" as KeyBindings

TestCase {
  id: test
  name: "BranchesModule"
  width: 500
  height: 700
  visible: true
  when: windowShown
  property var module: null
  property var requests: []
  property var navigated: []
  property int gitRefreshes: 0
  property int treeRefreshes: 0
  property var selections: []
  property var cancelled: []

  readonly property var places: ({
    ok: true, root: "/repo", current: "0.2.0", detached: false, truncated: false,
    branches: [
      { name: "0.2.0", kind: "both", remote: "origin", upstream: "origin/0.2.0", ahead: 3, behind: 1, gone: false, current: true, worktree: "/repo", at: 1789600000, author: "Kurt Buhler", subject: "Fix: tree" },
      { name: "main", kind: "both", remote: "origin", upstream: "origin/main", ahead: 0, behind: 0, gone: false, current: false, worktree: "", at: 1789500000, author: "Kurt Buhler", subject: "Release: 0.1.2" },
      { name: "scratch", kind: "local", remote: "", upstream: "", ahead: 0, behind: 0, gone: false, current: false, worktree: "/repo/target/wt/scratch", at: 1789400000, author: "Kurt Buhler", subject: "WIP" },
      { name: "old", kind: "local", remote: "", upstream: "origin/old", ahead: 0, behind: 0, gone: true, current: false, worktree: "", at: 1789300000, author: "Someone", subject: "Old work" },
      { name: "lonely", kind: "local", remote: "", upstream: "", ahead: 0, behind: 0, gone: false, current: false, worktree: "", at: 1789200000, author: "Someone", subject: "No upstream yet" },
      { name: "feature", kind: "remote", remote: "origin", upstream: "", ahead: 0, behind: 0, gone: false, current: false, worktree: "", at: 1789100000, author: "Other Dev", subject: "Feat: remote only" }
    ],
    worktrees: [
      { path: "/repo", branch: "0.2.0", head: "7a01419", main: true, current: true, locked: false, prunable: false, dirty: true, staged: 1, unstaged: 4, untracked: 2, conflicted: 0, modified: 4, added: 1, deleted: 0, renamed: 0, copied: 0, type_changed: 0, at: 1789600000 },
      { path: "/repo/target/wt/scratch", branch: "scratch", head: "abc1234", main: false, current: false, locked: true, prunable: false, dirty: true, staged: 0, unstaged: 0, untracked: 0, conflicted: 2, modified: 0, added: 0, deleted: 0, renamed: 0, copied: 0, type_changed: 0, at: 1789400000 },
      { path: "/repo/target/wt/free", branch: "", head: "def5678", main: false, current: false, locked: false, prunable: false, dirty: false, staged: 0, unstaged: 0, untracked: 0, conflicted: 0, modified: 0, added: 0, deleted: 0, renamed: 0, copied: 0, type_changed: 0, at: 1789000000 }
    ]
  })

  Item {
    id: files
    property string contextPath: "/repo/src"
    property string rootPath: "/repo"
    property bool autoHideSearch: false
    property int gitMetadataRefreshCount: 0
    property var installedAgents: []
    property var history: null
    property var keybindings: ({ plan: KeyBindings.compile({}) })
    function backendRequest(command, args, generation, callback) {
      requests.push({ command: command, args: args, callback: callback })
      return "request-" + requests.length
    }
    property var treeModel: ({})
    function indexOfTreePath(path) { return path === "/repo" ? 0 : -1 }
    function selectModelIndex(model, index, mode) { selections.push({ index: index, mode: mode }) }
    function cancelBackendRequest(id, generation) { cancelled.push(id) }
    function navigateToLocation(path, screen, mode) { navigated.push(path + ":" + mode); return "ok" }
    function requestVisibleGitMetadataRefresh(reason) { gitRefreshes++ }
    function refreshTree() { treeRefreshes++ }
  }
  QtObject {
    id: context
    property int contractVersion: 3
    property bool bladeOpen: true
    property bool collapsed: false
    property int tabIndex: 0
    property int cornerReserveLeft: 0
    property int cornerReserveRight: 0
    property string edge: "left"
    property var host: null
    property var screen: null
    property int slotIndex: 0
    property int tabCount: 1
    property var providerService: null
    property var paths: Paths
    property var ui: ({ url: function(name) { return Qt.resolvedUrl("../../ui/" + name + ".qml") } })
    property var metrics: ({ options: function(rows) { return Format.metricOptions(rows) } })
    property var state: ({ get: function(key, fallback) { return fallback }, set: function() {} })
    property int closes: 0
    function service() { return files }
    function focusNext() {}
    function focusPrevious() {}
    function closeBlade() { closes++ }
  }

  function descendant(item, name) {
    if (String(item).indexOf(name + "_") === 0) return item
    for (var child of item.children || []) {
      var found = descendant(child, name)
      if (found) return found
    }
    return null
  }

  function init() {
    requests = []; navigated = []; selections = []; cancelled = []; context.bladeOpen = true; files.contextPath = "/repo/src"; gitRefreshes = 0; treeRefreshes = 0; context.closes = 0
    var component = Qt.createComponent("../../modules/branches/Module.qml")
    compare(component.status, Component.Ready, component.errorString())
    module = component.createObject(test, { context: context, width: 385, height: 650 })
    component.destroy()
    verify(module !== null)
    tryVerify(function() { return requests.length > 0 })
  }
  function cleanup() {
    if (module) module.destroy()
    module = null
    wait(0)
  }

  function load(document) {
    compare(requests[requests.length - 1].command, "git-places")
    compare(requests[requests.length - 1].args.join(" "), "--path /repo/src")
    requests[requests.length - 1].callback(document)
    wait(0)
  }

  function tree() { return descendant(module, "ArtifactTree") }
  function leafRows() { return tree().rows.filter(function(row) { return row.kind === "leaf" }) }
  function rowNamed(name) { return leafRows().filter(function(row) { return row.item.name === name })[0] }
  function statusOf(name) { return tree().metricTextFor(rowNamed(name).item, "status") }

  function test_rows_groups_and_status_text() {
    load(places)
    compare(module.status, "6 branches, 2 worktrees")
    var groups = tree().rows.filter(function(row) { return row.kind === "group" }).map(function(row) { return row.label })
    compare(groups.join("|"), "Branches|Remote|Worktrees")
    compare(leafRows().map(function(row) { return row.item.name }).join(","), "0.2.0,main,scratch,scratch,old,lonely,feature,free")
    verify(!leafRows()[2].child)
    verify(leafRows()[3].child)
    compare(leafRows()[3].depth, 2)
    compare(leafRows()[3].item.kind, "worktree")
    compare(leafRows()[3].item.detail, "/repo/target/wt/scratch")
    compare(rowNamed("free").item.detail, "detached at def5678")
    compare(statusOf("0.2.0"), "↑3 ↓1 M4 A1 ?2")
    compare(statusOf("free"), "clean")
    compare(statusOf("main"), "↑0 ↓0")
    compare(statusOf("scratch"), "U2")
    compare(tree().metricTextFor(leafRows()[3].item, "status"), "U2 locked")
    compare(statusOf("old"), "gone")
    compare(statusOf("lonely"), "no upstream")
    compare(statusOf("feature"), "remote")
    verify(tree().metricMarkup(rowNamed("0.2.0").item, "status").indexOf("<font color=") === 0)
    compare(tree().metricMarkup(rowNamed("0.2.0").item, "status").split("</font>").length, 6)
    compare(tree().metricMarkup(rowNamed("old").item, "status"), "")
    compare(tree().metricMarkup(rowNamed("0.2.0").item, "kind"), "")
    compare(tree().metricTextFor(rowNamed("main").item, "kind"), "both")
    compare(tree().metricTextFor(rowNamed("feature").item, "author"), "Other Dev")
    compare(tree().metricTextFor(rowNamed("main").item, "updated"), Format.isoDate(new Date(1789500000 * 1000)))
    compare(tree().metricTextFor(rowNamed("main").item, "summary"), "Release: 0.1.2")
  }

  function test_main_checkout_is_never_a_worktree_row_and_folds_persist() {
    load(places)
    compare(leafRows().filter(function(row) { return row.item.kind === "worktree" }).map(function(row) { return row.item.path }).join(","),
            "/repo/target/wt/scratch,/repo/target/wt/free")
    tree().forceActiveFocus()
    tree().currentIndex = tree().rows.indexOf(rowNamed("scratch"))
    var count = requests.length
    keyClick(Qt.Key_Return)
    compare(requests.length, count)
    compare(leafRows().map(function(row) { return row.item.name }).join(","), "0.2.0,main,scratch,old,lonely,feature,free")
    load(places)
    compare(leafRows().map(function(row) { return row.item.name }).join(","), "0.2.0,main,scratch,old,lonely,feature,free")
  }

  function test_ahead_behind_arrows_and_current_mark() {
    var document = JSON.parse(JSON.stringify(places))
    document.branches[0].worktree = ""
    document.worktrees.shift()
    load(document)
    compare(statusOf("0.2.0"), "↑3 ↓1")
    compare(tree().leafGlyph(rowNamed("0.2.0").item), "")
    compare(tree().leafLabel(rowNamed("0.2.0").item), "0.2.0 󰄬")
    compare(tree().leafGlyph(rowNamed("main").item), "")
    verify(tree().leafGlyphColor(rowNamed("main").item) !== tree().leafGlyphColor(rowNamed("feature").item))
    compare(tree().leafGlyph(rowNamed("feature").item), "󰅡")
    compare(tree().leafGlyph(rowNamed("free").item), "󰉖")
    compare(tree().leafGlyph(leafRows()[3].item), "󰉖")
  }

  function test_search_and_filter() {
    load(places)
    module.query = "kind:remote"
    wait(0)
    compare(leafRows().map(function(row) { return row.item.name }).join(","), "feature")
    module.query = "Other Dev"
    wait(0)
    compare(leafRows().map(function(row) { return row.item.name }).join(","), "feature")
    module.query = "origin/main"
    wait(0)
    compare(leafRows().map(function(row) { return row.item.name }).join(","), "main")
    compare(module.status, "1 of 7")
    module.query = ""
  }

  function test_enter_switches_and_refreshes() {
    load(places)
    tree().forceActiveFocus()
    tree().currentIndex = tree().rows.indexOf(rowNamed("main"))
    keyClick(Qt.Key_Return)
    var request = requests[requests.length - 1]
    compare(request.command, "git-switch")
    compare(request.args.join(" "), "--path /repo --branch main")
    compare(module.status, "Switching…")
    request.callback({ ok: false, error: "working tree is dirty" })
    compare(module.status, "working tree is dirty")
    compare(gitRefreshes, 0)
    keyClick(Qt.Key_Return)
    requests[requests.length - 1].callback({ ok: true })
    compare(gitRefreshes, 1)
    compare(treeRefreshes, 1)
    tryVerify(function() { return requests[requests.length - 1].command === "git-places" })
    compare(module.status, "6 branches, 2 worktrees")
  }

  function test_worktree_navigation_and_close() {
    load(places)
    tree().forceActiveFocus()
    tree().currentIndex = tree().rows.indexOf(rowNamed("free"))
    compare(navigated.join(","), "/repo/target/wt/free:browse")
    navigated = []
    keyClick(Qt.Key_Return)
    compare(navigated.join(","), "/repo/target/wt/free:browse")
    tree().currentIndex = tree().rows.indexOf(leafRows()[3])
    navigated = ["/repo/target/wt/free:browse"]
    keyClick(Qt.Key_Return)
    compare(navigated.join(","), "/repo/target/wt/free:browse,/repo/target/wt/scratch:browse")
    tree().currentIndex = tree().rows.indexOf(rowNamed("scratch"))
    navigated = navigated.slice(0, 2)
    keyClick(Qt.Key_O)
    compare(navigated.length, 3)
    compare(navigated[2], "/repo/target/wt/scratch:browse")
    tree().currentIndex = tree().rows.indexOf(rowNamed("main"))
    keyClick(Qt.Key_O)
    compare(navigated.length, 3)
    compare(requests[requests.length - 1].command, "git-switch")
    compare(requests[requests.length - 1].args.join(" "), "--path /repo --branch main")
    requests[requests.length - 1].callback({ ok: true })
    keyClick(Qt.Key_Escape)
    compare(context.closes, 1)
  }

  function test_selection_reuses_tree_and_refreshes_coalesce() {
    load(places)
    var header = descendant(module, "PaneHeader")
    compare(header.statusGlyph, "")
    compare(header.status, "0.2.0 ↑3 ↓1 M4 A1 ?2")
    tree().selectIndex(tree().rows.indexOf(rowNamed("0.2.0")))
    compare(selections.length, 1)
    compare(selections[0].index, 0)
    compare(navigated.length, 0)
    tree().selectIndex(tree().rows.indexOf(rowNamed("scratch")))
    compare(navigated.join(","), "/repo/target/wt/scratch:browse")
    tree().selectIndex(tree().rows.indexOf(rowNamed("feature")))
    compare(navigated.length, 1)
    var count = requests.length
    files.gitMetadataRefreshCount++
    files.gitMetadataRefreshCount++
    module.refresh()
    compare(requests.length, count)
    tryVerify(function() { return requests.length === count + 1 })
    var stale = requests[requests.length - 1]
    files.contextPath = "/other"
    compare(cancelled.length, 1)
    stale.callback(places)
    compare(module.document, null)
    tryVerify(function() { return requests.length === count + 2 })
    compare(requests[requests.length - 1].args.join(" "), "--path /other")
    context.bladeOpen = false
    compare(cancelled.length, 2)
    requests[requests.length - 1].callback(places)
    compare(module.document, null)
  }

  function test_git_refresh_and_errors_reload() {
    load(places)
    var count = requests.length
    files.gitMetadataRefreshCount++
    tryVerify(function() { return requests.length === count + 1 })
    load({ ok: false, error: "no repository here" })
    compare(module.status, "no repository here")
    compare(module.items.length, 0)
  }
}
