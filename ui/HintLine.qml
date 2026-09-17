import QtQuick
import qs.Commons
import "../theme"

Item {
  id: line

  property string button: ""
  property string shortcut: ""
  property string glyph: ""
  property string text: ""
  property color foreground: Color.bar.text
  property real leadWidth: 0
  property bool keyGlyph: false
  property bool wrap: false
  property real gap: Style.space(6)
  readonly property color dim: Util.alpha(foreground, 0.62)
  readonly property bool hasLead: button !== "" || shortcut !== "" || glyph !== ""

  implicitWidth: label.x + label.implicitWidth
  implicitHeight: Math.max(lead.implicitHeight, label.implicitHeight, mouse.implicitHeight)

  MouseGlyph {
    id: mouse
    visible: line.button !== ""
    anchors.verticalCenter: parent.verticalCenter
    button: line.button
    color: line.dim
  }

  Text {
    id: lead
    textFormat: Text.PlainText
    visible: line.button === "" && line.hasLead
    anchors.verticalCenter: parent.verticalCenter
    width: line.leadWidth > 0 ? Math.min(line.leadWidth, line.width) : implicitWidth
    text: line.shortcut !== "" ? (line.keyGlyph ? "󰌌  " + line.shortcut : line.shortcut) : line.glyph
    color: line.dim
    elide: Text.ElideRight
    font.family: Style.font.family
    font.pixelSize: Typography.caption
    font.weight: line.shortcut !== "" ? Font.DemiBold : Font.Normal
    font.letterSpacing: line.shortcut !== "" ? 0.3 : 0
  }

  Text {
    id: label
    textFormat: Text.PlainText
    anchors.verticalCenter: parent.verticalCenter
    x: !line.hasLead ? 0
      : line.leadWidth > 0 ? line.leadWidth + line.gap
      : (line.button !== "" ? mouse.implicitWidth : lead.implicitWidth) + line.gap
    width: Math.max(0, line.width - x)
    text: line.text
    color: line.dim
    elide: line.wrap ? Text.ElideNone : Text.ElideMiddle
    wrapMode: line.wrap ? Text.WordWrap : Text.NoWrap
    font.family: Style.font.family
    font.pixelSize: Typography.caption
  }
}
