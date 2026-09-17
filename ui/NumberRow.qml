import QtQuick
import qs.Commons
import "../lib/SettingsForm.js" as Form
import "../theme"

Item {
  id: control

  property var row: ({})
  readonly property string shown: Form.formatValue(row, row.value)
  readonly property string range: Form.rangeText(row)
  readonly property bool atMinimum: Number(row.value) <= Number(row.min)
  readonly property bool atMaximum: Number(row.value) >= Number(row.max)

  signal committed(var value)

  implicitHeight: Style.space(28)

  function step(direction) {
    var base = field.activeFocus ? Form.coerce(control.row, field.text.trim()) : undefined
    var next = Form.stepValue(control.row, base === undefined ? control.row.value : base, direction)
    field.text = Form.formatValue(control.row, next)
    control.committed(next)
  }

  function commitText() {
    var parsed = Form.coerce(control.row, field.text.trim())
    if (parsed === undefined || parsed === control.row.value) {
      field.text = control.shown
      return
    }
    control.committed(parsed)
  }

  function revert() {
    field.text = control.shown
    control.forceActiveFocus()
  }

  onShownChanged: if (!field.activeFocus) field.text = shown

  component StepButton: Item {
    id: button
    required property string glyph
    signal clicked()
    width: Style.space(20)
    height: Style.space(20)
    opacity: enabled ? 1 : 0.3

    Rectangle {
      anchors.fill: parent
      radius: Math.min(Style.cornerRadius, Style.space(4))
      color: buttonPointer.containsMouse && button.enabled ? Util.alpha(Color.bar.text, 0.12) : Util.alpha(Color.bar.text, 0.06)
    }

    Text {
      textFormat: Text.PlainText
      anchors.centerIn: parent
      text: button.glyph
      color: buttonPointer.containsMouse && button.enabled ? Color.bar.text : Color.muted
      font.family: Style.font.family
      font.pixelSize: Typography.bodySmall
    }

    MouseArea {
      id: buttonPointer
      anchors.fill: parent
      hoverEnabled: true
      enabled: button.enabled
      cursorShape: Qt.PointingHandCursor
      onClicked: button.clicked()
    }
  }

  Text {
    textFormat: Text.PlainText
    anchors.left: parent.left
    anchors.right: rangeText.visible ? rangeText.left : minus.left
    anchors.rightMargin: Style.space(8)
    anchors.verticalCenter: parent.verticalCenter
    text: (control.row.glyph ? control.row.glyph + "  " : "") + String(control.row.label || "")
    color: Color.bar.text
    elide: Text.ElideRight
    font.family: Style.font.family
    font.pixelSize: Typography.bodySmall
  }

  Text {
    id: rangeText
    textFormat: Text.PlainText
    anchors.right: minus.left
    anchors.rightMargin: Style.space(8)
    anchors.verticalCenter: parent.verticalCenter
    visible: control.range !== ""
    text: control.range
    color: Color.muted
    font.family: Style.font.family
    font.pixelSize: Typography.caption
  }

  StepButton {
    id: minus
    anchors.right: fieldBox.left
    anchors.rightMargin: Style.space(3)
    anchors.verticalCenter: parent.verticalCenter
    glyph: "−"
    enabled: !control.atMinimum
    onClicked: control.step(-1)
  }

  Rectangle {
    id: fieldBox
    anchors.right: plus.left
    anchors.rightMargin: Style.space(3)
    anchors.verticalCenter: parent.verticalCenter
    width: Style.space(58)
    height: Style.space(22)
    radius: Math.min(Style.cornerRadius, Style.space(4))
    color: Util.alpha(Color.bar.text, field.activeFocus ? 0.10 : 0.06)
    border.width: field.activeFocus ? 1 : 0
    border.color: Util.alpha(Color.accent, 0.6)

    TextInput {
      id: field
      anchors.fill: parent
      anchors.leftMargin: Style.space(6)
      anchors.rightMargin: Style.space(6)
      verticalAlignment: TextInput.AlignVCenter
      horizontalAlignment: TextInput.AlignRight
      text: control.shown
      maximumLength: Form.MAXIMUM_FIELD_LENGTH
      inputMethodHints: Qt.ImhFormattedNumbersOnly
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

  StepButton {
    id: plus
    anchors.right: parent.right
    anchors.verticalCenter: parent.verticalCenter
    glyph: "+"
    enabled: !control.atMaximum
    onClicked: control.step(1)
  }
}
