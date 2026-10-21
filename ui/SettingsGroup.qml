import QtQuick
import qs.Commons
import "../theme"
import qs.Commons as Commons

Text {
  id: group

  property string title: ""
  readonly property bool settingsGroup: true

  textFormat: Text.PlainText
  width: parent ? parent.width : 0
  topPadding: Style.space(5)
  text: title.toUpperCase()
  color: Commons.Color.muted
  elide: Text.ElideRight
  font.family: Style.font.family
  font.pixelSize: Typography.caption
  font.letterSpacing: 0.4
}
