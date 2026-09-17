import QtQuick
import Quickshell
import Quickshell.Wayland
import qs.Commons
import "../theme"

PanelWindow {
  id: overlay

  required property var host

  property bool active: false

  function refreshActive() {
    active = host.dragActive && host.dragScope === "screen" && host.dragScreen === screen
  }

  Connections {
    target: overlay.host
    function onDragActiveChanged() { overlay.refreshActive() }
    function onDragScreenChanged() { overlay.refreshActive() }
    function onDragScopeChanged() { overlay.refreshActive() }
  }

  Component.onCompleted: refreshActive()
  readonly property int screenWidth: screen ? screen.width : 0
  readonly property int screenHeight: screen ? screen.height : 0

  visible: active
  color: "transparent"
  surfaceFormat.opaque: false
  exclusionMode: ExclusionMode.Ignore
  mask: Region { }

  anchors {
    top: true
    bottom: true
    left: true
    right: true
  }

  WlrLayershell.namespace: "omarchy-fileblade-drag"
  WlrLayershell.layer: WlrLayer.Overlay
  WlrLayershell.keyboardFocus: WlrKeyboardFocus.None

  Rectangle {
    id: card
    x: Math.max(0, Math.min(overlay.screenWidth - width, overlay.host.dragScreenX + Style.space(16)))
    y: Math.max(0, Math.min(overlay.screenHeight - height, overlay.host.dragScreenY - height / 2))
    width: content.implicitWidth + Style.space(24)
    height: content.implicitHeight + Style.space(16)
    radius: Style.cornerRadius
    color: Color.popups.background
    border.width: 1
    border.color: overlay.host.dropEdge !== "" && !overlay.host.dropNoop ? Color.accent : Color.popups.border

    Column {
      id: content
      anchors.centerIn: parent
      spacing: Style.space(3)

      Row {
        spacing: Style.space(7)

        Text {
          textFormat: Text.PlainText
          text: overlay.host.dragGlyph
          color: Color.accent
          font.family: Style.font.family
          font.pixelSize: Typography.body
        }

        Text {
          textFormat: Text.PlainText
          text: overlay.host.dragTitle
          color: Color.bar.text
          font.family: Style.font.family
          font.pixelSize: Typography.bodySmall
          font.weight: Font.DemiBold
          font.letterSpacing: 0.4
        }
      }

      Text {
        textFormat: Text.PlainText
        text: overlay.host.dropLabel
        color: overlay.host.dropEdge !== "" && !overlay.host.dropNoop ? Color.accent : Color.muted
        font.family: Style.font.family
        font.pixelSize: Typography.caption
      }
    }
  }
}
