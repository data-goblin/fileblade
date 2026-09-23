import QtQuick
import qs.Commons
import "../theme"

FocusScope {
  id: popoutHost

  property var host: null
  property var shell: null
  property var services: ({})
  property var screen: null
  property string moduleId: ""
  property bool opened: false
  property var popoutState: ({})
  property var moduleContext: null

  readonly property var definition: {
    var revision = host && host.registry ? host.registry.revision : -1
    return revision >= 0 && moduleId !== "" ? host.registry.module(moduleId) : null
  }
  readonly property bool compatible: !!definition && definition.compatible !== false
  readonly property string entryUrl: compatible ? String(definition.entryUrl) : ""
  readonly property var moduleItem: loader.item
  readonly property string title: moduleItem && moduleItem.title ? String(moduleItem.title) : (definition ? String(definition.name) : moduleId)
  readonly property string notice: {
    if (moduleId === "") return "Choose a module in the bar widget settings"
    if (!host) return "FileBlade is not loaded"
    if (!definition) return "No module called " + moduleId
    if (!compatible) return String(definition.name) + " needs a newer FileBlade"
    if (loader.status === Loader.Error) return String(definition.name) + " could not be loaded"
    return ""
  }

  signal closeRequested()

  Keys.onEscapePressed: function(event) {
    closeRequested()
    event.accepted = true
  }
  onActiveFocusChanged: if (activeFocus && loader.item) focusModule("")

  function setState(key, value) {
    var next = ({})
    var keys = Object.keys(popoutState)
    for (var i = 0; i < keys.length; i++) next[keys[i]] = popoutState[keys[i]]
    next[String(key)] = value
    popoutState = next
    return true
  }

  function focusModule(part) {
    var item = loader.item
    if (!item) {
      popoutHost.forceActiveFocus()
      return
    }
    if (typeof item.takeFocus === "function") item.takeFocus(String(part || ""))
    else item.forceActiveFocus()
  }

  function close() {
    closeRequested()
  }

  function unload() {
    if (moduleContext) {
      moduleContext.bladeOpen = false
      moduleContext.retired = true
    }
    loader.setSource("")
    if (moduleContext) moduleContext.destroy()
    moduleContext = null
  }

  function load() {
    unload()
    if (entryUrl === "" || !opened) return
    moduleContext = contextComponent.createObject(popoutHost, {
      moduleId: moduleId,
      moduleDir: String(definition.sourceDir || ""),
      providerId: String(definition.providerId || "")
    })
    moduleContext.definition = definition
    loader.setSource(entryUrl, { context: moduleContext })
    if (host && host.dirs) host.dirs.ensure(moduleId)
  }

  onOpenedChanged: load()
  onDefinitionChanged: if (opened) Qt.callLater(load)
  onModuleIdChanged: {
    popoutState = ({})
    if (opened) Qt.callLater(load)
  }
  Component.onDestruction: unload()

  Component {
    id: contextComponent
    BladeContext {
      host: popoutHost.host
      shell: popoutHost.shell
      services: popoutHost.services
      screen: popoutHost.screen
      edge: "popout"
      popout: popoutHost
      bladeOpen: !retired && popoutHost.opened
      bladeFocused: !retired && popoutHost.opened
      slotFocused: !retired && popoutHost.opened
      slotState: popoutHost.popoutState
    }
  }

  Loader {
    id: loader
    anchors.fill: parent
    focus: true
    onLoaded: if (popoutHost.opened) Qt.callLater(popoutHost.focusModule, "")
  }

  Text {
    textFormat: Text.PlainText
    anchors.centerIn: parent
    width: parent.width - Style.space(24)
    visible: popoutHost.notice !== ""
    text: popoutHost.notice
    color: Color.muted
    horizontalAlignment: Text.AlignHCenter
    wrapMode: Text.WordWrap
    font.family: Style.font.family
    font.pixelSize: Typography.bodySmall
  }
}
