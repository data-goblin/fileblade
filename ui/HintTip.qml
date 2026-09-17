import QtQuick
import qs.Commons
import qs.Ui
import "../theme"

Item {
  id: tip

  property string title: ""
  property var actions: []
  property var context: []
  property Item anchorItem: null
  property bool revealed: false
  property int maximumWidth: Style.space(220)
  readonly property real edgeGap: Style.space(6)
  readonly property color panelForeground: Color.tooltip.text
  readonly property color panelBackground: Color.tooltip.background
  readonly property color panelBorder: Color.tooltip.border
  readonly property string fontFamily: Style.font.family
  readonly property real fontSize: Typography.bodySmall
  readonly property var panelBorderSpec: Border.localOrSurfaceSpec(
    "tooltip", "border", panelBorder, Color.tooltip.border, Style.normalBorderWidth)

  implicitWidth: content.implicitWidth
  implicitHeight: content.implicitHeight
  width: implicitWidth
  height: implicitHeight
  z: 1000
  enabled: false

  function findSceneRoot(item) {
    var current = item
    while (current && current.parent) current = current.parent
    return current
  }

  function boundsItem() {
    var current = anchorItem
    var clipped = null
    while (current && current !== parent) {
      if (current.clip) clipped = current
      current = current.parent
    }
    return clipped || parent
  }

  function attach() {
    if (!anchorItem) anchorItem = parent
    var stage = tip.findSceneRoot(anchorItem)
    if (stage && parent !== stage) parent = stage
    place()
  }

  function place() {
    if (!anchorItem || !parent) return
    var anchor = anchorItem.mapToItem(parent, 0, 0)
    var bounds = boundsItem()
    var boundsOrigin = bounds ? bounds.mapToItem(parent, 0, 0) : Qt.point(0, 0)
    var left = boundsOrigin.x + edgeGap
    var right = boundsOrigin.x + (bounds ? bounds.width : parent.width) - edgeGap
    var top = boundsOrigin.y + edgeGap
    var bottom = boundsOrigin.y + (bounds ? bounds.height : parent.height) - edgeGap
    var wantedX = anchor.x + (anchorItem.width - width) / 2
    x = Math.max(left, Math.min(wantedX, right - width))
    var above = anchor.y - height - Style.space(4)
    var below = anchor.y + anchorItem.height + Style.space(4)
    y = above >= top ? above : Math.min(below, bottom - height)
  }

  onVisibleChanged: {
    if (visible) {
      place()
      revealTimer.restart()
    } else {
      revealTimer.stop()
      revealed = false
    }
  }
  onImplicitWidthChanged: if (revealed) place()
  onImplicitHeightChanged: if (revealed) place()
  Timer {
    interval: 0
    running: true
    onTriggered: tip.attach()
  }

  Timer {
    id: revealTimer
    interval: 400
    onTriggered: {
      if (!tip.visible) return
      tip.place()
      tip.revealed = true
    }
  }

  BorderSurface {
    anchors.fill: parent
    visible: tip.revealed
    color: tip.panelBackground
    borderSpec: tip.panelBorderSpec
    radius: Style.cornerRadius
  }

  Column {
    id: content
    visible: tip.revealed
    spacing: Style.space(3)
    leftPadding: Border.left(tip.panelBorderSpec) + Style.spacing.controlPaddingX
    rightPadding: Border.right(tip.panelBorderSpec) + Style.spacing.controlPaddingX
    topPadding: Border.top(tip.panelBorderSpec) + Style.spacing.controlPaddingY
    bottomPadding: Border.bottom(tip.panelBorderSpec) + Style.spacing.controlPaddingY

    Text {
      textFormat: Text.PlainText
      text: tip.title
      width: Math.min(implicitWidth, tip.maximumWidth - content.leftPadding - content.rightPadding)
      wrapMode: Text.WordWrap
      color: tip.panelForeground
      font.family: tip.fontFamily
      font.pixelSize: tip.fontSize
    }

    Repeater {
      model: tip.actions

      delegate: HintLine {
        required property var modelData
        button: String(modelData.button || "")
        shortcut: String(modelData.shortcut || "")
        glyph: String(modelData.glyph || "")
        text: String(modelData.text || "")
        keyGlyph: true
        foreground: tip.panelForeground
      }
    }

    Rectangle {
      visible: tip.context.length > 0
      width: parent.width - parent.leftPadding - parent.rightPadding
      height: 1
      color: Util.alpha(tip.panelForeground, 0.16)
    }

    Repeater {
      model: tip.context

      delegate: HintLine {
        required property var modelData
        glyph: String(modelData.glyph || "")
        shortcut: String(modelData.shortcut || "")
        text: String(modelData.text || "")
        foreground: tip.panelForeground
      }
    }
  }
}
