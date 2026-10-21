import QtQuick
import qs.Commons
import qs.Commons as Commons

Item {
  id: bar

  property real fraction: -1
  property color fillColor: Commons.Color.accent
  property color trackColor: Util.alpha(Commons.Color.bar.text, 0.10)
  property string tipTitle: ""
  property var tipContext: []
  property var tipActions: []
  property bool activatable: false
  readonly property bool filled: fraction >= 0
  readonly property real drawnFraction: Math.max(0, Math.min(1, fraction))

  signal activated()

  anchors.left: parent ? parent.left : undefined
  anchors.right: parent ? parent.right : undefined
  anchors.bottom: parent ? parent.bottom : undefined
  implicitHeight: Style.space(2)
  height: implicitHeight
  Accessible.role: Accessible.ProgressBar
  Accessible.name: tipTitle

  Rectangle {
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.bottom: parent.bottom
    height: 1
    color: bar.trackColor
  }

  Rectangle {
    id: fill
    anchors.left: parent.left
    anchors.bottom: parent.bottom
    height: parent.height
    width: bar.filled ? Math.round(parent.width * bar.drawnFraction) : 0
    visible: bar.filled
    color: bar.fillColor
    Behavior on width {
      enabled: fill.visible
      NumberAnimation { duration: 260; easing.type: Easing.OutCubic }
    }
  }

  MouseArea {
    id: pointer
    anchors.fill: parent
    anchors.bottomMargin: -Style.space(4)
    enabled: bar.filled
    hoverEnabled: true
    cursorShape: bar.activatable ? Qt.PointingHandCursor : Qt.ArrowCursor
    acceptedButtons: bar.activatable ? Qt.LeftButton : Qt.NoButton
    onClicked: if (bar.activatable) bar.activated()
  }

  HintTip {
    visible: pointer.containsMouse && bar.filled && bar.tipTitle !== ""
    title: bar.tipTitle
    actions: bar.tipActions
    context: bar.tipContext
  }
}
