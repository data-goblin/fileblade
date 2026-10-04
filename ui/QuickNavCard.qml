import QtQuick
import QtQuick.Controls
import qs.Commons
import qs.Ui
import "../lib/Highlight.js" as Highlight
import "../lib/KeyRouter.js" as KeyRouter
import "../theme"

FocusScope {
  id: card

  property string title: "QUICK NAV"
  property string note: ""
  property string placeholder: "Search…"
  property string status: ""
  property bool statusUrgent: false
  property var chips: []
  property var model: []
  property var glyphFor: function(row) { return "" }
  property var glyphColorFor: function(row) { return "" }
  property var glyphFamilyFor: function(row) { return "" }
  property alias text: input.text
  readonly property alias currentIndex: list.currentIndex
  readonly property alias count: list.count
  readonly property color secondaryTextColor: Util.alpha(Color.bar.text, 0.72)

  signal edited(string text)
  signal activated(int index, bool alternate)
  signal dismissed()
  signal chipToggled(string key, bool active)
  signal deepRequested()
  signal emptyBackspace()

  function focusInput() {
    input.forceActiveFocus()
    input.selectAll()
  }

  function moveCurrent(delta) {
    if (list.count === 0) return
    var current = list.currentIndex < 0 ? (delta > 0 ? -1 : list.count) : list.currentIndex
    list.currentIndex = Math.max(0, Math.min(list.count - 1, current + delta))
    list.positionViewAtIndex(list.currentIndex, ListView.Contain)
  }

  function resetCurrent() {
    list.currentIndex = list.count > 0 ? 0 : -1
    list.positionViewAtBeginning()
  }

  function clearCurrent() {
    list.currentIndex = -1
  }

  Keys.onPressed: function(event) {
    if (event.key !== Qt.Key_Escape) return
    card.dismissed()
    event.accepted = true
  }

  Rectangle {
    anchors.fill: parent
    color: Util.alpha(Color.background, 0.55)

    MouseArea {
      anchors.fill: parent
      onClicked: card.dismissed()
    }
  }

  Rectangle {
    id: panel
    anchors.top: parent.top
    anchors.topMargin: Style.space(40)
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.leftMargin: Style.space(10)
    anchors.rightMargin: Style.space(10)
    height: Math.min(parent.height - Style.space(80), header.height + inputRow.height + statusLine.height + list.count * Style.space(40) + Style.space(30))
    radius: Style.cornerRadius
    color: Color.popups.background
    border.width: 1
    border.color: Color.popups.border
    clip: true

    MouseArea {
      anchors.fill: parent
      acceptedButtons: Qt.AllButtons
    }

    Item {
      id: header
      anchors.top: parent.top
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.margins: Style.space(10)
      height: Style.space(24)

      Text {
        textFormat: Text.PlainText
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        text: String(card.title || "").toUpperCase()
        color: Color.bar.text
        font.family: Style.font.family
        font.pixelSize: Typography.bodySmall
        font.weight: Font.DemiBold
        font.letterSpacing: 0.6
      }

      Text {
        textFormat: Text.PlainText
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        text: String(card.note || "")
        color: card.secondaryTextColor
        font.family: Style.font.family
        font.pixelSize: Typography.caption
      }
    }

    Item {
      id: inputRow
      anchors.top: header.bottom
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.leftMargin: Style.space(10)
      anchors.rightMargin: Style.space(10)
      height: Style.space(36)

      TextField {
        id: input
        objectName: "quickNavInput"
        anchors.fill: parent
        leftPadding: Style.space(30)
        rightPadding: Style.space(10) + chipRow.width
        selectByMouse: true
        placeholderText: card.placeholder
        placeholderTextColor: card.secondaryTextColor
        color: Color.bar.text
        selectionColor: Util.alpha(Color.accent, 0.38)
        selectedTextColor: Color.bar.text
        font.family: Style.font.family
        font.pixelSize: Typography.body

        background: Rectangle {
          radius: Math.min(Style.cornerRadius, Style.space(4))
          color: Util.alpha(Color.bar.text, input.activeFocus ? 0.10 : 0.06)
          border.width: 1
          border.color: input.activeFocus ? Color.accent : Util.alpha(Color.bar.text, 0.18)
        }

        Text {
          textFormat: Text.PlainText
          anchors.left: parent.left
          anchors.leftMargin: Style.space(10)
          anchors.verticalCenter: parent.verticalCenter
          text: ""
          color: input.activeFocus ? Color.accent : card.secondaryTextColor
          font.family: Style.font.family
          font.pixelSize: Typography.bodySmall
        }

        Row {
          id: chipRow
          anchors.right: parent.right
          anchors.rightMargin: Style.space(8)
          anchors.verticalCenter: parent.verticalCenter
          spacing: Style.space(2)

          Repeater {
            model: Array.isArray(card.chips) ? card.chips.length : 0

            delegate: Rectangle {
              id: chip
              required property int index
              readonly property var spec: card.chips[index] || ({})
              readonly property bool active: spec.active === true
              width: Style.space(22)
              height: Style.space(20)
              color: active ? Util.alpha(Color.accent, 0.22) : (chipPointer.containsMouse ? Util.alpha(Color.bar.text, 0.10) : "transparent")
              border.width: active ? 1 : 0
              border.color: Util.alpha(Color.accent, 0.6)

              Text {
                textFormat: Text.PlainText
                anchors.centerIn: parent
                text: String(chip.spec.label || "")
                color: chip.active ? Color.accent : (chipPointer.containsMouse ? Color.bar.text : card.secondaryTextColor)
                font.family: Style.font.family
                font.pixelSize: Typography.caption
                font.bold: chip.active
              }

              PanelToolTip {
                visible: chipPointer.containsMouse
                text: String(chip.active ? (chip.spec.activeTip || chip.spec.tip || "") : (chip.spec.tip || ""))
              }

              MouseArea {
                id: chipPointer
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: {
                  card.chipToggled(String(chip.spec.key || ""), !chip.active)
                  input.forceActiveFocus()
                }
              }
            }
          }
        }

        onTextEdited: card.edited(text)

        Keys.priority: Keys.BeforeItem
        Keys.onPressed: function(event) {
          if (KeyRouter.listModeAction(event, false) === "deep") {
            card.deepRequested()
          } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            card.activated(list.currentIndex < 0 ? 0 : list.currentIndex, !!(event.modifiers & Qt.ShiftModifier))
          } else if (event.key === Qt.Key_Down || (event.key === Qt.Key_N && (event.modifiers & Qt.ControlModifier))) {
            card.moveCurrent(1)
          } else if (event.key === Qt.Key_Up || (event.key === Qt.Key_P && (event.modifiers & Qt.ControlModifier))) {
            card.moveCurrent(-1)
          } else if (event.key === Qt.Key_Escape) {
            card.dismissed()
          } else if (event.key === Qt.Key_Backspace && input.text === "") {
            card.emptyBackspace()
          } else if (event.key === Qt.Key_Tab
                     && !(event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier | Qt.ShiftModifier))) {
            card.moveCurrent(1)
          } else {
            return
          }
          event.accepted = true
        }
      }
    }

    Text {
      textFormat: Text.PlainText
      id: statusLine
      objectName: "quickNavStatus"
      anchors.top: inputRow.bottom
      anchors.topMargin: Style.space(4)
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.leftMargin: Style.space(12)
      anchors.rightMargin: Style.space(12)
      height: Style.space(18)
      verticalAlignment: Text.AlignVCenter
      text: card.status
      color: card.statusUrgent ? Color.urgent : card.secondaryTextColor
      elide: Text.ElideRight
      font.family: Style.font.family
      font.pixelSize: Typography.caption
    }

    ListView {
      id: list
      objectName: "quickNavResults"
      reuseItems: true
      anchors.top: statusLine.bottom
      anchors.topMargin: Style.space(4)
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.bottom: parent.bottom
      anchors.bottomMargin: Style.space(6)
      clip: true
      boundsBehavior: Flickable.StopAtBounds
      model: card.model
      currentIndex: -1
      ScrollBar.vertical: AccentScrollBar { }

      onCountChanged: if (count > 0 && currentIndex < 0) currentIndex = 0

      delegate: Rectangle {
        id: row
        required property int index
        required property var modelData

        readonly property bool current: list.currentIndex === index
        readonly property string glyphColor: String(card.glyphColorFor(modelData) || "")
        readonly property string glyphFamily: String(card.glyphFamilyFor(modelData) || "")
        width: ListView.view ? ListView.view.width : 0
        height: Style.space(40)
        color: current
          ? Color.menu.selectedBackground
          : (rowPointer.containsMouse ? Style.hoverFillFor(Color.bar.text, Color.accent) : "transparent")

        Text {
          textFormat: Text.PlainText
          id: rowIcon
          anchors.left: parent.left
          anchors.leftMargin: Style.space(12)
          anchors.verticalCenter: parent.verticalCenter
          width: Style.space(18)
          text: String(card.glyphFor(row.modelData) || "")
          color: row.glyphColor !== "" ? row.glyphColor : Color.accent
          horizontalAlignment: Text.AlignHCenter
          font.family: row.glyphFamily !== "" ? row.glyphFamily : Style.font.family
          font.pixelSize: Typography.body
        }

        Column {
          anchors.left: rowIcon.right
          anchors.leftMargin: Style.space(8)
          anchors.right: parent.right
          anchors.rightMargin: Style.space(12)
          anchors.verticalCenter: parent.verticalCenter
          spacing: 0

          Text {
            width: parent.width
            textFormat: Text.StyledText
            text: Highlight.markup(row.modelData.name, Highlight.parseSpans(row.modelData.nameSpans), Color.accent)
            color: row.current ? Color.accent : Color.bar.text
            elide: Text.ElideRight
            font.family: Style.font.family
            font.pixelSize: Typography.body
            font.weight: row.current ? Font.DemiBold : Font.Normal
          }

          Text {
            width: parent.width
            textFormat: Text.StyledText
            text: Highlight.markup(row.modelData.relative, Highlight.parseSpans(row.modelData.relativeSpans), Color.accent)
            color: card.secondaryTextColor
            elide: Text.ElideLeft
            font.family: Style.font.family
            font.pixelSize: Typography.caption
          }
        }

        MouseArea {
          id: rowPointer
          anchors.fill: parent
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onClicked: {
            list.currentIndex = row.index
            card.activated(row.index, false)
          }
        }
      }
    }
  }
}
