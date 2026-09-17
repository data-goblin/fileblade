import QtQuick
import QtQuick.Controls
import qs.Commons
import "../theme"

TextField {
  id: field

  property var context: null
  property var service: context ? context.service("files") : null
  readonly property bool autoHide: !!service && service.autoHideSearch === true
  property bool revealed: false
  function reveal() {
    revealed = true
    forceActiveFocus()
    selectAll()
  }
  onActiveFocusChanged: if (!activeFocus) revealed = false
  visible: !autoHide || revealed || activeFocus

  property string glyph: "󰍉"
  property string prompt: "Search..."
  property bool showOptions: false
  property bool showDeepOption: false
  property bool caseSensitive: false
  property bool regex: false
  property bool deep: false
  signal dismissed()
  signal deepToggled(bool deep)
  signal historyStepped(int delta)
  property bool browsingHistory: false
  signal advanced()
  signal cleared()
  signal optionsToggled(bool caseSensitive, bool regex)

  function chipSpec(index) {
    if (index === 0) return { key: "case", label: "Aa", title: "Match case", active: caseSensitive }
    if (index === 1) return { key: "regex", label: ".*", title: "Regular expression", active: regex }
    return { key: "deep", label: "fzf", title: "Search across the whole root and rank fuzzy path matches", shortcut: "Ctrl+F", active: deep }
  }

  function setOptions(nextCase, nextRegex) {
    var changed = caseSensitive !== !!nextCase || regex !== !!nextRegex
    caseSensitive = !!nextCase
    regex = !!nextRegex
    if (changed) optionsToggled(caseSensitive, regex)
  }

  height: visible ? Style.space(32) : 0
  leftPadding: Style.space(28)
  rightPadding: (text !== "" ? Style.space(28) : Style.space(8)) + (showOptions ? options.width + Style.space(4) : 0)
  selectByMouse: true
  color: Color.bar.text
  selectionColor: Util.alpha(Color.accent, 0.38)
  selectedTextColor: Color.bar.text
  placeholderText: field.prompt
  placeholderTextColor: Color.muted
  font.family: Style.font.family
  font.pixelSize: Typography.body

  Keys.onEscapePressed: function(event) {
    field.dismissed()
    event.accepted = true
  }

  Keys.onDownPressed: function(event) {
    if (field.browsingHistory) field.historyStepped(-1)
    else field.advanced()
    event.accepted = true
  }

  Keys.onUpPressed: function(event) {
    field.historyStepped(1)
    event.accepted = true
  }

  background: Rectangle {
    color: Util.alpha(Color.bar.text, field.activeFocus ? 0.10 : 0.06)
    radius: Math.min(Style.cornerRadius, Style.space(4))
  }

  Text {
    textFormat: Text.PlainText
    anchors.left: parent.left
    anchors.leftMargin: Style.space(9)
    anchors.verticalCenter: parent.verticalCenter
    text: field.glyph
    color: field.activeFocus ? Color.accent : Color.muted
    font.family: Style.font.family
    font.pixelSize: Typography.body
  }

  Row {
    id: options
    visible: field.showOptions
    anchors.right: parent.right
    anchors.rightMargin: field.text !== "" ? Style.space(26) : Style.space(6)
    anchors.verticalCenter: parent.verticalCenter
    spacing: Style.space(2)

    Repeater {
      model: field.showDeepOption ? 3 : 2

      delegate: Rectangle {
        id: toggle
        required property int index
        readonly property var modelData: field.chipSpec(index)
        width: toggle.modelData.key === "deep" ? Style.space(30) : Style.space(22)
        height: Style.space(20)
        radius: Math.min(Style.cornerRadius, Style.space(4))
        color: modelData.active ? Util.alpha(Color.accent, 0.22) : (togglePointer.containsMouse ? Util.alpha(Color.bar.text, 0.10) : "transparent")
        border.width: modelData.active ? 1 : 0
        border.color: Util.alpha(Color.accent, 0.6)

        Text {
          textFormat: Text.PlainText
          anchors.centerIn: parent
          text: toggle.modelData.label
          color: toggle.modelData.active ? Color.accent : (togglePointer.containsMouse ? Color.bar.text : Color.muted)
          font.family: Style.font.family
          font.pixelSize: Typography.caption
          font.bold: toggle.modelData.active
        }

        MouseArea {
          id: togglePointer
          anchors.fill: parent
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onClicked: {
            var key = String(toggle.modelData.key)
            field.forceActiveFocus()
            if (key === "case") field.setOptions(!field.caseSensitive, field.regex)
            else if (key === "regex") field.setOptions(field.caseSensitive, !field.regex)
            else field.deepToggled(!field.deep)
          }
        }

        HintTip {
          visible: togglePointer.containsMouse
          title: toggle.modelData.title
          actions: [{ button: "left", text: toggle.modelData.active ? "Turn off" : "Turn on" }].concat(toggle.modelData.shortcut ? [{ shortcut: toggle.modelData.shortcut }] : [])
        }
      }
    }
  }

  Text {
    textFormat: Text.PlainText
    visible: field.text !== ""
    anchors.right: parent.right
    anchors.rightMargin: Style.space(9)
    anchors.verticalCenter: parent.verticalCenter
    text: "×"
    color: clearPointer.containsMouse || field.activeFocus ? Color.bar.text : Color.muted
    font.family: Style.font.family
    font.pixelSize: Typography.title

    MouseArea {
      id: clearPointer
      anchors.fill: parent
      anchors.margins: -Style.space(4)
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: {
        field.text = ""
        field.cleared()
        field.forceActiveFocus()
      }
    }
  }
}
