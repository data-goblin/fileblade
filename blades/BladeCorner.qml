import QtQuick
import qs.Commons
import qs.Ui
import "../ui" as PluginUi
import "../theme"

Item {
  id: corner

  required property string glyph
  required property string tip

  property var tipActions: []

  signal activated()

  implicitWidth: Style.space(18)
  implicitHeight: Style.space(18)

  Text {
    textFormat: Text.PlainText
    anchors.centerIn: parent
    text: corner.glyph
    color: pointer.containsMouse ? Color.accent : Color.muted
    font.family: Style.font.family
    font.pixelSize: Typography.caption
  }

  MouseArea {
    id: pointer
    anchors.fill: parent
    anchors.margins: -Style.space(3)
    hoverEnabled: true
    cursorShape: Qt.PointingHandCursor
    onClicked: corner.activated()
  }

  PanelToolTip {
    visible: pointer.containsMouse && corner.tipActions.length === 0
    text: corner.tip
  }

  PluginUi.HintTip {
    visible: pointer.containsMouse && corner.tipActions.length > 0
    title: corner.tip
    actions: corner.tipActions
  }
}
