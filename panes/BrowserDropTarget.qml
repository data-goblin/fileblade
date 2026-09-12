import QtQuick
import "../lib/PathText.js" as PathText

DropArea {
  id: target

  required property var controller
  required property var rowItem

  keys: ["fileblade-entry", "text/uri-list"]
  enabled: rowItem.dropAllowed

  function droppedPaths(drop) {
    if (!drop.hasUrls) return rowItem.draggedPaths.slice()
    var paths = []
    for (var i = 0; i < drop.urls.length; i++) {
      var path = PathText.droppedPath(drop.urls[i])
      if (path) paths.push(path)
    }
    return paths
  }

  onEntered: if (rowItem.treeMode && !rowItem.expanded) hoverExpand.restart()
  onExited: hoverExpand.stop()
  onDropped: function(drop) {
    hoverExpand.stop()
    var paths = target.droppedPaths(drop)
    if (paths.length === 0) return
    controller.moveSelectionTo(rowItem.path, drop.proposedAction === Qt.CopyAction, paths)
    drop.acceptProposedAction()
  }

  Timer {
    id: hoverExpand
    interval: 500
    onTriggered: if (target.containsDrag && rowItem.dropAllowed && !rowItem.expanded) controller.setDirectoryExpanded(rowItem.path, true)
  }
}
