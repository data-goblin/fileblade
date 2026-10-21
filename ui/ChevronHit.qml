import QtQuick
import qs.Commons
import "../theme"
import qs.Commons as Commons

MouseArea {
  readonly property real glyphWidth: glyph.implicitWidth
  anchors.fill: parent
  hoverEnabled: true
  cursorShape: Qt.PointingHandCursor

  Text {
    id: glyph
    textFormat: Text.PlainText
    anchors.right: parent.right
    anchors.rightMargin: Style.space(6)
    anchors.verticalCenter: parent.verticalCenter
    text: "󰅀"
    color: Commons.Color.muted
    font.family: Style.font.family
    font.pixelSize: Typography.caption
  }
}
