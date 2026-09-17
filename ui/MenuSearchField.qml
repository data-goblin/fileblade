import QtQuick
import QtQuick.Controls as QQC
import qs.Commons
import "../theme"

QQC.TextField {
  id: field

  property string prompt: "Type to filter, Enter picks"
  property color textColor: Color.popups.text
  property var actionKeys: null

  signal moved(int delta)
  signal picked()
  signal dismissed()
  signal edgeRequested(bool last)

  leftPadding: Style.space(24)
  rightPadding: Style.space(8)
  placeholderText: prompt
  placeholderTextColor: Color.muted
  color: textColor
  selectByMouse: true
  selectionColor: Util.alpha(Color.accent, 0.38)
  selectedTextColor: textColor
  font.family: Style.font.family
  font.pixelSize: Typography.bodySmall

  background: Rectangle {
    radius: Math.min(Style.cornerRadius, Style.space(4))
    color: Util.alpha(field.textColor, field.activeFocus ? 0.10 : 0.06)
  }

  Keys.priority: Keys.BeforeItem
  Keys.onPressed: function(event) {
    if (actionKeys && (event.key === Qt.Key_Escape || event.key === Qt.Key_Return || event.key === Qt.Key_Enter)
        && actionKeys.isRepeat(event)) { event.accepted = true; return }
    var moves = {}
    moves[Qt.Key_Down] = 1
    moves[Qt.Key_Up] = -1
    if (event.key === Qt.Key_Escape) field.dismissed()
    else if (moves[event.key] !== undefined) field.moved(moves[event.key])
    else if (event.key === Qt.Key_Home || event.key === Qt.Key_PageUp) field.edgeRequested(false)
    else if (event.key === Qt.Key_End || event.key === Qt.Key_PageDown) field.edgeRequested(true)
    else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) field.picked()
    else return
    event.accepted = true
  }
  Keys.onReleased: function(event) { if (actionKeys) actionKeys.release(event) }

  Text {
    textFormat: Text.PlainText
    anchors.left: parent.left
    anchors.leftMargin: Style.space(7)
    anchors.verticalCenter: parent.verticalCenter
    text: "󰍉"
    color: field.activeFocus ? Color.accent : Color.muted
    font.family: Style.font.family
    font.pixelSize: Typography.bodySmall
  }
}
