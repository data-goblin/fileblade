import QtQuick
import qs.Commons
import "../lib/ImageGallery.js" as ImageGallery
import "../theme"

FocusScope {
  id: grid

  property var items: []
  property string query: ""
  property var filterKeys: []
  property bool caseSensitive: false
  property bool regex: false
  property int sizeStep: ImageGallery.DEFAULT_STEP
  property var thumbnails: null
  property int cursor: -1
  property string selectedPath: ""
  property bool showTimeline: true
  property bool showLabels: sizeStep >= 3
  property bool dragEnabled: true
  property var context: null
  readonly property var dropWheel: {
    var files = context && typeof context.service === "function" ? context.service("files") : null
    return files && files.dropWheel ? files.dropWheel : null
  }
  property var dragItem: null
  readonly property bool dragging: dragItem !== null
  property int gap: Style.space(4)
  property int padding: Style.space(6)
  property int headerHeight: Style.space(24)
  property int sectionGap: Style.space(8)
  readonly property int cell: Style.space(ImageGallery.cellFor(sizeStep))
  readonly property int labelHeight: showLabels ? Style.space(16) : 0
  readonly property int thumbnailEdge: ImageGallery.thumbnailEdge(sizeStep)
  readonly property real listWidth: Math.max(0, width - (timeline.visible ? timeline.width : 0))
  readonly property var filtered: ImageGallery.filter(items, query, filterKeys, { caseSensitive: caseSensitive, regex: regex })
  readonly property string filterInvalid: String(filtered.invalid || "")
  readonly property var layout: ImageGallery.layout(filtered.items, listWidth, {
    cell: cell, tileHeight: cell + labelHeight, gap: gap, padding: padding, headerHeight: headerHeight, sectionGap: sectionGap })
  readonly property int count: layout.items.length
  readonly property var current: cursor >= 0 && cursor < count ? layout.items[cursor] : null
  readonly property int cursorSection: {
    var row = ImageGallery.rowOf(layout, cursor)
    return row >= 0 ? layout.rows[row].section : -1
  }
  readonly property real scrollFraction: {
    var rows = layout.rows
    if (rows.length === 0 || layout.height <= 0) return 0
    var index = list.indexAt(1, list.contentY + 1)
    if (index < 0) return list.contentY <= 0 ? 0 : 1
    var item = list.itemAtIndex(index)
    var offset = item ? list.contentY - item.y : 0
    return Math.max(0, Math.min(1, (rows[index].y + offset) / Math.max(1, layout.height - list.height)))
  }
  readonly property alias flickable: list

  signal activated(var item)
  signal chosen(var item)
  signal contextRequested(var item, real x, real y)
  signal dragBegan(var item, point scene, int modifiers)
  signal dragMoved(var item, point scene, int modifiers)
  signal dragEnded(var item, bool canceled)
  signal sizeStepRequested(int delta)
  signal dismissRequested()

  clip: true

  function itemAt(index) {
    return index >= 0 && index < count ? layout.items[index] : null
  }

  function indexOfPath(path) {
    var wanted = String(path || "")
    for (var i = 0; i < count; i++)
      if (String(layout.items[i].path || "") === wanted) return i
    return -1
  }

  function setCursor(index, announce) {
    var next = index >= 0 && index < count ? index : -1
    if (next === cursor && announce !== true) return
    cursor = next
    ensureVisible(next)
    if (next >= 0) chosen(layout.items[next])
  }

  function move(direction) {
    setCursor(ImageGallery.moveCursor(layout, cursor, direction), true)
  }

  function ensureVisible(index) {
    var row = ImageGallery.rowOf(layout, index)
    if (row < 0) return
    var item = list.itemAtIndex(row)
    if (item && item.y >= list.contentY && item.y + item.height <= list.contentY + list.height) return
    list.positionViewAtIndex(row, ListView.Contain)
  }

  function scrubTo(fraction) {
    var position = Math.max(0, Math.min(1, fraction)) * Math.max(0, layout.height - list.height)
    var row = ImageGallery.rowAt(layout, position)
    if (row < 0) return
    list.positionViewAtIndex(row, ListView.Beginning)
    list.contentY += position - layout.rows[row].y
  }

  function page(direction) {
    var distance = Math.max(cell, list.height * 0.85)
    var minimum = Number(list.originY) || 0
    var maximum = minimum + Math.max(0, list.contentHeight - list.height)
    list.contentY = Math.max(minimum, Math.min(maximum, list.contentY + direction * distance))
  }

  function activateCurrent() {
    if (current) activated(current)
  }

  function beginTileDrag(item, scene, modifiers) {
    dragItem = item
    dragBegan(item, scene, modifiers)
  }

  function endTileDrag(canceled) {
    if (!dragging) return
    var item = dragItem
    dragItem = null
    dragEnded(item, canceled)
  }

  onCountChanged: if (cursor >= count) cursor = count - 1
  onVisibleChanged: if (!visible) endTileDrag(true)
  Component.onDestruction: endTileDrag(true)

  Keys.onReleased: function(event) {
    if (dropWheel && dropWheel.handleDragKeyRelease(event)) event.accepted = true
  }

  Keys.onPressed: function(event) {
    if (dropWheel && dropWheel.handleDragKey(event)) {
      event.accepted = true
      return
    }
    var plain = !(event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier))
    var control = (event.modifiers & Qt.ControlModifier) && !(event.modifiers & (Qt.AltModifier | Qt.MetaModifier))
    if (plain && (event.key === Qt.Key_Left || event.key === Qt.Key_H)) move("left")
    else if (plain && (event.key === Qt.Key_Right || event.key === Qt.Key_L)) move("right")
    else if (plain && (event.key === Qt.Key_Up || event.key === Qt.Key_K)) move("up")
    else if (plain && (event.key === Qt.Key_Down || event.key === Qt.Key_J)) move("down")
    else if (plain && (event.key === Qt.Key_Home || (event.key === Qt.Key_G && !(event.modifiers & Qt.ShiftModifier)))) move("home")
    else if (plain && (event.key === Qt.Key_End || (event.key === Qt.Key_G && (event.modifiers & Qt.ShiftModifier)))) move("end")
    else if (event.key === Qt.Key_PageDown || (control && event.key === Qt.Key_D)) page(1)
    else if (event.key === Qt.Key_PageUp || (control && (event.key === Qt.Key_U || event.key === Qt.Key_B))) page(-1)
    else if (plain && (event.key === Qt.Key_Return || event.key === Qt.Key_Enter || event.key === Qt.Key_O)) activateCurrent()
    else if (plain && (event.key === Qt.Key_Plus || event.key === Qt.Key_Equal)) sizeStepRequested(1)
    else if (plain && (event.key === Qt.Key_Minus || event.key === Qt.Key_Underscore)) sizeStepRequested(-1)
    else if (plain && event.key === Qt.Key_Escape) dismissRequested()
    else return
    event.accepted = true
  }

  ListView {
    id: list
    anchors.left: parent.left
    anchors.top: parent.top
    anchors.bottom: parent.bottom
    width: grid.listWidth
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    cacheBuffer: Math.max(grid.cell * 2, Style.space(320))
    model: grid.layout.rows.length
    header: Item { width: 1; height: grid.padding }
    footer: Item { width: 1; height: grid.padding }
    spacing: 0

    delegate: Item {
      id: rowItem
      required property int index
      readonly property var row: index < grid.layout.rows.length ? grid.layout.rows[index] : null
      readonly property bool tiles: !!row && row.kind === "tiles"
      readonly property int trailing: tiles ? trailingGap() : 0
      width: list.width
      height: row ? row.height + trailing : 0

      function trailingGap() {
        var rows = grid.layout.rows
        var next = index + 1 < rows.length ? rows[index + 1] : null
        if (!next) return 0
        return next.section !== row.section ? grid.sectionGap : grid.gap
      }

      Text {
        textFormat: Text.PlainText
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.leftMargin: grid.padding + Style.space(2)
        anchors.rightMargin: grid.padding
        height: rowItem.row ? rowItem.row.height : 0
        visible: !!rowItem.row && rowItem.row.kind === "header"
        text: rowItem.row && rowItem.row.kind === "header" ? String(rowItem.row.label).toUpperCase() : ""
        color: rowItem.row && rowItem.row.section === grid.cursorSection ? Color.accent : Color.muted
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
        font.family: Style.font.family
        font.pixelSize: Typography.caption
        font.weight: Font.DemiBold
        font.letterSpacing: 0.6
      }

      Row {
        anchors.left: parent.left
        anchors.leftMargin: grid.padding
        anchors.top: parent.top
        spacing: grid.gap
        visible: rowItem.tiles

        Repeater {
          model: rowItem.tiles ? rowItem.row.count : 0

          delegate: ImageTile {
            id: tile
            required property int index
            readonly property int itemIndex: rowItem.tiles ? rowItem.row.start + index : -1
            width: grid.cell
            height: grid.cell + grid.labelHeight
            item: grid.itemAt(itemIndex)
            thumbnails: grid.thumbnails
            edge: grid.thumbnailEdge
            current: itemIndex === grid.cursor
            selected: grid.selectedPath !== "" && !!item && String(item.path || "") === grid.selectedPath
            showLabel: grid.showLabels
            labelHeight: grid.labelHeight
            dragEnabled: grid.dragEnabled
            onClicked: function(mouse) {
              grid.forceActiveFocus()
              grid.setCursor(itemIndex, true)
              if (mouse.button === Qt.RightButton) {
                var point = tile.mapToItem(grid, mouse.x, mouse.y)
                grid.contextRequested(tile.item, point.x, point.y)
              }
            }
            onDoubleClicked: {
              grid.setCursor(itemIndex, false)
              if (tile.item) grid.activated(tile.item)
            }
            onDragBegan: function(scene, modifiers) {
              grid.forceActiveFocus()
              grid.setCursor(itemIndex, true)
              grid.beginTileDrag(tile.item, scene, modifiers)
            }
            onDragMoved: function(scene, modifiers) { if (grid.dragging) grid.dragMoved(tile.item, scene, modifiers) }
            onDragEnded: function(canceled) { grid.endTileDrag(canceled) }
          }
        }
      }
    }
  }

  ScrollEdgeFade {
    anchors.fill: list
    flickable: list
    surfaceColor: Qt.lighter(Color.bar.background, 1.035)
    fadeHeight: Style.space(28)
  }

  TimelineScrubber {
    id: timeline
    anchors.right: parent.right
    anchors.top: parent.top
    anchors.bottom: parent.bottom
    visible: grid.showTimeline && grid.filtered.items.length > 0
    sections: grid.layout.sections
    contentHeight: grid.layout.height
    viewportHeight: list.height
    fraction: grid.scrollFraction
    onScrubbed: function(fraction) { grid.scrubTo(fraction) }
  }

  Text {
    textFormat: Text.PlainText
    anchors.centerIn: list
    width: list.width - Style.space(24)
    visible: grid.count === 0
    text: grid.filterInvalid !== "" ? "Invalid pattern" : (grid.query !== "" ? "No matches" : "No images")
    color: Color.muted
    horizontalAlignment: Text.AlignHCenter
    wrapMode: Text.WordWrap
    font.family: Style.font.family
    font.pixelSize: Typography.bodySmall
  }
}
