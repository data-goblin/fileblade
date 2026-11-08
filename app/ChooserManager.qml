import QtQuick
import Quickshell
import qs.Commons

Item {
  id: manager
  required property var service
  property bool transportEnabled: Quickshell.env("FILEBLADE_CHOOSER") !== "0"
  property var sessions: Object.create(null)
  property int revision: 0
  property int generation: 0
  property string watchId: ""
  property string error: ""
  property int failures: 0

  property var sessionComponent: null

  function present(offers) {
    if (offers.length && !sessionComponent) {
      sessionComponent = Qt.createComponent(Util.fileUrl(service.pluginDir + "/app/chooser/ChooserSession.qml"))
      if (sessionComponent.status !== Component.Ready) { error = sessionComponent.errorString(); return }
    }
    var retained = Object.create(null)
    for (var offer of offers) {
      retained[offer.handle] = true
      if (!sessions[offer.handle]) sessions[offer.handle] = sessionComponent.createObject(manager, { manager: manager, offer: offer })
    }
    for (var handle of Object.keys(sessions)) {
      if (retained[handle]) continue
      sessions[handle].close()
      sessions[handle].destroy()
      delete sessions[handle]
    }
  }

  function watch() {
    if (!transportEnabled || !service.backendReady || watchId) return
    var current = ++generation
    watchId = service.backendRequest("chooser", ["watch", "--revision", String(revision)], current, function(response) {
      if (current !== manager.generation) return
      manager.watchId = ""
      if (!response || !response.ok) {
        manager.error = String(response && response.error || "Chooser authority unavailable")
        manager.failures++
        retry.interval = Math.min(30000, 1000 * Math.pow(2, manager.failures - 1))
        retry.restart()
        return
      }
      manager.failures = 0
      manager.error = ""
      manager.revision = response.revision
      manager.present(response.offers)
      Qt.callLater(manager.watch)
    }, null, 20000, { interactive: true })
  }

  function stop() {
    generation++
    if (watchId) service.cancelBackendRequest(watchId, generation - 1, true)
    watchId = ""
    retry.stop()
    for (var handle of Object.keys(sessions)) sessions[handle].cancel()
    present([])
  }

  onTransportEnabledChanged: if (transportEnabled) watch(); else stop()
  Connections {
    target: manager.service
    function onBackendReadyChanged() { if (manager.service.backendReady) manager.watch(); else manager.stop() }
  }
  Timer { id: retry; interval: 1000; onTriggered: manager.watch() }
  Component.onCompleted: watch()
  Component.onDestruction: stop()
}
