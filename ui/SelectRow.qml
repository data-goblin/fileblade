import QtQuick
import qs.Commons
import "../lib/SettingsForm.js" as Form
import "../theme"

Item {
  id: control

  property var row: ({})
  readonly property string current: Form.optionLabel(row, row.value)

  signal committed(var value)

  implicitHeight: Style.space(28)

  Text {
    textFormat: Text.PlainText
    anchors.left: parent.left
    anchors.right: valueBox.left
    anchors.rightMargin: Style.space(8)
    anchors.verticalCenter: parent.verticalCenter
    text: String(control.row.label || "")
    color: Color.bar.text
    elide: Text.ElideRight
    font.family: Style.font.family
    font.pixelSize: Typography.bodySmall
  }

  Rectangle {
    id: valueBox
    anchors.right: parent.right
    anchors.verticalCenter: parent.verticalCenter
    width: Math.min(valueLabel.implicitWidth + Style.space(30), Math.floor(control.width * 0.6))
    height: Style.space(22)
    radius: Math.min(Style.cornerRadius, Style.space(4))
    color: Util.alpha(Color.bar.text, valuePointer.containsMouse || menu.visible ? 0.12 : 0.06)

    Text {
      id: valueLabel
      textFormat: Text.PlainText
      anchors.left: parent.left
      anchors.right: chevron.left
      anchors.leftMargin: Style.space(8)
      anchors.rightMargin: Style.space(4)
      anchors.verticalCenter: parent.verticalCenter
      text: control.current
      color: Color.bar.text
      elide: Text.ElideRight
      font.family: Style.font.family
      font.pixelSize: Typography.caption
    }

    Text {
      id: chevron
      textFormat: Text.PlainText
      anchors.right: parent.right
      anchors.rightMargin: Style.space(6)
      anchors.verticalCenter: parent.verticalCenter
      text: "󰅀"
      color: Color.muted
      font.family: Style.font.family
      font.pixelSize: Typography.caption
    }

    MouseArea {
      id: valuePointer
      anchors.fill: parent
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: {
        menu.rows = Form.popupRows(control.row, control.row.value)
        menu.present()
      }
    }
  }

  OptionPopup {
    id: menu
    x: control.width - menuWidth
    y: control.height + Style.space(2)
    menuWidth: Math.min(Style.space(220), Math.max(Style.space(120), control.width))
    prompt: "Type to filter, Enter picks"
    onPicked: function(key) { control.committed(key) }
  }
}
