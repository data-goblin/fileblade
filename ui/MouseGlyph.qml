import QtQuick
import qs.Commons
import qs.Commons as Commons

Item {
  id: glyph

  property string button: "left"
  property color color: Commons.Color.tooltip.text
  readonly property real unit: Style.space(1)

  implicitWidth: unit * 8
  implicitHeight: unit * 11

  Rectangle {
    anchors.fill: parent
    radius: width / 2
    color: "transparent"
    border.width: 1
    border.color: Util.alpha(glyph.color, 0.55)
  }

  Rectangle {
    visible: glyph.button === "left" || glyph.button === "right"
    x: glyph.button === "left" ? glyph.unit : Math.round(glyph.width / 2)
    y: glyph.unit
    width: Math.round(glyph.width / 2) - glyph.unit
    height: Math.round(glyph.height / 2) - glyph.unit
    topLeftRadius: glyph.button === "left" ? width : 0
    topRightRadius: glyph.button === "right" ? width : 0
    color: glyph.color
  }

  Rectangle {
    visible: glyph.button === "middle"
    anchors.horizontalCenter: parent.horizontalCenter
    y: glyph.unit * 2
    width: glyph.unit * 2
    height: glyph.unit * 3
    radius: width / 2
    color: glyph.color
  }
}
