import QtQuick
import QtQuick.Controls
import qs.Commons
import "../theme"

FocusScope {
  id: root

  required property var controller
  required property var hostWindow

  implicitHeight: controller.pickerMode === "save" ? Style.space(112) : Style.space(78)

  function focusName() {
    if (controller.pickerMode === "save") {
      nameField.forceActiveFocus()
      nameField.selectAll()
    }
  }

  Keys.onEscapePressed: function(event) {
    controller.cancelPicker()
    event.accepted = true
  }

  component PickerButton: Button {
    id: control
    property bool primary: false
    property bool danger: false
    implicitWidth: Math.max(Style.space(70), label.implicitWidth + Style.space(20))
    implicitHeight: Style.space(30)
    opacity: enabled ? 1 : 0.58
    contentItem: Text {
      textFormat: Text.PlainText
      id: label
      text: control.text
      color: control.primary ? Color.background : Color.bar.text
      horizontalAlignment: Text.AlignHCenter
      verticalAlignment: Text.AlignVCenter
      font.family: Style.font.family
      font.pixelSize: Typography.bodySmall
      font.weight: control.primary ? Font.DemiBold : Font.Normal
    }
    background: Rectangle {
      radius: Math.min(Style.cornerRadius, Style.space(4))
      color: control.primary
        ? (control.hovered
          ? (control.danger ? Color.urgent : Color.accent)
          : Util.alpha(control.danger ? Color.urgent : Color.accent, 0.84))
        : (control.hovered || control.activeFocus ? Style.hoverFillFor(Color.bar.text, Color.accent) : Util.alpha(Color.bar.text, 0.06))
      border.width: control.primary ? 0 : 1
      border.color: control.activeFocus ? Color.accent : Util.alpha(Color.bar.text, 0.18)
    }
  }

  Rectangle {
    anchors.fill: parent
    color: Qt.lighter(Color.bar.background, 1.035)
  }

  Rectangle {
    anchors.top: parent.top
    anchors.left: parent.left
    anchors.right: parent.right
    height: 1
    color: Util.alpha(Color.bar.text, 0.22)
  }

  Column {
    anchors.fill: parent
    anchors.margins: Style.space(9)
    spacing: Style.space(6)

    Item {
      width: parent.width
      height: Style.space(25)

      Column {
        anchors.left: parent.left
        anchors.right: buttonRow.left
        anchors.rightMargin: Style.space(8)
        anchors.verticalCenter: parent.verticalCenter
        spacing: 0

        Text {
          textFormat: Text.PlainText
          width: parent.width
          text: controller.pickerTitle
          color: Color.bar.text
          elide: Text.ElideRight
          font.family: Style.font.family
          font.pixelSize: Typography.bodySmall
          font.weight: Font.DemiBold
        }

        Text {
          textFormat: Text.PlainText
          width: parent.width
          text: {
            if (controller.pickerSaveValidationBusy) return "Checking destination…"
            if (controller.pickerOverwriteArmed) return "A file with this name already exists"
            if (controller.operationError) return controller.operationError
            if (controller.pickerExtensions.length > 0) return "Types: ." + controller.pickerExtensions.join(", .")
            if (controller.pickerMode === "folder") return "Choose one or more folders"
            return controller.selectedCount + " selected"
          }
          color: controller.operationError || controller.pickerOverwriteArmed ? Color.urgent : Color.muted
          elide: Text.ElideRight
          font.family: Style.font.family
          font.pixelSize: Typography.caption
        }
      }

      Row {
        id: buttonRow
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        spacing: Style.space(5)

        PickerButton {
          text: "Cancel"
          onClicked: controller.cancelPicker()
        }

        PickerButton {
          text: controller.pickerMode === "save"
            ? (controller.pickerSaveValidationBusy ? "Checking…"
              : (controller.pickerOverwriteArmed ? "Replace" : "Save"))
            : "Open"
          primary: true
          danger: controller.pickerMode === "save" && controller.pickerOverwriteArmed
          enabled: !controller.pickerSaveValidationBusy
          onClicked: controller.confirmPicker()
        }
      }
    }

    TextField {
      id: nameField
      width: parent.width
      height: controller.pickerMode === "save" ? Style.space(34) : 0
      visible: controller.pickerMode === "save"
      leftPadding: Style.space(9)
      rightPadding: Style.space(9)
      selectByMouse: true
      text: controller.pickerFileName
      placeholderText: "File name"
      placeholderTextColor: Color.muted
      color: Color.bar.text
      selectionColor: Util.alpha(Color.accent, 0.38)
      selectedTextColor: Color.bar.text
      font.family: Style.font.family
      font.pixelSize: Typography.body
      background: Rectangle {
        radius: Math.min(Style.cornerRadius, Style.space(4))
        color: Util.alpha(Color.bar.text, nameField.activeFocus ? 0.10 : 0.06)
        border.width: 1
        border.color: nameField.activeFocus ? Color.accent : Util.alpha(Color.bar.text, 0.18)
      }
      onTextEdited: controller.pickerFileName = text
      onAccepted: controller.confirmPicker()
      Keys.onEscapePressed: function(event) {
        controller.cancelPicker()
        event.accepted = true
      }
    }
  }

  Connections {
    target: controller
    function onPickerActiveChanged() {
      if (controller.pickerActive && controller.pickerMode === "save") Qt.callLater(root.focusName)
    }
  }
}
