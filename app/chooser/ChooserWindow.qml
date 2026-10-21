import QtQuick
import QtQuick.Controls
import Quickshell
import qs.Commons
import "../../panes" as Panes
import "../../ui" as PluginUi
import qs.Commons as Commons

FloatingWindow {
  id: window
  required property var session
  readonly property var controller: session.service
  property alias actionKeys: keys
  property bool shortcutsOpen: false
  title: session.offer.title || "Choose a file — FileBlade"
  visible: session.opened && !!controller
  implicitWidth: 900
  implicitHeight: 720
  minimumSize: Qt.size(480, 360)
  color: Commons.Color.bar.background
  onClosed: session.cancel()

  function focusPart(part) { if (content.item) content.item.focusPart(part) }

  PluginUi.ActionKeyGuard { id: keys; active: window.visible; releaseRoot: window.contentItem }

  Loader {
    anchors.fill: parent
    active: !!window.controller
    sourceComponent: Component {
      Item {
        Panes.TreePane {
          id: tree
          anchors { top: parent.top; left: parent.left; right: parent.right; bottom: filters.top }
          controller: window.controller
          hostWindow: window
          mediaMode: false
        }
        ComboBox {
          id: filters
          palette.button: Commons.Color.bar.background
          palette.buttonText: Commons.Color.bar.text
          palette.base: Commons.Color.bar.background
          palette.text: Commons.Color.bar.text
          palette.highlight: Commons.Color.accent
          palette.highlightedText: Commons.Color.background
          anchors { left: parent.left; right: parent.right; bottom: footer.top }
          visible: session.filters.length > 0
          height: visible ? implicitHeight : 0
          model: session.filters.map(function(filter) { return filter[0] })
          currentIndex: session.filterIndex
          onActivated: session.filterIndex = currentIndex
        }
        Panes.PickerBar {
          id: footer
          acceptLabel: String(session.offer.accept_label || "").replace(/__|_(?=.)/g, function(marker) {
            return marker === "__" ? "_" : ""
          })
          anchors { bottom: parent.bottom; left: parent.left; right: parent.right }
          controller: window.controller
          hostWindow: window
          enabled: !session.busy
        }
        function focusPart(part) {
          if (part === "name") footer.focusName()
          else if (part === "search") tree.focusSearch()
          else if (part === "location") tree.focusLocation()
          else tree.focusTree()
        }
      }
    }
    id: content
  }
}
