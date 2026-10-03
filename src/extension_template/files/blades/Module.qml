import QtQuick
import qs.Commons

FocusScope {
  id: module

  property var context: null

  readonly property string title: "{{PLUGIN_NAME}}"
  readonly property var files: context ? context.service("files") : null
  readonly property var provider: context ? context.providerService : null
  readonly property bool loading: !provider || provider.ready !== true
  readonly property string providerStatus: loading ? "Loading…"
    : "Shared by " + provider.viewCount + (provider.viewCount === 1 ? " view" : " views")
  readonly property string selectedPath: files && files.selectedPath ? String(files.selectedPath) : ""
  readonly property string selectedName: context && selectedPath !== "" ? String(context.paths.name(selectedPath)) : ""
  readonly property string caption: String(setting("caption", ""))
  readonly property bool showSelection: setting("showSelection", true) === true
  readonly property bool active: !!context && context.bladeOpen !== false && context.collapsed !== true
  readonly property color paneBackground: Qt.lighter(Color.bar.background, 1.035)
  readonly property var shortcuts: [
    {
      title: "{{PLUGIN_NAME}}",
      items: [
        { shortcut: "Enter", text: "Open the selected path" },
        { shortcut: "e", text: "Open the selected path in the editor" },
        { shortcut: "Tab / Shift+Tab", text: "Next / previous slot" },
        { shortcut: "Esc", text: "Close the blade" }
      ]
    }
  ]

  function setting(key, fallback) {
    if (!context) return fallback
    if (context.settings && context.settings.has(key)) return context.settings.get(key)
    return context.state.get(key, fallback)
  }

  function takeFocus(part) {
    module.forceActiveFocus()
  }

  function openSelection(inEditor) {
    if (!files || selectedPath === "") return false
    if (inEditor) files.openInEditor(selectedPath)
    else files.openDefault(selectedPath, context.screen)
    return true
  }

  Component.onCompleted: if (provider) provider.attach(context)
  Component.onDestruction: if (provider) provider.detach(context)

  Keys.onPressed: function(event) {
    if (!context) return
    if (event.key === Qt.Key_Tab) context.focusNext()
    else if (event.key === Qt.Key_Backtab) context.focusPrevious()
    else if (event.key === Qt.Key_Escape) context.closeBlade()
    else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) openSelection(false)
    else if (event.key === Qt.Key_E && event.modifiers === Qt.NoModifier) openSelection(true)
    else return
    event.accepted = true
  }

  Rectangle {
    anchors.fill: parent
    color: module.paneBackground
  }

  MouseArea {
    anchors.fill: parent
    onClicked: module.forceActiveFocus()
  }

  Text {
    id: heading
    textFormat: Text.PlainText
    anchors.left: parent.left
    anchors.top: parent.top
    anchors.margins: Style.space(10)
    text: module.title.toUpperCase()
    color: module.activeFocus || handle.containsMouse ? Color.accent : Color.bar.text
    font.family: Style.font.family
    font.pixelSize: Style.font.bodySmall
    font.weight: Font.DemiBold
    font.letterSpacing: 0.6
  }

  MouseArea {
    id: handle
    anchors.fill: heading
    anchors.margins: -Style.space(5)
    hoverEnabled: true
    acceptedButtons: Qt.LeftButton
    cursorShape: pressed ? Qt.ClosedHandCursor : Qt.OpenHandCursor
    onPressed: function(mouse) { if (module.context) module.context.handlePressed(handle, mouse.x, mouse.y) }
    onPositionChanged: function(mouse) { if (module.context && (mouse.buttons & Qt.LeftButton)) module.context.handleMoved(handle, mouse.x, mouse.y) }
    onReleased: function(mouse) { if (module.context) module.context.handleReleased(handle, mouse.x, mouse.y) }
    onCanceled: if (module.context) module.context.handleCanceled()
  }

  Column {
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.top: heading.bottom
    anchors.margins: Style.space(10)
    spacing: Style.space(6)

    Text {
      id: selectionLabel
      width: parent.width
      textFormat: Text.PlainText
      elide: Text.ElideMiddle
      visible: module.showSelection
      text: module.selectedName !== "" ? module.selectedName : "Nothing selected"
      color: module.selectedName !== "" ? Color.bar.text : Color.muted
      font.family: Style.font.family
      font.pixelSize: Style.font.body
    }

    Text {
      id: providerLabel
      width: parent.width
      textFormat: Text.PlainText
      elide: Text.ElideRight
      text: module.providerStatus
      color: Color.muted
      font.family: Style.font.family
      font.pixelSize: Style.font.caption
    }

    Text {
      id: captionLabel
      width: parent.width
      textFormat: Text.PlainText
      wrapMode: Text.WordWrap
      text: module.caption !== "" ? module.caption : "Enter opens the selection, e edits it, ? lists the keys"
      color: Color.muted
      font.family: Style.font.family
      font.pixelSize: Style.font.caption
    }
  }
}
