import QtQuick
import QtQuick.Controls
import qs.Commons
import "../../ui" as PluginUi
import "../../lib/Highlight.js" as Highlight
import "../../lib/FileIcons.js" as FileIcons
import "../../lib/KeyRouter.js" as KeyRouter
import "../../theme"

FocusScope {
  id: overlay

  required property var controller
  required property var context

  readonly property bool active: controller.quickNavActive
  readonly property string channel: controller.quickNavChannel
  readonly property var provider: controller.channelProvider(channel)
  readonly property color secondaryTextColor: Util.alpha(Color.bar.text, 0.72)
  readonly property var builtin: ({
    folders: { prefix: "z", title: "QUICK NAV", placeholder: "Folder name…" },
    files: { prefix: "", title: "FILES", note: "indexed files", placeholder: "File name…  > actions  ~ recent  ? contents" },
    recent: { prefix: "~", title: "RECENT", note: "frecency", placeholder: "Recently opened file…" }
  })
  readonly property var channelInfo: provider || builtin[channel] || builtin.folders
  readonly property var prefixes: ({ ">": "actions", "~": "recent", "?": "grep" })

  visible: active

  Keys.onPressed: function(event) {
    if (event.key !== Qt.Key_Escape) return
    overlay.close()
    event.accepted = true
  }

  function focusInput() {
    input.forceActiveFocus()
    input.selectAll()
  }

  function close() {
    controller.stopQuickNav()
    controller.focusTree(context.screen)
  }

  function toggleDeepSearch() {
    controller.stopQuickNav()
    controller.toggleSearchDeep()
    controller.bladeHost.focusModule("files", context.screen, "search")
  }

  function edited(text) {
    var first = text.charAt(0)
    if (text.length >= 1 && prefixes[first] && channel !== prefixes[first]) {
      if (first === "?") {
        controller.stopQuickNav()
        controller.searchQuery = "content:\"\""
        controller.bladeHost.focusModule("files", context.screen, "search")
        return
      }
      controller.setQuickNavChannel(prefixes[first])
      input.text = text.slice(1)
      controller.searchQuery = input.text
      return
    }
    if (text.length >= 3 && first === ":" && text.charAt(2) === " " && controller.channelForPrefix(text.charAt(1))) {
      controller.setQuickNavChannel(controller.channelForPrefix(text.charAt(1)))
      input.text = text.slice(3)
      controller.searchQuery = input.text
      return
    }
    controller.searchQuery = text
  }

  function activateIndex(index, alternate) {
    var model = controller.searchModel
    var target = index < 0 ? 0 : index
    if (target >= model.count) return
    var row = model.get(target)
    if (!row) return
    if (provider) {
      controller.activateChannelRow(channel, target)
      close()
      return
    }
    if (!row.path) return
    if (channel === "folders" || (row.isDir && !alternate)) {
      controller.navigateToLocation(row.path, context.screen, "browse")
      return
    }
    if (alternate || channel === "recent") {
      controller.openDefault(row.path, context.screen, !!row.isDir)
      close()
      return
    }
    var parent = row.path.slice(0, row.path.lastIndexOf("/")) || "/"
    controller.navigateToLocation(parent, context.screen, "browse")
    controller.selectPath(row.path, false, row.name, "", "", "", 0)
    close()
  }

  function moveCurrent(delta) {
    if (list.count === 0) return
    var current = list.currentIndex < 0 ? (delta > 0 ? -1 : list.count) : list.currentIndex
    list.currentIndex = Math.max(0, Math.min(list.count - 1, current + delta))
    list.positionViewAtIndex(list.currentIndex, ListView.Contain)
  }

  function glyphFor(row) {
    if (row.kind === "Action") return ""
    return FileIcons.entryIcon(row.name, row.isDir, row.isSymlink, false, row.isGitRepo)
  }

  onActiveChanged: {
    if (active) Qt.callLater(focusInput)
    else list.currentIndex = -1
  }

  Connections {
    target: overlay.controller
    function onQuickNavSelectionRevisionChanged() {
      list.currentIndex = list.count > 0 ? 0 : -1
      list.positionViewAtBeginning()
    }
    function onSearchQueryChanged() {
      if (input.text !== overlay.controller.searchQuery) input.text = overlay.controller.searchQuery
    }
  }

  Rectangle {
    anchors.fill: parent
    color: Util.alpha(Color.background, 0.55)

    MouseArea {
      anchors.fill: parent
      onClicked: overlay.close()
    }
  }

  Rectangle {
    id: card
    anchors.top: parent.top
    anchors.topMargin: Style.space(40)
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.leftMargin: Style.space(10)
    anchors.rightMargin: Style.space(10)
    height: Math.min(parent.height - Style.space(80), header.height + inputRow.height + statusLine.height + list.count * Style.space(40) + Style.space(30))
    radius: Style.cornerRadius
    color: Color.popups.background
    border.width: 1
    border.color: Color.popups.border
    clip: true

    MouseArea {
      anchors.fill: parent
      acceptedButtons: Qt.AllButtons
    }

    Item {
      id: header
      anchors.top: parent.top
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.margins: Style.space(10)
      height: Style.space(24)

      Text {
        textFormat: Text.PlainText
        anchors.left: parent.left
        anchors.verticalCenter: parent.verticalCenter
        text: String(overlay.channelInfo.title || overlay.channelInfo.name || "").toUpperCase()
        color: Color.bar.text
        font.family: Style.font.family
        font.pixelSize: Typography.bodySmall
        font.weight: Font.DemiBold
        font.letterSpacing: 0.6
      }

      Text {
        textFormat: Text.PlainText
        anchors.right: parent.right
        anchors.verticalCenter: parent.verticalCenter
        text: String(overlay.channelInfo.note || "")
        color: overlay.secondaryTextColor
        font.family: Style.font.family
        font.pixelSize: Typography.caption
      }
    }

    Item {
      id: inputRow
      anchors.top: header.bottom
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.leftMargin: Style.space(10)
      anchors.rightMargin: Style.space(10)
      height: Style.space(36)

      TextField {
        id: input
        anchors.fill: parent
        leftPadding: Style.space(30)
        rightPadding: Style.space(10)
        selectByMouse: true
        text: overlay.controller.searchQuery
        placeholderText: String(overlay.channelInfo.placeholder || "Search…")
        placeholderTextColor: overlay.secondaryTextColor
        color: Color.bar.text
        selectionColor: Util.alpha(Color.accent, 0.38)
        selectedTextColor: Color.bar.text
        font.family: Style.font.family
        font.pixelSize: Typography.body

        background: Rectangle {
          radius: Math.min(Style.cornerRadius, Style.space(4))
          color: Util.alpha(Color.bar.text, input.activeFocus ? 0.10 : 0.06)
          border.width: 1
          border.color: input.activeFocus ? Color.accent : Util.alpha(Color.bar.text, 0.18)
        }

        Text {
          textFormat: Text.PlainText
          anchors.left: parent.left
          anchors.leftMargin: Style.space(10)
          anchors.verticalCenter: parent.verticalCenter
          text: ""
          color: input.activeFocus ? Color.accent : overlay.secondaryTextColor
          font.family: Style.font.family
          font.pixelSize: Typography.bodySmall
        }

        onTextEdited: overlay.edited(text)

        Keys.priority: Keys.BeforeItem
        Keys.onPressed: function(event) {
          if (KeyRouter.listModeAction(event, false) === "deep") {
            overlay.toggleDeepSearch()
          } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
            overlay.activateIndex(list.currentIndex, !!(event.modifiers & Qt.ShiftModifier))
          } else if (event.key === Qt.Key_Down || (event.key === Qt.Key_N && (event.modifiers & Qt.ControlModifier))) {
            overlay.moveCurrent(1)
          } else if (event.key === Qt.Key_Up || (event.key === Qt.Key_P && (event.modifiers & Qt.ControlModifier))) {
            overlay.moveCurrent(-1)
          } else if (event.key === Qt.Key_Escape) {
            overlay.close()
          } else if (event.key === Qt.Key_Backspace && input.text === "" && overlay.channel !== overlay.controller.quickNavHome) {
            overlay.controller.setQuickNavChannel(overlay.controller.quickNavHome)
          } else if (event.key === Qt.Key_Tab
                     && !(event.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier | Qt.ShiftModifier))) {
            overlay.moveCurrent(1)
          } else {
            return
          }
          event.accepted = true
        }
      }
    }

    Text {
      textFormat: Text.PlainText
      id: statusLine
      anchors.top: inputRow.bottom
      anchors.topMargin: Style.space(4)
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.leftMargin: Style.space(12)
      anchors.rightMargin: Style.space(12)
      height: Style.space(18)
      verticalAlignment: Text.AlignVCenter
      text: {
        if (overlay.controller.searchBusy) return (overlay.controller.searchSpinner ? overlay.controller.searchSpinner + "  " : "") + "Looking…"
        if (overlay.controller.searchError) return overlay.controller.searchError
        var count = overlay.controller.searchModel.count
        if (count === 0) return "No matches"
        return count + (count === 1 ? " match" : " matches")
      }
      color: overlay.controller.searchError ? Color.urgent : overlay.secondaryTextColor
      elide: Text.ElideRight
      font.family: Style.font.family
      font.pixelSize: Typography.caption
    }

    ListView {
      id: list
      objectName: "quickNavResults"
      reuseItems: true
      anchors.top: statusLine.bottom
      anchors.topMargin: Style.space(4)
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.bottom: parent.bottom
      anchors.bottomMargin: Style.space(6)
      clip: true
      boundsBehavior: Flickable.StopAtBounds
      model: overlay.controller.searchModel
      currentIndex: -1
      ScrollBar.vertical: PluginUi.AccentScrollBar { }

      onCountChanged: if (count > 0 && currentIndex < 0) currentIndex = 0

      delegate: Rectangle {
        id: row
        required property int index
        required property string name
        required property string path
        required property string relative
        required property string nameSpans
        required property string relativeSpans
        required property string kind
        required property bool isDir
        required property bool isSymlink
        required property bool isGitRepo

        readonly property bool current: list.currentIndex === index
        width: ListView.view ? ListView.view.width : 0
        height: Style.space(40)
        color: current
          ? Color.menu.selectedBackground
          : (rowPointer.containsMouse ? Style.hoverFillFor(Color.bar.text, Color.accent) : "transparent")

        Text {
          textFormat: Text.PlainText
          id: rowIcon
          anchors.left: parent.left
          anchors.leftMargin: Style.space(12)
          anchors.verticalCenter: parent.verticalCenter
          width: Style.space(18)
          text: overlay.glyphFor(row)
          color: (row.isDir && overlay.controller.folderColor(row.path)) || (row.isDir || row.kind === "Action" ? Color.accent : overlay.secondaryTextColor)
          horizontalAlignment: Text.AlignHCenter
          font.family: Style.font.family
          font.pixelSize: Typography.body
        }

        Column {
          anchors.left: rowIcon.right
          anchors.leftMargin: Style.space(8)
          anchors.right: parent.right
          anchors.rightMargin: Style.space(12)
          anchors.verticalCenter: parent.verticalCenter
          spacing: 0

          Text {
            width: parent.width
            textFormat: Text.StyledText
            text: Highlight.markup(row.name, Highlight.parseSpans(row.nameSpans), Color.accent)
            color: row.current ? Color.accent : Color.bar.text
            elide: Text.ElideRight
            font.family: Style.font.family
            font.pixelSize: Typography.body
            font.weight: row.current ? Font.DemiBold : Font.Normal
          }

          Text {
            width: parent.width
            textFormat: Text.StyledText
            text: Highlight.markup(row.relative, Highlight.parseSpans(row.relativeSpans), Color.accent)
            color: overlay.secondaryTextColor
            elide: Text.ElideMiddle
            font.family: Style.font.family
            font.pixelSize: Typography.caption
          }
        }

        MouseArea {
          id: rowPointer
          anchors.fill: parent
          hoverEnabled: true
          cursorShape: Qt.PointingHandCursor
          onClicked: {
            list.currentIndex = row.index
            overlay.activateIndex(row.index, false)
          }
        }
      }
    }
  }
}
