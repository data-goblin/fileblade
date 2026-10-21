import QtQuick
import qs.Commons
import "../../../theme"
import qs.Commons as Commons

Text {
  textFormat: Text.PlainText
  color: Commons.Color.bar.text
  font.family: Style.font.family
  font.pixelSize: Typography.body
  elide: Text.ElideRight
}
