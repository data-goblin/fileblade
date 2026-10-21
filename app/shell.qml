import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import qs.Commons as Commons

ShellRoot {
  id: root

  readonly property var loadedService: service.item
  property bool activationPending: Quickshell.env("FILEBLADE_ACTIVATE") === "1"
  property string openPending: Quickshell.env("FILEBLADE_OPEN")
  onLoadedServiceChanged: Style.service = loadedService
  readonly property string sourceDir: Quickshell.env("FILEBLADE_SOURCE_DIR")
  property var shellConfig: ({})
  readonly property var barConfig: shellConfig.bar || ({ position: "top" })
  readonly property var bar: BarVisibility {
    service: root.loadedService
    barConfig: root.barConfig
  }

  FileView {
    path: Quickshell.env("HOME") + "/.config/omarchy/shell.json"
    watchChanges: true
    printErrors: false
    onFileChanged: reload()
    onLoaded: {
      try { root.shellConfig = JSON.parse(text()) }
      catch (error) { console.error("FileBlade: invalid shell configuration: " + error) }
    }
  }

  FileView {
    path: root.sourceDir + "/manifest.json"
    printErrors: false
    onLoadFailed: {
      console.error("FileBlade: native runtime manifest is unavailable: " + path)
      Qt.quit()
    }
    onLoaded: {
      try {
        var manifest = JSON.parse(text())
        manifest.__sourceDir = root.sourceDir
        service.setSource(Util.fileUrl(root.sourceDir + "/Service.qml"), {
          shell: root,
          manifest: manifest,
          pluginRegistry: null
        })
      } catch (error) {
        console.error("FileBlade: invalid native runtime manifest: " + error)
        Qt.quit()
      }
    }
  }

  Loader {
    id: service
    onStatusChanged: {
      if (status === Loader.Error) {
        console.error("FileBlade: native browser could not load")
        Qt.quit()
      }
    }
  }

  Connections {
    target: root.loadedService
    function onStateReadyChanged() { root.activateWhenReady() }
  }

  Connections {
    target: root.loadedService ? root.loadedService.bladeHost : null
    function onLayoutReadyChanged() { root.activateWhenReady() }
  }

  function activateWhenReady() {
    if (!loadedService || !loadedService.stateReady || !loadedService.bladeHost.layoutReady) return
    if (activationPending) {
      activationPending = false
      Qt.callLater(function() { root.loadedService.setOpen(true) })
    }
    if (openPending) {
      var path = openPending
      openPending = ""
      Qt.callLater(function() { opener.open(path, Quickshell.env("FILEBLADE_SELECT"), Quickshell.env("FILEBLADE_PROPERTIES") === "1") })
    }
  }

  Drain { id: drain; shell: root; service: root.loadedService }
  OpenRequest { id: opener; service: root.loadedService }

  Loader {
    active: service.status === Loader.Ready && Quickshell.env("FILEBLADE_QUALIFICATION") === "1"
    sourceComponent: Component { Qualification { service: root.loadedService } }
  }

  Loader {
    active: service.status === Loader.Ready
    sourceComponent: Component {
      ChooserManager {
        id: chooser
        service: root.loadedService
        Loader {
          active: Quickshell.env("FILEBLADE_QUALIFICATION") === "1"
          sourceComponent: Component { ChooserProbe { manager: chooser } }
        }
      }
    }
  }

  IpcHandler {
    target: "fileblade.native"
    function drainBegin(token: string, timeoutMs: string): string { return drain.begin(token, Number(timeoutMs)) }
    function drainStatus(token: string): string { return drain.status(token) }
    function drainCommit(token: string): string { return drain.commit(token) }
    function drainAbort(token: string): string { return drain.abort(token) }
    function open(path: string, select: string): string { return opener.open(path, select, false) }
    function openProperties(path: string, select: string): string { return opener.open(path, select, true) }
    function status(): string {
      return JSON.stringify({
        loaded: service.status === Loader.Ready,
        sourceDir: root.sourceDir,
        theme: Commons.Color.currentThemePath,
        background: String(Commons.Color.background),
        foreground: String(Commons.Color.foreground),
        accent: String(Commons.Color.accent),
        barSize: root.bar.barSize,
        barPosition: root.barConfig.position,
        fontSize: Style.font.body,
        rounding: Style.cornerRadius,
        gapsOut: Style.gapsOut,
        resolvedFontFamily: Style.resolvedFontFamily
      })
    }
  }
}
