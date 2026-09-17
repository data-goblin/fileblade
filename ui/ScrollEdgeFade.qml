import QtQuick
import qs.Commons
import qs.Ui
import "../theme"

Item {
  id: root

  property var flickable: null
  property color surfaceColor: Color.bar.background
  property int fadeHeight: Style.space(48)
  property real threshold: 1
  readonly property real beginningY: flickable ? Number(flickable.originY) || 0 : 0
  readonly property real endingY: flickable
    ? beginningY + Math.max(0, Number(flickable.contentHeight) - Number(flickable.height))
    : 0
  readonly property bool scrollable: !!flickable && flickable.visible && flickable.height > 0
    && flickable.contentHeight > flickable.height + threshold
  readonly property bool hasAbove: scrollable && flickable.contentY > beginningY + threshold
  readonly property bool hasBelow: scrollable && flickable.contentY < endingY - threshold

  function page(direction) {
    if (!flickable) return
    var distance = Math.max(Style.space(120), flickable.height * 0.66)
    flickable.contentY = Math.max(beginningY, Math.min(endingY, flickable.contentY + direction * distance))
  }

  visible: hasAbove || hasBelow

  Item {
    id: topFade
    anchors.top: parent.top
    anchors.left: parent.left
    anchors.right: parent.right
    height: Math.min(root.fadeHeight, root.height)
    visible: root.hasAbove

    Rectangle {
      anchors.fill: parent
      gradient: Gradient {
        GradientStop { position: 0.0; color: root.surfaceColor }
        GradientStop { position: 0.32; color: Util.alpha(root.surfaceColor, 0.84) }
        GradientStop { position: 1.0; color: Util.alpha(root.surfaceColor, 0.0) }
      }
    }

    Text {
      textFormat: Text.PlainText
      anchors.horizontalCenter: parent.horizontalCenter
      anchors.top: parent.top
      anchors.topMargin: Style.space(5)
      text: "⌃"
      color: topPointer.containsMouse ? Color.accent : Color.bar.text
      font.family: Style.font.family
      font.pixelSize: Typography.title
      font.weight: Font.DemiBold

      MouseArea {
        id: topPointer
        anchors.fill: parent
        anchors.margins: -Style.space(7)
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.page(-1)
      }

      HintTip {
        visible: topPointer.containsMouse
        title: "More above"
        actions: [{ button: "left", text: "Page up" }]
      }
    }
  }

  Item {
    id: bottomFade
    anchors.bottom: parent.bottom
    anchors.left: parent.left
    anchors.right: parent.right
    height: Math.min(root.fadeHeight, root.height)
    visible: root.hasBelow

    Rectangle {
      anchors.fill: parent
      gradient: Gradient {
        GradientStop { position: 0.0; color: Util.alpha(root.surfaceColor, 0.0) }
        GradientStop { position: 0.68; color: Util.alpha(root.surfaceColor, 0.84) }
        GradientStop { position: 1.0; color: root.surfaceColor }
      }
    }

    Text {
      textFormat: Text.PlainText
      anchors.horizontalCenter: parent.horizontalCenter
      anchors.bottom: parent.bottom
      anchors.bottomMargin: Style.space(5)
      text: "⌄"
      color: bottomPointer.containsMouse ? Color.accent : Color.bar.text
      font.family: Style.font.family
      font.pixelSize: Typography.title
      font.weight: Font.DemiBold

      MouseArea {
        id: bottomPointer
        anchors.fill: parent
        anchors.margins: -Style.space(7)
        hoverEnabled: true
        cursorShape: Qt.PointingHandCursor
        onClicked: root.page(1)
      }

      HintTip {
        visible: bottomPointer.containsMouse
        title: "More below"
        actions: [{ button: "left", text: "Page down" }]
      }
    }
  }
}
