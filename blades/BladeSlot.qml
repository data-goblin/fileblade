import QtQuick
import "../lib/TabIdentity.js" as TabIdentity
import qs.Commons
import "../ui" as PluginUi
import "../theme"

FocusScope {
  id: slot
  clip: true

  required property var host
  required property string edge
  required property int slotIndex
  required property var hostWindow
  property bool bladeOpen: false
  property bool bladeFocused: false
  property int pendingCloseTab: -1
  property var pendingCloseTarget: null
  property real moduleMenuX: 0
  readonly property bool modulePickerOpen: moduleMenu.visible

  readonly property bool hostActive: !hostWindow || hostWindow.surfaceActive !== false
  readonly property var slotData: host.slotAt(edge, slotIndex)
  readonly property string slotId: slotData ? String(slotData.id || "") : ""
  readonly property var tabs: host.slotTabs(edge, slotIndex)
  readonly property int activeTab: host.slotActiveTab(edge, slotIndex)
  readonly property bool tabbed: tabs.length > 1
  readonly property int tabBarHeight: tabbed ? host.tabBarHeight : 0
  readonly property string moduleId: host.slotModuleAt(edge, slotIndex, activeTab)
  readonly property var moduleInfo: {
    var revision = host.registry.revision
    return revision >= 0 ? host.registry.module(moduleId) : null
  }
  readonly property var disabledModuleInfo: {
    var revision = host.registry.revision
    return revision >= 0 && !moduleInfo ? host.registry.disabledModule(moduleId) : null
  }
  readonly property bool moduleDisabled: !!disabledModuleInfo
  readonly property bool contractIncompatible: !!moduleInfo && moduleInfo.compatible === false
  readonly property bool moduleNeedsUpdate: !!moduleInfo && moduleInfo.needsUpdate === true
  readonly property string providerError: {
    var id = moduleInfo && moduleInfo.providerId ? String(moduleInfo.providerId) : ""
    var reported = slot.host && slot.host.providerErrors ? slot.host.providerErrors : ({})
    return id && reported[id] ? String(reported[id]) : ""
  }
  readonly property int requiredContractVersion: moduleInfo ? Number(moduleInfo.hostContract) || 1 : 1
  readonly property string entryUrl: moduleInfo && !contractIncompatible ? String(moduleInfo.entryUrl) : ""
  readonly property var moduleItem: loader.item
  readonly property string title: moduleItem && moduleItem.title
    ? String(moduleItem.title)
    : (moduleInfo ? String(moduleInfo.name) : (disabledModuleInfo ? String(disabledModuleInfo.name) : moduleId))
  readonly property var view: moduleItem && moduleItem.view ? moduleItem.view : null
  readonly property var settingsComponent: moduleItem && moduleItem.settings ? moduleItem.settings : null
  readonly property var settingsContext: loader.moduleContext || context
  readonly property var shortcuts: moduleItem && moduleItem.shortcuts ? moduleItem.shortcuts : []
  readonly property bool dropTargetHere: host.dragActive && host.dropEdge === edge && host.dropTabSlot === slotIndex
    && host.dropTabBand === "tabs" && !host.dropNoop && (!hostWindow || host.dragScreen === hostWindow.screen)
  readonly property int dropTabIndex: dropTargetHere ? host.dropTabIndex : -1
  readonly property bool loadFailed: loader.status === Loader.Error

  function takeFocus(part) {
    if (moduleMenu.visible) {
      moduleMenu.focusSearch()
      return
    }
    var item = loader.item
    if (!item) {
      slot.forceActiveFocus()
      return
    }
    if (typeof item.takeFocus === "function") item.takeFocus(String(part || ""))
    else item.forceActiveFocus()
  }

  function tabInsertionIndex(x) {
    return tabBar.insertionIndexAt(x)
  }

  function reload() {
    if (!entryUrl || !hostActive) {
      loader.loadModule("")
      return
    }
    loader.loadModule(entryUrl)
    host.dirs.ensure(moduleId)
  }

  function scheduleReload() {
    reloadTimer.restart()
  }

  function requestCloseTab(tabIndex) {
    var index = Number(tabIndex)
    if (tabs.length <= 1 || !host.validIndex(index, tabs.length)) return false
    pendingCloseTab = index
    pendingCloseTarget = TabIdentity.capture(tabs, index, slotId)
    closeTabDialog.open("Close tab?\n" + host.tabTitle(edge, slotIndex, index),
                        [{ key: "cancel", label: "Cancel" }, { key: "close", label: "Close", danger: true }])
    return true
  }

  function confirmCloseTab() {
    var index = pendingCloseTab
    var target = pendingCloseTarget
    pendingCloseTab = -1
    pendingCloseTarget = null
    if (tabs.length > 1 && TabIdentity.matches(tabs, slotId, target)) host.removeTab(edge, slotIndex, index)
  }

  function dropStaleCloseTab() {
    if (!closeTabDialog.opened || TabIdentity.matches(tabs, slotId, pendingCloseTarget)) return
    pendingCloseTab = -1
    pendingCloseTarget = null
    closeTabDialog.close()
  }

  onTabsChanged: dropStaleCloseTab()

  function moduleRows() {
    var revision = host.registry.revision
    var rows = []
    var section = ""
    for (var i = 0; i < host.registry.order.length && revision >= 0; i++) {
      var module = host.registry.module(host.registry.order[i])
      if (!module) continue
      if (module.singleton && host.findModule(module.id) && module.id !== "files") continue
      var category = String(module.category || "")
      if (category !== section) {
        section = category
        rows.push({ kind: "separator", label: category })
      }
      rows.push({ key: module.id, glyph: String(module.glyph || "󰏗"), label: String(module.name || module.id), section: category })
    }
    return rows
  }

  function openModulePicker(anchorX) {
    moduleMenuX = Math.max(0, Math.min(Number(anchorX) || 0, width - moduleMenu.menuWidth))
    moduleMenu.rows = moduleRows()
    host.focusSlot(edge, slotIndex, hostWindow ? hostWindow.screen : null, "")
    moduleMenu.present()
    return true
  }

  function opensSettings(event) {
    var unavailable = !entryUrl || loadFailed || contractIncompatible || providerError !== ""
    var activate = event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_O
    var modified = event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier)
    if (!unavailable || !activate || modified) return false
    host.setSettingsOpen(true, edge)
    return true
  }

  onEntryUrlChanged: scheduleReload()
  onModuleIdChanged: scheduleReload()
  onSlotIdChanged: { dropStaleCloseTab(); scheduleReload() }
  onActiveTabChanged: scheduleReload()
  readonly property point hoverPosition: hoverWatch.point.scenePosition
  property real lastHoverX: -1
  property real lastHoverY: -1

  function overlayOpen() {
    if (host.settingsOpen || host.dragActive || moduleMenu.visible) return true
    return !!hostWindow && hostWindow.shortcutsOpen === true
  }

  function followPointer() {
    if (!hoverWatch.hovered) return
    var previousX = lastHoverX
    var previousY = lastHoverY
    lastHoverX = hoverPosition.x
    lastHoverY = hoverPosition.y
    if (previousX < 0) return
    if (Math.abs(lastHoverX - previousX) + Math.abs(lastHoverY - previousY) < 2) return
    var ownsFocus = slot.activeFocus && host.focusedEdge === edge && host.focusedScreen === context.screen
    if (context.collapsed || ownsFocus || !slot.hostActive || overlayOpen()) return
    host.focusSlot(edge, slotIndex, hostWindow ? hostWindow.screen : null, "")
  }

  onHoverPositionChanged: followPointer()
  onHostActiveChanged: scheduleReload()
  onActiveFocusChanged: context.reportFocus(activeFocus)
  Component.onCompleted: scheduleReload()

  HoverHandler {
    id: hoverWatch
    blocking: false
    onHoveredChanged: {
      slot.lastHoverX = hovered ? point.scenePosition.x : -1
      slot.lastHoverY = hovered ? point.scenePosition.y : -1
    }
  }

  Timer {
    id: reloadTimer
    interval: 0
    repeat: false
    onTriggered: slot.reload()
  }

  BladeContext {
    id: context
    host: slot.host
    shell: slot.host.shell
    services: slot.host.services
    hostWindow: slot.hostWindow
    screen: slot.hostWindow ? slot.hostWindow.screen : null
    edge: slot.edge
    slotIndex: slot.slotIndex
    slotId: slot.slotId
    moduleId: slot.moduleId
    moduleDir: slot.moduleInfo ? String(slot.moduleInfo.sourceDir) : ""
    providerId: slot.moduleInfo ? String(slot.moduleInfo.providerId || "") : ""
    tabIndex: slot.activeTab
    tabCount: slot.tabs.length
    bladeOpen: slot.bladeOpen
    bladeFocused: slot.bladeFocused
    slotFocused: slot.activeFocus
    collapsed: slot.host.slotCollapsed(slot.edge, slot.slotIndex)
    slotState: slot.host.slotState(slot.edge, slot.slotId)
    definition: slot.moduleInfo
    modulePickerRequested: function(anchorX) { return slot.openModulePicker(anchorX) }
  }

  Keys.onPressed: function(event) {
    if (event.key === Qt.Key_Z && event.modifiers === Qt.AltModifier) {
      context.toggleCollapsed()
      event.accepted = true
      return
    }
    if (slot.opensSettings(event)) {
      event.accepted = true
      return
    }
    if (!slot.tabbed) return
    var forward = event.key === Qt.Key_Tab || event.key === Qt.Key_BracketRight || event.key === Qt.Key_PageDown
    var backward = event.key === Qt.Key_Backtab || event.key === Qt.Key_BracketLeft || event.key === Qt.Key_PageUp
    if (!(forward || backward) || !(event.modifiers & Qt.ControlModifier)) return
    slot.host.cycleSlotTab(slot.edge, slot.slotIndex, forward ? 1 : -1)
    event.accepted = true
  }

  BladeTabBar {
    id: tabBar
    anchors.top: parent.top
    anchors.left: parent.left
    anchors.right: parent.right
    height: slot.tabBarHeight
    visible: slot.tabbed
    slot: slot
    context: context
    dropIndex: slot.dropTabIndex
    z: 5
  }

  BladeModuleLoader {
    id: loader
    liveContext: context
    anchors.top: slot.tabbed ? tabBar.bottom : parent.top
    anchors.bottom: parent.bottom
    anchors.left: parent.left
    anchors.right: parent.right
    asynchronous: false
    onStatusChanged: {
      if (status === Loader.Error)
        console.warn("data-goblin.fileblade: blade module " + slot.moduleId + " failed to load")
    }
  }

  PluginUi.ActionDialog {
    id: closeTabDialog
    anchors.fill: parent
    z: 60
    onCanceled: { slot.pendingCloseTab = -1; slot.pendingCloseTarget = null }
    onChosen: function(key) { if (key === "close") slot.confirmCloseTab() }
  }

  PluginUi.OptionPopup {
    id: moduleMenu
    x: slot.moduleMenuX
    y: slot.host.tabBarHeight + Style.space(2)
    menuWidth: Style.space(220)
    heading: "Add module"
    prompt: "Find module…"
    onPicked: function(key) { slot.host.addTab(slot.edge, slot.slotIndex, key, slot.host.tabSeedState(key)) }
  }

  Rectangle {
    anchors.fill: parent
    visible: !slot.entryUrl || slot.loadFailed || slot.contractIncompatible || slot.providerError !== ""
    color: Qt.lighter(Color.bar.background, 1.02)

    Column {
      anchors.centerIn: parent
      width: Math.max(0, parent.width - Style.space(40))
      spacing: Style.space(8)

      Text {
        textFormat: Text.PlainText
        width: parent.width
        text: slot.loadFailed || slot.contractIncompatible || slot.providerError !== "" ? "󰅚" : "󰐕"
        color: slot.loadFailed || slot.contractIncompatible || slot.providerError !== "" ? Color.urgent : Color.muted
        horizontalAlignment: Text.AlignHCenter
        font.family: Style.font.family
        font.pixelSize: Typography.title * 1.6
      }

      Text {
        textFormat: Text.PlainText
        width: parent.width
        text: slot.providerError !== ""
          ? "Module " + slot.moduleId + " could not start"
          : slot.moduleNeedsUpdate
          ? "Module " + slot.moduleId + " needs an update for this version of Omarchy"
          : slot.contractIncompatible
          ? "Module " + slot.moduleId + " requires blade contract " + slot.requiredContractVersion
          : slot.loadFailed
          ? "Module " + slot.moduleId + " failed to load"
          : slot.moduleDisabled
          ? "Module " + slot.moduleId + " is installed but disabled"
          : (slot.moduleId ? "Unknown module " + slot.moduleId : "Empty slot")
        color: slot.loadFailed || slot.contractIncompatible ? Color.urgent : Color.bar.text
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.WordWrap
        font.family: Style.font.family
        font.pixelSize: Typography.body
      }

      Text {
        textFormat: Text.PlainText
        width: parent.width
        text: slot.providerError !== ""
          ? slot.providerError + ". Check the shell log, then reload or choose another module."
          : slot.moduleNeedsUpdate
          ? "Update the Omarchy plugin that provides this module, then reopen this blade."
          : slot.contractIncompatible
          ? "This host supports contract " + context.contractVersion + ". Update the blades host or choose another module."
          : slot.loadFailed
          ? "Check the shell log, then pick another module."
          : slot.moduleDisabled
          ? "Enable its Omarchy plugin, then reopen this blade or choose another module."
          : "Pick a module for this slot in blade settings."
        color: Color.muted
        horizontalAlignment: Text.AlignHCenter
        wrapMode: Text.WordWrap
        font.family: Style.font.family
        font.pixelSize: Typography.caption
      }

      Rectangle {
        anchors.horizontalCenter: parent.horizontalCenter
        width: chooseLabel.implicitWidth + Style.space(20)
        height: Style.space(28)
        radius: Math.min(Style.cornerRadius, Style.space(4))
        color: choosePointer.containsMouse ? Color.accent : Util.alpha(Color.accent, 0.82)

        Text {
          textFormat: Text.PlainText
          id: chooseLabel
          anchors.centerIn: parent
          text: "Choose module"
          color: Color.background
          font.family: Style.font.family
          font.pixelSize: Typography.bodySmall
          font.weight: Font.DemiBold
        }

        MouseArea {
          id: choosePointer
          anchors.fill: parent
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onClicked: slot.host.setSettingsOpen(true, slot.edge)
        }
      }
    }
  }
}
