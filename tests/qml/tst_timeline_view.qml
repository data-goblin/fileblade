import QtQuick
import QtTest
import "../../ui" as Ui
import "../../lib/MediaBins.js" as Bins

TestCase {
  id: test
  name: "MediaTimelineView"
  width: 500
  height: 650
  when: windowShown
  visible: true

  Component { id: factory; Ui.MediaTimeline { x: 300; width: 92; headerWidth: 360; height: 400; contentHeight: 12000; viewportHeight: 400 } }
  SignalSpy { id: seekSpy; signalName: "seekRequested" }

  function create() {
    var rows = []
    for (var i = 0; i < 100; i++) rows.push({ path: "/" + i, date: i < 30 ? "2024-02-29" : "2024-12-20" })
    var view = createTemporaryObject(factory, test, { records: Bins.ordered(rows) })
    verify(view !== null)
    seekSpy.target = view
    seekSpy.clear()
    waitForRendering(view)
    return view
  }

  function test_seek_drill_and_keyboard_leave_content_filter_alone() {
    var view = create()
    compare(view.detail.level, "months")
    compare(view.period.key, "2024-02")
    compare(view.canGoUp, true)
    compare(view.coarserLevel, "years")
    verify(view.canDrill)
    var count = view.records.length
    compare(view.showEmptyPeriods, false)
    compare(view.detail.bins.length, 2)
    compare(view.detail.bins[0].key, "2024-02")
    compare(view.detail.bins[1].key, "2024-12")
    var records = JSON.stringify(view.records)
    verify(view.seek(1, 0))
    compare(seekSpy.count, 1)
    compare(seekSpy.signalArguments[0][0], 30 * 124)
    compare(view.period.key, "2024-12")
    view.forceActiveFocus()
    keyClick(Qt.Key_Right)
    compare(view.detail.level, "weeks")
    compare(view.parents[0].key, "2024-12")
    compare(view.detail.bins.length, 1)
    compare(view.detail.bins[0].count, 70)
    compare(view.records.length, count)
    compare(seekSpy.count, 1)
    verify(view.seek(0, 0))
    keyClick(Qt.Key_Right)
    compare(view.detail.level, "days")
    compare(view.detail.bins.length, 1)
    compare(view.detail.bins[0].key, "2024-12-20")
    compare(view.detail.bins[0].count, 70)
    compare(view.canDrill, false)
    keyClick(Qt.Key_Left)
    compare(view.detail.level, "weeks")
    keyClick(Qt.Key_Left)
    compare(view.detail.level, "months")
    compare(view.period.key, "2024-12")
    keyClick(Qt.Key_Home)
    compare(view.period.key, "2024-02")
    keyClick(Qt.Key_Down)
    compare(view.period.key, "2024-12")
    keyClick(Qt.Key_Up)
    compare(view.period.key, "2024-02")
    keyClick(Qt.Key_End)
    compare(view.period.key, "2024-12")
    compare(view.records.length, count)
    compare(JSON.stringify(view.records), records)
  }

  function test_pointer_seek_uses_visible_bins_and_optional_empty_bins_are_inert() {
    var view = create()
    var records = JSON.stringify(view.records)
    compare(view.showEmptyPeriods, false)
    compare(view.detail.bins.length, 2)
    mouseClick(view, 50, view.axisTop + view.rowHeight * 1.5)
    compare(seekSpy.count, 1)
    compare(view.period.key, "2024-12")
    mouseClick(view, 50, view.axisTop + view.rowHeight * 0.5)
    compare(seekSpy.count, 2)
    compare(view.period.key, "2024-02")
    var outline = findChild(view, "timeline-viewport")
    verify(outline !== null)
    compare(outline.visible, true)
    compare(outline.radius, 0)
    var width = outline.width
    mouseMove(view, 50, view.axisTop + view.rowHeight * 1.5)
    compare(outline.width, width)
    compare(view.contentY, 0)
    view.contentY = 3500
    compare(view.viewport.active[0], true)
    compare(view.viewport.active[1], true)
    view.contentY = 3800
    compare(view.viewport.active[0], false)
    compare(view.viewport.active[1], true)
    view.contentY = 0
    view.showEmptyPeriods = true
    compare(view.detail.bins.length, 13)
    compare(view.detail.bins[5].key, "2024-06")
    compare(view.detail.bins[5].count, 0)
    mouseClick(view, 50, view.axisTop + view.rowHeight * 11.5)
    compare(seekSpy.count, 3)
    compare(view.period.key, "2024-12")
    mouseClick(view, 50, view.axisTop + view.rowHeight * 5.5)
    compare(seekSpy.count, 3)
    compare(view.period.key, "2024-12")
    view.contentY = 3500
    compare(view.viewport.active[1], true)
    compare(view.viewport.active[11], true)
    view.contentY = 3800
    compare(view.viewport.active[1], false)
    compare(view.viewport.active[11], true)
    view.showEmptyPeriods = false
    compare(view.detail.bins.length, 2)
    compare(view.period.key, "2024-12")
    compare(view.viewport.active[0], false)
    compare(view.viewport.active[1], true)
    compare(view.records.length, 100)
    compare(JSON.stringify(view.records), records)
  }

  function test_outline_drag_seeks_without_hover_geometry_changes() {
    var view = create()
    var start = view.outlineTop + view.outlineHeight / 2
    mousePress(view, 50, start)
    mouseMove(view, 50, view.axisTop + view.rowHeight * 1.5, 20)
    verify(seekSpy.count > 0)
    verify(seekSpy.signalArguments[seekSpy.count - 1][0] > 3000)
    mouseRelease(view, 50, view.axisTop + view.rowHeight * 1.5)
    compare(view.records.length, 100)
  }

  function test_header_buttons_have_names_and_keyboard_activation() {
    var view = create()
    var down = findChild(view, "timeline-down")
    var up = findChild(view, "timeline-up")
    verify(down.Accessible.name.indexOf("Show finer dates") === 0)
    compare(up.Accessible.name, "Show coarser dates")
    compare(up.enabled, true)
    down.forceActiveFocus()
    keyClick(Qt.Key_Space)
    compare(view.detail.level, "weeks")
    up.forceActiveFocus()
    keyClick(Qt.Key_Return)
    compare(view.detail.level, "months")
    view.records = Bins.ordered([{ path: "/undated", date: "" }])
    compare(view.parents.length, 0)
    compare(view.period.key, "undated")
    compare(view.canDrill, false)
  }

  function test_hour_detail_keeps_days_and_seeks_hour_bars() {
    var view = createTemporaryObject(factory, test, { records: Bins.ordered([
      { path: "/a", date: "2024-06-01T02:00:00" },
      { path: "/b", date: "2024-06-01T02:30:00" },
      { path: "/c", date: "2024-06-01T14:00:00" },
      { path: "/d", date: "2024-06-02T05:00:00" },
      { path: "/e", date: "2024-06-02" }
    ]), levelChoice: "days" })
    verify(view !== null)
    seekSpy.target = view
    seekSpy.clear()
    compare(view.detail.bins.length, 2)
    var days = view.detail.bins.map(function(day) { return day.key })
    view.forceActiveFocus()
    keyClick(Qt.Key_Right)
    compare(view.hourly, true)
    compare(view.detail.level, "hours")
    compare(view.detail.bins.map(function(day) { return day.key }), days)
    compare(view.detail.count, 5)
    compare(view.detail.bins[0].hours.length, 24)
    compare(view.detail.bins[0].hours[2].count, 2)
    compare(view.detail.bins[0].hours[14].count, 1)
    compare(view.detail.bins[1].hours[5].count, 1)
    compare(view.detail.bins[1].hours[0].count, 0)
    compare(view.canDrill, false)
    var bars = findChild(view, "timeline-hours-2024-06-01")
    verify(bars !== null && bars.visible)
    mouseClick(view, bars.x + bars.width * 14.5 / 24, view.axisTop + view.rowHeight / 2)
    compare(seekSpy.count, 1)
    compare(seekSpy.signalArguments[0][0], 248)
    keyClick(Qt.Key_Left)
    compare(view.detail.level, "days")
    compare(view.detail.bins.map(function(day) { return day.key }), days)
    compare(view.records.length, 5)
  }
}
