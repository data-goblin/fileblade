import QtQuick
import qs.Commons
import qs.Ui
import "../theme"

Item {
  id: corner

  required property string glyph
  required property string tip
  property bool active: false

  signal activated()

  implicitWidth: Style.space(18)
  implicitHeight: Style.space(18)

  Text {
    textFormat: Text.PlainText
    anchors.centerIn: parent
    text: corner.glyph
    color: corner.active || pointer.containsMouse ? Color.accent : Color.muted
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

  property var tipActions: []
  property var tipContext: []

  PanelToolTip {
    visible: (pointer.containsMouse) && tipActions.length === 0
    text: tip
  }

  HintTip {
    visible: (pointer.containsMouse) && tipActions.length > 0
    title: tip
    actions: tipActions
    context: tipContext
  }
}
