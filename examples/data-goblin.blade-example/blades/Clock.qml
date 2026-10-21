import QtQuick
import qs.Commons
import qs.Commons as Commons

FocusScope {
  id: module

  required property var context

  readonly property string title: "Clock"
  readonly property var formats: ["HH:mm:ss", "hh:mm AP", "dddd d MMMM", "yyyy-MM-dd HH:mm"]
  readonly property string format: String(setting("format", formats[0]))
  readonly property int formatIndex: Math.max(0, formats.indexOf(format))
  readonly property string caption: String(setting("caption", ""))
  readonly property real sizeScale: Math.max(0.6, Math.min(2, Number(setting("scale", 1)) || 1))
  property date now: new Date()

  function setting(key, fallback) {
    return context.settings && context.settings.has(key) ? context.settings.get(key) : context.state.get(key, fallback)
  }

  function takeFocus(part) {
    module.forceActiveFocus()
  }

  function cycleFormat() {
    var next = formats[(formatIndex + 1) % formats.length]
    if (context.settings && context.settings.has("format")) context.settings.set("format", next)
    else context.state.set("format", next)
  }

  Timer {
    interval: 1000
    running: true
    repeat: true
    onTriggered: module.now = new Date()
  }

  Keys.onPressed: function(event) {
    if (event.key === Qt.Key_Space || event.key === Qt.Key_Return) {
      cycleFormat()
    } else if (event.key === Qt.Key_Tab) {
      context.focusNext()
    } else if (event.key === Qt.Key_Escape) {
      context.closeBlade()
    } else {
      return
    }
    event.accepted = true
  }

  Rectangle {
    anchors.fill: parent
    color: Qt.lighter(Commons.Color.bar.background, 1.035)
  }

  Text {
    textFormat: Text.PlainText
    id: clockTitle
    anchors.left: parent.left
    anchors.top: parent.top
    anchors.margins: Style.space(10)
    text: "CLOCK"
    color: module.activeFocus || clockHandle.containsMouse ? Commons.Color.accent : Commons.Color.bar.text
    font.family: Style.font.family
    font.pixelSize: Style.font.bodySmall
    font.weight: Font.DemiBold
    font.letterSpacing: 0.6
  }

  MouseArea {
    id: clockHandle
    anchors.fill: clockTitle
    anchors.margins: -Style.space(5)
    hoverEnabled: true
    acceptedButtons: Qt.LeftButton
    cursorShape: pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor
    onPressed: function(mouse) { if (module.context) module.context.handlePressed(clockHandle, mouse.x, mouse.y) }
    onPositionChanged: function(mouse) { if (module.context && (mouse.buttons & Qt.LeftButton)) module.context.handleMoved(clockHandle, mouse.x, mouse.y) }
    onReleased: function(mouse) { if (module.context) module.context.handleReleased(clockHandle, mouse.x, mouse.y) }
    onCanceled: if (module.context) module.context.handleCanceled()
  }

  Column {
    anchors.centerIn: parent
    spacing: Style.space(6)

    Text {
      textFormat: Text.PlainText
      anchors.horizontalCenter: parent.horizontalCenter
      text: Qt.formatDateTime(module.now, module.format)
      color: Commons.Color.bar.text
      font.family: Style.font.family
      font.pixelSize: Math.round(Style.font.title * 1.5 * module.sizeScale)
      font.weight: Font.DemiBold
    }

    Text {
      textFormat: Text.PlainText
      anchors.horizontalCenter: parent.horizontalCenter
      text: module.caption !== "" ? module.caption : "from " + context.moduleId + "  ·  Space cycles the format"
      color: Commons.Color.muted
      font.family: Style.font.family
      font.pixelSize: Style.font.caption
    }
  }

  MouseArea {
    anchors.fill: parent
    onClicked: {
      module.forceActiveFocus()
      module.cycleFormat()
    }
  }
}
