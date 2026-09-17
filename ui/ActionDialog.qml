import QtQuick
import qs.Commons
import "../theme"

Item {
  id: dialog

  property bool opened: false
  property string message: ""
  property var options: []
  property int selectedIndex: 0
  property Item returnFocusItem: null
  property var actionKeys: ownKeys
  ActionKeyGuard { id: ownKeys; active: dialog.opened }
  readonly property string title: message.split("\n")[0] || ""
  readonly property var lines: message.split("\n").slice(1).filter(function(line) { return line.trim() !== "" })
  readonly property color foreground: Color.bar.text
  readonly property color dim: Util.alpha(foreground, 0.62)

  signal chosen(string key)
  signal canceled()

  function open(text, choices) {
    if (!opened) returnFocusItem = dialog.Window.window ? dialog.Window.window.activeFocusItem : null
    message = text
    options = choices
    selectedIndex = 0
    opened = true
    forceActiveFocus()
  }

  function close() {
    var target = returnFocusItem
    var restore = activeFocus && dialog.Window.window && dialog.Window.window.active
    returnFocusItem = null
    opened = false
    if (restore && target && target.visible && target.enabled && target.Window.window === dialog.Window.window)
      target.forceActiveFocus()
  }

  function pick(index) {
    var option = index >= 0 && index < options.length ? options[index] : null
    close()
    if (!option || option.key === "cancel") dialog.canceled()
    else dialog.chosen(String(option.key))
  }

  function optionColor(option) {
    return option && option.danger === true ? Color.urgent : foreground
  }

  visible: opened
  focus: opened

  Keys.onPressed: function(event) {
    if (event.key === Qt.Key_Escape) {
      if (actionKeys.isRepeat(event)) { event.accepted = true; return }
      close()
      dialog.canceled()
    } else if (event.key === Qt.Key_Left || event.key === Qt.Key_H || event.key === Qt.Key_Backtab) {
      selectedIndex = (selectedIndex + options.length - 1) % Math.max(1, options.length)
    } else if (event.key === Qt.Key_Right || event.key === Qt.Key_L || event.key === Qt.Key_Tab) {
      selectedIndex = (selectedIndex + 1) % Math.max(1, options.length)
    } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
      if (!actionKeys.isRepeat(event)) pick(selectedIndex)
    }
    event.accepted = true
  }
  Keys.onReleased: function(event) { actionKeys.release(event) }

  Rectangle {
    anchors.fill: parent
    color: Util.alpha(Color.background, 0.6)

    MouseArea {
      anchors.fill: parent
      onClicked: {
        dialog.close()
        dialog.canceled()
      }
    }
  }

  Rectangle {
    id: card
    anchors.centerIn: parent
    width: Math.min(parent.width - Style.space(24), Style.space(340))
    height: body.implicitHeight + body.topPadding + body.bottomPadding
    radius: Style.cornerRadius
    color: Color.popups.background
    border.width: 1
    border.color: Color.popups.border

    MouseArea {
      anchors.fill: parent
      acceptedButtons: Qt.AllButtons
    }

    Column {
      id: body
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.top: parent.top
      leftPadding: Style.spacing.controlPaddingX
      rightPadding: Style.spacing.controlPaddingX
      topPadding: Style.spacing.controlPaddingY
      bottomPadding: Style.spacing.controlPaddingY
      spacing: Style.space(3)

      Text {
        textFormat: Text.PlainText
        width: parent.width - parent.leftPadding - parent.rightPadding
        text: dialog.title
        color: dialog.foreground
        elide: Text.ElideMiddle
        font.family: Style.font.family
        font.pixelSize: Typography.bodySmall
      }

      Repeater {
        model: dialog.lines

        delegate: Text {
          required property var modelData
          readonly property bool pathLike: String(modelData).indexOf("/") >= 0
          textFormat: Text.PlainText
          width: body.width - body.leftPadding - body.rightPadding
          text: String(modelData)
          color: dialog.dim
          elide: pathLike ? Text.ElideMiddle : Text.ElideNone
          wrapMode: pathLike ? Text.NoWrap : Text.WordWrap
          font.family: Style.font.family
          font.pixelSize: Typography.caption
        }
      }

      Rectangle {
        width: parent.width - parent.leftPadding - parent.rightPadding
        height: 1
        color: Util.alpha(dialog.foreground, 0.16)
      }

      Row {
        id: choices
        anchors.right: parent.right
        anchors.rightMargin: parent.rightPadding
        spacing: Style.space(6)

        Repeater {
          model: dialog.options

          delegate: Item {
            id: option
            required property int index
            required property var modelData
            readonly property bool selected: index === dialog.selectedIndex
            readonly property bool active: selected || optionPointer.containsMouse
            readonly property color tone: dialog.optionColor(modelData)
            implicitWidth: label.implicitWidth + Style.space(20)
            implicitHeight: Math.max(Style.space(28), label.implicitHeight + Style.space(12))

            Rectangle {
              anchors.fill: parent
              radius: Math.min(Style.cornerRadius, Style.space(4))
              color: optionPointer.containsMouse ? Util.alpha(option.tone, 0.14) : "transparent"
            }

            Text {
              id: label
              anchors.centerIn: parent
              textFormat: Text.PlainText
              text: String(modelData.label || "")
              color: option.active ? option.tone : Util.alpha(option.tone, 0.72)
              font.family: Style.font.family
              font.pixelSize: Typography.caption
              font.weight: option.active ? Font.DemiBold : Font.Normal
              font.letterSpacing: 0.3
            }

            Rectangle {
              anchors.left: parent.left
              anchors.right: parent.right
              anchors.leftMargin: Style.space(10)
              anchors.rightMargin: Style.space(10)
              anchors.bottom: parent.bottom
              anchors.bottomMargin: Style.space(4)
              height: 2
              radius: 1
              visible: option.selected
              color: option.tone
            }

            MouseArea {
              id: optionPointer
              anchors.fill: parent
              hoverEnabled: true
              cursorShape: Qt.PointingHandCursor
              onClicked: dialog.pick(option.index)
            }
          }
        }
      }
    }
  }
}
