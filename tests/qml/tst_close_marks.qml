import QtQuick
import QtTest
import "../../blades" as Blades
import "../../ui" as PluginUi

TestCase {
  id: test
  name: "CloseMarks"
  width: 480
  height: 160
  visible: true
  when: windowShown

  QtObject {
    id: fakeHost
    property bool dragActive: false
    property string dragEdge: ""
    property int dragIndex: -1
    property int dragTab: -1
    property bool settingsOpen: false
    property string settingsEdge: ""
    property var titles: ["FileBlade", "Fabric", "Databricks"]
    property var closed: []
    function slotModuleAt(edge, slotIndex, tabIndex) { return ["files", "fabric", "databricks"][tabIndex] || "" }
    function tabTitle(edge, slotIndex, tabIndex) { return titles[tabIndex] || "" }
    function slots(edge) { return [{}] }
    function removeTab(edge, slotIndex, tabIndex) { closed = closed.concat([tabIndex]) }
    function setTabStateValue() { return true }
    function sendTabAcross() { return true }
    function moveTabToSlot() { return true }
  }

  Item {
    id: fakeSlot
    property var host: fakeHost
    property string edge: "left"
    property int slotIndex: 0
    property int activeTab: 0
    property string moduleId: "files"
    property var hostWindow: null
    property var tabs: [{ module: "files", state: {} }, { module: "fabric", state: {} }, { module: "databricks", state: {} }]
    property var requested: []
    function requestCloseTab(index) { requested = requested.concat([index]) }
    function requestCloseTabsAfter(index) {}
    function openModulePicker(x) {}
  }

  QtObject {
    id: fakeContext
    property bool collapsed: false
    property real cornerReserveRight: 0
    property bool dragging: false
    property var host: fakeHost
    function handlePressed() {}
    function handleMoved() {}
    function handleReleased() {}
    function handleCanceled() {}
    function selectTab(index) { fakeSlot.activeTab = index }
    function service(name) { return null }
    function toggleCollapsed() {}
    function toggleSettings() {}
  }

  Blades.BladeTabBar {
    id: bar
    width: 480
    height: 32
    slot: fakeSlot
    context: fakeContext
  }

  QtObject {
    id: fakeView
    property var options: [{ key: "size", label: "Size", shortLabel: "Size" }, { key: "modified", label: "Modified", shortLabel: "Modified" }]
    property var columns: ["size", "modified"]
    property var sorts: []
    property bool filterActive: false
    property var removed: []
    function sortFor(key) { return null }
    function kindOf(key) { return "number" }
    function removeColumn(index) { removed = removed.concat([index]) }
  }

  PluginUi.MetricPicker {
    id: column
    x: 120
    y: 80
    width: 120
    view: fakeView
    columnIndex: 0
    triggerWidth: 120
  }

  function findAll(item, predicate, found) {
    if (predicate(item)) found.push(item)
    var kids = item.children || []
    for (var i = 0; i < kids.length; i++) findAll(kids[i], predicate, found)
    return found
  }

  function textItems(value) {
    return findAll(bar, function(item) { return item.text === value && item.visible !== false && item.width > 0 }, [])
  }

  function init() {
    fakeView.removed = []
    fakeSlot.requested = []
    fakeSlot.activeTab = 0
  }

  function test_the_close_mark_sits_right_of_every_tab_title() {
    var marks = textItems("×")
    compare(marks.length, fakeHost.titles.length)
    for (var i = 0; i < fakeHost.titles.length; i++) {
      var title = textItems(fakeHost.titles[i].toUpperCase())[0]
      verify(title, "title " + fakeHost.titles[i])
      var titleBox = title.mapToItem(bar, 0, 0)
      var mark = null
      for (var j = 0; j < marks.length; j++) if (marks[j].parent === title.parent) mark = marks[j]
      verify(mark, "close mark beside " + fakeHost.titles[i])
      var markBox = mark.mapToItem(bar, 0, 0)
      verify(markBox.x >= titleBox.x + title.width, fakeHost.titles[i] + " close mark is right of its title")
    }
  }

  function test_hovering_reveals_the_right_hand_mark_and_clicking_it_asks_to_close() {
    var title = textItems("FABRIC")[0]
    var center = title.mapToItem(bar, title.width / 2, title.height / 2)
    mouseMove(bar, center.x, center.y)
    var mark = null
    var marks = textItems("×")
    for (var i = 0; i < marks.length; i++) if (marks[i].parent === title.parent) mark = marks[i]
    tryCompare(mark, "opacity", 1)
    var markCenter = mark.mapToItem(bar, mark.width / 2, mark.height / 2)
    verify(markCenter.x > center.x)
    mouseMove(bar, markCenter.x, markCenter.y)
    mouseClick(bar, markCenter.x, markCenter.y)
    compare(fakeSlot.requested.join(","), "1")
  }

  function test_a_column_label_shows_its_remove_mark_at_its_right_end() {
    var label = findAll(column, function(item) { return item.text === "SIZE" }, [])[0]
    verify(label)
    mouseMove(column, column.width / 2, column.height / 2)
    var mark = findAll(column, function(item) { return item.text === "×" }, [])[0]
    tryCompare(mark, "visible", true)
    var markCenter = mark.mapToItem(column, mark.width / 2, mark.height / 2)
    verify(markCenter.x > column.width / 2, "remove mark is on the right half of the column label")
    verify(column.width - (mark.x + mark.width) < mark.width, "remove mark hugs the right edge")
    mouseMove(column, markCenter.x, markCenter.y)
    mouseClick(column, markCenter.x, markCenter.y)
    compare(fakeView.removed.join(","), "0")
  }
}
