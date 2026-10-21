import QtQuick
import QtQuick.Controls as Controls
import qs.Commons
import qs.Ui
import "../theme"
import qs.Commons as Commons

FocusScope {
  id: dialog
  required property var preferences
  property bool presented: false
  readonly property var choices: [
    { days: 0, label: "Never" }, { days: 1, label: "1 day" }, { days: 7, label: "7 days" },
    { days: 30, label: "30 days" }, { days: 90, label: "90 days" }
  ]
  property int keyboardIndex: 0
  visible: presented && preferences.ready && !preferences.trashAnswered
  z: 100
  onVisibleChanged: if (visible) forceActiveFocus()

  function choose(index) {
    if (index < 0 || index >= choices.length || preferences.saving) return
    keyboardIndex = index
    preferences.pendingTrashDays = choices[index].days
    var row = choiceRows.itemAt(index)
    if (!row) return
    var top = body.y + row.y
    if (top < viewport.contentY) viewport.contentY = top
    else if (top + row.height > viewport.contentY + viewport.height)
      viewport.contentY = top + row.height - viewport.height
  }

  function confirm() {
    if (preferences.pendingTrashDays >= 0 && !preferences.saving)
      preferences.setTrashRetentionDays(preferences.pendingTrashDays, true)
  }

  Keys.priority: Keys.BeforeItem
  Keys.onPressed: function(event) {
    if (event.key === Qt.Key_Up || event.key === Qt.Key_K) choose(keyboardIndex < 0 ? 0 : (keyboardIndex + choices.length - 1) % choices.length)
    else if (event.key === Qt.Key_Down || event.key === Qt.Key_J) choose((keyboardIndex + 1) % choices.length)
    else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) confirm()
    event.accepted = true
  }
  Keys.onReleased: function(event) { event.accepted = true }

  Rectangle { anchors.fill: parent; color: Commons.Color.bar.background }
  MouseArea { anchors.fill: parent; onClicked: dialog.forceActiveFocus(); onWheel: function(wheel) { wheel.accepted = true } }

  Flickable {
    id: viewport
    anchors.fill: parent
    anchors.margins: Style.space(16)
    clip: true
    contentWidth: width
    contentHeight: Math.max(height, body.implicitHeight)
    interactive: contentHeight > height
    boundsBehavior: Flickable.StopAtBounds
    Controls.ScrollBar.vertical: AccentScrollBar {}

    Column {
      id: body
      anchors.horizontalCenter: parent.horizontalCenter
      y: Math.max(0, (viewport.height - implicitHeight) / 2)
      width: Math.min(viewport.width, Style.space(420))
      spacing: Style.space(12)

      Row {
        width: parent.width
        spacing: Style.space(12)

        Text {
          id: trashIcon
          text: "󰩹"
          textFormat: Text.PlainText
          color: Commons.Color.bar.text
          font.family: Style.font.family
          font.pixelSize: Style.font.iconLarge
        }

        Text {
          width: parent.width - trashIcon.implicitWidth - parent.spacing
          text: "Should FileBlade automatically empty the trash?"
          textFormat: Text.PlainText
          wrapMode: Text.Wrap
          color: Commons.Color.bar.text
          font.pixelSize: Typography.title
          font.family: Style.font.family
        }
      }
      Text {
        width: parent.width
        text: "This permanently deletes old items from your shared desktop Trash, including items trashed by other apps, and FileBlade artifact bins."
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: Commons.Color.bar.text
        font.pixelSize: Typography.body
        font.family: Style.font.family
      }
      Repeater {
        id: choiceRows
        model: dialog.choices
        delegate: Rectangle {
          required property var modelData
          required property int index
          width: parent.width
          height: Style.space(34)
          color: dialog.preferences.pendingTrashDays === modelData.days ? Util.alpha(Commons.Color.accent, 0.2) : "transparent"
          border.width: 1
          border.color: dialog.preferences.pendingTrashDays === modelData.days ? Commons.Color.accent : Util.alpha(Commons.Color.bar.text, 0.25)
          Text {
            anchors.left: parent.left
            anchors.leftMargin: Style.space(12)
            anchors.verticalCenter: parent.verticalCenter
            text: modelData.label
            textFormat: Text.PlainText
            color: Commons.Color.bar.text
            font.pixelSize: Typography.body
            font.family: Style.font.family
          }
          MouseArea { anchors.fill: parent; onClicked: { dialog.forceActiveFocus(); dialog.choose(index) } }
        }
      }
      Text {
        width: parent.width
        text: "You can change this later in settings 󰒓."
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: Commons.Color.bar.text
        font.pixelSize: Typography.bodySmall
        font.family: Style.font.family
      }
      Text {
        width: parent.width
        visible: text !== ""
        text: dialog.preferences.error
        textFormat: Text.PlainText
        wrapMode: Text.Wrap
        color: Commons.Color.urgent
        font.pixelSize: Typography.bodySmall
        font.family: Style.font.family
      }
      Button {
        fontFamily: Style.font.family
        text: dialog.preferences.saving ? "Saving…" : "Confirm"
        enabled: dialog.preferences.pendingTrashDays >= 0 && !dialog.preferences.saving
        onClicked: dialog.confirm()
      }
    }
  }
}
