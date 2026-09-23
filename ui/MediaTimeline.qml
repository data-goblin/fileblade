import QtQuick
import qs.Commons
import "../lib/MediaBins.js" as Bins
import "../lib/MediaDates.js" as Dates
import "../theme"

FocusScope {
  id: timeline

  property var records: []
  property var calendarRule: ({ firstDay: 1, minimumDays: 4 })
  property int columns: 1
  property real pitch: 124
  property real tileHeight: 120
  property real contentY: 0
  property real contentHeight: 0
  property real viewportHeight: 0
  property bool showEmptyPeriods: false
  property real headerWidth: width
  property var parents: []
  property string navigatedKey: ""
  property string levelChoice: ""
  property bool hourly: false
  property real dragOffset: 0
  property string monthFormat: ""
  property string weekFormat: ""
  property string dayFormat: ""
  readonly property int headerHeight: Style.space(26)
  readonly property real axisTop: headerHeight + Style.space(6)
  readonly property real axisHeight: Math.max(0, height - axisTop - Style.space(6))
  readonly property int capacity: Math.max(2, Math.floor(axisHeight / Style.space(18)))
  readonly property var globalLadder: ["years", "months", "weeks", "days"]
  readonly property var autoDetail: Bins.overview(records, capacity, calendarRule, showEmptyPeriods)
  readonly property var globalDetail: {
    if (levelChoice === "") return autoDetail
    var result = null
    try { result = Bins.build(records, levelChoice, null, calendarRule) } catch (error) { return autoDetail }
    if (!result || !result.bins.length) return autoDetail
    return showEmptyPeriods ? Bins.compact(result, capacity) : Bins.compact(Bins.withoutEmpty(result), capacity)
  }
  readonly property var dateDetail: parents.length ? Bins.child(records, parents[parents.length - 1], capacity, calendarRule, showEmptyPeriods) : globalDetail
  readonly property var detail: hourly ? Bins.hourly(records, dateDetail, calendarRule) : dateDetail
  readonly property string coarserLevel: {
    var index = globalLadder.indexOf(detail.level)
    return index > 0 ? globalLadder[index - 1] : ""
  }
  readonly property var bounds: Bins.geometry(detail.bins, Math.max(1, columns), pitch, tileHeight)
  readonly property var viewport: Bins.viewport(bounds, contentY, viewportHeight)
  readonly property bool spansYears: {
    var seen = null
    for (var i = 0; i < detail.bins.length; i++) {
      var year = detail.bins[i].weekYear !== undefined ? detail.bins[i].weekYear : detail.bins[i].year
      if (year === undefined || year === null) continue
      if (seen === null) seen = year
      else if (seen !== year) return true
    }
    return false
  }
  readonly property bool sparseRows: !hourly && detail.bins.length > 0 && detail.bins.length * Style.space(26) <= axisHeight && detail.bins.every(function(bin) { return bin.count === 1 })
  readonly property real rowHeight: Math.min(Style.space(19), sparseRows ? Style.space(26) : axisHeight / Math.max(1, detail.bins.length))
  readonly property real occupiedHeight: records.length ? (Math.ceil(records.length / Math.max(1, columns)) - 1) * pitch + tileHeight : 0
  readonly property bool showOutline: occupiedHeight > viewportHeight && viewport.start !== null
  readonly property int activePeriod: {
    for (var i = 0; i < detail.bins.length; i++) if (detail.bins[i].key === navigatedKey) return i
    return viewport.first
  }
  readonly property var period: activePeriod >= 0 ? detail.bins[activePeriod] : null
  readonly property var nextDetail: !hourly && period ? Bins.child(records, period, capacity, calendarRule, showEmptyPeriods) : null
  readonly property bool canDrill: !hourly && (detail.level === "days"
    ? detail.bins.some(function(day) { return day.level === "days" && day.indices.some(function(index) { return records[index].date.hour !== null }) })
    : !!nextDetail && period.count > 0 && nextDetail.bins.length <= capacity)
  readonly property bool canGoUp: hourly || parents.length > 0 || coarserLevel !== ""
  readonly property string periodLabel: period ? label(period, false) : (parents.length ? label(parents[parents.length - 1], false) : "All dates")
  readonly property string levelLabel: ({ ranges: "Years", years: "Years", months: "Months", weeks: "Weeks", days: "Days", hours: "Hours" })[detail.level] || "Dates"
  readonly property color lightBlue: "#89b4fa"
  readonly property color darkBlue: Qt.darker(lightBlue, 2.25)
  readonly property real outlineTop: viewport.start === null ? 0 : axisTop + viewport.start * rowHeight
  readonly property real outlineHeight: viewport.start === null ? 0 : Math.max(1, (viewport.end - viewport.start) * rowHeight)

  function barWidth(count) {
    if (!(count > 0) || !(detail.maximum > 0)) return 0
    var span = Style.space(23)
    var scaled = span * Math.sqrt(count / detail.maximum)
    return Math.max(1, Math.min(span, Math.round(scaled)))
  }

  signal seekRequested(real position)

  width: Style.space(92)
  activeFocusOnTab: true
  Accessible.role: Accessible.Pane
  Accessible.name: "Media date timeline"
  Accessible.description: "Up and Down seek periods. Left shows coarser dates. Right shows finer dates."

  function formatted(pattern, start) {
    if (String(pattern || "") === "") return ""
    var date = Dates.calendar(start)
    var moment = new Date(date.year, date.month - 1, date.day || 1)
    var text = Qt.formatDate(moment, String(pattern))
    return String(text || "")
  }

  function label(entry, compact) {
    if (entry.level === "undated") return "Undated"
    if (entry.level === "unknown") return compact ? "Unknown" : "Unknown precision"
    if (entry.level === "ranges" || entry.level === "years") return entry.key
    if (entry.level === "weeks") {
      if (!compact) return entry.weekYear + " · Week " + Dates.pad(entry.week)
      if (weekFormat !== "") {
        return String(weekFormat).replace("yyyy", entry.weekYear)
          .replace("yy", String(entry.weekYear).slice(-2)).replace("ww", Dates.pad(entry.week))
      }
      return spansYears
        ? String(entry.weekYear).slice(-2) + " Wk" + Dates.pad(entry.week)
        : "Wk " + Dates.pad(entry.week)
    }
    if (entry.level === "hours") {
      var when = Dates.calendar(Math.floor(entry.start))
      var name = Qt.locale().standaloneMonthName(when.month - 1, Locale.ShortFormat)
      var hour = Dates.pad(entry.hour) + ":00"
      return compact ? hour : name + " " + Dates.pad(when.day) + " " + hour
    }
    var date = Dates.calendar(entry.start)
    var month = Qt.locale().standaloneMonthName(date.month - 1, Locale.ShortFormat)
    if (entry.level === "days") {
      if (!compact) return Dates.dayKey(entry.start)
      var day = formatted(dayFormat, entry.start)
      return day !== "" ? day : month + " " + Dates.pad(date.day)
    }
    if (!compact) return month + " " + date.year
    var monthText = formatted(monthFormat, entry.start)
    return monthText !== "" ? monthText : String(date.year).slice(-2) + "-" + month
  }

  function reset() { hourly = false; parents = []; navigatedKey = ""; levelChoice = "" }

  function drill() {
    if (!canDrill) return
    if (detail.level === "days") { hourly = true; return }
    parents = parents.concat([period])
    navigatedKey = ""
  }

  function up() {
    if (hourly) { hourly = false; return }
    if (parents.length > 0) {
      var previous = parents[parents.length - 1]
      parents = parents.slice(0, -1)
      navigatedKey = previous.key
      return
    }
    if (coarserLevel === "") return
    levelChoice = coarserLevel
    navigatedKey = ""
  }

  function seek(index, fraction) {
    var position = Bins.seek(bounds, index, fraction, contentHeight, viewportHeight)
    if (position === null) return false
    navigatedKey = detail.bins[index].key
    seekRequested(position)
    return true
  }

  function seekAt(y, x) {
    var position = Math.max(0, Math.min(detail.bins.length - 0.00001, (y - axisTop) / Math.max(1, rowHeight)))
    var day = detail.bins[Math.floor(position)]
    if (hourly && day && day.hours && x >= Style.space(43)) {
      var hour = Math.max(0, Math.min(23, Math.floor((x - Style.space(43)) * 24 / Math.max(1, width - Style.space(43)))))
      var hourBounds = Bins.geometry(day.hours, Math.max(1, columns), pitch, tileHeight)
      var target = Bins.seek(hourBounds, hour, 0, contentHeight, viewportHeight)
      if (target !== null) { navigatedKey = day.key; seekRequested(target) }
      return
    }
    seek(Math.floor(position), position - Math.floor(position))
  }

  function movePeriod(direction, edge) {
    var index = edge ? (direction > 0 ? 0 : detail.bins.length - 1) : activePeriod + direction
    for (; index >= 0 && index < detail.bins.length; index += direction) if (seek(index, 0)) return
  }

  onRecordsChanged: if (hourly || parents.length || navigatedKey !== "") reset()
  onCalendarRuleChanged: if (hourly || parents.length || navigatedKey !== "") reset()
  onShowEmptyPeriodsChanged: reset()
  onCapacityChanged: Qt.callLater(function() {
    if (timeline.hourly && timeline.dateDetail.level !== "days") timeline.hourly = false
    if (timeline.parents.length && timeline.detail.bins.length > timeline.capacity) timeline.reset()
  })

  Keys.onPressed: function(event) {
    if (event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier)) return
    if (event.key === Qt.Key_Left) up()
    else if (event.key === Qt.Key_Right) drill()
    else if (event.key === Qt.Key_Up) movePeriod(-1, false)
    else if (event.key === Qt.Key_Down) movePeriod(1, false)
    else if (event.key === Qt.Key_Home) movePeriod(1, true)
    else if (event.key === Qt.Key_End) movePeriod(-1, true)
    else return
    event.accepted = true
  }

  Item {
    x: timeline.width - timeline.headerWidth
    width: timeline.headerWidth
    height: timeline.headerHeight

    Text {
      anchors.left: parent.left
      anchors.right: buttons.left
      anchors.rightMargin: Style.space(6)
      anchors.verticalCenter: parent.verticalCenter
      textFormat: Text.PlainText
      text: timeline.periodLabel + " · " + timeline.levelLabel
      color: Color.muted
      elide: Text.ElideRight
      font.family: Style.font.family
      font.pixelSize: Typography.caption
    }

    Row {
      id: buttons
      anchors.right: parent.right
      height: parent.height
      Repeater {
        model: 2
        delegate: Rectangle {
          id: button
          required property int index
          objectName: index === 0 ? "timeline-up" : "timeline-down"
          width: Style.space(26)
          height: parent.height
          enabled: index === 0 ? timeline.canGoUp : timeline.canDrill
          activeFocusOnTab: enabled
          color: "transparent"
          border.width: activeFocus ? 1 : 0
          border.color: Color.accent
          Accessible.role: Accessible.Button
          Accessible.name: index === 0 ? "Show coarser dates" : "Show finer dates for " + timeline.periodLabel
          Accessible.onPressAction: activate()
          function activate() { if (enabled) { if (index === 0) timeline.up(); else timeline.drill() } }
          Keys.onPressed: function(event) {
            if (event.key !== Qt.Key_Return && event.key !== Qt.Key_Enter && event.key !== Qt.Key_Space) return
            activate()
            event.accepted = true
          }
          Text {
            anchors.centerIn: parent
            text: button.index === 0 ? "▴" : "▾"
            color: button.enabled ? Color.bar.text : Color.muted
            opacity: button.enabled ? 1 : 0.45
            font.pixelSize: Typography.body
          }
          MouseArea {
            anchors.fill: parent
            cursorShape: parent.enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
            onClicked: { button.forceActiveFocus(); button.activate() }
          }
        }
      }
    }
  }

  Repeater {
    model: timeline.detail.bins
    delegate: Item {
      id: mark
      required property int index
      required property var modelData
      readonly property bool inViewport: timeline.viewport.active[index] === true
      x: 0
      y: timeline.axisTop + index * timeline.rowHeight
      width: timeline.width
      height: timeline.rowHeight
      Accessible.role: modelData.count > 0 ? Accessible.Button : Accessible.StaticText
      Accessible.name: timeline.label(modelData, false) + ": " + modelData.count + " media"
      Accessible.onPressAction: timeline.seek(index, 0)

      Rectangle {
        anchors.fill: parent
        visible: timeline.sparseRows && timeline.activePeriod === mark.index
        color: timeline.darkBlue
        radius: Style.space(3)
      }
      Text {
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        width: timeline.sparseRows ? parent.width : Style.space(43)
        textFormat: Text.PlainText
        text: timeline.label(mark.modelData, !timeline.sparseRows)
        color: timeline.sparseRows && timeline.activePeriod === mark.index ? timeline.lightBlue : Color.muted
        font.family: Style.font.family
        font.pixelSize: Typography.caption
        elide: Text.ElideRight
      }
      Rectangle {
        id: countMark
        visible: !timeline.sparseRows && !timeline.hourly
        anchors.right: countText.left
        anchors.rightMargin: Style.space(4)
        anchors.verticalCenter: parent.verticalCenter
        width: timeline.barWidth(mark.modelData.count)
        height: Style.space(5)
        color: mark.inViewport ? timeline.lightBlue : timeline.darkBlue
        Repeater {
          model: mark.modelData.level === "undated" ? Math.floor(countMark.width / Style.space(4)) : 0
          delegate: Rectangle {
            required property int index
            x: index * Style.space(4)
            width: 1
            height: countMark.height
            color: Color.bar.background
          }
        }
      }
      Text {
        id: countText
        visible: !timeline.sparseRows && !timeline.hourly
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        width: Style.space(22)
        textFormat: Text.PlainText
        text: mark.modelData.count >= 1000 ? (mark.modelData.count / 1000).toFixed(1) + "k" : mark.modelData.count
        color: mark.inViewport ? timeline.lightBlue : Color.muted
        horizontalAlignment: Text.AlignRight
        font.family: Style.font.family
        font.pixelSize: Typography.caption
        fontSizeMode: Text.HorizontalFit
        minimumPixelSize: Typography.caption * 0.8
      }

      Item {
        objectName: "timeline-hours-" + mark.modelData.key
        visible: timeline.hourly && !!mark.modelData.hours
        x: Style.space(43)
        width: parent.width - x
        height: Math.max(1, parent.height - Style.space(3))
        anchors.bottom: parent.bottom
        Repeater {
          model: timeline.hourly ? mark.modelData.hours || [] : []
          delegate: Rectangle {
            required property int index
            required property var modelData
            x: index * parent.width / 24
            width: Math.max(1, parent.width / 24 - 0.5)
            height: modelData.count > 0 ? Math.max(1, parent.height * modelData.count / Math.max(1, timeline.detail.maximum)) : 0
            anchors.bottom: parent.bottom
            color: mark.inViewport ? timeline.lightBlue : timeline.darkBlue
            Accessible.role: Accessible.Button
            Accessible.name: timeline.label(modelData, false) + ": " + modelData.count + " media"
          }
        }
      }
    }
  }

  Rectangle {
    objectName: "timeline-viewport"
    x: 0
    y: timeline.outlineTop
    width: timeline.width
    height: timeline.outlineHeight
    visible: timeline.showOutline
    color: "transparent"
    border.width: 1
    border.color: timeline.lightBlue
  }

  MouseArea {
    x: 0
    y: timeline.axisTop
    width: parent.width
    height: Math.min(timeline.axisHeight, timeline.detail.bins.length * timeline.rowHeight)
    cursorShape: Qt.PointingHandCursor
    onPressed: function(mouse) {
      timeline.forceActiveFocus()
      var y = mouse.y + timeline.axisTop
      var onOutline = !timeline.hourly && timeline.showOutline && y >= timeline.outlineTop && y <= timeline.outlineTop + timeline.outlineHeight
      timeline.dragOffset = onOutline ? y - timeline.outlineTop : 0
      if (!onOutline) timeline.seekAt(y, mouse.x)
    }
    onPositionChanged: function(mouse) { if (pressed) timeline.seekAt(mouse.y + timeline.axisTop - timeline.dragOffset, mouse.x) }
  }
}
