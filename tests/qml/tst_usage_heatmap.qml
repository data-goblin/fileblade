import QtQuick
import QtTest
import qs.Commons
import "../../ui" as Ui

TestCase {
  id: test
  name: "UsageHeatmap"
  width: 1100
  height: 200
  when: windowShown
  visible: true

  readonly property var payload: ({ ok: true, schemaVersion: 1, kind: "skill", coverageStart: "2026-01-07", until: "2026-09-16", ingestPending: false,
    days: [["2026-01-07", 1, 1, 0, 0, 0], ["2026-08-03", 2, 2, 0, 0, 0], ["2026-09-14", 6, 4, 2, 1, 0], ["2026-09-15", 3, 2, 1, 0, 1]] })

  Component { id: factory; Ui.UsageHeatmap { unitLabel: "skill uses"; calendarRule: ({ firstDay: 1, minimumDays: 4 }) } }
  SignalSpy { id: dismissedSpy; signalName: "dismissed" }

  function create(width) {
    var view = createTemporaryObject(factory, test, { width: width })
    verify(view !== null)
    view.payload = payload
    dismissedSpy.target = view
    dismissedSpy.clear()
    waitForRendering(view)
    return view
  }

  function cell(view, day) { return findChild(view, "usage-day-" + day) }
  function sameColor(actual, expected) { verify(Qt.colorEqual(actual, expected), actual + " is not " + expected) }
  function dateText(year, month, day) { return Qt.locale().toString(new Date(year, month - 1, day), "ddd d MMM yyyy") }

  function test_week_count_follows_width_with_the_newest_week_on_the_right() {
    for (var sample of [[280, 28], [385, 38], [1000, 100]]) {
      var view = create(sample[0])
      compare(view.weeks, sample[1], "weeks at " + sample[0])
      compare(view.implicitHeight, 68)
      var newest = cell(view, "2026-09-14")
      compare(newest.x + newest.width, sample[0])
      compare(newest.y, 0)
      verify(newest.visible)
      verify(!cell(view, "2026-09-17").visible)
      var oldest = cell(view, "2026-09-14").x - (sample[1] - 1) * view.pitch
      verify(oldest >= 0 && oldest < view.pitch)
    }
  }

  function test_widening_keeps_existing_colours_and_levels_come_from_the_whole_window() {
    var view = create(280)
    var keys = ["2026-08-03", "2026-09-14", "2026-09-15", "2026-09-16", "2026-09-01"]
    var before = keys.map(function(key) { return String(cell(view, key).color) })
    sameColor(cell(view, "2026-09-14").color, Util.alpha(Color.accent, 1.0))
    sameColor(cell(view, "2026-09-15").color, Util.alpha(Color.accent, 0.75))
    sameColor(cell(view, "2026-08-03").color, Util.alpha(Color.accent, 0.50))
    sameColor(cell(view, "2026-09-16").color, Util.alpha(Color.bar.text, 0.06))
    verify(cell(view, "2026-01-07") === null)
    view.width = 1000
    waitForRendering(view)
    compare(keys.map(function(key) { return String(cell(view, key).color) }), before)
    sameColor(cell(view, "2026-01-07").color, Util.alpha(Color.accent, 0.30))
  }

  function test_days_before_coverage_have_no_fill() {
    var view = create(385)
    sameColor(cell(view, "2026-01-06").color, "transparent")
    sameColor(cell(view, "2025-12-29").color, "transparent")
    sameColor(cell(view, "2026-01-08").color, Util.alpha(Color.bar.text, 0.06))
    view.payload = Object.assign({}, payload, { coverageStart: null, days: [] })
    sameColor(cell(view, "2026-09-16").color, "transparent")
  }

  function test_keyboard_cursor_reads_the_day_under_it() {
    var view = create(385)
    view.forceActiveFocus()
    verify(view.activeFocus)
    compare(view.Accessible.description, dateText(2026, 9, 16) + ": no skill uses")
    var outline = findChild(view, "usage-cursor")
    verify(outline.visible)
    keyClick(Qt.Key_Up)
    compare(view.Accessible.description, dateText(2026, 9, 15) + ": 3 skill uses (2 agent, 1 you, 1 failed)")
    compare(outline.x, cell(view, "2026-09-15").x - 1)
    compare(outline.y, cell(view, "2026-09-15").y - 1)
    keyClick(Qt.Key_Up)
    compare(view.Accessible.description, dateText(2026, 9, 14) + ": 6 skill uses (4 agent, 2 you, 1 scheduled)")
    keyClick(Qt.Key_Right)
    compare(view.Accessible.description, dateText(2026, 9, 14) + ": 6 skill uses (4 agent, 2 you, 1 scheduled)")
    for (var week = 0; week < 6; week++) keyClick(Qt.Key_Left)
    compare(view.Accessible.description, dateText(2026, 8, 3) + ": 2 skill uses (2 agent)")
    view.unitLabel = "MCP calls"
    keyClick(Qt.Key_Home)
    compare(view.Accessible.description, dateText(2025, 12, 29) + ": no history yet")
    keyClick(Qt.Key_Left)
    compare(view.Accessible.description, dateText(2025, 12, 29) + ": no history yet")
    keyClick(Qt.Key_End)
    compare(view.Accessible.description, dateText(2026, 9, 16) + ": no MCP calls")
    view.payload = Object.assign({}, payload, { days: [["2026-09-16", 1, 1, 0, 0, 0]] })
    compare(view.Accessible.description, dateText(2026, 9, 16) + ": 1 MCP call (1 agent)")
    keyClick(Qt.Key_Escape)
    compare(dismissedSpy.count, 1)
  }

  function test_hover_names_the_day_under_the_pointer() {
    var view = create(385)
    var scene = test
    while (scene.parent) scene = scene.parent
    var tip = null
    tryVerify(function() { tip = findChild(scene, "usage-tip"); return tip !== null })
    var target = cell(view, "2026-09-15")
    mouseMove(view, target.x + 2, target.y + 2)
    verify(tip.visible)
    compare(tip.title, dateText(2026, 9, 15))
    compare(view.describe(view.tipDay), dateText(2026, 9, 15) + ": 3 skill uses (2 agent, 1 you, 1 failed)")
    tryCompare(tip, "revealed", true)
    target = cell(view, "2026-09-14")
    mouseMove(view, target.x + 2, target.y + 2)
    compare(tip.title, dateText(2026, 9, 14))
    compare(view.describe(view.tipDay), dateText(2026, 9, 14) + ": 6 skill uses (4 agent, 2 you, 1 scheduled)")
    mouseMove(view, 0, 0)
    verify(!tip.visible)
  }

  function test_unreadable_payload_hides_the_grid() {
    var view = create(385)
    view.payload = { ok: true, schemaVersion: 1, until: "2026-13-01", days: [] }
    verify(!view.visible)
    view.payload = null
    verify(!view.visible)
    compare(view.Accessible.description, "")
  }

  function test_selected_day_remains_outlined_inside_its_cell_without_focus() {
    var view = create(385)
    view.selectedDay = "2026-09-15"
    var selected = cell(view, view.selectedDay)
    var other = cell(view, "2026-09-14")
    compare(selected.opacity, 1)
    compare(selected.border.width, 1)
    sameColor(selected.border.color, Color.bar.text)
    compare(selected.width, view.cell)
    compare(selected.height, view.cell)
    compare(other.opacity, 0.25)
    compare(other.border.width, 0)
    test.forceActiveFocus()
    verify(!view.activeFocus)
    compare(selected.border.width, 1)
    view.selectedDay = ""
    compare(selected.border.width, 0)
    compare(other.opacity, 1)
  }
}
