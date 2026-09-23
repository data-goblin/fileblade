import QtQuick
import "../../ui" as PluginUi
import "MediaModel.js" as MediaModel

PluginUi.MediaView {
  id: content

  readonly property alias provider: mediaProvider
  readonly property var matches: MediaModel.matching(mediaProvider.rows, pane.mediaQuery,
    { caseSensitive: controller.searchCaseSensitive, regex: controller.searchRegex }, controller.treeFilter)
  items: matches.rows
  sorts: [{ key: "modified", desc: pane.mediaSortDescending }]
  onSortsChanged: {
    anchorPath = ""
    Qt.callLater(function() { content.positionViewAtIndex(0, GridView.Beginning) })
  }
  busy: mediaProvider.busy
  message: mediaProvider.error || (matches.invalid ? "Invalid pattern" : (pane.mediaQuery !== "" && !busy ? "No matching media" : ""))
  sizeStep: pane.mediaSizeStep

  MediaProvider {
    id: mediaProvider
    controller: content.controller
    active: content.pane.focusEnabled
    recursive: content.pane.mediaRecursive
    descriptor: content.pane.mediaLocationDescriptor
    onBusyChanged: if (!busy) content.pane.reconcileMediaSelection()
  }

  onKeyPressed: function(event) {
    var plain = !(event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier))
    if (plain && [Qt.Key_Plus, Qt.Key_Equal, Qt.Key_Minus, Qt.Key_Underscore].indexOf(event.key) >= 0) {
      pane.mediaSizeStep = Math.max(0, Math.min(4, pane.mediaSizeStep + (event.key === Qt.Key_Minus || event.key === Qt.Key_Underscore ? -1 : 1)))
      event.accepted = true
    } else if (plain && [Qt.Key_Left, Qt.Key_Right, Qt.Key_Up, Qt.Key_Down].indexOf(event.key) >= 0) {
      var delta = event.key === Qt.Key_Left ? -1 : (event.key === Qt.Key_Right ? 1 : (event.key === Qt.Key_Up ? -columns : columns))
      pane.moveCurrent(content, false, delta, pane.visualMode || !!(event.modifiers & Qt.ShiftModifier), false)
      event.accepted = true
    } else pane.handleListKey(event, content, false)
  }
}
