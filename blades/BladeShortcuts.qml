import QtQuick
import QtQuick.Controls
import qs.Commons
import qs.Ui
import "../ui" as PluginUi
import "../theme"

Item {
  id: root

  required property var host
  required property var surface

  readonly property var sectionList: visible ? sections() : []
  property real measuredLead: 0
  readonly property real leadWidth: Math.min(measuredLead, Math.floor(content.width * 0.45))
  onSectionListChanged: measuredLead = measureLead(sectionList)
  readonly property var hostSection: ({
    title: "BLADES",
    items: [
      { shortcut: "Tab/Shift+Tab", text: "Next / previous module" },
      { shortcut: "Alt+Z", text: "Collapse / expand module" },
      { shortcut: "Ctrl+Tab  Ctrl+]/[", text: "Next / previous tab" },
      { shortcut: "Ctrl+PgDn/PgUp", text: "Next / previous tab" },
      { shortcut: ",  or Enter/o on empty slot", text: "Blade settings" },
      { shortcut: "Esc", text: "Back, then close" },
      { shortcut: "?", text: "This list" },
      { glyph: "󰇀", text: "Drag a title to move its module" }
    ]
  })
  readonly property var hyprlandSection: ({
    title: "HYPRLAND (your bindings.lua)",
    items: [
      { shortcut: "toggleBladeFocus l/r", text: "Focus or close a blade" },
      { shortcut: "focusDirection l/r/u/d", text: "Focus window or blade" },
      { shortcut: "windowToggle", text: "Float, dock or undock" },
      { shortcut: "windowSwap l/r/u/d", text: "Swap window or blade slot" },
      { shortcut: "windowClose", text: "Close window or blade" }
    ]
  })

  function measureLead(list) {
    var widest = 0
    for (var i = 0; i < list.length; i++) {
      var items = list[i].items || []
      for (var j = 0; j < items.length; j++) {
        leadMetrics.text = String(items[j].shortcut || items[j].glyph || "")
        widest = Math.max(widest, leadMetrics.advanceWidth)
      }
    }
    return Math.ceil(widest)
  }

  function close() {
    surface.shortcutsOpen = false
    host.focusBlade(surface.edge, surface.screen, -1, "")
  }

  function sections() {
    var result = [hostSection, hyprlandSection]
    for (var i = 0; i < surface.slots.length; i++) {
      var item = surface.slotItem(i)
      if (!item || !item.shortcuts) continue
      for (var j = 0; j < item.shortcuts.length; j++) result.push(item.shortcuts[j])
    }
    return result
  }

  TextMetrics {
    id: leadMetrics
    font.family: Style.font.family
    font.pixelSize: Typography.caption
    font.weight: Font.DemiBold
    font.letterSpacing: 0.3
  }

  focus: visible
  Keys.onPressed: function(event) {
    if (event.key === Qt.Key_Escape || event.text === "?" || event.key === Qt.Key_Q) {
      if (!surface.actionKeys || !surface.actionKeys.isRepeat(event)) root.close()
      event.accepted = true
    }
  }

  Rectangle {
    anchors.fill: parent
    color: Util.alpha(Color.background, 0.48)

    MouseArea {
      anchors.fill: parent
      onClicked: root.close()
    }
  }

  Rectangle {
    id: card
    anchors.centerIn: parent
    width: Math.min(parent.width - Style.space(32), Style.space(380))
    height: Math.min(parent.height - Style.space(64), Math.ceil(content.implicitHeight) + Style.space(24))
    radius: Style.cornerRadius
    color: Color.popups.background
    border.width: 1
    border.color: Color.popups.border
    clip: true

    MouseArea {
      anchors.fill: parent
      acceptedButtons: Qt.AllButtons
    }

    Flickable {
      anchors.fill: parent
      anchors.margins: Style.space(12)
      contentWidth: width
      contentHeight: Math.ceil(content.implicitHeight)
      interactive: contentHeight > height
      clip: true
      boundsBehavior: Flickable.StopAtBounds
      ScrollBar.vertical: PluginUi.AccentScrollBar { }

      Column {
        id: content
        width: parent.width
        spacing: Style.space(10)

        Item {
          width: parent.width
          height: Style.space(22)

          Text {
            textFormat: Text.PlainText
            anchors.left: parent.left
            anchors.verticalCenter: parent.verticalCenter
            text: "SHORTCUTS"
            color: Color.bar.text
            font.family: Style.font.family
            font.pixelSize: Typography.bodySmall
            font.weight: Font.DemiBold
            font.letterSpacing: 0.5
          }

          Text {
            textFormat: Text.PlainText
            anchors.right: parent.right
            anchors.verticalCenter: parent.verticalCenter
            text: "×"
            color: closePointer.containsMouse ? Color.bar.text : Color.muted
            font.family: Style.font.family
            font.pixelSize: Typography.title

            MouseArea {
              id: closePointer
              anchors.fill: parent
              anchors.margins: -Style.space(7)
              hoverEnabled: true
              cursorShape: Qt.PointingHandCursor
              onClicked: root.close()
            }
          }
        }

        Repeater {
          model: root.sectionList

          delegate: Column {
            required property var modelData
            width: parent ? parent.width : 0
            spacing: Style.space(3)

            Text {
              textFormat: Text.PlainText
              width: parent.width
              text: String(modelData.title || "").toUpperCase()
              color: Color.accent
              font.family: Style.font.family
              font.pixelSize: Typography.caption
              font.letterSpacing: 0.4
            }

            Repeater {
              model: modelData.items || []

              delegate: PluginUi.HintLine {
                required property var modelData
                width: parent ? parent.width : 0
                leadWidth: root.leadWidth
                wrap: true
                shortcut: String(modelData.shortcut || "")
                glyph: String(modelData.glyph || "")
                button: String(modelData.button || "")
                text: String(modelData.text || "")
              }
            }
          }
        }
      }
    }
  }
}
