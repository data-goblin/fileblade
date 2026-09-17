import QtQuick
import qs.Commons
import "../theme"

Rectangle {
  id: root

  property bool image: false
  property bool imageAllowed: false
  property url imageSource: ""
  property string imageMessage: ""
  property bool imageLoading: false
  property string imageError: ""
  property bool textPreview: false
  property string trustedTextMarkup: ""
  property bool loading: false
  property string error: ""
  property color surfaceColor: Color.bar.background
  readonly property bool hovered: previewHover.hovered

  function resetScroll() {
    textScroller.contentX = 0
    textScroller.contentY = 0
  }

  function clamp(value, contentSize, viewportSize) {
    return Math.max(0, Math.min(Math.max(0, contentSize - viewportSize), value))
  }

  function scrollPreview(event) {
    var pixel = event.pixelDelta || Qt.point(0, 0)
    var angle = event.angleDelta || Qt.point(0, 0)
    var step = Style.space(34)
    var dx = Number(pixel.x) || Number(angle.x) / 120 * step
    var dy = Number(pixel.y) || Number(angle.y) / 120 * step
    if ((event.modifiers & Qt.ShiftModifier) && !dx) {
      dx = dy
      dy = 0
    }
    textScroller.contentX = clamp(textScroller.contentX - dx, textScroller.contentWidth, textScroller.width)
    textScroller.contentY = clamp(textScroller.contentY - dy, textScroller.contentHeight, textScroller.height)
    event.accepted = true
  }

  readonly property color boxColor: Qt.tint(surfaceColor, Util.alpha(Color.bar.text, 0.035))

  visible: image || textPreview
  height: visible ? Math.min(Style.space(220), Math.max(Style.space(126), width * 0.62)) : 0
  radius: Math.min(Style.cornerRadius, Style.space(5))
  color: boxColor
  border.width: 1
  border.color: Util.alpha(Color.bar.text, 0.15)
  clip: true

  Image {
    id: previewImage
    anchors.fill: parent
    anchors.margins: Style.space(7)
    visible: root.image && root.imageAllowed
    source: root.imageSource
    asynchronous: true
    cache: true
    retainWhileLoading: true
    fillMode: Image.PreserveAspectFit
    sourceSize.width: Math.max(1, width)
    sourceSize.height: Math.max(1, height)
  }

  Text {
    textFormat: Text.PlainText
    anchors.centerIn: parent
    width: parent.width - Style.space(24)
    visible: root.image && root.imageAllowed && (root.imageError !== "" || previewImage.status === Image.Error || root.imageLoading)
    text: root.imageError !== "" ? root.imageError : (root.imageLoading ? "Rendering preview…" : "Preview unavailable")
    color: root.imageLoading ? Color.muted : Color.urgent
    horizontalAlignment: Text.AlignHCenter
    wrapMode: Text.WordWrap
    font.family: Style.font.family
    font.pixelSize: Typography.bodySmall
  }

  Text {
    textFormat: Text.PlainText
    anchors.centerIn: parent
    width: parent.width - Style.space(24)
    visible: root.image && !root.imageAllowed
    text: root.imageMessage
    color: Color.muted
    horizontalAlignment: Text.AlignHCenter
    wrapMode: Text.WordWrap
    font.family: Style.font.family
    font.pixelSize: Typography.bodySmall
  }

  Flickable {
    id: textScroller
    anchors.fill: parent
    anchors.margins: Style.space(7)
    visible: root.textPreview
    contentWidth: width
    contentHeight: Math.max(height, previewText.implicitHeight + Style.space(18))
    clip: true
    interactive: false
    boundsBehavior: Flickable.StopAtBounds

    Text {
      id: previewText
      width: textScroller.width
      textFormat: Text.RichText
      text: root.trustedTextMarkup
      color: Color.bar.text
      font.family: Style.font.family
      font.pixelSize: Typography.caption
      wrapMode: Text.Wrap
    }
  }

  Text {
    textFormat: Text.PlainText
    anchors.centerIn: parent
    width: parent.width - Style.space(24)
    visible: root.textPreview && (root.loading || root.error !== "" || (!root.loading && root.trustedTextMarkup === ""))
    text: root.loading ? "Loading preview…" : (root.error !== "" ? root.error : "Empty file")
    color: root.error !== "" ? Color.urgent : Color.muted
    horizontalAlignment: Text.AlignHCenter
    wrapMode: Text.WordWrap
    font.family: Style.font.family
    font.pixelSize: Typography.bodySmall
  }

  HoverHandler { id: previewHover }

  WheelHandler {
    enabled: root.textPreview
    target: null
    blocking: true
    onWheel: function(event) { root.scrollPreview(event) }
  }

  ScrollEdgeFade {
    anchors.fill: textScroller
    flickable: textScroller
    surfaceColor: root.boxColor
    fadeHeight: Style.space(34)
    z: 2
  }
}
