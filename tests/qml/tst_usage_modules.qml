import QtQuick
import QtTest
import "../../ui" as Ui
import "../../lib/PathText.js" as Paths
import "../../lib/KeyBindings.js" as KeyBindings

TestCase {
  id: test
  name: "UsageModules"
  width: 500
  height: 700
  visible: true
  when: windowShown
  property var module: null
  property var requests: []
  property int opened: 0
  property var cancelled: []

  Item {
    id: files
    property string contextPath: "/project"
    property bool autoHideSearch: false
    property var installedAgents: []
    property bool agentManagementEnabled: false
    property var artifactActions: null
    property var history: null
    property var keybindings: ({ plan: KeyBindings.compile({}) })
    function backendRequest(command, args, generation, callback) {
      requests.push({ command: command, args: args, callback: callback, id: "request-" + (requests.length + 1) })
      return "request-" + requests.length
    }
    function cancelBackendRequest(id) { cancelled.push(id) }
    function backendSubscribe() { return "watch" }
    function openDefault() { test.opened++ }
  }

  Ui.ArtifactInventory {
    id: inventory
    files: files
    providerId: "fileblade.core.mcp"
    providerRoot: "/core/mcp"
    activityMethod: "usage"
  }
  QtObject {
    id: provider
    property var inventory: inventory
    function attach(context) { inventory.attach(context) }
    function detach(context) { inventory.detach(context) }
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
    property int slotIndex: 0
    property int tabCount: 1
    property var providerService: provider
    property var paths: ({ canonical: Paths.fileUrl, parent: Paths.parent, join: Paths.join, within: Paths.within })
    property var ui: ({ url: function(name) { return Qt.resolvedUrl("../../ui/" + name + ".qml") } })
    property var metrics: ({ options: function(rows) { return [] } })
    property var state: ({ get: function(key, fallback) { return fallback }, set: function() {} })
    function service() { return files }
    function focusNext() {}
    function focusPrevious() {}
    function closeBlade() {}
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
    requests = []; cancelled = []; opened = 0; files.autoHideSearch = false
    files.contextPath = "/project"; context.collapsed = false
    inventory.items = []
    inventory.activity = null; inventory.activityError = ""
  }
  function cleanup() {
    if (module) module.destroy()
    module = null
    wait(0)
  }
  function test_navigation_and_activity_lifecycle_data() {
    return [{ tag: "skills", name: "skills" }, { tag: "mcp", name: "mcp" }]
  }
  function test_navigation_and_activity_lifecycle(data) {
    var component = Qt.createComponent("../../modules/" + data.name + "/blades/Module.qml")
    compare(component.status, Component.Ready, component.errorString())
    module = component.createObject(test, { context: context, width: 385, height: 650 })
    component.destroy()
    verify(module !== null)
    inventory.activity = { ok: true, schemaVersion: 1, until: "2026-09-16", coverageStart: "2026-09-14", days: [["2026-09-14", 2, 2, 0, 0, 0]] }
    var search = descendant(module, "PaneSearchField")
    var heatmap = descendant(module, "UsageHeatmap")
    var tree = descendant(module, "ArtifactTree")
    verify(search && heatmap && tree)
    verify(inventory.activityEnabled)
    for (var autoHide of [false, true]) {
      files.autoHideSearch = autoHide
      search.reveal()
      verify(search.activeFocus)
      keyClick(Qt.Key_Tab)
      verify(heatmap.activeFocus)
      keyClick(Qt.Key_End)
      keyClick(Qt.Key_Up)
      verify(heatmap.Accessible.description.indexOf("15") >= 0)
      keyClick(Qt.Key_Backtab, Qt.ShiftModifier)
      verify(search.activeFocus)
      keyClick(Qt.Key_Tab)
      keyClick(Qt.Key_Tab)
      verify(tree.activeFocus)
    }
    inventory.activityError = "usage store unavailable"
    verify(module.status.indexOf("Activity: usage store unavailable") >= 0)
    heatmap.forceActiveFocus()
    module.view.navigationTriggered("activity")
    wait(0)
    verify(!inventory.activityEnabled)
    verify(tree.activeFocus)
    inventory.refresh(); inventory.startActivity()
    module.view.navigationTriggered("activity")
    wait(0)
    verify(inventory.activityEnabled)
    module.height = 200
    wait(0)
    verify(!inventory.activityEnabled)
    if (data.name === "mcp") {
      tree.items = [{ id: "first", name: "docs", agent: "claude", scope: "user", source: { path: "~/config", redacted: false }, observed: [{ kind: "tool", name: "search", uses: 2, failed: 1 }] }]
      tree.currentIndex = tree.firstLeafIndex()
      tree.forceActiveFocus()
      keyClick(Qt.Key_Right)
      verify(tree.expandedFolders.first)
      compare(opened, 0)
      keyClick(Qt.Key_L)
      compare(tree.rowAt(tree.currentIndex).item.name, "search")
      compare(opened, 0)
      tree.currentIndex = tree.firstLeafIndex()
      keyClick(Qt.Key_O)
      compare(opened, 1)
    }
  }

  function helperRequests(method) {
    return requests.filter(function(request) { return request.args[request.args.indexOf("--method") + 1] === method })
  }

  function scopedRequests() {
    return helperRequests("usage").filter(function(request) { return request.args[request.args.length - 1].indexOf("--items") >= 0 })
  }

  function test_skill_selection_scopes_history_and_day_clicks_filter_without_rescanning_data() {
    return [{ tag: "narrow", width: 280 }, { tag: "wide", width: 1000 }]
  }

  function test_skill_selection_scopes_history_and_day_clicks_filter_without_rescanning(data) {
    test.width = data.width
    var component = Qt.createComponent("../../modules/skills/blades/Module.qml")
    module = component.createObject(test, { context: context, width: data.width, height: 650 })
    component.destroy()
    verify(module !== null)
    inventory.items = [
      { id: "alpha", name: "alpha", path: "/skills/alpha/SKILL.md", scope: "user", source: "" },
      { id: "beta", name: "beta", path: "/skills/beta/SKILL.md", scope: "user", source: "" }
    ]
    inventory.activity = { ok: true, schemaVersion: 1, until: "2026-09-16", coverageStart: "2026-09-14", days: [["2026-09-14", 7, 7, 0, 0, 0]] }
    var tree = descendant(module, "ArtifactTree")
    var heatmap = descendant(module, "UsageHeatmap")
    tree.currentIndex = tree.rows.findIndex(function(row) { return row.item && row.item.id === "alpha" })
    compare(module.selectedSkillId, "alpha")
    tryVerify(function() { return scopedRequests().length > 0 })
    var history = scopedRequests().slice(-1)[0]
    var arguments = JSON.parse(history.args[history.args.indexOf("--arguments") + 1])
    compare(JSON.parse(arguments[arguments.indexOf("--items") + 1])[0].id, "alpha")
    history.callback(Object.assign({}, inventory.activity, { days: [["2026-09-14", 2, 2, 0, 0, 0]] }))
    compare(heatmap.countsFor(heatmap.ordinal("2026-09-14"))[0], 2)

    var before = helperRequests("list").length
    var square = findChild(heatmap, "usage-day-2026-09-14")
    mouseClick(heatmap, square.x + 2, square.y + 2)
    compare(heatmap.selectedDay, "2026-09-14")
    var day = helperRequests("usage-day").slice(-1)[0]
    arguments = JSON.parse(day.args[day.args.indexOf("--arguments") + 1])
    compare(JSON.parse(arguments[arguments.indexOf("--items") + 1]).map(function(item) { return item.id }), ["alpha", "beta"])
    day.callback({ ok: true, items: [{ id: "beta" }] })
    compare(tree.visibleItems.map(function(item) { return item.id }), ["beta"])
    compare(module.selectedSkillId, "alpha")
    compare(helperRequests("list").length, before)
    compare(square.border.width, 1)

    heatmap.forceActiveFocus()
    heatmap.cursor = heatmap.ordinal("2026-09-15")
    keyClick(Qt.Key_Return)
    compare(heatmap.selectedDay, "2026-09-15")
    compare(tree.visibleItems.map(function(item) { return item.id }), ["beta"])
    var pending = helperRequests("usage-day").slice(-1)[0]
    module.clearDay()
    pending.callback({ ok: true, items: [] })
    compare(heatmap.selectedDay, "")
    compare(tree.visibleItems.length, 2)
    verify(cancelled.indexOf(pending.id) >= 0)
    compare(square.opacity, 1)

    tree.selectIndex(tree.rows.findIndex(function(row) { return row.item && row.item.id === "beta" }))
    tryVerify(function() { return scopedRequests().slice(-1)[0] !== history })
    var beta = scopedRequests().slice(-1)[0]
    tree.currentIndex = 0
    beta.callback(Object.assign({}, inventory.activity, { days: [] }))
    compare(module.selectedSkillId, "")
    compare(heatmap.countsFor(heatmap.ordinal("2026-09-14"))[0], 7)
    verify(cancelled.indexOf(beta.id) >= 0)
    files.contextPath = "/other"
    compare(module.dayFilter, "")
    compare(module.selectedSkillId, "")
  }
}
