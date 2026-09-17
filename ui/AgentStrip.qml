import QtQuick
import qs.Commons
import qs.Ui
import "../lib/Agents.js" as Agents
import "../theme"

Row {
  id: strip

  property var installed: []
  property var applied: []
  property bool interactive: true
  readonly property var agents: Agents.ordered(installed)
  readonly property bool allApplied: agents.length > 0 && agents.every(function(id) { return strip.isApplied(id) })

  signal toggled(string agentId, bool on)
  signal allRequested(bool on)

  spacing: Style.space(3)

  function isApplied(agentId) {
    var wanted = String(agentId)
    var count = applied && applied.length !== undefined ? Number(applied.length) : 0
    for (var i = 0; i < count; i++) if (String(applied[i]) === wanted) return true
    return false
  }

  Item {
    width: Style.space(16)
    height: Style.space(16)
    visible: strip.interactive && strip.agents.length > 1

    Text {
      textFormat: Text.PlainText
      anchors.centerIn: parent
      text: Agents.ALL_GLYPH
      color: strip.allApplied ? Color.accent : (allPointer.containsMouse ? Qt.lighter(Color.muted, 1.35) : Color.muted)
      font.family: Style.font.family
      font.pixelSize: Typography.title
    }

    MouseArea {
      id: allPointer
      anchors.fill: parent
      anchors.margins: -Style.space(2)
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: strip.allRequested(!strip.allApplied)
    }

    HintTip {
      visible: allPointer.containsMouse
      title: "All agents"
      actions: [{ button: "left", text: strip.allApplied ? "Remove from all" : "Apply to all" }]
      context: strip.allApplied ? [{ glyph: "󰄬", text: "Applied" }] : []
    }
  }

  Repeater {
    model: strip.agents

    delegate: AgentIcon {
      required property var modelData
      agentId: String(modelData)
      applied: strip.isApplied(modelData)
      interactive: strip.interactive
      onClicked: strip.toggled(agentId, !applied)
    }
  }
}
