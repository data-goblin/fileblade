import QtQuick
import qs.Commons
import "../theme"

Item {
  id: notice

  property string text: ""
  property int lifetimeMs: 8000
  property color foreground: Color.urgent

  signal dismissed()

  visible: text !== ""
  implicitHeight: text === "" ? 0 : Math.max(label.implicitHeight, close.implicitHeight)

  onTextChanged: {
    if (text === "") expiry.stop()
    else expiry.restart()
  }

  Timer {
    id: expiry
    interval: notice.lifetimeMs
    onTriggered: notice.dismissed()
  }

  MouseArea {
    anchors.fill: parent
    hoverEnabled: true
    acceptedButtons: Qt.NoButton
    onEntered: expiry.stop()
    onExited: if (notice.text !== "") expiry.restart()
  }

  Text {
    id: label
    textFormat: Text.PlainText
    anchors.left: parent.left
    anchors.right: close.left
    anchors.rightMargin: Style.space(6)
    anchors.top: parent.top
    text: notice.text
    color: notice.foreground
    wrapMode: Text.WordWrap
    font.family: Style.font.family
    font.pixelSize: Typography.bodySmall
  }

  Item {
    id: close
    anchors.right: parent.right
    anchors.top: parent.top
    width: Style.space(14)
    height: label.implicitHeight > 0 ? Math.min(label.implicitHeight, Style.space(16)) : Style.space(14)

    Text {
      textFormat: Text.PlainText
      anchors.centerIn: parent
      text: "󰅖"
      color: pointer.containsMouse ? Color.accent : Util.alpha(notice.foreground, 0.7)
      font.family: Style.font.family
      font.pixelSize: Typography.bodySmall
    }

    MouseArea {
      id: pointer
      anchors.fill: parent
      anchors.margins: -Style.space(3)
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: notice.dismissed()
    }
  }
}
