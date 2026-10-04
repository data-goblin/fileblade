import QtQuick
import Quickshell
import qs.Commons

Item {
  id: root
  required property var service
  required property var barConfig
  property bool barHidden: false
  readonly property string position: ["top", "bottom", "left", "right"].indexOf(String(barConfig.position)) >= 0 ? String(barConfig.position) : "top"
  readonly property int barSize: barConfig.position === "left" || barConfig.position === "right"
    ? Style.bar.sizeVertical : Style.bar.sizeHorizontal
  readonly property string toggles: Quickshell.env("HOME") + "/.local/state/omarchy/toggles"
  property string watchId: ""
  property int generation: 0
  property bool probeAgain: false
  property bool probeRunning: false

  function refresh() {
    if (!service || !service.backendReady) return
    if (probeRunning) { probeAgain = true; return }
    probeRunning = true
    var current = generation
    service.backendRequest("native-bar-state", [], current, function(response) {
      root.probeRunning = false
      if (current === root.generation && response.ok) root.barHidden = response.hidden
      if (root.probeAgain || current !== root.generation) {
        root.probeAgain = false
        root.refresh()
      }
    })
  }

  function subscribe() {
    if (!service || !service.backendReady) return
    var previous = generation++
    if (watchId) service.cancelBackendRequest(watchId, previous, true)
    var current = generation
    watchId = service.backendSubscribe([toggles], current,
      function(event) { if (current === root.generation) root.refresh() },
      function(response) { if (current === root.generation) root.refresh() },
      function(response) {
        if (current !== root.generation) return
        root.watchId = ""
        retry.restart()
      })
  }

  onServiceChanged: Qt.callLater(subscribe)
  Connections {
    target: root.service
    function onBackendReadyChanged() { if (root.service.backendReady) root.subscribe() }
  }
  Timer { id: retry; interval: 1000; onTriggered: root.subscribe() }
  Component.onDestruction: {
    if (service && watchId) service.cancelBackendRequest(watchId, generation, true)
  }
}
