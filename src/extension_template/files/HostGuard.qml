import QtQuick
import QtQuick.Effects
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import qs.Commons
import "HostGuard.js" as HostGuard

Item {
  id: guard

  property string pluginId: ""
  property bool dismissed: false
  property bool installing: false

  property string sourceDir: ""
  property var observation: null
  property int retryDelay: 1000

  readonly property var plan: HostGuard.plan(observation, pluginId)
  readonly property bool showing: !dismissed && !!plan.show

  function refresh() {
    if (dismissed || worker.running || !pluginId) return
    worker.command = ["sh", "-c", HostGuard.STATUS_SCRIPT, "fileblade-host-status", pluginId]
    worker.running = true
    deadline.restart()
  }

  function accept(text, exitCode) {
    deadline.stop()
    var next = { schemaVersion: 1, state: "unknown", plugins: observation ? observation.plugins : [] }
    if (exitCode === HostGuard.HOST_ABSENT) {
      next = { schemaVersion: 1, state: "missing", plugins: [{ id: pluginId, name: pluginId, enabled: true }] }
    } else {
      try {
        var parsed = JSON.parse(text)
        if (exitCode === 0 && parsed && parsed.schemaVersion === 1) next = parsed
      } catch (error) {}
    }
    observation = next
    retryDelay = next.state === "ready" ? 30000 : Math.min(retryDelay * 2, 30000)
    retry.restart()
  }

  onPluginIdChanged: Qt.callLater(refresh)
  onSourceDirChanged: Qt.callLater(refresh)
  Component.onCompleted: Qt.callLater(refresh)
  Component.onDestruction: if (worker.running) worker.signal(15)
  onDismissedChanged: {
    if (dismissed) {
      retry.stop()
      if (worker.running) worker.signal(15)
    }
  }

  Process {
    id: worker
    stdout: StdioCollector { id: output; waitForEnd: true }
    onExited: function(exitCode) { guard.accept(output.text, exitCode) }
  }
  Timer { id: deadline; interval: 5000; onTriggered: worker.signal(15) }
  Timer { id: retry; interval: guard.retryDelay; onTriggered: guard.refresh() }

  function install() {
    if (installing || plan.command.length === 0) return
    installing = true
    Quickshell.execDetached(plan.command)
    enableDeadline.restart()
  }

  Timer { id: enableDeadline; interval: 22000; onTriggered: { guard.installing = false; guard.refresh() } }

  Loader {
    active: guard.showing
    sourceComponent: PanelWindow {
      anchors { top: true; bottom: true; left: true; right: true }
      color: "transparent"
      exclusionMode: ExclusionMode.Ignore
      mask: Region { item: card }
      WlrLayershell.namespace: "fileblade-host-guard"
      WlrLayershell.layer: WlrLayer.Overlay
      WlrLayershell.keyboardFocus: WlrKeyboardFocus.OnDemand

      Rectangle {
        id: card
        anchors.centerIn: parent
        width: Style.space(440)
        height: column.implicitHeight + Style.space(40)
        radius: Style.cornerRadius
        color: Color.popups.background
        border.color: Color.popups.border
        border.width: 1
        focus: true
        Keys.onEscapePressed: guard.dismissed = true
        Keys.onReturnPressed: guard.install()
        Keys.onEnterPressed: guard.install()

        Text {
          textFormat: Text.PlainText
          anchors.top: parent.top
          anchors.right: parent.right
          anchors.topMargin: Style.space(8)
          anchors.rightMargin: Style.space(12)
          text: "×"
          color: closePointer.containsMouse ? Color.bar.text : Color.muted
          font.family: Style.font.family
          font.pixelSize: Style.font.title

          MouseArea {
            id: closePointer
            anchors.fill: parent
            anchors.margins: -Style.space(6)
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: guard.dismissed = true
          }
        }

        Column {
          id: column
          anchors { left: parent.left; right: parent.right; top: parent.top; margins: Style.space(20) }
          spacing: Style.space(12)

          Image {
            id: logo
            width: Style.space(180)
            height: Math.round(width / 4)
            source: Qt.resolvedUrl("assets/fileblade-logo.png")
            asynchronous: true
            fillMode: Image.PreserveAspectFit
            smooth: true
            visible: false
          }

          Rectangle {
            id: logoFill
            width: logo.width
            height: logo.height
            color: Color.accent
            visible: false
          }

          MultiEffect {
            source: logoFill
            width: logo.width
            height: logo.height
            visible: logo.status === Image.Ready
            maskEnabled: true
            maskSource: logo
          }

          Text {
            width: parent.width
            textFormat: Text.PlainText
            wrapMode: Text.WordWrap
            text: guard.plan.message
            color: Color.popups.text
            font.family: Style.font.family
            font.pixelSize: Style.font.body
          }

          Text {
            width: parent.width
            textFormat: Text.PlainText
            wrapMode: Text.WordWrap
            text: "Waiting for FileBlade: " + guard.plan.names.join(", ")
            color: Color.muted
            font.family: Style.font.family
            font.pixelSize: Style.font.caption
          }

          Text {
            width: parent.width
            textFormat: Text.PlainText
            wrapMode: Text.WordWrap
            visible: guard.observation && guard.observation.state === "missing"
            text: HostGuard.HOST_REPOSITORY
            color: Color.muted
            font.family: Style.font.family
            font.pixelSize: Style.font.caption
          }

          Rectangle {
            anchors.right: parent.right
            visible: guard.plan.command.length > 0
            width: installLabel.implicitWidth + Style.space(24)
            height: Style.space(26)
            radius: 0
            color: guard.installing ? Util.alpha(Color.muted, 0.25) : (installPointer.containsMouse ? Qt.lighter(Color.accent, 1.1) : Color.accent)

            Text {
              id: installLabel
              textFormat: Text.PlainText
              anchors.centerIn: parent
              text: guard.installing ? "Enabling…" : guard.plan.action
              color: guard.installing ? Color.muted : Color.background
              font.family: Style.font.family
              font.pixelSize: Style.font.body
              font.weight: Font.DemiBold
            }

            MouseArea {
              id: installPointer
              anchors.fill: parent
              hoverEnabled: true
              enabled: !guard.installing
              cursorShape: Qt.PointingHandCursor
              onClicked: guard.install()
            }
          }
        }
      }
    }
  }
}
