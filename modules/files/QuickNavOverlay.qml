import QtQuick
import qs.Commons
import "../../ui" as PluginUi
import "../../lib/FileIcons.js" as FileIcons

FocusScope {
  id: overlay

  required property var controller
  required property var context

  readonly property bool active: controller.quickNavActive
  readonly property string channel: controller.quickNavChannel
  readonly property var provider: controller.channelProvider(channel)
  readonly property var builtin: ({
    folders: { prefix: "z", title: "QUICK NAV", placeholder: "Folder name…" },
    files: { prefix: "", title: "FILES", note: "indexed files", placeholder: "File name…  > actions  ~ recent  ? contents" },
    recent: { prefix: "~", title: "RECENT", note: "frecency", placeholder: "Recently opened file…" }
  })
  readonly property var channelInfo: provider || builtin[channel] || builtin.folders
  readonly property var prefixes: ({ ">": "actions", "~": "recent", "?": "grep" })

  visible: active

  function focusInput() {
    card.focusInput()
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
      card.text = text.slice(1)
      controller.searchQuery = card.text
      return
    }
    if (text.length >= 3 && first === ":" && text.charAt(2) === " " && controller.channelForPrefix(text.charAt(1))) {
      controller.setQuickNavChannel(controller.channelForPrefix(text.charAt(1)))
      card.text = text.slice(3)
      controller.searchQuery = card.text
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
    card.moveCurrent(delta)
  }

  function glyphFor(row) {
    if (row.kind === "Action") return ""
    return FileIcons.entryIcon(row.name, row.isDir, row.isSymlink, false, row.isGitRepo)
  }

  onActiveChanged: {
    if (active) Qt.callLater(focusInput)
    else card.clearCurrent()
  }

  Connections {
    target: overlay.controller
    function onQuickNavSelectionRevisionChanged() {
      card.resetCurrent()
    }
    function onSearchQueryChanged() {
      if (card.text !== overlay.controller.searchQuery) card.text = overlay.controller.searchQuery
    }
  }

  PluginUi.QuickNavCard {
    id: card
    anchors.fill: parent
    title: String(overlay.channelInfo.title || overlay.channelInfo.name || "")
    note: String(overlay.channelInfo.note || "")
    placeholder: String(overlay.channelInfo.placeholder || "Search…")
    text: overlay.controller.searchQuery
    model: overlay.controller.searchModel
    status: {
      if (overlay.controller.searchBusy) return (overlay.controller.searchSpinner ? overlay.controller.searchSpinner + "  " : "") + "Looking…"
      if (overlay.controller.searchError) return overlay.controller.searchError
      var count = overlay.controller.searchModel.count
      if (count === 0) return "No matches"
      return count + (count === 1 ? " match" : " matches")
    }
    statusUrgent: !!overlay.controller.searchError
    chips: [
      { key: "case", label: "Aa", active: overlay.controller.quickNavCaseSensitive, tip: "Ignoring case", activeTip: "Matching case" },
      { key: "hidden", label: "󰈉", active: overlay.controller.quickNavShowHidden, tip: "Skipping hidden directories", activeTip: "Including hidden directories" }
    ]
    glyphFor: function(row) { return overlay.glyphFor(row) }
    glyphColorFor: function(row) {
      return (row.isDir && overlay.controller.folderColor(row.path)) || (row.isDir || row.kind === "Action" ? Color.accent : card.secondaryTextColor)
    }
    onEdited: function(text) { overlay.edited(text) }
    onActivated: function(index, alternate) { overlay.activateIndex(index, alternate) }
    onDismissed: overlay.close()
    onChipToggled: function(key, active) { overlay.controller.setQuickNavOption(key, active) }
    onDeepRequested: overlay.toggleDeepSearch()
    onEmptyBackspace: {
      if (overlay.channel !== overlay.controller.quickNavHome) overlay.controller.setQuickNavChannel(overlay.controller.quickNavHome)
    }
  }
}
