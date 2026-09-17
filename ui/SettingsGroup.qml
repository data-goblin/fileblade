import QtQuick
import qs.Commons
import "../theme"

Text {
  id: group

  property string title: ""
  readonly property bool settingsGroup: true

  textFormat: Text.PlainText
  width: parent ? parent.width : 0
  topPadding: Style.space(5)
  text: title.toUpperCase()
  color: Color.muted
  elide: Text.ElideRight
  font.family: Style.font.family
  font.pixelSize: Typography.caption
  font.letterSpacing: 0.4
}
