import QtQuick
import QtQuick.Effects
import qs.Commons
import qs.Ui
import "../ui" as PluginUi
import "../lib/FileIcons.js" as FileIcons
import "../lib/KeyRouter.js" as KeyRouter
import "../lib/Highlight.js" as Highlight
import "../lib/PathText.js" as PathText
import "../theme"

FocusScope {
  id: root

  required property var controller
  required property var hostWindow
  property var context: null
  property bool focusEnabled: true
  PluginUi.ActionKeyGuard { id: actionKeys; active: root.activeFocus; shared: root.hostWindow ? root.hostWindow.actionKeys : null }
  PluginUi.TreeKeys { id: treeKeys; active: scroller.activeFocus; scope: "properties"; plan: controller.keybindings.plan }
  Keys.onReleased: function(event) { actionKeys.release(event) }
  TapHandler {
    acceptedButtons: Qt.BackButton | Qt.ForwardButton
    gesturePolicy: TapHandler.ReleaseWithinBounds
    enabled: root.focusEnabled && root.activeFocus
    onTapped: function(eventPoint, button) {
      if (button === Qt.BackButton) root.runKeyAction("back")
      else if (button === Qt.ForwardButton) root.runKeyAction("forward")
    }
  }

  function targetScreen() {
    return hostWindow ? hostWindow.screen : null
  }

  function forcePaneFocus() {
    if (focusEnabled) scroller.forceActiveFocus()
  }

  function returnToTree() {
    controller.focusTree(targetScreen())
  }

  function focusNext() {
    if (context) context.focusNext()
    else returnToTree()
  }

  function focusPrevious() {
    if (context) context.focusPrevious()
    else returnToTree()
  }

  function openMenu(mode, anchor) {
    var originX = context ? Number(context.surfaceOriginX) || 0 : 0
    var edge = context ? String(context.edge || "left") : "left"
    var placement = { edge: edge, keyboard: true }
    if (anchor && hostWindow && hostWindow.contentItem) {
      var point = anchor.mapToItem(hostWindow.contentItem, edge === "right" ? 0 : anchor.width, 0)
      controller.openActionMenu(mode || "actions", targetScreen(), point.x + originX, point.y - Style.space(4), undefined, placement)
      return
    }
    var x = edge === "right" ? originX + Style.space(2) : root.width - Style.space(8) + originX
    controller.openActionMenu(mode || "actions", targetScreen(), x, Style.space(70), undefined, placement)
  }

  function clampedContentY(value) {
    return Math.max(0, Math.min(Math.max(0, scroller.contentHeight - scroller.height), value))
  }

  function runKeyAction(action) {
    var actions = {
      help: function() { if (hostWindow) hostWindow.shortcutsOpen = true },
      location: function() { controller.focusLocation(targetScreen()) },
      hidden: function() { controller.toggleHidden() },
      back: function() { controller.goBack() },
      forward: function() { controller.goForward() },
      up: function() { controller.goUp() },
      home: function() { controller.goHome() },
      "scroll-up": function() { scroller.contentY = clampedContentY(scroller.contentY - Style.space(34)) },
      "scroll-down": function() { scroller.contentY = clampedContentY(scroller.contentY + Style.space(34)) },
      "page-up": function() { scroller.contentY = clampedContentY(scroller.contentY - scroller.height * 0.8) },
      "page-down": function() { scroller.contentY = clampedContentY(scroller.contentY + scroller.height * 0.8) },
      first: function() { scroller.contentY = 0 },
      last: function() { scroller.contentY = clampedContentY(scroller.contentHeight) },
      "focus-next": function() { focusNext() },
      "focus-previous": function() { focusPrevious() },
      dismiss: function() { if (context) context.closeBlade(); else returnToTree() },
      tree: function() { returnToTree() },
      search: function() { controller.focusSearch(targetScreen()) },
      "open-with": function() { openMenu("open-with") },
      actions: function() { openMenu("actions", moreButton) },
      activate: function() { controller.openDefault(entry.path, targetScreen(), !!entry.is_dir) },
      open: function() { controller.openDefault(entry.path, targetScreen(), !!entry.is_dir) },
      editor: function() { controller.openInEditor(entry.path) },
      reveal: function() { controller.revealInFileManager(entry.path, !!entry.is_dir, targetScreen()) },
      copy: function() { controller.copySelection(false) },
      cut: function() { controller.copySelection(true) },
      paste: function() { controller.pasteInto(controller.selectionDestination()) },
      undo: function() { controller.history.undoOperation() },
      redo: function() { controller.history.redoOperation() },
      "skip-refused": function() { controller.history.skipRefused() },
      "new-folder": function() { openMenu("new-folder") },
      "new-file": function() { openMenu("new-file") },
      rename: function() { openMenu("rename") },
      trash: function() { controller.requestTrash() },
      close: function() { controller.setOpen(false) }
    }
    var handler = actions[action]
    if (!handler) return false
    handler()
    return true
  }

  function handleKey(event) {
    var state = {
      actionable: actionableEntry,
      count: controller.selectedCount,
      deleted: isDeleted,
      directory: actionableEntry && !!entry.is_dir
    }
    var repeated = actionKeys.isRepeat(event)
    var action = treeKeys.action(event, repeated, KeyRouter.propertyAction(event, state))
    if (action.indexOf("key-") === 0 || (["open", "activate"].indexOf(action) >= 0 && !actionableEntry)) {
      event.accepted = true
      return
    }
    if (KeyRouter.ignoresAutoRepeat(action, event.key) && repeated) {
      event.accepted = true
      return
    }
    if (!runKeyAction(action)) return
    event.accepted = true
  }

  readonly property var entry: controller.selectedMetadata
  readonly property bool hasSelection: controller.selectedCount > 0
  readonly property bool multiple: controller.selectedCount > 1
  readonly property bool hasEntry: controller.selectedCount === 1 && entry && String(entry.path || "") !== ""
  readonly property bool isDeleted: hasEntry && !!entry.is_deleted
  readonly property bool actionableEntry: hasEntry && !isDeleted
  readonly property string imageMime: hasEntry ? String(entry.mime || "") : ""
  readonly property bool isImage: actionableEntry && !entry.is_dir && imageMime.indexOf("image/") === 0
  readonly property int imagePreviewByteLimit: 16 * 1024 * 1024
  readonly property bool imageDecodable: ["image/jpeg", "image/png", "image/webp"].indexOf(imageMime) >= 0
  readonly property bool imagePreviewAllowed: isImage && imageDecodable
    && !entry.is_symlink
    && ["image/jpeg", "image/png", "image/webp"].indexOf(imageMime) >= 0
    && Number(entry.size || -1) >= 0
    && Number(entry.size || -1) <= imagePreviewByteLimit
  readonly property string imagePreviewKey: imagePreviewAllowed
    ? [String(entry.path || ""), String(entry.stat_fingerprint || ""), imageMime, String(entry.size)].join("\n")
    : ""
  readonly property color paneBackground: Qt.lighter(Color.bar.background, 1.035)
  property string previewSource: ""
  property string thumbnailRequestId: ""
  property int thumbnailGeneration: 0
  property bool imageLoading: false
  property string imageError: ""
  readonly property bool previewableFile: actionableEntry && !entry.is_dir && !isImage
  readonly property bool isText: previewableFile && !entry.is_symlink && Number(entry.size || 0) >= 0 && Number(entry.size || 0) <= 262144
  readonly property string textPreviewKey: isText
    ? [String(entry.path || ""), String(entry.stat_fingerprint || ""), imageMime, String(entry.size)].join("\n")
    : ""
  property var previewLines: []
  property int previewGeneration: 0
  property string previewRequestId: ""
  property bool previewBusy: false
  property bool previewLoading: false
  property string previewError: ""
  readonly property string previewMarkup: renderPreview(previewLines)

  Timer {
    id: previewLoadingTimer
    interval: 90
    onTriggered: {
      if (!root.previewBusy) return
      root.previewLines = []
      root.previewLoading = true
    }
  }

  onTextPreviewKeyChanged: requestPreview()
  onImagePreviewKeyChanged: requestThumbnail()
  Component.onDestruction: {
    previewLoadingTimer.stop()
    if (previewRequestId) controller.cancelBackendRequest(previewRequestId, previewGeneration)
    if (thumbnailRequestId) controller.cancelBackendRequest(thumbnailRequestId, thumbnailGeneration)
  }

  function requestThumbnail() {
    if (thumbnailRequestId) controller.cancelBackendRequest(thumbnailRequestId, thumbnailGeneration)
    thumbnailRequestId = ""
    thumbnailGeneration++
    previewSource = ""
    imageError = ""
    imageLoading = imagePreviewAllowed
    if (!imagePreviewAllowed) return
    var generation = thumbnailGeneration
    var key = imagePreviewKey
    var args = ["--path", String(entry.path || ""), "--key", key, "--width", "1024", "--height", "1024"]
    thumbnailRequestId = controller.backendRequest("thumbnail", args, generation, function(response) {
      if (!root || generation !== root.thumbnailGeneration || key !== root.imagePreviewKey) return
      root.thumbnailRequestId = ""
      root.imageLoading = false
      var ready = response && response.ok && String(response.path || "") !== ""
      root.previewSource = ready ? localFileUrl(String(response.path)) + "?v=" + encodeURIComponent(key) : ""
      root.imageError = ready ? "" : String(response && response.error || "Preview unavailable")
    })
  }

  function imagePreviewMessage() {
    if (!entry) return ""
    if (entry.is_symlink) return "Linked images open externally to avoid following a link inside the shell."
    if (Number(entry.size || -1) < 0 || Number(entry.size || -1) > imagePreviewByteLimit)
      return "This image is too large to preview inside the shell."
    return ""
  }

  function requestPreview() {
    previewLoadingTimer.stop()
    if (previewRequestId) controller.cancelBackendRequest(previewRequestId, previewGeneration)
    previewRequestId = ""
    previewGeneration++
    previewBusy = false
    previewLoading = false
    previewError = ""
    filePreview.resetScroll()
    if (!isText) {
      previewLines = []
      if (previewableFile && entry.is_symlink) previewError = "Linked files open externally to avoid following a link inside the shell."
      return
    }
    previewBusy = true
    var generation = previewGeneration
    var key = textPreviewKey
    previewLoadingTimer.restart()
    previewRequestId = controller.backendRequest("preview", ["--path", String(entry.path || ""), "--lines", "80"], generation, function(response) {
      if (!root || generation !== root.previewGeneration || key !== root.textPreviewKey) return
      root.previewRequestId = ""
      previewLoadingTimer.stop()
      root.previewBusy = false
      root.previewLoading = false
      var ready = response && response.ok && !response.binary && Array.isArray(response.lines)
      root.previewLines = ready ? response.lines : []
      root.previewError = ready ? "" : String(response && response.error || "Preview unavailable")
    })
  }

  function paletteColor(index) {
    var names = ["black", "red", "green", "yellow", "blue", "magenta", "cyan", "white"]
    var palette = controller.themeFolderPalette || ({})
    var named = palette[names[index % 8]]
    if (typeof named === "string" && named) return named
    if (index % 8 === 1) return String(Color.urgent)
    if ([4, 5, 6].indexOf(index % 8) >= 0) return String(Color.accent)
    return String(Color.bar.text)
  }

  function renderPreview(lines) {
    var html = []
    for (var i = 0; i < (lines || []).length; i++) {
      var line = ""
      var runs = lines[i] || []
      for (var j = 0; j < runs.length; j++) {
        var piece = Highlight.escapeHtml(runs[j].text).replace(/ /g, "&nbsp;")
        if (Number(runs[j].color) >= 0) piece = "<font color=\"" + paletteColor(Number(runs[j].color)) + "\">" + piece + "</font>"
        if (runs[j].bold) piece = "<b>" + piece + "</b>"
        line += piece
      }
      html.push(line)
    }
    return html.join("<br>")
  }

  function localFileUrl(path) {
    return PathText.fileUrl(path)
  }

  readonly property var detailRows: {
    if (!hasEntry) return []
    var rows = [
      { label: "Path", value: String(entry.path || "") },
      { label: "Type", value: String(entry.kind || "") },
      { label: "MIME", value: String(entry.mime || "") },
      { label: "Size", value: String(entry.size_text || "—") },
      { label: "Modified", value: String(entry.modified || "") },
      { label: "Created", value: String(entry.created || "") },
      { label: "Accessed", value: String(entry.accessed || "") },
      { label: "Permissions", value: String(entry.permissions || "") + (entry.mode ? "  (" + entry.mode + ")" : "") },
      { label: "Owner", value: String(entry.owner || "") + (entry.group ? ":" + entry.group : "") }
    ]
    if (controller.gitEnabled && entry.git_status) rows.splice(2, 0, { label: "Git status", value: String(entry.git_status_label || entry.git_status).split(" · ").join("\n") })
    if (controller.gitEnabled && entry.git_repo_root) rows.splice(entry.git_status ? 3 : 2, 0,
      { label: "Repo", value: String(entry.git_repo_name || "") },
      { label: "Branch", value: String(entry.git_branch || "") },
      { label: "Worktree", value: String(entry.git_worktree || "") })
    if (entry.link_target) rows.push({ label: "Target", value: String(entry.link_target) })
    return rows.filter(function(row) { return row.value !== "" })
  }

  component ActionButton: Rectangle {
    id: button
    required property string label
    property bool primary: false
    signal clicked()

    implicitWidth: labelText.implicitWidth + Style.space(18)
    implicitHeight: Style.space(28)
    radius: Math.min(Style.cornerRadius, Style.space(4))
    color: primary
      ? (pointer.containsMouse ? Color.accent : Util.alpha(Color.accent, 0.82))
      : (pointer.containsMouse ? Style.hoverFillFor(Color.bar.text, Color.accent) : Util.alpha(Color.bar.text, 0.07))
    border.width: primary ? 0 : 1
    border.color: Util.alpha(Color.bar.text, 0.18)

    Text {
      textFormat: Text.PlainText
      id: labelText
      anchors.centerIn: parent
      text: button.label
      color: button.primary ? Color.background : Color.bar.text
      font.family: Style.font.family
      font.pixelSize: Typography.bodySmall
      font.weight: button.primary ? Font.DemiBold : Font.Normal
    }

    MouseArea {
      id: pointer
      anchors.fill: parent
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: button.clicked()
    }
  }

  component HeaderGlyph: Item {
    id: glyphButton
    required property string glyph
    property string tipTitle: ""
    property var tipActions: []
    signal clicked()
    width: Style.space(22)
    height: Style.space(22)

    Text {
      textFormat: Text.PlainText
      anchors.centerIn: parent
      text: glyphButton.glyph
      color: glyphPointer.containsMouse ? Color.accent : Color.muted
      font.family: Style.font.family
      font.pixelSize: Typography.body
    }

    MouseArea {
      id: glyphPointer
      anchors.fill: parent
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: glyphButton.clicked()
    }

    PluginUi.HintTip {
      visible: glyphPointer.containsMouse
      title: glyphButton.tipTitle
      actions: glyphButton.tipActions
    }
  }

  function entryGlyph() {
    if (!entry) return ""
    return FileIcons.entryIcon(String(entry.name || ""), !!entry.is_dir, !!entry.is_symlink, false, !!entry.is_git_repo)
  }

  function entryGlyphColor() {
    if (!entry) return Color.muted
    return controller.folderColor(String(entry.path || "")) || (entry.is_dir ? Color.accent : Color.muted)
  }

  readonly property var propertyGlyphs: ({
    "Path": "󰟙", "Type": "󰠲", "MIME": "󰧮", "Size": "󰋊", "Modified": "󰢧", "Created": "󰃳", "Accessed": "󰛐",
    "Permissions": "󰍁", "Owner": "󰀓", "Git status": "󰊢", "Repo": "󰊢", "Branch": "󰘬", "Worktree": "󰉖", "Target": "󰌹"
  })

  function propertyGlyph(label) {
    return controller.propertyIcons ? String(propertyGlyphs[label] || "󰋽") : ""
  }

  component DetailRow: Column {
    id: detail
    required property string label
    required property string value
    readonly property string glyph: root.propertyGlyph(label)

    width: parent ? parent.width : 0
    spacing: Style.space(2)

    Row {
      width: parent.width
      spacing: Style.space(5)

      Text {
        textFormat: Text.PlainText
        visible: detail.glyph !== ""
        width: visible ? Style.space(14) : 0
        text: detail.glyph
        color: Util.alpha(Color.muted, 0.8)
        horizontalAlignment: Text.AlignHCenter
        font.family: Style.font.family
        font.pixelSize: Typography.caption
      }

      Text {
        textFormat: Text.PlainText
        text: detail.label.toUpperCase()
        color: Color.muted
        font.family: Style.font.family
        font.pixelSize: Typography.caption
        font.letterSpacing: 0.4
      }
    }

    Text {
      textFormat: Text.PlainText
      width: parent.width
      text: detail.value
      color: detail.label === "Branch" ? Color.muted : Color.bar.text
      wrapMode: detail.label === "Path" || detail.label === "Target" ? Text.WrapAnywhere : Text.Wrap
      font.family: Style.font.family
      font.pixelSize: Typography.bodySmall
    }
  }

  Rectangle {
    anchors.fill: parent
    color: root.paneBackground
  }

  PluginUi.PaneHeader {
    id: header
    anchors.top: parent.top
    context: root.context
    title: "PROPERTIES"
    reservedLeft: root.context ? root.context.cornerReserveLeft : 0
    reservedRight: root.context ? root.context.cornerReserveRight : 0
    highlighted: root.activeFocus
    status: controller.metadataBusy ? "Reading…" : ""
  }

  Image {
    id: emptyLogo
    anchors.centerIn: scroller
    width: Math.max(Style.space(60), Math.floor(scroller.width * 0.6))
    height: Math.round(width / 4)
    source: Qt.resolvedUrl("../assets/fileblade-logo.png")
    fillMode: Image.PreserveAspectFit
    smooth: true
    asynchronous: true
    visible: false
  }

  MultiEffect {
    source: emptyLogo
    anchors.fill: emptyLogo
    visible: !root.hasSelection && emptyLogo.status === Image.Ready
    colorization: 1
    colorizationColor: Color.muted
  }

  Flickable {
    id: scroller
    anchors.top: header.bottom
    anchors.bottom: parent.bottom
    anchors.left: parent.left
    anchors.right: parent.right
    contentWidth: width
    contentHeight: content.implicitHeight + Style.space(18)
    clip: true
    boundsBehavior: Flickable.StopAtBounds
    interactive: !filePreview.hovered
    Keys.onPressed: function(event) { root.handleKey(event) }

    Column {
      id: content
      anchors.left: parent.left
      anchors.right: parent.right
      anchors.leftMargin: Style.space(10)
      anchors.rightMargin: Style.space(10)
      anchors.top: parent.top
      anchors.topMargin: Style.space(9)
      spacing: Style.space(9)

      Text {
        textFormat: Text.PlainText
        width: parent.width
        visible: !root.hasSelection
        text: "Select a file or folder to inspect it."
        color: Color.muted
        wrapMode: Text.WordWrap
        font.family: Style.font.family
        font.pixelSize: Typography.body
      }

      Item {
        width: parent.width
        height: Style.space(24)
        visible: root.hasSelection

        Text {
          textFormat: Text.PlainText
          id: headerIcon
          anchors.left: parent.left
          anchors.verticalCenter: parent.verticalCenter
          width: visible ? Style.space(17) : 0
          visible: root.hasEntry
          horizontalAlignment: Text.AlignHCenter
          text: root.entryGlyph()
          color: root.entryGlyphColor()
          font.family: Style.font.family
          font.pixelSize: Typography.body
        }

        Text {
          textFormat: Text.PlainText
          anchors.left: headerIcon.right
          anchors.leftMargin: headerIcon.visible ? Style.space(6) : 0
          anchors.right: headerActions.left
          anchors.rightMargin: Style.space(6)
          anchors.verticalCenter: parent.verticalCenter
          text: root.multiple
            ? controller.selectedCount + " items selected"
            : (root.entry ? String(root.entry.name || "") : "")
          color: Color.bar.text
          elide: Text.ElideRight
          font.family: Style.font.family
          font.pixelSize: Typography.title
          font.weight: Font.DemiBold
        }

        Row {
          id: headerActions
          anchors.right: parent.right
          anchors.verticalCenter: parent.verticalCenter
          spacing: Style.space(2)
          visible: !root.isDeleted

          HeaderGlyph {
            visible: root.hasEntry
            glyph: root.entry && root.entry.is_dir ? "󰉖" : "󰏌"
            tipTitle: root.entry && root.entry.is_dir ? "Open folder" : "Open"
            tipActions: [{ button: "left", text: "Open" }, { shortcut: "Enter" }]
            onClicked: controller.openDefault(root.entry.path, root.targetScreen(), !!root.entry.is_dir)
          }

          HeaderGlyph {
            id: moreButton
            glyph: "󰇘"
            tipTitle: "More"
            tipActions: [{ button: "left", text: "Actions" }, { shortcut: "m" }]
            onClicked: root.openMenu("actions", moreButton)
          }
        }
      }

      Text {
        textFormat: Text.PlainText
        width: parent.width
        visible: root.multiple
        text: controller.humanSize(controller.selectedTotalSize) + " across "
          + controller.selectedCount + " items"
        color: Color.muted
        font.family: Style.font.family
        font.pixelSize: Typography.bodySmall
      }

      PluginUi.FilePreview {
        id: filePreview
        width: parent.width
        image: root.isImage && (root.imageDecodable || root.entry.is_symlink)
        imageAllowed: root.imagePreviewAllowed
        imageSource: root.previewSource
        imageMessage: root.imagePreviewMessage()
        imageLoading: root.imageLoading
        imageError: root.imageError
        textPreview: root.isText
        trustedTextMarkup: root.previewMarkup
        loading: root.previewLoading
        error: root.previewError
        surfaceColor: root.paneBackground
      }

      Text {
        textFormat: Text.PlainText
        width: parent.width
        visible: controller.operationNotice !== ""
        text: controller.operationNotice
        color: Color.muted
        elide: Text.ElideRight
        font.family: Style.font.family
        font.pixelSize: Typography.caption
      }

      Flow {
        width: parent.width
        visible: controller.operationCancellable
        spacing: Style.space(6)

        ActionButton {
          label: "Stop " + String(controller.operationLabel || "operation").toLowerCase() + "…"
          onClicked: root.openMenu("cancel-operation")
        }
      }

      PluginUi.ErrorNotice {
        width: parent.width
        text: controller.operationError
        onDismissed: controller.operationError = ""
      }

      PluginUi.ErrorNotice {
        width: parent.width
        visible: root.hasEntry && controller.metadataError !== ""
        text: controller.metadataError
        onDismissed: controller.metadataError = ""
      }

      Flow {
        id: detailGrid
        width: parent.width
        spacing: Style.space(9)
        readonly property int cellMinimum: Math.ceil(sample.width) + Style.space(12)
        readonly property int columns: Math.max(1, Math.min(3, Math.floor((width + spacing) / (cellMinimum + spacing))))
        readonly property real cell: Math.floor((width - spacing * (columns - 1)) / columns)

        TextMetrics {
          id: sample
          font.family: Style.font.family
          font.pixelSize: Typography.bodySmall
          text: "2026-09-02 16:31:27"
        }

        Repeater {
          model: root.detailRows
          delegate: DetailRow {
            required property var modelData
            label: String(modelData.label)
            value: String(modelData.value)
            width: detailGrid.columns > 1 && label !== "Path" && label !== "Target" ? detailGrid.cell : detailGrid.width
          }
        }
      }
    }
  }

  PluginUi.ScrollEdgeFade {
    anchors.left: scroller.left
    anchors.right: scroller.right
    anchors.top: scroller.top
    anchors.bottom: scroller.bottom
    flickable: scroller
    surfaceColor: root.paneBackground
    visible: !filePreview.hovered && (hasAbove || hasBelow)
    z: 24
  }
}
