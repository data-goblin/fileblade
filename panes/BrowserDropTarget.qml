import QtQuick

DropArea {
  id: target

  required property var controller
  required property var rowItem

  keys: ["fileblade-entry"]
  enabled: rowItem.dropAllowed

  onEntered: if (rowItem.treeMode && !rowItem.expanded) hoverExpand.restart()
  onExited: hoverExpand.stop()
  onDropped: function(drop) {
    hoverExpand.stop()
    var copyInstead = drop.proposedAction === Qt.CopyAction
    if (!controller.dropWheel.dropInto(rowItem.path, copyInstead)) controller.moveSelectionTo(rowItem.path, copyInstead, rowItem.draggedPaths.slice())
    drop.acceptProposedAction()
  }

  Timer {
    id: hoverExpand
    interval: 500
    onTriggered: if (target.containsDrag && rowItem.dropAllowed && !rowItem.expanded) controller.setDirectoryExpanded(rowItem.path, true)
  }
}
