import QtQuick
import qs.Commons
import "../theme"

Text {
  required property string letter
  required property bool active
  required property point at

  textFormat: Text.PlainText
  visible: text !== ""
  x: at.x - width / 2
  y: at.y - height / 2
  text: letter.toUpperCase()
  color: active ? Color.popups.text : Color.muted
  opacity: active ? 0.9 : 0.7
  font.family: Style.font.family
  font.pixelSize: Typography.caption
  font.weight: Font.DemiBold
}
