import QtQuick
import QtQuick.Effects
import qs.Commons
import qs.Ui
import "../lib/Agents.js" as Agents
import "../theme"

Item {
  id: icon

  property string agentId: ""
  property bool applied: false
  property bool interactive: true
  readonly property var agent: Agents.byId(agentId)
  readonly property string label: Agents.label(agentId)
  readonly property bool hasMark: !!agent && !!agent.svg && mark.status === Image.Ready
  readonly property bool hot: pointer.containsMouse
  readonly property color appliedColor: Color.accent
  readonly property color inactiveColor: Color.muted
  readonly property color tone: applied ? appliedColor : (hot ? Qt.lighter(inactiveColor, 1.35) : inactiveColor)

  signal clicked()

  implicitWidth: Style.space(16)
  implicitHeight: Style.space(16)

  Image {
    id: mark
    anchors.fill: parent
    anchors.margins: Style.space(1)
    visible: false
    source: icon.agent && icon.agent.svg ? Qt.resolvedUrl("agents/" + icon.agent.svg) : ""
    sourceSize.width: Style.space(32)
    sourceSize.height: Style.space(32)
    fillMode: Image.PreserveAspectFit
    smooth: true
  }

  MultiEffect {
    anchors.fill: mark
    source: mark
    visible: icon.hasMark
    colorization: 1
    colorizationColor: icon.tone
  }

  Text {
    textFormat: Text.PlainText
    anchors.centerIn: parent
    visible: !icon.hasMark
    text: icon.agent && icon.agent.glyph ? icon.agent.glyph : (icon.label.charAt(0) || "?")
    color: icon.tone
    font.family: Style.font.family
    font.pixelSize: Typography.title
  }

  MouseArea {
    id: pointer
    anchors.fill: parent
    anchors.margins: -Style.space(2)
    enabled: icon.interactive
    hoverEnabled: icon.interactive
    cursorShape: Qt.PointingHandCursor
    onClicked: icon.clicked()
  }

  HintTip {
    visible: pointer.containsMouse
    title: icon.label
    actions: icon.interactive ? [{ button: "left", text: icon.applied ? "Remove" : "Apply" }] : []
    context: [{ glyph: icon.applied ? "󰄬" : "󰅖", text: icon.applied ? "Applied" : "Not applied" }]
  }
}
