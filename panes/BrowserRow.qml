import QtQuick
import qs.Commons
import qs.Ui
import "../lib/DragPlan.js" as DragPlan
import "../lib/FileIcons.js" as FileIcons
import "../lib/Highlight.js" as Highlight
import "../lib/GitSummary.js" as GitSummary
import "../theme"

Rectangle {
  id: row

  required property var controller
  required property var pane

  required property int index
  required property string name
  required property string path
  required property bool isDir
  required property bool isSymlink
  required property bool isGitRepo
  required property bool gitDeleted
  required property bool gitIgnored
  required property string gitRepoRoot
  required property string gitStatus
  required property string gitStatusLabel
  required property string gitIndexStatus
  required property string gitWorktreeStatus
  required property string gitOriginalPath
  required property int gitModifiedCount
  required property int gitDeletedCount
  required property int gitNewCount
  required property int gitUntrackedCount
  required property string gitSummary
  required property string gitRepoName
  required property string gitBranch
  required property string gitWorktree
  required property string sizeText
  required property real size
  required property string kind
  required property string mime
  required property string modified
  required property string created
  required property string relative
  required property string nameSpans
  required property string relativeSpans
  required property int depth
  required property bool expanded
  required property bool loading
  required property string error

  property bool treeMode: true
  property bool showPath: !treeMode
  readonly property bool pathVisible: showPath && relative !== "" && relative !== "." && relative !== name
  property bool favoriteMode: false
  property bool customInteraction: false
  property bool metadataDimmed: false
  property bool customSelected: false
  property bool urgent: false
  property ListView ownerView: null
  readonly property bool hovered: hoverTracker.hovered
  readonly property bool moreRow: kind === "More"
  readonly property string moreParent: moreRow ? path.slice(0, -5) : ""
  readonly property bool hiddenEntry: !moreRow && name !== "." && name !== ".." && name.charAt(0) === "."
  readonly property bool favoriteAvailable: favoriteMode || depth > 0
  readonly property var draggedPaths: controller.dropWheel.dragPaths
  readonly property bool dropAllowed: !customInteraction && row.isDir && DragPlan.canDrop(draggedPaths, row.path)
  readonly property bool dropHovered: dropTarget.containsDrag && dropAllowed
  property real dragScrollStep: 0
  property bool dragCanceled: false

  signal customClicked(var mouse)
  signal customDoubleClicked(var mouse)

  function loadMore() {
    if (moreRow) controller.loadMoreChildren(moreParent)
  }

  Component.onCompleted: loadMore()
  ListView.onReused: loadMore()

  width: ListView.view ? ListView.view.width : 0
  height: pathVisible ? Style.space(42) : Style.space(30)
  function priorityText(key, width) {
    var places = { repo: row.gitRepoName, branch: row.gitBranch, worktree: row.gitWorktree }
    var place = places[key]
    if (place !== undefined) return place || "—"
    var value = controller.priorityValueFor(key, row.name, row.isDir, row.isSymlink, row.sizeText, row.kind, row.mime, row.modified, row.created)
    var narrow = (key === "modified" || key === "created") && width < Style.space(104)
    return narrow ? String(value).slice(0, 10) : value
  }

  function gitDetailMarker(key) {
    return ({ modified: "M", deleted: "D", "new": "?" })[key] || ""
  }

  function gitDetailCount(key) {
    return ({ modified: row.gitModifiedCount, deleted: row.gitDeletedCount, "new": row.gitUntrackedCount })[key] || 0
  }

  function gitDetailText(key) {
    var marker = gitDetailMarker(key)
    var count = gitDetailCount(key)
    if (count > 0) return marker + String(count)
    if ((key === "modified" && row.gitStatus === "M")
        || (key === "deleted" && row.gitStatus === "D")
        || (key === "new" && row.gitStatus === "?")) return row.gitStatus
    return ""
  }

  function hasGitDetailText() {
    for (var i = 0; i < controller.gitStatusDetails.length; i++)
      if (gitDetailText(String(controller.gitStatusDetails[i])) !== "") return true
    return false
  }

  function gitDetailSummary() {
    var parts = []
    for (var i = 0; i < controller.gitStatusDetails.length; i++) {
      var text = gitDetailText(String(controller.gitStatusDetails[i]))
      if (text !== "") parts.push(text)
    }
    return parts.join(" ")
  }

  readonly property bool selectable: customInteraction || (!gitDeleted && !moreRow && controller.pickerAllowsEntry(path, isDir, mime))
  readonly property bool selected: customInteraction ? customSelected : controller.isSelected(path)
  readonly property bool favorite: controller.isFavorite(path)
  readonly property string entryColor: gitDeleted ? "" : controller.folderColor(path)
  readonly property bool colorsName: entryColor !== "" && controller.folderColorScope !== "icon"
  readonly property bool colorsRow: entryColor !== "" && controller.folderColorScope === "row"
  readonly property color mutedEntryColor: colorsRow ? Util.alpha(entryColor, 0.68) : Color.muted
  readonly property bool showsGitIgnored: controller.gitEnabled && gitIgnored
  readonly property bool colorsGit: controller.gitEnabled && !gitDeleted && !gitIgnored && gitStatus !== ""
  readonly property bool showsRepoSummary: controller.gitEnabled && isGitRepo && treeMode
    && !favoriteMode && !customInteraction && depth === 0 && path === controller.rootPath
    && controller.gitSummaryFields.length > 0
  readonly property var repositorySummary: showsRepoSummary ? GitSummary.describe(gitSummary, controller.gitSummaryFields) : ({ text: "", tooltip: "" })
  readonly property color gitEntryColor: controller.gitStatusColor(gitStatus)

  function repositorySummaryMarkup(parts, separator) {
    return (parts || repositorySummary.tokens || []).map(function(part) {
      var color = part.marker === "ahead" ? Color.accent
        : (part.marker === "behind" ? controller.gitStatusColor("M") : (part.marker ? controller.gitStatusColor(part.marker) : Color.muted))
      return '<font color="' + color + '">' + Highlight.escapeHtml(part.text) + '</font>'
    }).join(separator || " ")
  }

  color: dropHovered
    ? Util.alpha(Color.accent, 0.24)
    : (selected
      ? Color.menu.selectedBackground
      : (row.hovered
        ? (colorsRow ? Util.alpha(entryColor, Style.hoverFillAlpha + 0.08) : Style.hoverFillFor(Color.bar.text, Color.accent))
        : (colorsRow ? Util.alpha(entryColor, 0.10) : "transparent")))
  border.width: dropHovered ? 1 : 0
  border.color: Color.accent
  opacity: dragHandler.active ? 0.62 : (gitDeleted ? 0.78 : (selectable ? 1.0 : 0.38))

  function choose(modifiers) {
    if (ownerView) {
      ownerView.currentIndex = index
      ownerView.forceActiveFocus()
    }
    controller.selectModelIndex(ownerView.model, index, pane.selectionMode(modifiers || Qt.NoModifier))
  }

  function activate() {
    if (gitDeleted) return
    if (moreRow) {
      loadMore()
      return
    }
    if (!controller.isSelected(path)) choose(Qt.NoModifier)
    if (favoriteMode) {
      controller.activateFavorite({ path: path, isDir: isDir }, pane.targetScreen())
      return
    }
    if (isDir) {
      controller.navigateToLocation(path, pane.targetScreen(), "browse")
      return
    }
    pane.activateIndex(ownerView, treeMode, index)
  }

  function openMenu(mode, localX, localY, atRowEdge) {
    if (gitDeleted) return
    if (!controller.isSelected(path)) choose(Qt.NoModifier)
    var edge = pane.context ? String(pane.context.edge || "left") : "left"
    var outward = edge === "right" ? -Style.space(2) : Math.max(1, width - Style.space(2))
    var x = atRowEdge ? outward : Number(localX)
    var y = atRowEdge ? Math.max(1, height - Style.space(2)) : Number(localY)
    var point = row.mapToItem(pane.hostWindow.contentItem, x, y)
    controller.openActionMenu(mode || "actions", pane.targetScreen(), point.x + pane.originX(), point.y, undefined, { edge: edge, keyboard: !!atRowEdge })
  }

  Text {
    textFormat: Text.PlainText
    id: favoriteGlyph
    x: Style.space(3) + row.depth * Style.space(13)
    anchors.verticalCenter: parent.verticalCenter
    width: Style.space(14)
    horizontalAlignment: Text.AlignHCenter
    visible: !row.gitDeleted && !row.moreRow
      && (row.hiddenEntry || (!row.customInteraction && row.favoriteAvailable && (row.favorite || row.hovered)))
    text: row.hiddenEntry ? "󰈉" : (row.favorite ? "" : "☆")
    color: row.favorite ? Color.accent : Color.muted
    font.family: Style.font.family
    font.pixelSize: Typography.bodySmall
  }

  Text {
    textFormat: Text.PlainText
    id: disclosure
    x: favoriteGlyph.x + favoriteGlyph.width
    anchors.verticalCenter: parent.verticalCenter
    width: Style.space(14)
    horizontalAlignment: Text.AlignHCenter
    visible: !row.customInteraction && row.treeMode && !row.favoriteMode && row.isDir
    text: row.loading ? "󰇘" : FileIcons.expanderIcon(row.expanded)
    color: row.error ? Color.urgent : Color.muted
    font.family: Style.font.family
    font.pixelSize: Typography.body
  }

  Text {
    textFormat: Text.PlainText
    id: icon
    x: disclosure.x + disclosure.width + Style.space(1)
    anchors.verticalCenter: parent.verticalCenter
    width: Style.space(17)
    horizontalAlignment: Text.AlignHCenter
    text: row.kind === "Match"
      ? ""
      : (row.moreRow
        ? "󰇘"
        : FileIcons.entryIcon(row.name, row.isDir, row.isSymlink, row.expanded && row.treeMode, row.isGitRepo))
    color: row.urgent
      ? Color.urgent
      : (row.gitDeleted
      ? Color.urgent
      : (row.showsGitIgnored || row.hiddenEntry
        ? row.mutedEntryColor
        : (row.colorsGit
          ? row.gitEntryColor
          : (row.entryColor || (row.isDir ? Color.accent : Color.muted)))))
    font.family: Style.font.family
    font.pixelSize: Typography.body
  }

  Column {
    anchors.left: icon.right
    anchors.leftMargin: Style.space(4)
    anchors.right: repoSummary.visible ? repoSummary.left : (priorityMetadata.visible ? priorityMetadata.left : gitBadge.left)
    anchors.rightMargin: Style.space(5)
    anchors.verticalCenter: parent.verticalCenter
    spacing: 0

    Text {
      width: parent.width
      textFormat: row.nameSpans !== "" ? Text.StyledText : Text.PlainText
      text: row.nameSpans !== ""
        ? Highlight.markup(row.name, Highlight.parseSpans(row.nameSpans), Color.accent)
        : row.name
      color: row.urgent
        ? Color.urgent
        : (row.gitDeleted
        ? Color.urgent
        : (row.moreRow || row.showsGitIgnored || row.hiddenEntry
          ? row.mutedEntryColor
          : (row.colorsGit
            ? row.gitEntryColor
            : (row.colorsName ? row.entryColor : (row.selected && row.isDir ? Color.accent : Color.bar.text)))))
      elide: Text.ElideRight
      font.family: Style.font.family
      font.pixelSize: Typography.body
      font.weight: row.selected && row.isDir ? Font.DemiBold : Font.Normal
      font.strikeout: row.gitDeleted
    }

    Text {
      width: parent.width
      visible: row.pathVisible
      textFormat: row.relativeSpans !== "" ? Text.StyledText : Text.PlainText
      text: row.relativeSpans !== ""
        ? Highlight.markup(row.relative, Highlight.parseSpans(row.relativeSpans), Color.accent)
        : row.relative
      color: row.mutedEntryColor
      elide: Text.ElideMiddle
      font.family: Style.font.family
      font.pixelSize: Typography.caption
    }
  }

  Row {
    id: priorityMetadata
    anchors.right: gitBadge.left
    anchors.rightMargin: Style.space(2) + pane.columnAdderReserve
    anchors.verticalCenter: parent.verticalCenter
    visible: controller.priorityColumns.length > 0
    opacity: row.showsRepoSummary || row.metadataDimmed ? 0 : 1
    spacing: Style.space(2)

    Behavior on opacity {
      NumberAnimation { duration: 90 }
    }

    Repeater {
      model: controller.priorityColumns.length

      delegate: Text {
        required property int index
        readonly property string key: index < controller.priorityColumns.length ? String(controller.priorityColumns[index]) : "none"
        textFormat: Text.PlainText
        width: pane.priorityColumnWidth(row.width, key)
        text: row.priorityText(key, width)
        color: row.colorsRow ? row.mutedEntryColor : (row.selected ? Color.bar.text : Color.muted)
        horizontalAlignment: Text.AlignRight
        verticalAlignment: Text.AlignVCenter
        elide: Text.ElideRight
        font.family: Style.font.family
        font.pixelSize: Typography.caption
        font.weight: row.selected ? Font.Medium : Font.Normal
      }
    }
  }

  Item {
    id: gitBadge
    visible: controller.gitEnabled && !row.showsRepoSummary
    anchors.right: parent.right
    anchors.rightMargin: pane.gitColumnInset
    anchors.verticalCenter: parent.verticalCenter
    width: pane.gitColumnWidth
    height: Style.space(24)

    Row {
      anchors.fill: parent
      spacing: pane.gitDetailSpacing
      visible: !row.showsGitIgnored && row.error === "" && controller.gitStatusDetails.length > 0 && row.hasGitDetailText()

      Repeater {
        model: controller.gitStatusDetails.length

        delegate: Text {
          required property int index
          readonly property string detailKey: String(controller.gitStatusDetails[index])
          textFormat: Text.PlainText
          width: pane.gitDetailSlotWidth
          height: gitBadge.height
          text: row.gitDetailText(detailKey)
          color: controller.gitStatusColor(row.gitDetailMarker(detailKey))
          horizontalAlignment: Text.AlignRight
          verticalAlignment: Text.AlignVCenter
          font.family: Style.font.family
          font.pixelSize: Typography.caption
          font.weight: Font.Bold
        }
      }
    }

    Text {
      anchors.fill: parent
      visible: row.showsGitIgnored || row.error !== "" || controller.gitStatusDetails.length === 0 || !row.hasGitDetailText()
      textFormat: Text.PlainText
      text: row.error ? "" : (row.showsGitIgnored ? "" : row.gitStatus)
      color: row.error ? Color.urgent : (row.showsGitIgnored ? row.mutedEntryColor : controller.gitStatusColor(row.gitStatus))
      horizontalAlignment: Text.AlignHCenter
      verticalAlignment: Text.AlignVCenter
      font.family: Style.font.family
      font.pixelSize: row.error ? Typography.bodySmall : Typography.caption
      font.weight: Font.Bold
    }

    PanelToolTip {
      visible: gitPointer.containsMouse
      text: row.error || (row.gitDetailSummary() !== "" ? row.gitDetailSummary() + " — " + row.gitStatusLabel : row.gitStatusLabel)
    }

    MouseArea {
      id: gitPointer
      anchors.fill: parent
      hoverEnabled: true
      acceptedButtons: Qt.NoButton
    }
  }

  Item {
    id: repoSummary
    objectName: "repositorySummary"
    visible: row.showsRepoSummary
    readonly property real columnEnd: controller.priorityColumns.length > 0
      ? priorityMetadata.x + pane.priorityColumnWidth(row.width, String(controller.priorityColumns[0])) : gitBadge.x
    x: columnEnd - width
    anchors.verticalCenter: parent.verticalCenter
    width: Math.max(0, Math.min(summaryIdentity.implicitWidth + summaryCounts.implicitWidth
      + (summaryIdentity.text && summaryCounts.text ? Style.space(8) : 0), columnEnd - icon.x - icon.width - Style.space(45)))
    height: Math.max(summaryIdentity.implicitHeight, summaryCounts.implicitHeight)

    Text {
      id: summaryIdentity
      anchors.left: parent.left
      anchors.right: summaryCounts.left
      anchors.rightMargin: text && summaryCounts.text ? Style.space(8) : 0
      anchors.verticalCenter: parent.verticalCenter
      textFormat: Text.PlainText
      text: row.repositorySummary.identity || ""
      color: Color.muted
      font.family: Style.font.family
      font.pixelSize: Typography.caption
      elide: Text.ElideMiddle
    }

    Text {
      id: summaryCounts
      anchors.right: parent.right
      anchors.verticalCenter: parent.verticalCenter
      width: Math.min(implicitWidth, Math.max(0, parent.width - (summaryIdentity.text ? Style.space(35) : 0)))
      textFormat: Text.StyledText
      text: row.repositorySummaryMarkup()
      font.family: Style.font.family
      font.pixelSize: Typography.caption
      font.weight: Font.Bold
      horizontalAlignment: Text.AlignRight
      elide: Text.ElideRight
    }

    PanelToolTip {
      id: summaryTip
      visible: row.showsRepoSummary && pointer.containsMouse
        && pointer.mouseX >= repoSummary.x && pointer.mouseX <= repoSummary.x + repoSummary.width
      text: row.repositorySummary.tooltip
      contentItem: Text {
        textFormat: Text.StyledText
        text: row.repositorySummaryMarkup(row.repositorySummary.tooltipTokens || [{ text: summaryTip.text, marker: "" }], "<br>")
        color: summaryTip.panelForeground
        font.family: summaryTip.fontFamily
        font.pixelSize: summaryTip.fontSize
        leftPadding: Border.left(summaryTip.panelBorderSpec) + Style.spacing.controlPaddingX
        rightPadding: Border.right(summaryTip.panelBorderSpec) + Style.spacing.controlPaddingX
        topPadding: Border.top(summaryTip.panelBorderSpec) + Style.spacing.controlPaddingY
        bottomPadding: Border.bottom(summaryTip.panelBorderSpec) + Style.spacing.controlPaddingY
      }
    }
  }

  MouseArea {
    id: pointer
    anchors.fill: parent
    hoverEnabled: true
    acceptedButtons: Qt.LeftButton | Qt.RightButton
    cursorShape: dragHandler.active ? Qt.ClosedHandCursor : Qt.PointingHandCursor

    onClicked: function(mouse) {
      if (row.customInteraction) {
        row.customClicked(mouse)
        return
      }
      if (row.moreRow) {
        row.loadMore()
        return
      }
      if (!row.gitDeleted && row.favoriteAvailable && mouse.x >= favoriteGlyph.x && mouse.x <= favoriteGlyph.x + favoriteGlyph.width) {
        controller.toggleFavorite(row.path, row.name, row.isDir, row.isSymlink, row.mime, row.isGitRepo)
        return
      }
      if (row.gitDeleted) {
        row.choose(mouse.modifiers)
        return
      }
      if (mouse.button === Qt.RightButton) {
        row.openMenu("actions", mouse.x, mouse.y, false)
        return
      }
      row.choose(mouse.modifiers)
      var disclosureEdge = Style.space(38) + row.depth * Style.space(13)
      if (row.treeMode && !row.favoriteMode && row.isDir && mouse.x <= disclosureEdge) controller.toggleDirectory(row.index)
    }

    onDoubleClicked: function(mouse) {
      if (row.customInteraction) {
        row.customDoubleClicked(mouse)
        mouse.accepted = true
        return
      }
      row.activate()
      mouse.accepted = true
    }
  }

  HoverHandler { id: hoverTracker }

  Drag.active: false
  Drag.source: row
  Drag.keys: ["fileblade-entry"]
  Drag.hotSpot.x: dragHandler.centroid.position.x
  Drag.hotSpot.y: dragHandler.centroid.position.y
  Drag.supportedActions: Qt.CopyAction | Qt.MoveAction
  Drag.proposedAction: dragHandler.centroid.modifiers & Qt.ControlModifier ? Qt.CopyAction : Qt.MoveAction
  Drag.mimeData: ({ "text/uri-list": controller.selectionUris(row.draggedPaths) })

  function dragPoint() {
    var scene = dragHandler.centroid.scenePosition
    var window = pane.hostWindow
    var outside = !window || !window.containsScenePoint(scene.x, scene.y)
    var originY = pane.context ? Number(pane.context.surfaceOriginY) || 0 : 0
    return { x: pane.originX() + scene.x, y: originY + scene.y, outside: outside }
  }

  function beginDropDrag() {
    if (pane.context) pane.context.requestFocus("")
    if (ownerView) ownerView.forceActiveFocus()
    var point = dragPoint()
    var docked = !pane.context || pane.context.docked
    var paths = DragPlan.disjointPaths(controller.selectedPaths)
    var entries = DragPlan.entriesForPaths(controller.selectedEntries, paths)
    entries.sort(function(left, right) { return (String(left.path || "") === row.path ? -1 : 0) - (String(right.path || "") === row.path ? -1 : 0) })
    controller.dropWheel.beginDrag(paths, entries, pane.targetScreen(), docked, point.x, point.y)
  }

  function updateDropDrag() {
    var point = dragPoint()
    controller.dropWheel.updateDrag(point.x, point.y, point.outside, dragHandler.centroid.modifiers)
    updateDragScroll()
  }

  function updateDragScroll() {
    if (!ownerView || ownerView.height <= 0) {
      dragScrollStep = 0
      return
    }
    var point = ownerView.mapFromItem(null, dragHandler.centroid.scenePosition.x, dragHandler.centroid.scenePosition.y)
    var fraction = point.y / ownerView.height
    dragScrollStep = fraction <= 0.05 ? -8 : (fraction <= 0.15 ? -5 : (fraction >= 0.95 ? 8 : (fraction >= 0.85 ? 5 : 0)))
  }

  function scrollDraggedList() {
    if (!ownerView || dragScrollStep === 0) return
    var minimum = Number(ownerView.originY) || 0
    var maximum = minimum + Math.max(0, ownerView.contentHeight - ownerView.height)
    ownerView.contentY = Math.max(minimum, Math.min(maximum, ownerView.contentY + dragScrollStep))
  }

  DragHandler {
    id: dragHandler
    target: null
    acceptedButtons: Qt.LeftButton
    enabled: !row.customInteraction && row.selectable && !row.gitDeleted
    cursorShape: active ? Qt.ClosedHandCursor : Qt.PointingHandCursor
    onActiveChanged: {
      if (active) {
        row.dragCanceled = false
        if (!controller.isSelected(row.path)) row.choose(Qt.NoModifier)
        row.beginDropDrag()
        row.Drag.active = true
        return
      }
      row.dragScrollStep = 0
      if (row.Drag.active) {
        if (row.dragCanceled) row.Drag.cancel()
        else row.Drag.drop()
      }
      controller.dropWheel.endDrag(undefined, undefined, undefined)
    }
    onCanceled: row.dragCanceled = true
    onCentroidChanged: if (active) row.updateDropDrag()
  }

  BrowserDropTarget {
    id: dropTarget
    anchors.fill: parent
    controller: row.controller
    rowItem: row
  }

  Timer {
    interval: 16
    repeat: true
    running: dragHandler.active && row.dragScrollStep !== 0
    onTriggered: row.scrollDraggedList()
  }
}
