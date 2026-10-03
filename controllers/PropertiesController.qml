import QtQuick
import "../lib/PropertiesSubject.js" as PropertiesSubject

Item {
  id: controller

  property var selection: null
  property QtObject owner: null
  property string ownerModuleId: ""
  property string ownerName: ""
  property var subject: null
  property string selectionKey: ""
  readonly property int version: PropertiesSubject.VERSION
  readonly property var limits: PropertiesSubject.LIMITS
  readonly property var kinds: PropertiesSubject.KINDS
  readonly property bool active: subject !== null && owner !== null && owner.retired !== true

  signal actionTriggered(string ownerModuleId, string actionId)

  visible: false

  onOwnerChanged: if (owner === null && subject !== null) clear()

  function keyOf(paths) {
    return Array.isArray(paths) ? paths.join("\n") : ""
  }

  function inspect(context, value) {
    if (!context || typeof context !== "object" || context.retired === true) return false
    var id = String(context.moduleId || "")
    if (!id) return false
    var normalized = PropertiesSubject.normalize(value)
    if (!normalized) return false
    try {
      owner = context
    } catch (error) {
      return false
    }
    if (owner !== context) return false
    var definition = context.definition
    ownerModuleId = id
    ownerName = definition && definition.name ? String(definition.name).slice(0, 64) : id
    selectionKey = keyOf(selection ? selection.selectedPaths : [])
    subject = normalized
    return true
  }

  function release(context) {
    if (!context || owner === null || context !== owner) return false
    clear()
    return true
  }

  function clear() {
    subject = null
    owner = null
    ownerModuleId = ""
    ownerName = ""
    selectionKey = ""
  }

  function trigger(actionId) {
    var id = String(actionId || "")
    if (!active || !PropertiesSubject.hasAction(subject, id)) return false
    actionTriggered(ownerModuleId, id)
    return true
  }

  Connections {
    target: controller.owner
    ignoreUnknownSignals: true

    function onRetiredChanged() {
      if (controller.owner !== null && controller.owner.retired === true) controller.clear()
    }
  }

  Connections {
    target: controller.selection
    ignoreUnknownSignals: true

    function onSelectedPathsChanged() {
      if (controller.subject !== null && controller.keyOf(controller.selection.selectedPaths) !== controller.selectionKey) controller.clear()
    }

    function onChosen() {
      if (controller.subject !== null) controller.clear()
    }
  }
}
