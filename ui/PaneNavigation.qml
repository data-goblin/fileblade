import QtQuick
import qs.Commons
import "../theme"

Item {
  id: navigation

  property var actions: []
  property real maximumWidth: -1
  signal triggered(string key)

  readonly property int buttonWidth: Style.space(28)
  readonly property int buttonSpacing: Style.space(2)
  readonly property real naturalWidth: actions.length > 0
    ? actions.length * buttonWidth + (actions.length - 1) * buttonSpacing
    : 0
  readonly property int visibleActionCount: maximumWidth < 0
    ? actions.length
    : Math.max(0, Math.min(actions.length,
        Math.floor((maximumWidth + buttonSpacing) / (buttonWidth + buttonSpacing))))
  implicitWidth: buttons.implicitWidth
  implicitHeight: Style.space(24)
  visible: actions.length > 0

  Row {
    id: buttons
    anchors.fill: parent
    spacing: Style.space(2)

    Repeater {
      model: navigation.actions.length

      delegate: Rectangle {
        id: button
        required property int index
        readonly property var spec: index < navigation.actions.length
          ? navigation.actions[index]
          : ({})

        width: navigation.buttonWidth
        height: buttons.height
        visible: index < navigation.visibleActionCount
        enabled: spec.enabled !== false
        opacity: enabled ? 1 : 0.32
        radius: Math.min(Style.cornerRadius, Style.space(4))
        color: spec.active === true || pointer.containsMouse ? Color.menu.selectedBackground : "transparent"

        Text {
          textFormat: Text.PlainText
          anchors.centerIn: parent
          text: String(button.spec.glyph || "")
          color: button.spec.active === true ? Color.accent : Color.bar.text
          font.family: Style.font.family
          font.pixelSize: Typography.body
        }

        MouseArea {
          id: pointer
          anchors.fill: parent
          enabled: button.enabled
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onClicked: navigation.triggered(String(button.spec.key || ""))
        }

        HintTip {
          visible: String(button.spec.title || "") !== "" && pointer.containsMouse
          title: String(button.spec.title || "")
          actions: Array.isArray(button.spec.actions) ? button.spec.actions : []
          context: Array.isArray(button.spec.context) ? button.spec.context : []
        }
      }
    }
  }
}
