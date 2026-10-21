import QtQuick
import QtQuick.Controls
import qs.Commons
import qs.Commons as Commons

ScrollBar {
  id: control

  policy: ScrollBar.AsNeeded
  interactive: false
  padding: 0
  implicitWidth: 1
  implicitHeight: 1

  background: null

  contentItem: Rectangle {
    implicitWidth: 1
    implicitHeight: 1
    visible: control.policy === ScrollBar.AlwaysOn || (control.policy === ScrollBar.AsNeeded && control.size < 0.995)
    color: Commons.Color.accent
  }
}
