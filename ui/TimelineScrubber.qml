import QtQuick
import qs.Commons
import "../lib/ImageGallery.js" as ImageGallery
import "../theme"
import qs.Commons as Commons

Item {
  id: rail

  property var sections: []
  property real contentHeight: 0
  property real viewportHeight: 0
  property real fraction: 0
  property bool showYears: true
  readonly property bool engaged: pointer.containsMouse || pointer.pressed
  readonly property real trackX: width - Style.space(9)
  readonly property real trackTop: Style.space(8)
  readonly property real trackHeight: Math.max(1, height - trackTop * 2)
  readonly property var years: ImageGallery.spacedMarks(ImageGallery.yearMarks(sections, contentHeight), trackHeight, Style.space(18))
  readonly property var months: ImageGallery.monthMarks(sections, contentHeight)
  readonly property real thumbLength: contentHeight > 0
    ? Math.max(Style.space(10), Math.min(trackHeight, trackHeight * viewportHeight / Math.max(1, contentHeight)))
    : 0
  readonly property real thumbY: trackTop + Math.max(0, Math.min(1, fraction)) * Math.max(0, trackHeight - thumbLength)
  readonly property real pointerFraction: Math.max(0, Math.min(1, (pointer.mouseY - trackTop) / trackHeight))
  readonly property var pointerSection: engaged ? ImageGallery.sectionAtFraction(sections, contentHeight, pointerFraction) : null
  readonly property var activeSection: ImageGallery.sectionAtFraction(sections, contentHeight, fraction * Math.max(0, contentHeight - viewportHeight) / Math.max(1, contentHeight))

  signal scrubbed(real fraction)
  signal scrubEnded()

  width: showYears ? Style.space(44) : Style.space(14)
  visible: sections.length > 0 && contentHeight > 0

  function scrubTo(y) {
    var span = Math.max(1, trackHeight - thumbLength)
    scrubbed(Math.max(0, Math.min(1, (y - trackTop - thumbLength / 2) / span)))
  }

  Rectangle {
    x: rail.trackX
    y: rail.trackTop
    width: 1
    height: rail.trackHeight
    color: Util.alpha(Commons.Color.bar.text, 0.16)
  }

  Repeater {
    model: rail.months

    delegate: Rectangle {
      required property var modelData
      x: rail.trackX - Style.space(1)
      y: Math.round(rail.trackTop + modelData.fraction * rail.trackHeight) - height / 2
      width: Style.space(3)
      height: width
      radius: width / 2
      visible: !modelData.undated
      color: Util.alpha(Commons.Color.bar.text, rail.engaged ? 0.6 : 0.34)
    }
  }

  Repeater {
    model: rail.showYears ? rail.years : []

    delegate: Text {
      required property var modelData
      textFormat: Text.PlainText
      x: 0
      width: rail.trackX - Style.space(6)
      y: Math.round(rail.trackTop + modelData.fraction * rail.trackHeight) - height / 2
      text: modelData.label
      color: rail.activeSection && rail.activeSection.year === Number(modelData.label) ? Commons.Color.accent : Commons.Color.muted
      horizontalAlignment: Text.AlignRight
      font.family: Style.font.family
      font.pixelSize: Typography.caption
    }
  }

  Rectangle {
    id: thumb
    x: rail.trackX - width + 1
    y: Math.round(rail.thumbY)
    width: rail.engaged ? Style.space(3) : Style.space(2)
    height: Math.round(rail.thumbLength)
    radius: width / 2
    color: Commons.Color.accent
    opacity: rail.engaged ? 1 : 0.75
  }

  Rectangle {
    id: pill
    visible: rail.engaged && !!rail.pointerSection
    x: rail.trackX - width - Style.space(6)
    y: Math.round(Math.max(0, Math.min(rail.height - height, pointer.mouseY - height / 2)))
    width: pillLabel.implicitWidth + Style.space(14)
    height: pillLabel.implicitHeight + Style.space(6)
    radius: Math.min(Style.cornerRadius, Style.space(4))
    color: Commons.Color.tooltip.background
    border.width: 1
    border.color: Commons.Color.tooltip.border

    Text {
      id: pillLabel
      textFormat: Text.PlainText
      anchors.centerIn: parent
      text: rail.pointerSection ? String(rail.pointerSection.shortLabel) : ""
      color: Commons.Color.tooltip.text
      font.family: Style.font.family
      font.pixelSize: Typography.bodySmall
    }
  }

  Rectangle {
    visible: pill.visible
    x: pill.x + pill.width
    y: Math.round(pointer.mouseY)
    width: rail.trackX - x + 1
    height: 1
    color: Commons.Color.accent
  }

  MouseArea {
    id: pointer
    anchors.fill: parent
    anchors.leftMargin: -Style.space(4)
    hoverEnabled: true
    cursorShape: Qt.PointingHandCursor
    onPressed: function(mouse) { rail.scrubTo(mouse.y) }
    onPositionChanged: function(mouse) { if (pressed) rail.scrubTo(mouse.y) }
    onReleased: rail.scrubEnded()
  }
}
