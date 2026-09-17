import QtQuick
import qs.Commons
import qs.Ui
import "../lib/SettingsForm.js" as Form
import "../theme"

Item {
  id: control

  property var row: ({})
  property string suggestion: ""
  readonly property bool pathRow: row.type === "path"
  readonly property string current: String(row.value === undefined || row.value === null ? "" : row.value)
  readonly property int limit: Math.max(1, Number(row.maxLength) || Form.MAXIMUM_FIELD_LENGTH)

  signal committed(var value)

  implicitHeight: Style.space(28)

  function commitText() {
    var parsed = Form.coerce(control.row, field.text)
    if (parsed === undefined || parsed === control.current) {
      field.text = control.current
      return
    }
    control.committed(parsed)
  }

  function revert() {
    field.text = control.current
    control.forceActiveFocus()
  }

  function useSuggestion() {
    if (control.suggestion === "") return
    field.text = control.suggestion
    control.commitText()
  }

  onCurrentChanged: if (!field.activeFocus) field.text = current

  Text {
    id: title
    textFormat: Text.PlainText
    anchors.left: parent.left
    anchors.verticalCenter: parent.verticalCenter
    width: Math.min(implicitWidth, Math.floor(parent.width * 0.4))
    text: String(control.row.label || "")
    color: Color.bar.text
    elide: Text.ElideRight
    font.family: Style.font.family
    font.pixelSize: Typography.bodySmall
  }

  Rectangle {
    id: fieldBox
    anchors.left: title.right
    anchors.leftMargin: Style.space(10)
    anchors.right: control.pathRow ? useButton.left : parent.right
    anchors.rightMargin: control.pathRow ? Style.space(4) : 0
    anchors.verticalCenter: parent.verticalCenter
    height: Style.space(22)
    radius: Math.min(Style.cornerRadius, Style.space(4))
    color: Util.alpha(Color.bar.text, field.activeFocus ? 0.10 : 0.06)
    border.width: field.activeFocus ? 1 : 0
    border.color: Util.alpha(Color.accent, 0.6)

    Text {
      textFormat: Text.PlainText
      anchors.fill: field
      verticalAlignment: Text.AlignVCenter
      visible: field.text === "" && !field.activeFocus
      text: String(control.row.placeholder || "")
      color: Color.muted
      elide: Text.ElideRight
      font.family: Style.font.family
      font.pixelSize: Typography.caption
    }

    TextInput {
      id: field
      anchors.fill: parent
      anchors.leftMargin: Style.space(6)
      anchors.rightMargin: Style.space(6)
      verticalAlignment: TextInput.AlignVCenter
      text: control.current
      maximumLength: control.limit
      color: Color.bar.text
      selectionColor: Util.alpha(Color.accent, 0.38)
      selectedTextColor: Color.bar.text
      selectByMouse: true
      clip: true
      font.family: Style.font.family
      font.pixelSize: Typography.caption
      onAccepted: control.commitText()
      onActiveFocusChanged: if (!activeFocus) control.commitText()
      Keys.onEscapePressed: function(event) {
        control.revert()
        event.accepted = true
      }
    }
  }

  Item {
    id: useButton
    anchors.right: parent.right
    anchors.verticalCenter: parent.verticalCenter
    width: Style.space(22)
    height: Style.space(22)
    visible: control.pathRow
    enabled: control.suggestion !== ""
    opacity: enabled ? 1 : 0.3

    Text {
      textFormat: Text.PlainText
      anchors.centerIn: parent
      text: "󰈔"
      color: usePointer.containsMouse && useButton.enabled ? Color.accent : Color.muted
      font.family: Style.font.family
      font.pixelSize: Typography.bodySmall
    }

    MouseArea {
      id: usePointer
      anchors.fill: parent
      hoverEnabled: true
      enabled: useButton.enabled
      cursorShape: Qt.PointingHandCursor
      onClicked: control.useSuggestion()
    }

    HintTip {
      visible: usePointer.containsMouse
      title: useButton.enabled ? "Use the selected path" : "Select a file or folder first"
      actions: useButton.enabled ? [{ button: "left", text: "Use" }] : []
    }
  }
}
