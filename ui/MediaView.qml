import QtQuick
import qs.Commons
import "../lib/ImageGallery.js" as ImageGallery
import "../lib/DragPlan.js" as DragPlan
import "../lib/MediaDates.js" as MediaDates
import "../lib/MediaBins.js" as MediaBins
import "../theme"
import qs.Commons as Commons

FocusScope {
  id: media

  required property var controller
  required property var pane
  property var items: []
  property var sorts: []
  readonly property var dateRecords: MediaBins.ordered(items, sorts)
  readonly property var rows: dateRecords.map(function(record) { return record.item })
  property int sizeStep: ImageGallery.DEFAULT_STEP
  property bool busy: false
  property string message: ""
  property string anchorPath: ""
  property real dragScrollStep: 0
  property bool ownsDrag: false
  readonly property int count: rows.length
  readonly property var calendarRule: MediaDates.localeRule(Qt.locale().name, Qt.locale().firstDayOfWeek)
  readonly property alias timeline: timeline
  readonly property bool contentActiveFocus: activeFocus && !timeline.activeFocus
  readonly property var model: ({ count: rows.length, get: function(index) { return media.rows[index] } })
  property alias currentIndex: grid.currentIndex
  readonly property alias flickable: grid
  readonly property int columns: Math.max(1, Math.floor(grid.width / grid.cellWidth))
  readonly property real contentHeight: grid.contentHeight
  readonly property real originY: grid.originY
  property alias contentY: grid.contentY
  readonly property int cell: Math.max(1, Math.min(Style.space(ImageGallery.cellFor(sizeStep)), grid.width))
  readonly property int labelHeight: sizeStep >= 3 ? Style.space(16) : 0
  signal keyPressed(var event)
  signal sizeStepRequested(int step)

  function indexOfPath(path) {
    for (var i = 0; i < rows.length; i++) if (rows[i].path === path) return i
    return -1
  }

  function positionViewAtIndex(index, mode) { grid.positionViewAtIndex(index, mode) }
  function itemAtIndex(index) { return grid.itemAtIndex(index) }

  function rememberAnchor() {
    var index = grid.indexAt(1, grid.contentY + 1)
    if (index >= 0 && index < rows.length) anchorPath = rows[index].path
  }

  function restoreAnchor(path) {
    grid.forceLayout()
    currentIndex = indexOfPath(controller.selectedPath)
    var index = indexOfPath(path || anchorPath)
    if (index >= 0) grid.positionViewAtIndex(index, GridView.Beginning)
  }

  function choose(index, modifiers, preserve) {
    pane.reconcileMediaSelection()
    pane.selectIndex(media, false, index, preserve && controller.isSelected(rows[index].path) ? "keep" : pane.selectionMode(modifiers))
  }

  function point(scene) {
    var window = pane.hostWindow
    return { x: pane.originX() + scene.x, y: (pane.context ? Number(pane.context.surfaceOriginY) || 0 : 0) + scene.y,
      outside: !window || !window.containsScenePoint(scene.x, scene.y) }
  }

  function openMenu(index, mode, x, y, keyboard) {
    if (index < 0 || index >= count || !pane.mediaAllows(mode || "actions")) return
    pane.reconcileMediaSelection()
    choose(index, Qt.NoModifier, true)
    var position = media.mapToItem(pane.hostWindow.contentItem, x, y)
    controller.openActionMenu(mode || "actions", pane.targetScreen(), position.x + pane.originX(), position.y, undefined,
      { edge: pane.context ? pane.context.edge : "left", keyboard: !!keyboard })
  }

  function reflow() {
    var saved = anchorPath
    Qt.callLater(function() { media.restoreAnchor(saved) })
  }

  function cancelOwnedDrag() {
    dragScrollStep = 0
    if (ownsDrag) controller.dropWheel.cancelDrag()
    ownsDrag = false
  }

  onVisibleChanged: if (!visible) cancelOwnedDrag()
  Component.onDestruction: cancelOwnedDrag()
  onSizeStepChanged: reflow()
  onWidthChanged: reflow()
  onRowsChanged: {
    currentIndex = indexOfPath(controller.selectedPath)
    reflow()
  }

  Keys.priority: Keys.AfterItem
  Keys.onPressed: function(event) { keyPressed(event) }
  Keys.onReleased: function(event) { if (controller.dropWheel.handleDragKeyRelease(event)) event.accepted = true }

  ThumbnailCache { id: mediaThumbnails; files: media.visible ? media.controller : null }

  GridView {
    id: grid
    anchors.left: parent.left
    anchors.right: timeline.left
    anchors.top: parent.top
    anchors.bottom: parent.bottom
    anchors.margins: Style.space(6)
    anchors.topMargin: timeline.headerHeight + Style.space(6)
    model: media.rows
    cellWidth: Math.min(width, media.cell + Style.space(4))
    cellHeight: media.cell + media.labelHeight + Style.space(4)
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    cacheBuffer: cellHeight
    currentIndex: -1
    highlightFollowsCurrentItem: false
    onContentYChanged: if (!moving && !flicking) media.rememberAnchor()
    onMovementEnded: media.rememberAnchor()

    delegate: ImageTile {
      id: tile
      required property int index
      required property var modelData
      width: media.cell
      height: media.cell + media.labelHeight
      item: modelData
      thumbnails: media.visible && y + height > grid.contentY && y < grid.contentY + grid.height
        ? mediaThumbnails : null
      edge: ImageGallery.thumbnailEdge(media.sizeStep)
      current: index === media.currentIndex
      dragEnabled: media.pane.mediaAllows("copy") && media.pane.mediaAllows("cut")
      selected: !!media.controller.selectedPathLookup[String(modelData.path)]
      showLabel: media.labelHeight > 0
      labelHeight: media.labelHeight
      property point dragPosition: Qt.point(0, 0)
      property int dragModifiers: 0
      Drag.active: false
      Drag.source: tile
      Drag.keys: ["fileblade-entry"]
      Drag.hotSpot: dragPosition
      Drag.supportedActions: Qt.CopyAction | Qt.MoveAction
      Drag.proposedAction: dragModifiers & Qt.ControlModifier ? Qt.CopyAction : Qt.MoveAction
      Drag.mimeData: ({ "text/uri-list": media.controller.selectionUris(media.controller.dropWheel.dragPaths) })
      function openMenu(mode, x, y, keyboard) {
        var point = tile.mapToItem(media, x, y)
        media.openMenu(index, mode, point.x, point.y, keyboard)
      }
      onClicked: function(mouse) {
        media.forceActiveFocus()
        media.choose(index, mouse.modifiers, mouse.button === Qt.RightButton)
        if (mouse.button === Qt.RightButton) openMenu("actions", mouse.x, mouse.y, false)
      }
      onDoubleClicked: media.pane.activateIndex(media, false, index, true)
      onDragBegan: function(scene, modifiers) {
        media.forceActiveFocus()
        media.choose(index, modifiers, true)
        if (media.pane.context) media.pane.context.requestFocus("")
        var at = media.point(scene)
        var paths = DragPlan.disjointPaths(media.controller.selectedPaths)
        var entries = DragPlan.entriesForPaths(media.controller.selectedEntries, paths)
        media.ownsDrag = true
        media.controller.dropWheel.beginDrag(paths, entries, media.pane.targetScreen(), !media.pane.context || media.pane.context.docked, at.x, at.y, null, tile)
        dragPosition = tile.mapFromItem(null, scene.x, scene.y)
        dragModifiers = modifiers
        tile.Drag.active = true
      }
      onDragMoved: function(scene, modifiers) {
        if (!tile.Drag.active) { media.dragScrollStep = 0; return }
        var at = media.point(scene)
        media.controller.dropWheel.updateDrag(at.x, at.y, at.outside, modifiers)
        dragPosition = tile.mapFromItem(null, scene.x, scene.y)
        dragModifiers = modifiers
        var fraction = grid.mapFromItem(null, scene.x, scene.y).y / Math.max(1, grid.height)
        media.dragScrollStep = fraction < 0.08 ? -8 : (fraction > 0.92 ? 8 : 0)
      }
      onDragEnded: function(canceled) {
        media.dragScrollStep = 0
        media.ownsDrag = false
        if (tile.Drag.active) {
          if (canceled) tile.Drag.cancel()
          else tile.Drag.drop()
        }
        if (canceled) media.controller.dropWheel.cancelDrag()
        else media.controller.dropWheel.endDrag(undefined, undefined, undefined)
      }
    }
  }

  MediaTimeline {
    id: timeline
    anchors.top: parent.top
    anchors.bottom: parent.bottom
    anchors.right: parent.right
    anchors.rightMargin: Style.space(6)
    headerWidth: media.width - Style.space(12)
    records: media.dateRecords
    calendarRule: media.calendarRule
    columns: media.columns
    pitch: grid.cellHeight
    tileHeight: media.cell + media.labelHeight
    contentY: grid.contentY - grid.originY
    contentHeight: grid.contentHeight
    viewportHeight: grid.height
    showEmptyPeriods: media.pane.mediaShowEmptyPeriods === true
    onSeekRequested: function(position) {
      grid.cancelFlick()
      grid.contentY = grid.originY + position
      media.rememberAnchor()
    }
  }

  Timer {
    interval: 16
    repeat: true
    running: media.visible && media.controller.dropWheel.dragActive && media.dragScrollStep !== 0
    onTriggered: grid.contentY = Math.max(grid.originY, Math.min(grid.originY + Math.max(0, grid.contentHeight - grid.height), grid.contentY + media.dragScrollStep))
  }

  Text {
    textFormat: Text.PlainText
    anchors.centerIn: parent
    width: Math.max(0, parent.width - Style.space(24))
    visible: media.count === 0
    text: media.message || (media.busy ? "Looking for media…" : "No media in this folder")
    color: Commons.Color.muted
    horizontalAlignment: Text.AlignHCenter
    wrapMode: Text.WordWrap
    font.family: Style.font.family
    font.pixelSize: Typography.bodySmall
  }
}
