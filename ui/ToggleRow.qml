import QtQuick
import qs.Commons
import "../theme"

Item {
  id: row

  property string label: ""
  property string glyph: ""
  property string detail: ""
  property bool checked: false
  property bool heading: false

  signal toggled()

  implicitHeight: Style.space(28)

  Text {
    textFormat: Text.PlainText
    anchors.left: parent.left
    anchors.verticalCenter: parent.verticalCenter
    anchors.right: detailText.visible ? detailText.left : toggle.left
    anchors.rightMargin: Style.space(8)
    text: (row.glyph !== "" ? row.glyph + "  " : "") + row.label
    color: row.heading ? Color.muted : Color.bar.text
    elide: Text.ElideRight
    font.family: Style.font.family
    font.pixelSize: row.heading ? Typography.caption : Typography.bodySmall
    font.letterSpacing: row.heading ? 0.4 : 0
  }

  Text {
    id: detailText
    textFormat: Text.PlainText
    anchors.right: toggle.left
    anchors.rightMargin: Style.space(8)
    anchors.verticalCenter: parent.verticalCenter
    visible: row.detail !== ""
    text: row.detail
    color: Color.muted
    font.family: Style.font.family
    font.pixelSize: Typography.caption
  }

  Rectangle {
    id: toggle
    anchors.right: parent.right
    anchors.verticalCenter: parent.verticalCenter
    width: Style.space(30)
    height: Style.space(16)
    radius: height / 2
    color: row.checked ? Util.alpha(Color.accent, 0.78) : Util.alpha(Color.bar.text, 0.18)

    Rectangle {
      anchors.verticalCenter: parent.verticalCenter
      x: row.checked ? parent.width - width - Style.space(2) : Style.space(2)
      width: Style.space(12)
      height: width
      radius: width / 2
      color: row.checked ? Color.background : Color.bar.text
    }
  }

  MouseArea {
    anchors.fill: parent
    cursorShape: Qt.PointingHandCursor
    onClicked: row.toggled()
  }
}
