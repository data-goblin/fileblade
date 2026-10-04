import QtQuick
import Quickshell
import Quickshell.Wayland

PanelWindow {
  id: reservation

  required property string edge
  required property bool beforeBar
  required property int zone

  readonly property bool isRight: edge === "right"

  visible: zone > 0
  implicitWidth: 1
  color: "transparent"
  surfaceFormat.opaque: false
  mask: Region { }
  exclusionMode: ExclusionMode.Normal
  exclusiveZone: zone

  anchors {
    top: true
    bottom: true
    left: !reservation.isRight
    right: reservation.isRight
  }

  WlrLayershell.namespace: "omarchy-fileblade-" + edge + "-reserve"
  WlrLayershell.layer: beforeBar ? WlrLayer.Bottom : WlrLayer.Overlay
  WlrLayershell.keyboardFocus: WlrKeyboardFocus.None
}
