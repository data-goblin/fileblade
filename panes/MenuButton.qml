import QtQuick
import qs.Commons
import "../ui" as PluginUi
import "../theme"

Rectangle {
  id: control
  required property var menu
  property string text: ""
  property bool danger: false
  property bool primary: false
  property string swatch: ""
  property string appIcon: ""
  property string shortcut: ""
  property bool menuFocusable: true
  readonly property bool menuHighlighted: control.enabled && (pointer.containsMouse || control.activeFocus)
  signal clicked()

  width: parent ? parent.width : implicitWidth
  implicitHeight: Style.space(29)
  radius: Math.min(Style.cornerRadius, Style.space(4))
  color: control.primary
    ? (pointer.containsMouse ? Color.accent : Util.alpha(Color.accent, 0.84))
    : (control.menuHighlighted ? Color.menu.selectedBackground : "transparent")
  border.width: !control.primary && control.activeFocus ? 1 : 0
  border.color: Color.menu.selectedBorder
  opacity: enabled ? 1.0 : 0.42
  activeFocusOnTab: visible && enabled

  Text {
    textFormat: Text.PlainText
    anchors.left: parent.left
    anchors.leftMargin: Style.space(10) + (control.swatch ? Style.space(19) : (control.appIcon ? Style.space(27) : 0))
    anchors.right: shortcutHint.visible ? shortcutHint.left : parent.right
    anchors.rightMargin: Style.space(10)
    height: parent.height
    text: control.text
    color: control.primary ? Color.background
      : (control.danger ? Color.urgent
        : (control.menuHighlighted ? Color.menu.selectedText : Color.menu.text))
    elide: Text.ElideRight
    verticalAlignment: Text.AlignVCenter
    font.family: Style.font.family
    font.pixelSize: Typography.body
    font.weight: control.primary ? Font.DemiBold : Font.Normal
  }

  Text {
    id: shortcutHint
    textFormat: Text.PlainText
    visible: control.shortcut !== "" && !control.primary
    anchors.right: parent.right
    anchors.rightMargin: Style.space(10)
    anchors.verticalCenter: parent.verticalCenter
    text: control.shortcut
    color: control.menuHighlighted ? Color.menu.selectedText : Util.alpha(Color.menu.text, 0.55)
    font.family: Style.font.family
    font.pixelSize: Typography.caption
  }

  PluginUi.SafeApplicationIcon {
    anchors.left: parent.left
    anchors.leftMargin: Style.space(10)
    anchors.verticalCenter: parent.verticalCenter
    visible: control.appIcon !== ""
    iconName: control.appIcon
    fallbackGlyph: "󰏗"
    fallbackColor: control.menuHighlighted ? Color.bar.text : Color.accent
  }

  Rectangle {
    visible: control.swatch !== ""
    anchors.left: parent.left
    anchors.leftMargin: Style.space(10)
    anchors.verticalCenter: parent.verticalCenter
    width: Style.space(11)
    height: width
    radius: width / 2
    color: control.swatch || "transparent"
    border.width: 1
    border.color: Util.alpha(Color.bar.text, 0.34)
  }

  MouseArea {
    id: pointer
    anchors.fill: parent
    enabled: control.enabled
    hoverEnabled: true
    preventStealing: true
    cursorShape: Qt.PointingHandCursor
    onEntered: control.forceActiveFocus()
    onClicked: control.clicked()
  }

  Keys.onPressed: function(event) {
    if (!control.enabled) return
    if (event.key === Qt.Key_Up || event.key === Qt.Key_Down) {
      control.menu.focusRelative(control, event.key === Qt.Key_Up ? -1 : 1)
      event.accepted = true
    } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_Space) {
      if (!control.menu.actionKeys.isRepeat(event)) control.clicked()
      event.accepted = true
    }
  }
  Keys.onReleased: function(event) { control.menu.actionKeys.release(event) }

  Keys.onTabPressed: function(event) {
    control.menu.focusRelative(control, 1)
    event.accepted = true
  }

  Keys.onBacktabPressed: function(event) {
    control.menu.focusRelative(control, -1)
    event.accepted = true
  }
}
