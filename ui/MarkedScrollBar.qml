import QtQuick
import qs.Commons
import "../lib/ScrollMarks.js" as ScrollMarks
import qs.Commons as Commons

Item {
  id: bar

  property var flickable: null
  property var marks: []
  readonly property real markHeight: Style.space(2)
  readonly property int slots: Math.max(1, Math.floor(height / Math.max(1, markHeight)))
  readonly property real origin: flickable ? Number(flickable.originY) || 0 : 0
  readonly property real extent: flickable ? Math.max(0, Number(flickable.contentHeight) - Number(flickable.height)) : 0
  readonly property bool scrollable: !!flickable && flickable.visible && flickable.height > 0 && extent > 1
  readonly property real thumbLength: scrollable
    ? Math.min(height, Math.max(Style.space(12), height * flickable.height / Math.max(1, flickable.contentHeight)))
    : 0
  readonly property real progress: scrollable ? Math.max(0, Math.min(1, (flickable.contentY - origin) / extent)) : 0
  readonly property real thumbY: Math.round(progress * (height - thumbLength))
  readonly property bool engaged: pointer.containsMouse || pointer.pressed
  readonly property var viewRange: flickable
    ? ScrollMarks.viewRange(flickable.contentY, origin, flickable.height, flickable.contentHeight)
    : ({ start: 0, end: 1 })
  readonly property real awayOpacity: 0.5

  width: Style.space(4)
  visible: scrollable

  function scrollToPointer(y) {
    if (!scrollable) return
    var span = Math.max(1, height - thumbLength)
    var fraction = Math.max(0, Math.min(1, (y - thumbLength / 2) / span))
    flickable.contentY = origin + fraction * extent
  }

  Repeater {
    model: bar.marks
    delegate: Rectangle {
      required property var modelData
      x: 0
      y: Math.round(modelData.fraction * (bar.height - height))
      width: bar.width - Style.space(1)
      height: bar.markHeight
      color: modelData.color
      opacity: ScrollMarks.inView(modelData.fraction, bar.viewRange) ? 1 : bar.awayOpacity
    }
  }

  Rectangle {
    id: thumb
    anchors.right: parent.right
    y: bar.thumbY
    width: bar.engaged ? Style.space(3) : Style.space(1)
    height: bar.thumbLength
    color: Commons.Color.accent
    opacity: bar.engaged ? 1 : 0.7
  }

  MouseArea {
    id: pointer
    anchors.fill: parent
    anchors.leftMargin: -Style.space(3)
    hoverEnabled: true
    cursorShape: Qt.PointingHandCursor
    onPressed: function(mouse) { bar.scrollToPointer(mouse.y) }
    onPositionChanged: function(mouse) { if (pressed) bar.scrollToPointer(mouse.y) }
  }
}
