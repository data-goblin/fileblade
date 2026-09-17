import QtQuick
import qs.Commons
import qs.Ui
import "../../ui" as PluginUi
import "../../theme"

Rectangle {
  id: tabs

  required property var moduleRoot
  readonly property var items: moduleRoot.noteItems
  readonly property int activeIndex: moduleRoot.activeNoteIndex
  readonly property bool overflowing: tabRow.implicitWidth > width - Style.space(8)
  property int editIndex: -1
  property int menuIndex: -1
  property int pendingVisibleIndex: -1

  height: Style.space(32)
  color: Qt.lighter(Color.bar.background, 1.035)

  function tabItem(index) {
    return index >= 0 && index < repeater.count ? repeater.itemAt(index) : null
  }

  function scrollBy(delta) {
    viewport.contentX = Math.max(0, Math.min(viewport.contentWidth - viewport.width, viewport.contentX + delta))
  }

  function ensureVisible(index) {
    pendingVisibleIndex = index
    visibleTimer.restart()
  }

  Timer {
    id: visibleTimer
    interval: 0
    onTriggered: {
      var item = tabs.tabItem(tabs.pendingVisibleIndex)
      if (!item || !tabs.overflowing) return
      if (item.x < viewport.contentX) viewport.contentX = item.x
      else if (item.x + item.width > viewport.contentX + viewport.width)
        viewport.contentX = item.x + item.width - viewport.width
    }
  }

  function beginRename(index) {
    menu.close()
    editIndex = index
    renameField.text = items[index] ? String(items[index].label || "") : ""
    ensureVisible(index)
    Qt.callLater(function() {
      renameField.forceActiveFocus()
      renameField.selectAll()
    })
  }

  function commitRename() {
    var index = editIndex
    editIndex = -1
    if (index >= 0) moduleRoot.renameNote(index, renameField.text)
    moduleRoot.takeFocus("")
  }

  function openMenu(index, x) {
    editIndex = -1
    menuIndex = index
    menu.x = Math.max(0, Math.min(viewport.x + x - viewport.contentX, width - menu.width))
    menu.rows = [
      { key: "rename", glyph: "󰏫", label: "Rename…" },
      { key: "close", glyph: "×", label: "Close note", enabled: items.length > 1 }
    ]
    menu.present()
  }

  Text {
    id: previous
    anchors.left: parent.left
    anchors.verticalCenter: parent.verticalCenter
    visible: tabs.overflowing
    width: visible ? Style.space(22) : 0
    height: parent.height
    text: "<"
    textFormat: Text.PlainText
    color: viewport.contentX > 0 && previousPointer.containsMouse ? Color.accent : Color.muted
    horizontalAlignment: Text.AlignHCenter
    verticalAlignment: Text.AlignVCenter
    font.family: Style.font.family
    font.pixelSize: Typography.bodySmall

    MouseArea {
      id: previousPointer
      anchors.fill: parent
      enabled: viewport.contentX > 0
      hoverEnabled: true
      cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
      onClicked: tabs.scrollBy(-Math.max(Style.space(80), viewport.width * 0.7))
    }
  }

  Text {
    id: next
    anchors.right: parent.right
    anchors.rightMargin: Style.space(4)
    anchors.verticalCenter: parent.verticalCenter
    visible: tabs.overflowing
    width: visible ? Style.space(22) : 0
    height: parent.height
    text: ">"
    textFormat: Text.PlainText
    color: viewport.contentX < viewport.contentWidth - viewport.width && nextPointer.containsMouse ? Color.accent : Color.muted
    horizontalAlignment: Text.AlignHCenter
    verticalAlignment: Text.AlignVCenter
    font.family: Style.font.family
    font.pixelSize: Typography.bodySmall

    MouseArea {
      id: nextPointer
      anchors.fill: parent
      enabled: viewport.contentX < viewport.contentWidth - viewport.width
      hoverEnabled: true
      cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
      onClicked: tabs.scrollBy(Math.max(Style.space(80), viewport.width * 0.7))
    }
  }

  Flickable {
    id: viewport
    anchors.top: parent.top
    anchors.bottom: parent.bottom
    anchors.left: previous.visible ? previous.right : parent.left
    anchors.right: next.visible ? next.left : parent.right
    anchors.leftMargin: Style.space(4)
    anchors.rightMargin: Style.space(4)
    contentWidth: tabRow.implicitWidth
    contentHeight: height
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    interactive: tabs.overflowing

    Row {
      id: tabRow
      height: parent.height
      spacing: Style.space(2)

      Repeater {
        id: repeater
        model: tabs.items.length

        delegate: Rectangle {
          id: tab
          required property int index
          readonly property bool current: index === tabs.activeIndex
          width: Math.min(Style.space(140), Math.max(Style.space(64), contents.implicitWidth + Style.space(16)))
          height: tabRow.height
          color: current ? Qt.lighter(Color.bar.background, 1.08) : "transparent"

          Rectangle {
            anchors.bottom: parent.bottom
            anchors.left: parent.left
            anchors.right: parent.right
            height: 2
            visible: tab.current
            color: moduleRoot.activeFocus ? Color.accent : Color.muted
          }

          MouseArea {
            id: tabPointer
            anchors.fill: parent
            hoverEnabled: true
            acceptedButtons: Qt.LeftButton | Qt.MiddleButton | Qt.RightButton
            cursorShape: Qt.PointingHandCursor
            onClicked: function(mouse) {
              if (mouse.button === Qt.MiddleButton) moduleRoot.closeNote(tab.index)
              else if (mouse.button === Qt.RightButton) tabs.openMenu(tab.index, tab.x)
              else moduleRoot.selectNote(tab.index)
            }
            onDoubleClicked: function(mouse) {
              if (mouse.button === Qt.LeftButton) tabs.beginRename(tab.index)
            }
          }

          Row {
            id: contents
            anchors.centerIn: parent
            spacing: Style.space(4)
            z: 1

            Text {
              id: label
              width: Math.min(implicitWidth, Style.space(100))
              text: tabs.items[tab.index] ? String(tabs.items[tab.index].label || "") : ""
              textFormat: Text.PlainText
              elide: Text.ElideRight
              color: tab.current
                ? (moduleRoot.activeFocus ? Color.accent : Color.muted)
                : (tabPointer.containsMouse ? Color.bar.text : Color.muted)
              font.family: Style.font.family
              font.pixelSize: Typography.caption
              font.weight: Font.DemiBold
            }

            Text {
              visible: tabs.items.length > 1
              text: "×"
              textFormat: Text.PlainText
              color: closePointer.containsMouse ? Color.accent : Color.muted
              font.family: Style.font.family
              font.pixelSize: Typography.bodySmall

              MouseArea {
                id: closePointer
                anchors.fill: parent
                anchors.margins: -Style.space(3)
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: moduleRoot.closeNote(tab.index)
              }
            }
          }
        }
      }

      Rectangle {
        width: Style.space(24)
        height: tabRow.height
        color: "transparent"

        Text {
          anchors.centerIn: parent
          text: "+"
          textFormat: Text.PlainText
          color: addPointer.containsMouse && addPointer.enabled ? Color.accent : Color.muted
          font.family: Style.font.family
          font.pixelSize: Typography.body
        }

        MouseArea {
          id: addPointer
          anchors.fill: parent
          enabled: tabs.items.length < moduleRoot.maximumNotes
          hoverEnabled: true
          cursorShape: enabled ? Qt.PointingHandCursor : Qt.ArrowCursor
          onClicked: moduleRoot.createNote()
        }

        PluginUi.HintTip {
          visible: addPointer.containsMouse
          title: addPointer.enabled ? "New note" : "64 note limit reached"
          actions: addPointer.enabled ? [{ button: "left", text: "Create a note" }] : []
        }
      }
    }
  }

  PluginUi.OptionPopup {
    id: menu
    y: tabs.height + Style.space(2)
    menuWidth: Style.space(180)
    prompt: "Note…"
    onPicked: function(key) {
      var index = tabs.menuIndex
      if (key === "rename") tabs.beginRename(index)
      else if (key === "close") moduleRoot.closeNote(index)
    }
  }

  Rectangle {
    visible: tabs.editIndex >= 0
    x: {
      var item = tabs.tabItem(tabs.editIndex)
      return Math.max(0, Math.min(item ? viewport.x + item.x - viewport.contentX : viewport.x, tabs.width - width))
    }
    y: Style.space(3)
    width: Style.space(140)
    height: parent.height - Style.space(6)
    radius: Math.min(Style.cornerRadius, Style.space(4))
    color: Color.bar.background
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
      selectionColor: Qt.rgba(Color.accent.r, Color.accent.g, Color.accent.b, 0.38)
      selectedTextColor: Color.bar.text
      selectByMouse: true
      clip: true
      font.family: Style.font.family
      font.pixelSize: Typography.caption
      onAccepted: tabs.commitRename()
      onActiveFocusChanged: if (!activeFocus && tabs.editIndex >= 0) tabs.commitRename()
      Keys.onEscapePressed: function(event) {
        tabs.editIndex = -1
        moduleRoot.takeFocus("")
        event.accepted = true
      }
    }
  }

  onActiveIndexChanged: ensureVisible(activeIndex)
  onWidthChanged: ensureVisible(activeIndex)
}
