import QtQuick
import qs.Commons
import "../theme"
import qs.Commons as Commons

Item {
  id: tile

  property var item: null
  property var thumbnails: null
  property int edge: 256
  property bool current: false
  property bool selected: false
  property bool showLabel: false
  property int labelHeight: 0
  property bool dragEnabled: true
  property string source: ""
  property string error: ""
  property bool loading: false
  property int token: 0
  property var pending: null
  readonly property bool hovered: hover.hovered
  readonly property string path: item ? String(item.path || "") : ""
  readonly property string stamp: item ? String(item.stamp || "") : ""
  readonly property string label: item ? String(item.name || "") : ""
  readonly property real square: Math.max(0, height - labelHeight)

  signal clicked(var mouse)
  signal doubleClicked()
  signal dragBegan(point scene, int modifiers)
  signal dragMoved(point scene, int modifiers)
  signal dragEnded(bool canceled)

  function releaseRequest() {
    token++
    var request = pending
    pending = null
    if (request && request.cache && typeof request.cache.cancel === "function") request.cache.cancel(request.callback)
  }

  function fetch() {
    releaseRequest()
    var mine = token
    source = ""
    error = ""
    if (!thumbnails || path === "") {
      loading = false
      return
    }
    loading = true
    var request = { cache: thumbnails, callback: null }
    request.callback = function(result) {
      if (!tile || mine !== tile.token) return
      tile.pending = null
      tile.loading = false
      if (result && result.ok) tile.source = String(result.url)
      else tile.error = String(result && result.error || "Preview unavailable")
    }
    pending = request
    thumbnails.request(path, stamp, edge, request.callback)
  }

  onPathChanged: fetch()
  onStampChanged: fetch()
  onEdgeChanged: fetch()
  onThumbnailsChanged: fetch()
  Component.onCompleted: fetch()
  Component.onDestruction: releaseRequest()

  Rectangle {
    id: frame
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.top: parent.top
    height: tile.square
    radius: Math.min(Style.cornerRadius, Style.space(5))
    color: Util.alpha(Commons.Color.bar.text, tile.hovered ? 0.09 : 0.05)
    border.width: tile.current ? Style.space(2) : (tile.selected ? 1 : 0)
    border.color: tile.current ? Commons.Color.accent : Util.alpha(Commons.Color.accent, 0.7)
    clip: true

    Image {
      id: picture
      anchors.fill: parent
      anchors.margins: Style.space(3)
      source: tile.source
      visible: tile.source !== ""
      asynchronous: true
      cache: true
      smooth: true
      mipmap: true
      fillMode: Image.PreserveAspectFit
      sourceSize.width: tile.edge
      sourceSize.height: tile.edge
      opacity: status === Image.Ready ? 1 : 0
      Behavior on opacity { NumberAnimation { duration: 120 } }
    }

    Text {
      textFormat: Text.PlainText
      anchors.centerIn: parent
      width: parent.width - Style.space(8)
      visible: tile.source === "" || picture.status === Image.Error
      text: tile.error !== "" || picture.status === Image.Error ? "󰋩" : (tile.loading ? "…" : "")
      color: tile.error !== "" ? Commons.Color.urgent : Commons.Color.muted
      horizontalAlignment: Text.AlignHCenter
      elide: Text.ElideRight
      font.family: Style.font.family
      font.pixelSize: Typography.body
    }
  }

  Text {
    textFormat: Text.PlainText
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.top: frame.bottom
    anchors.leftMargin: Style.space(2)
    anchors.rightMargin: Style.space(2)
    height: tile.labelHeight
    visible: tile.showLabel && tile.labelHeight > 0
    text: tile.label
    color: tile.current ? Commons.Color.accent : Commons.Color.muted
    elide: Text.ElideMiddle
    verticalAlignment: Text.AlignVCenter
    font.family: Style.font.family
    font.pixelSize: Typography.caption
  }

  HoverHandler { id: hover }

  MouseArea {
    id: pointer
    anchors.fill: parent
    acceptedButtons: Qt.LeftButton | Qt.RightButton
    cursorShape: drag.active ? Qt.ClosedHandCursor : Qt.PointingHandCursor
    onClicked: function(mouse) { tile.clicked(mouse) }
    onDoubleClicked: function(mouse) {
      if (mouse.button === Qt.LeftButton) tile.doubleClicked()
      mouse.accepted = true
    }
  }

  DragHandler {
    id: drag
    target: null
    acceptedButtons: Qt.LeftButton
    enabled: tile.dragEnabled && tile.path !== ""
    cursorShape: active ? Qt.ClosedHandCursor : Qt.PointingHandCursor
    property bool canceled: false
    onActiveChanged: {
      if (active) {
        canceled = false
        tile.dragBegan(centroid.scenePosition, centroid.modifiers)
        return
      }
      tile.dragEnded(canceled)
    }
    onCanceled: canceled = true
    onCentroidChanged: if (active) tile.dragMoved(centroid.scenePosition, centroid.modifiers)
  }
}
