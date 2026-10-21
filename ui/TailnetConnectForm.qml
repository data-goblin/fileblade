import QtQuick
import QtQuick.Controls
import qs.Commons
import "../theme"
import qs.Commons as Commons

FocusScope {
  id: form
  required property var drives
  property string locationId: ""
  property string expectedHost: ""
  property bool opened: false
  property Item returnFocusItem: null
  anchors.fill: parent
  visible: opened
  focus: opened
  z: 10

  function close() {
    var target = returnFocusItem
    var restore = activeFocus && form.Window.window && form.Window.window.active
    opened = false
    returnFocusItem = null
    if (restore && target && target.visible && target.enabled) target.forceActiveFocus()
  }

  Keys.onEscapePressed: function(event) { close(); event.accepted = true }

  Rectangle {
    anchors.fill: parent
    color: Util.alpha(Commons.Color.background, 0.6)
    MouseArea { anchors.fill: parent; acceptedButtons: Qt.AllButtons }
  }

  Rectangle {
    id: card
    anchors.centerIn: parent
    width: Math.min(parent.width - Style.space(16), Style.space(340))
    height: body.implicitHeight + Style.space(24)
    color: Commons.Color.popups.background
    border.color: Commons.Color.popups.border
    radius: 0
  }

  function show(id, host, user, path) {
    if (!opened) returnFocusItem = form.Window.window ? form.Window.window.activeFocusItem : null
    locationId = id
    expectedHost = host
    userField.text = user
    pathField.text = path
    saveBox.checked = false
    opened = true
    focusInput()
  }

  function focusInput() { userField.forceActiveFocus() }

  function submit() {
    if (!userField.text || pathField.text.charAt(0) !== "/") return
    drives.connectPeer(locationId, expectedHost, userField.text, pathField.text, saveBox.checked)
    close()
  }

  component Field: TextField {
    width: body.width
    color: Commons.Color.bar.text
    selectByMouse: true
    font.family: Style.font.family
    font.pixelSize: Typography.bodySmall
    selectionColor: Util.alpha(Commons.Color.accent, 0.38)
    background: Rectangle { color: Util.alpha(Commons.Color.bar.text, 0.06); border.color: parent.activeFocus ? Commons.Color.accent : Commons.Color.popups.border; radius: 0 }
  }

  component Action: Button {
    contentItem: Text {
      text: parent.text
      textFormat: Text.PlainText
      color: parent.enabled ? Commons.Color.bar.text : Commons.Color.muted
      font.family: Style.font.family
      font.pixelSize: Typography.bodySmall
      horizontalAlignment: Text.AlignHCenter
      verticalAlignment: Text.AlignVCenter
    }
    background: Rectangle { color: parent.down ? Util.alpha(Commons.Color.accent, 0.2) : Commons.Color.popups.background; border.color: parent.activeFocus ? Commons.Color.accent : Commons.Color.popups.border; radius: 0 }
  }

  Column {
    id: body
    anchors.centerIn: card
    width: card.width - Style.space(24)
    spacing: Style.space(8)
    Text {
      text: "Connect to " + form.expectedHost
      textFormat: Text.PlainText
      width: parent.width
      wrapMode: Text.WrapAnywhere
      color: Commons.Color.bar.text
      font.family: Style.font.family
      font.pixelSize: Typography.bodySmall
    }
    Label { text: "SSH user"; color: Commons.Color.muted; font.family: Style.font.family }
    Field { id: userField; objectName: "tailnetUser"; maximumLength: 128; onAccepted: pathField.text.charAt(0) === "/" ? form.submit() : pathField.forceActiveFocus() }
    Label { text: "Remote folder"; color: Commons.Color.muted; font.family: Style.font.family }
    Field { id: pathField; objectName: "tailnetPath"; maximumLength: 4096; onAccepted: form.submit() }
    CheckBox {
      id: saveBox
      text: "Remember this location"
      font.family: Style.font.family
      contentItem: Text { text: saveBox.text; color: Commons.Color.bar.text; font: saveBox.font; leftPadding: Style.space(24); verticalAlignment: Text.AlignVCenter }
      indicator: Rectangle {
        x: 0; y: (saveBox.height - height) / 2
        width: Style.space(16); height: width; radius: 0
        color: saveBox.checked ? Commons.Color.accent : Commons.Color.popups.background
        border.color: saveBox.activeFocus ? Commons.Color.accent : Commons.Color.popups.border
      }
    }
    Row {
      spacing: Style.space(8)
      Action { text: "Cancel"; onClicked: form.close() }
      Action { text: "Connect"; objectName: "tailnetConnect"; enabled: userField.text !== "" && pathField.text.charAt(0) === "/"; onClicked: form.submit() }
    }
  }
}
