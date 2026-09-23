import QtQuick
import "../ui" as PluginUi

Item {
  id: root

  required property var controller
  PluginUi.ActionKeyGuard { id: actionKeys; active: root.controller.dragActive }

  function handleRelease(event) {
    actionKeys.release(event)
    if (event.key === controller.modifierKey && !event.isAutoRepeat) controller.modifierHeld = false
    if (!controller.dragActive || !controller.wheelOpen || !controller.wheelFromDrag
        || event.key !== controller.modifierKey) return false
    if (!event.isAutoRepeat) releaseTimer.restart()
    return true
  }

  function handlePress(event) {
    if (!controller.dragActive) return false
    if (event.key === Qt.Key_Escape) {
      controller.cancelDrag()
      return true
    }
    var repeated = actionKeys.isRepeat(event)
    if (event.key === controller.modifierKey) {
      if (!repeated) controller.modifierHeld = true
      releaseTimer.stop()
      return repeated || controller.wheelOpen || controller.modifierPressed()
    }
    if (!controller.wheelOpen || !controller.wheelFromDrag) return false
    if (repeated) return true
    return controller.activateKey(event.text)
  }

  function cancelRelease() {
    releaseTimer.stop()
  }

  Timer {
    id: releaseTimer
    interval: 120
    onTriggered: {
      if (root.controller.dragActive && root.controller.wheelOpen && root.controller.wheelFromDrag)
        root.controller.close()
    }
  }
}
