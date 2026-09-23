import QtQuick
import QtQuick.Controls as Controls
import qs.Commons
import qs.Ui
import "../../ui" as PluginUi
import "WelcomePlan.js" as WelcomePlan
import "../../theme"

FocusScope {
  id: module
  property var context: null
  readonly property string title: "Welcome"
  readonly property var files: context ? context.service("files") : null
  readonly property var host: files ? files.bladeHost : null
  readonly property var welcome: files ? files.welcome : null
  readonly property bool updated: !!welcome && welcome.updated
  readonly property string appVersion: files ? String(files.appVersion || "") : ""
  readonly property var manifest: files ? files.manifest : null
  readonly property string issuesUrl: WelcomePlan.issuesUrl(manifest)
  readonly property string releaseUrl: WelcomePlan.releaseUrl(manifest, appVersion)
  readonly property string releaseNotesUrl: WelcomePlan.releaseNotesUrl(manifest)
  readonly property var shortcuts: [
    { title: "Welcome", items: [{ shortcut: "Enter", text: "Open Files" }, { shortcut: "Esc", text: "Close Welcome" }] }
  ]
  property var catalog: WelcomePlan.LOCAL_CATALOG
  property int helpIndex: 0
  property string error: ""

  function takeFocus(part) { module.forceActiveFocus() }
  function info(id) { return host && host.registry ? host.registry.module(id) : null }
  function acceptCatalog(text) {
    var next = WelcomePlan.catalog(text, context ? context.contractVersion : 0)
    if (!next) return false
    catalog = next
    return true
  }
  function openModule(id) {
    if (!info(id)) { error = "This blade is unavailable."; return false }
    var found = host.findModule(id)
    if (!found) {
      var welcomeSlot = host.findModule("welcome")
      if (!host.addSlot(welcomeSlot ? welcomeSlot.edge : "right", id, -1)) { error = "The blade could not be added."; return false }
      found = host.findModule(id)
    }
    if (!found) return false
    host.setSlotTab(found.edge, found.index, found.tab)
    host.setOpen(found.edge, true)
    host.focusModule(id, host.preferredScreen(found.edge), "")
    error = ""
    return true
  }
  function openLink(url) {
    if (String(url || "") === "") { error = "This link is unavailable."; return false }
    Qt.openUrlExternally(url)
    error = ""
    return true
  }
  function dismiss() {
    if (!files || !files.welcome.dismiss()) error = "Welcome could not be closed. Try again when the layout is writable."
  }
  Keys.onPressed: function(event) {
    if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) { openModule("files"); event.accepted = true }
    else if (event.key === Qt.Key_Escape) { dismiss(); event.accepted = true }
  }

  component Action: Controls.Button {
    id: action
    property string detail: ""
    property string glyph: ""
    property string iconUrl: ""
    property bool inline: false
    width: parent ? parent.width : 0
    padding: action.inline ? Style.space(4) : Style.space(8)
    implicitHeight: body.implicitHeight + padding * 2
    hoverEnabled: true
    Keys.onReturnPressed: clicked()
    Keys.onEnterPressed: clicked()
    onActiveFocusChanged: {
      if (activeFocus) scroll.contentY = Math.max(0, Math.min(mapToItem(content, 0, 0).y, scroll.contentHeight - scroll.height))
    }
    background: Rectangle {
      radius: 0
      color: action.hovered || action.activeFocus ? Util.alpha(Color.accent, 0.15) : "transparent"
      border.width: action.activeFocus ? 1 : 0
      border.color: Color.accent
    }
    contentItem: Row {
      spacing: Style.space(8)
      PluginUi.ModuleIcon {
        width: Style.space(16)
        height: width
        glyph: action.glyph
        iconUrl: action.iconUrl
        visible: glyph !== "" || iconUrl !== ""
        color: Color.accent
      }
      Column {
        id: body
        width: parent.width - (action.glyph !== "" || action.iconUrl !== "" ? Style.space(24) : 0)
        spacing: Style.space(3)
        Item {
          width: parent.width
          height: Math.max(nameText.implicitHeight, detailInline.implicitHeight)
          Text {
            id: nameText
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            width: detailInline.visible ? Math.max(0, parent.width - detailInline.implicitWidth - Style.space(8)) : parent.width
            textFormat: Text.PlainText
            text: action.text
            elide: action.inline ? Text.ElideRight : Text.ElideNone
            color: action.enabled ? Color.bar.text : Color.muted
            wrapMode: action.inline ? Text.NoWrap : Text.WordWrap
            font.family: Style.font.family
            font.pixelSize: Typography.body
          }
          Text {
            id: detailInline
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            visible: action.inline && action.detail !== ""
            textFormat: Text.PlainText
            text: action.detail
            color: Color.muted
            font.family: Style.font.family
            font.pixelSize: Typography.bodySmall
          }
        }
        Text {
          width: parent.width
          visible: !action.inline && action.detail !== ""
          textFormat: Text.PlainText
          text: action.detail
          color: Color.muted
          wrapMode: Text.WordWrap
          font.family: Style.font.family
          font.pixelSize: Typography.bodySmall
        }
      }
    }
  }

  Rectangle { anchors.fill: parent; color: Qt.lighter(Color.background, 1.035) }
  Flickable {
    id: scroll
    anchors.fill: parent
    anchors.margins: Style.space(12)
    contentWidth: width
    contentHeight: content.height
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    Controls.ScrollBar.vertical: Controls.ScrollBar {}
    Column {
      id: content
      width: scroll.width
      spacing: Style.space(8)
      Text {
        width: parent.width
        textFormat: Text.PlainText
        text: WelcomePlan.HEADING
        color: Color.bar.text
        font.family: Style.font.family
        font.pixelSize: Typography.body
        font.weight: Font.DemiBold
      }
      Text {
        width: parent.width
        textFormat: Text.PlainText
        text: WelcomePlan.BODY
        color: Color.muted
        wrapMode: Text.WordWrap
        font.family: Style.font.family
        font.pixelSize: Typography.bodySmall
      }
      PluginUi.SettingsGroup { visible: module.updated; title: "What's new" }
      Text {
        width: parent.width
        visible: module.updated
        textFormat: Text.PlainText
        text: "FileBlade was updated to " + module.appVersion + "."
        color: Color.muted
        wrapMode: Text.WordWrap
        font.family: Style.font.family
        font.pixelSize: Typography.bodySmall
      }
      Column {
        width: parent.width
        visible: module.updated
        spacing: Style.space(2)
        Action { inline: true; text: "Release downloads"; detail: "On GitHub"; onClicked: module.openLink(module.releaseUrl) }
        Action { inline: true; text: "Release notes"; detail: "On GitHub"; onClicked: module.openLink(module.releaseNotesUrl) }
      }
      Column {
        width: parent.width
        spacing: Style.space(2)
        Repeater {
          model: WelcomePlan.CORE
          delegate: Action {
            required property var modelData
            readonly property var entry: module.info(modelData.id)
            inline: true
            text: modelData.name
            detail: modelData.description
            glyph: entry ? entry.glyph : ""
            iconUrl: entry ? entry.iconUrl : ""
            enabled: !!entry
            onClicked: module.openModule(modelData.id)
          }
        }
      }
      PluginUi.SettingsGroup { title: "Help · available offline" }
      Repeater {
        model: WelcomePlan.HELP
        delegate: Action {
          required property var modelData
          required property int index
          text: modelData.name
          detail: module.helpIndex === index ? modelData.text : ""
          onClicked: module.helpIndex = index
        }
      }
      Column {
        width: parent.width
        spacing: Style.space(2)
        Action { inline: true; text: "Keyboard reference"; detail: "Local file"; onClicked: module.openLink(Qt.resolvedUrl("../../docs/agent-written/keybindings.md")) }
        Action { inline: true; text: "Extension authoring guide"; detail: "Local file"; onClicked: module.openLink(Qt.resolvedUrl("../../EXTENSIONS.md")) }
        Action { inline: true; text: "Report a problem"; detail: "GitHub issues"; onClicked: module.openLink(module.issuesUrl) }
      }
      PluginUi.SettingsGroup { title: "Extensions" }
      Text {
        width: parent.width
        visible: module.catalog.entries.length === 0
        textFormat: Text.PlainText
        text: "Extensions can add more blades. No optional extensions are listed here yet. Installed blades remain available from +."
        color: Color.muted
        wrapMode: Text.WordWrap
        font.family: Style.font.family
        font.pixelSize: Typography.bodySmall
      }
      Column {
        width: parent.width
        spacing: Style.space(2)
        Repeater {
          model: module.catalog.entries
          delegate: Action {
            required property var modelData
            inline: true
            text: modelData.name
            detail: modelData.compatible ? modelData.lifecycle : "Needs newer host"
            onClicked: module.openLink(modelData.source)
          }
        }
      }
      Text {
        width: parent.width
        visible: text !== ""
        textFormat: Text.PlainText
        text: module.error
        color: Color.urgent
        wrapMode: Text.WordWrap
        font.family: Style.font.family
        font.pixelSize: Typography.bodySmall
      }
      Item { width: parent.width; height: Style.space(4) }
      Button {
        bordered: true
        focusable: true
        fontFamily: Style.font.family
        text: WelcomePlan.DISMISS
        onClicked: module.dismiss()
      }
      Text {
        width: parent.width
        textFormat: Text.PlainText
        text: "Reopen Welcome with + in a blade."
        color: Color.muted
        wrapMode: Text.WordWrap
        font.family: Style.font.family
        font.pixelSize: Typography.bodySmall
      }
    }
  }
}
