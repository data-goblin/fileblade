import QtQuick
import qs.Commons
import qs.Ui
import "../ui" as PluginUi
import "../theme"

Item {
  id: bar
  clip: false

  required property var slot
  required property var context
  property bool interactive: true
  property int dropIndex: -1
  property int pendingVisibleTab: -1

  function tabItem(index) {
    return index >= 0 && index < tabRepeater.count ? tabRepeater.itemAt(index) : null
  }

  function insertionIndexAt(x) {
    var index = 0
    for (var i = 0; i < tabRepeater.count; i++) {
      var item = tabRepeater.itemAt(i)
      if (item && x > tabViewport.x + item.x - tabViewport.contentX + item.width / 2) index = i + 1
    }
    return index
  }

  function dropLineX() {
    if (tabRepeater.count === 0) return tabViewport.x
    var last = tabItem(tabRepeater.count - 1)
    if (dropIndex >= tabRepeater.count)
      return tabViewport.x + last.x - tabViewport.contentX + last.width + Math.round(tabRow.spacing / 2)
    var item = tabItem(Math.max(0, dropIndex))
    return item ? tabViewport.x + item.x - tabViewport.contentX : tabViewport.x
  }

  function scrollTabs(delta) {
    tabViewport.contentX = Math.max(0, Math.min(tabViewport.contentWidth - tabViewport.width, tabViewport.contentX + delta))
  }

  function ensureTabVisible(index) {
    pendingVisibleTab = index
    visibleTabTimer.restart()
  }

  Timer {
    id: visibleTabTimer
    interval: 0
    onTriggered: {
      var item = bar.tabItem(bar.pendingVisibleTab)
      if (!item || !tabArea.overflowing) return
      if (item.x < tabViewport.contentX) tabViewport.contentX = item.x
      else if (item.x + item.width > tabViewport.contentX + tabViewport.width)
        tabViewport.contentX = item.x + item.width - tabViewport.width
    }
  }

  function tabDragged(index) {
    var host = bar.slot.host
    return host.dragActive && host.dragEdge === bar.slot.edge && host.dragIndex === bar.slot.slotIndex
      && host.dragTab === index && bar.slot.tabs.length > 1
  }

  Rectangle {
    anchors.fill: parent
    color: Qt.lighter(Color.bar.background, bar.interactive ? 1.035 : 1.08)
    opacity: bar.interactive ? 1 : 0.94
  }

  Text {
    id: disclosure
    textFormat: Text.PlainText
    anchors.left: parent.left
    anchors.leftMargin: Style.space(7)
    anchors.verticalCenter: parent.verticalCenter
    text: bar.context.collapsed ? "›" : "⌄"
    color: disclosurePointer.containsMouse ? Color.accent : Color.muted
    font.family: Style.font.family
    font.pixelSize: Typography.body

    MouseArea {
      id: disclosurePointer
      anchors.fill: parent
      anchors.margins: -Style.space(4)
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: bar.context.toggleCollapsed()
    }
  }

  Item {
    id: tabArea
    anchors.top: parent.top
    anchors.bottom: parent.bottom
    anchors.left: disclosure.right
    anchors.leftMargin: Style.space(5)
    anchors.right: fileActions.visible ? fileActions.left : parent.right
    anchors.rightMargin: Style.space(4)
    readonly property bool overflowing: tabRow.implicitWidth > width

    Text {
      id: previousTab
      anchors.left: parent.left
      anchors.verticalCenter: parent.verticalCenter
      visible: tabArea.overflowing
      width: visible ? Style.space(22) : 0
      height: parent.height
      text: "<"
      textFormat: Text.PlainText
      color: tabViewport.contentX > 0 && previousTabPointer.containsMouse ? Color.accent : Color.muted
      horizontalAlignment: Text.AlignHCenter
      verticalAlignment: Text.AlignVCenter
      font.family: Style.font.family
      font.pixelSize: Typography.bodySmall

      MouseArea {
        id: previousTabPointer
        anchors.fill: parent
        enabled: tabViewport.contentX > 0
        hoverEnabled: true
        cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: bar.scrollTabs(-Math.max(Style.space(80), tabViewport.width * 0.7))
      }
    }

    Text {
      id: nextTab
      anchors.right: parent.right
      anchors.verticalCenter: parent.verticalCenter
      visible: tabArea.overflowing
      width: visible ? Style.space(22) : 0
      height: parent.height
      text: ">"
      textFormat: Text.PlainText
      color: tabViewport.contentX < tabViewport.contentWidth - tabViewport.width && nextTabPointer.containsMouse
        ? Color.accent : Color.muted
      horizontalAlignment: Text.AlignHCenter
      verticalAlignment: Text.AlignVCenter
      font.family: Style.font.family
      font.pixelSize: Typography.bodySmall

      MouseArea {
        id: nextTabPointer
        anchors.fill: parent
        enabled: tabViewport.contentX < tabViewport.contentWidth - tabViewport.width
        hoverEnabled: true
        cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
        onClicked: bar.scrollTabs(Math.max(Style.space(80), tabViewport.width * 0.7))
      }
    }

    Flickable {
      id: tabViewport
      anchors.top: parent.top
      anchors.bottom: parent.bottom
      anchors.left: previousTab.visible ? previousTab.right : parent.left
      anchors.right: nextTab.visible ? nextTab.left : parent.right
      contentWidth: tabRow.implicitWidth
      contentHeight: height
      clip: true
      boundsBehavior: Flickable.StopAtBounds
      interactive: tabArea.overflowing

      Row {
        id: tabRow
        height: parent.height
        spacing: Style.space(2)

    Repeater {
      id: tabRepeater
      model: bar.slot.tabs.length

      delegate: Rectangle {
        id: tab
        required property int index
        readonly property bool current: index === bar.slot.activeTab
        readonly property string moduleId: bar.slot.host.slotModuleAt(bar.slot.edge, bar.slot.slotIndex, index)
        height: parent ? parent.height : 0
        width: tabContent.implicitWidth + Style.space(16)
        color: current ? Qt.lighter(Color.bar.background, 1.06) : "transparent"
        opacity: bar.tabDragged(index) ? 0.45 : 1

        Rectangle {
          anchors.top: parent.top
          anchors.left: parent.left
          anchors.right: parent.right
          height: 2
          visible: tab.current
          color: bar.slot.activeFocus ? Color.accent : Color.muted
        }

        Row {
          id: tabContent
          anchors.centerIn: parent
          visible: bar.editTab !== tab.index
          spacing: Style.space(4)
          z: 1

          Text {
            textFormat: Text.PlainText
            visible: bar.slot.tabs.length > 1
            text: "×"
            color: closePointer.containsMouse ? Color.urgent : Color.muted
            font.family: Style.font.family
            font.pixelSize: Typography.bodySmall

            MouseArea {
              id: closePointer
              anchors.fill: parent
              anchors.margins: -Style.space(3)
              hoverEnabled: true
              cursorShape: Qt.PointingHandCursor
              onClicked: bar.slot.requestCloseTab(tab.index)
            }
          }

          Text {
            textFormat: Text.PlainText
            id: label
            text: bar.slot.host.tabTitle(bar.slot.edge, bar.slot.slotIndex, tab.index).toUpperCase()
            color: tab.current
              ? (bar.slot.activeFocus ? Color.accent : Color.muted)
              : (pointer.containsMouse ? Color.bar.text : Color.muted)
            font.family: Style.font.family
            font.pixelSize: Typography.caption
            font.weight: Font.DemiBold
            font.letterSpacing: 0.6
          }
        }

        MouseArea {
          id: pointer
          anchors.fill: parent
          enabled: bar.interactive
          hoverEnabled: bar.interactive
          acceptedButtons: Qt.LeftButton | Qt.MiddleButton | Qt.RightButton
          cursorShape: pressed ? Qt.ClosedHandCursor : Qt.PointingHandCursor

          onPressed: function(mouse) {
            if (mouse.button !== Qt.LeftButton) return
            bar.context.handlePressed(pointer, mouse.x, mouse.y, tab.index)
          }

          onPositionChanged: function(mouse) {
            if (mouse.buttons & Qt.LeftButton) bar.context.handleMoved(pointer, mouse.x, mouse.y)
          }

          onReleased: function(mouse) {
            if (mouse.button === Qt.MiddleButton) {
              bar.slot.host.removeTab(bar.slot.edge, bar.slot.slotIndex, tab.index)
              return
            }
            if (mouse.button === Qt.RightButton) {
              bar.openTabMenu(tab.index, tab.x)
              return
            }
            if (!bar.context.dragging) bar.context.selectTab(tab.index)
            bar.context.handleReleased(pointer, mouse.x, mouse.y)
          }

          onCanceled: bar.context.handleCanceled()
        }
      }
    }

    Rectangle {
      id: addTab
      visible: bar.interactive
      height: parent ? parent.height : 0
      width: Style.space(22)
      color: "transparent"

      Text {
        textFormat: Text.PlainText
        anchors.centerIn: parent
        text: "+"
        color: addPointer.containsMouse ? Color.accent : Color.muted
        font.family: Style.font.family
        font.pixelSize: Typography.body
      }

      MouseArea {
        id: addPointer
        anchors.fill: parent
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: bar.slot.openModulePicker(tabViewport.x + addTab.x - tabViewport.contentX)
      }

      PluginUi.HintTip {
        visible: addPointer.containsMouse
        title: "Add module"
        actions: [{ button: "left", text: "Choose a module" }]
      }
    }
      }
    }
  }

  Row {
    id: fileActions
    anchors.right: parent.right
    anchors.rightMargin: Style.space(7) + bar.context.cornerReserveRight
    anchors.verticalCenter: parent.verticalCenter
    spacing: Style.space(5)
    visible: bar.slot.moduleId === "files"
    z: 2

    PluginUi.PaneCorner {
      glyph: "󰑐"
      tip: "Refresh"
      tipActions: [{ button: "left", text: "Refresh" }, { shortcut: "Shift+R" }]
      onActivated: {
        var controller = bar.context.service("files")
        if (controller) controller.refreshTree()
      }
    }

    PluginUi.PaneCorner {
      glyph: "󰒓"
      tip: "Settings"
      tipActions: [{ button: "left", text: "Open settings" }, { shortcut: "," }]
      active: bar.context.host && bar.context.host.settingsOpen && bar.context.host.settingsEdge === bar.slot.edge
      onActivated: bar.context.toggleSettings()
    }
  }

  property int menuTab: -1
  property real menuX: 0
  property int editTab: -1

  function tabLabel(index) {
    var tabs = bar.slot.tabs
    var entry = index >= 0 && index < tabs.length ? tabs[index] : null
    var state = entry && entry.state && typeof entry.state === "object" ? entry.state : ({})
    return String(state.label || "")
  }

  function openTabMenu(index, x) {
    editTab = -1
    menuTab = index
    menuX = tabViewport.x + x - tabViewport.contentX
    var moduleId = bar.slot.host.slotModuleAt(bar.slot.edge, bar.slot.slotIndex, index)
    var rows = moduleId === "files"
      ? [
          { key: "rename", glyph: "󰏫", label: "Rename…" },
          { key: "clear", glyph: "󰅖", label: "Clear name", enabled: tabLabel(index) !== "" }
        ]
      : []
    rows.push(bar.slot.edge === "right"
      ? { key: "send", glyph: "←", label: "Move to left blade" }
      : { key: "send", glyph: "→", label: "Move to right blade" })
    rows.push({ key: "close", glyph: "×", label: "Close tab", enabled: bar.slot.tabs.length > 1 })
    tabMenu.rows = rows
    tabMenu.present()
  }

  function closeTabMenu() {
    menuTab = -1
  }

  function beginRename(index) {
    closeTabMenu()
    editTab = index
    var current = tabLabel(index)
    renameField.text = current !== "" ? current : bar.slot.host.tabTitle(bar.slot.edge, bar.slot.slotIndex, index)
    renameField.forceActiveFocus()
    renameField.selectAll()
  }

  function commitRename() {
    var index = editTab
    editTab = -1
    if (index < 0) return
    var text = renameField.text.trim()
    bar.slot.host.setTabStateValue(bar.slot.edge, bar.slot.slotIndex, index, "label", text !== "" ? text : null)
    bar.slot.forceActiveFocus()
  }

  function sendTab(index) {
    closeTabMenu()
    bar.slot.host.sendTabAcross(bar.slot.edge, bar.slot.slotIndex, index, bar.slot.hostWindow ? bar.slot.hostWindow.screen : null)
  }

  function clearLabel(index) {
    closeTabMenu()
    bar.slot.host.setTabStateValue(bar.slot.edge, bar.slot.slotIndex, index, "label", null)
  }

  PluginUi.OptionPopup {
    id: tabMenu
    x: Math.max(0, Math.min(bar.menuX, bar.width - width))
    y: bar.height + Style.space(2)
    menuWidth: Style.space(180)
    prompt: "Tab…"
    onPicked: function(key) {
      var index = bar.menuTab
      if (key === "rename") bar.beginRename(index)
      else if (key === "clear") bar.clearLabel(index)
      else if (key === "send") bar.sendTab(index)
      else if (key === "close") bar.slot.requestCloseTab(index)
    }
  }

  Rectangle {
    visible: bar.editTab >= 0
    x: {
      var item = bar.tabItem(bar.editTab)
      return item ? tabViewport.x + item.x - tabViewport.contentX : tabViewport.x
    }
    y: Style.space(3)
    width: Style.space(140)
    height: parent.height - Style.space(6)
    radius: Math.min(Style.cornerRadius, Style.space(4))
    color: Util.alpha(Color.bar.text, 0.10)
    border.width: 1
    border.color: Color.accent
    z: 7

    TextInput {
      id: renameField
      anchors.fill: parent
      anchors.leftMargin: Style.space(8)
      anchors.rightMargin: Style.space(8)
      verticalAlignment: TextInput.AlignVCenter
      color: Color.bar.text
      selectionColor: Util.alpha(Color.accent, 0.38)
      selectedTextColor: Color.bar.text
      selectByMouse: true
      clip: true
      font.family: Style.font.family
      font.pixelSize: Typography.caption
      font.weight: Font.DemiBold
      onAccepted: bar.commitRename()
      Keys.onEscapePressed: function(event) {
        bar.editTab = -1
        bar.slot.forceActiveFocus()
        event.accepted = true
      }
      onActiveFocusChanged: if (!activeFocus && bar.editTab >= 0) bar.commitRename()
    }
  }

  Rectangle {
    visible: bar.dropIndex >= 0
    x: Math.round(bar.dropLineX()) - 1
    y: Style.space(4)
    width: 2
    height: parent.height - Style.space(8)
    radius: 1
    color: Color.accent
    z: 5
  }

  Connections {
    target: bar.slot
    function onActiveTabChanged() { bar.ensureTabVisible(bar.slot.activeTab) }
  }

  onWidthChanged: ensureTabVisible(slot.activeTab)
}
