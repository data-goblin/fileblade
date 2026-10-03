import QtQuick

Item {
  id: provider
  visible: false

  property string providerId: ""
  property string providerRoot: ""
  property var files: null
  property url inventoryComponentUrl: ""
  property bool retired: false
  property var attached: []

  readonly property var observers: attached
  readonly property int viewCount: attached.length
  readonly property bool ready: !retired && files !== null
  readonly property string error: ""

  function attach(context) {
    if (retired || !context) return false
    if (!files && typeof context.service === "function") files = context.service("files")
    if (String(inventoryComponentUrl) === "" && context.ui && typeof context.ui.url === "function") inventoryComponentUrl = context.ui.url("ArtifactInventory")
    if (attached.indexOf(context) < 0) attached = attached.concat([context])
    return true
  }

  function detach(context) {
    attached = attached.filter(function(value) { return value !== context })
  }

  function shutdown() {
    retired = true
    attached = []
  }
}
