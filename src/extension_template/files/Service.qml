import QtQuick

Provider {
  id: service

  property var shell: null
  property var manifest: null
  property var pluginRegistry: null

  readonly property string pluginDir: decodeURIComponent(String(Qt.resolvedUrl(".")).replace(/^file:\/\//, "")).replace(/\/$/, "")

  providerId: manifest && manifest.id ? String(manifest.id) : ""
  providerRoot: pluginDir

  Loader {
    active: !!service.pluginRegistry && !!service.manifest && !!service.manifest.id
    source: "HostGuard.qml"
    onLoaded: {
      item.pluginId = Qt.binding(function() { return service.manifest ? String(service.manifest.id) : "" })
      item.sourceDir = service.pluginDir
    }
  }
}
