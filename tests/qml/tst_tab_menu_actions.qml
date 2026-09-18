import QtQuick
import QtTest
import "../../blades"

TestCase {
  name: "TabMenuActions"

  Item {
    id: host
    property var layout: ({})
    property int replaced: 0

    function normalizeEdge(edge) { return edge === "right" ? "right" : "left" }
    function slots(edge) { return layout[edge].slots }
    function slotTabs(edge, index) { return layout[edge].slots[index].modules }
    function validIndex(index, length) { return Number.isInteger(index) && index >= 0 && index < length }
    function validMoveTab(index, length) { return validIndex(Number(index), length) }
    function cloneLayout(source) { return JSON.parse(JSON.stringify(source)) }
    function replaceLayout(next) { layout = next; replaced++ }
    function updateBlade(edge, change) {
      var next = cloneLayout(layout)
      change(next[edge])
      replaceLayout(next)
    }
  }

  BladeTabs {
    id: tabs
    host: host
  }

  function slot(id, names, active) {
    return { id: id, modules: names.map(function(name) { return { module: name, state: {} } }), active: active, fraction: 0.5 }
  }

  function init() {
    host.layout = { left: { open: true, slots: [slot("upper", ["files", "notes", "skills", "hooks"], 3), slot("lower", ["memory"], 0)] },
                    right: { open: false, slots: [] } }
    host.replaced = 0
  }

  function names(edge, index) {
    return host.slotTabs(edge, index).map(function(tab) { return tab.module }).join(",")
  }

  function test_close_to_the_right_keeps_the_tab_and_everything_before_it() {
    verify(tabs.removeTabsAfter("left", 0, 1))
    compare(names("left", 0), "files,notes")
    compare(host.slots("left")[0].active, 1)
  }

  function test_close_to_the_right_keeps_an_earlier_active_tab() {
    host.layout.left.slots[0].active = 0
    verify(tabs.removeTabsAfter("left", 0, 2))
    compare(names("left", 0), "files,notes,skills")
    compare(host.slots("left")[0].active, 0)
  }

  function test_close_to_the_right_refuses_the_last_tab_and_bad_indexes() {
    verify(!tabs.removeTabsAfter("left", 0, 3))
    verify(!tabs.removeTabsAfter("left", 0, 4))
    verify(!tabs.removeTabsAfter("left", 1, 0))
    compare(host.replaced, 0)
  }

  function test_move_to_bottom_appends_and_activates_in_the_lower_slot() {
    verify(tabs.moveTabToSlot("left", 0, 1, 1))
    compare(names("left", 0), "files,skills,hooks")
    compare(names("left", 1), "memory,notes")
    compare(host.slots("left")[1].active, 1)
    compare(host.slots("left")[0].active, 2)
  }

  function test_move_to_top_from_a_single_tab_slot_removes_that_slot() {
    verify(tabs.moveTabToSlot("left", 1, 0, 0))
    compare(host.slots("left").length, 1)
    compare(names("left", 0), "files,notes,skills,hooks,memory")
    compare(host.slots("left")[0].active, 4)
  }
}
