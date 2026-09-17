import QtQuick
import "../lib/PathText.js" as PathText
import qs.Commons
import "../lib/FileIcons.js" as FileIcons
import "../lib/Format.js" as Format
import "../lib/SearchQuery.js" as SearchQuery
import "../lib/ArtifactTreeFolders.js" as ArtifactTreeFolders
import "../lib/KeyRouter.js" as KeyRouter
import "../lib/KeyBindings.js" as KeyBindings
import "../lib/KeyedRows.js" as KeyedRows
import "../lib/ScrollMarks.js" as ScrollMarks
import "../controllers" as Controllers
import "../theme"

FocusScope {
  id: tree

  property var items: []
  property var view: null
  property var context: null
  readonly property var files: context ? context.service("files") : null
  property var groupsFor: function(item) { return [] }
  property var leafLabel: function(item) { return String(item.name || "") }
  property var leafDetail: function(item) { return String(item.detail || "") }
  property var leafGlyph: function(item) { return tree.defaultGlyph(item) }
  property bool expandableItems: false
  property bool loadFolderChildren: false
  property var childrenFor: function(item) { return directories.children(tree.itemPath(item)) }
  property var editPathFor: function(item) { return item && item.path ? String(item.path) : "" }
  property var fileActionsFor: function(item) { return true }
  property var dropSpec: function(item) { return null }
  property var groupBadge: function(path) { return "" }
  property var groupGlyph: function(path) { return "" }
  property var groupGlyphStruck: function(path) { return false }
  property var searchText: function(item) { return String(item.name || "") + " " + String(item.detail || "") + " " + String(item.path || "") }
  property var searchFields: function(item) { return ({}) }
  property var filterKeys: []
  property var specialMetricValue: function(item, key) { return undefined }
  property var appliedAgents: function(item) { return Array.isArray(item.agents) ? item.agents : [] }
  property var rowAction: function(item) { return null }
  property var rowMark: function(item) { return tree.defaultMark(item) }
  property var keyHandler: null
  ActionKeyGuard { id: actionKeys; active: tree.activeFocus; shared: tree.context && tree.context.hostWindow ? tree.context.hostWindow.actionKeys : null }
  TreeKeys {
    id: treeKeys
    active: tree.activeFocus
    scope: "artifacts"
    plan: tree.files ? tree.files.keybindings.plan : KeyBindings.compile({})
  }
  property var installedAgents: []
  property var collapsed: ({})
  property var expandedFolders: ({})
  property int childrenRevision: 0
  readonly property string folderError: expansion.error || directories.error
  property var expansionScope: null
  property string query: ""
  property bool caseSensitive: false
  property bool regex: false
  property color surfaceColor: Color.bar.background
  property int currentIndex: 0
  property string cursorKey: ""
  property bool syncingRows: false
  property var marks: []
  readonly property string metricKey: view ? view.metricKey : "off"
  readonly property string metricKind: view ? view.metricKind : "text"
  readonly property string sortKey: view ? view.sortKey : ""
  readonly property bool sortDescending: view ? view.sortDescending : false
  readonly property var filter: view ? view.filter : ({})
  readonly property bool searching: String(query).trim().length > 0
  readonly property var querySpec: SearchQuery.parse(query, filterKeys, { caseSensitive: caseSensitive, regex: regex })
  readonly property var visibleItems: filterItems()
  readonly property real metricMaximum: maximumMetric()
  readonly property var rows: buildRows()

  signal activated(var item)
  signal revealed(var item)
  signal searchRequested()
  signal filterRequested()
  signal focusNextRequested()
  signal focusPreviousRequested()
  signal dismissRequested()
  signal agentToggled(var item, string agentId, bool on)
  signal agentsAllRequested(var item, bool on)
  signal actionRequested(var item)
  signal folderToggled(var item, bool expanded)
  signal changed(var response)

  ArtifactDirectories {
    id: directories
    files: tree.files
    active: tree.loadFolderChildren && tree.expandableItems && tree.visible
    paths: tree.rows.filter(function(row) { return row.kind === "leaf" && ArtifactTreeFolders.expanded(tree, row.item) })
      .map(function(row) { return tree.itemPath(row.item) })
  }

  Controllers.TreeExpansion {
    id: expansion
    active: tree.visible
    revision: tree.rows
    rowCount: tree.rows.length
    loading: tree.loadFolderChildren && !directories.limited && directories.paths.some(function(path) {
      return directories.cache[path] === undefined
    })
    nextEntry: function() { return ArtifactTreeFolders.nextBranchEntry(tree, tree.expansionScope) }
    expandEntry: function(entry) {
      if (entry.group) tree.toggleGroup(entry.key, true)
      else ArtifactTreeFolders.toggle(tree, entry.item)
    }
  }

  function refreshFolders() { directories.refresh(true) }
  onItemsChanged: { treeKeys.reset(); expansion.stop(true); ArtifactTreeFolders.prune(tree); directories.refresh() }

  function itemPath(item) {
    if (!item) return ""
    if (item.path) return String(item.path)
    return item.source && item.source.path ? String(item.source.path) : ""
  }

  function linkTarget(item) {
    if (!item) return ""
    var source = item.source && typeof item.source === "object" ? item.source : ({})
    var target = item.linkTarget || item.link_target || source.linkTarget || source.link_target || ""
    if (!target && !isBinned(item)) target = item.realpath || source.realpath || ""
    target = String(target)
    if (itemPath(item).slice(-9) === "/SKILL.md" && target.slice(-9) === "/SKILL.md") target = target.slice(0, -9)
    return target !== itemPath(item) ? target : ""
  }
  function isLinked(item) {
    if (!item) return false
    var source = item.source && typeof item.source === "object" ? item.source : ({})
    return !!(item.linkTarget || item.link_target || item.is_symlink || item.isSymlink || item.alias === true
      || source.linkTarget || source.link_target || source.is_symlink || source.isSymlink
      || (!isBinned(item) && linkTarget(item)))
  }

  function isBinned(item) {
    return !!item && String(item.kind || "") === "bin"
  }
  function defaultMark(item) {
    if (!item || item.gitIgnored || item.git_ignored) return ""
    var source = item.source && typeof item.source === "object" ? item.source : ({})
    return String(item.gitStatus || item.git_status || source.gitStatus || source.git_status || "")
  }
  function markStatus(row) {
    return row && row.kind === "leaf" ? String(tree.rowMark(row.item) || "") : ""
  }
  function markColor(status) {
    return files && typeof files.gitStatusColor === "function" ? files.gitStatusColor(status) : Color.accent
  }
  readonly property bool marksEnabled: !files || files.scrollMarks !== false
  onMarksEnabledChanged: markRefresh.restart()
  function refreshMarks() {
    var collected = marksEnabled ? ScrollMarks.collect(rows, ruler.slots, markStatus) : []
    var next = []
    for (var i = 0; i < collected.length; i++) next.push({ fraction: collected[i].fraction, color: markColor(collected[i].status) })
    marks = next
  }
  function defaultGlyph(item) {
    if (!item) return ""
    var path = String(item.path || "")
    var name = path ? path.slice(path.lastIndexOf("/") + 1) : String(item.name || "")
    var isDir = !!(item.is_dir || item.isDir)
    return FileIcons.entryIcon(name, isDir, tree.isLinked(item), ArtifactTreeFolders.expanded(tree, item), !!(item.is_git_repo || item.isGitRepo))
  }
  function dropEntries(item, spec) {
    var rows = spec && Array.isArray(spec.entries) && spec.entries.length > 0 ? spec.entries : [item]
    var result = []
    for (var i = 0; i < rows.length; i++) {
      var row = rows[i] || {}
      var path = String(row.path || item.path || "")
      var name = String(row.name || path.slice(path.lastIndexOf("/") + 1))
      var isDir = !!(row.is_dir || row.isDir), isSymlink = tree.isLinked(row)
      result.push({ name: name, isDir: isDir, isSymlink: isSymlink,
        glyph: isSymlink ? FileIcons.entryIcon(name, isDir, true, false, false) : String(row.glyph || (spec && spec.glyph) || tree.leafGlyph(item) || "")
      })
    }
    return result
  }

  function dragPoint(scene) {
    var window = context ? context.hostWindow : null
    var outside = !window || !window.containsScenePoint(scene.x, scene.y)
    var originX = context ? Number(context.surfaceOriginX) || 0 : 0
    var originY = context ? Number(context.surfaceOriginY) || 0 : 0
    return { x: originX + scene.x, y: originY + scene.y, outside: outside }
  }

  function beginDropDrag(item, scene) {
    var wheel = files ? files.dropWheel : null
    if (!wheel || !entryFor(item)) return false
    var spec = tree.dropSpec(item) || null
    var paths = spec && Array.isArray(spec.paths) && spec.paths.length > 0 ? spec.paths : [String(item.path)]
    var point = dragPoint(scene)
    tree.forceActiveFocus()
    return wheel.beginDrag(paths, dropEntries(item, spec), context ? context.screen : null, !context || context.docked, point.x, point.y, spec)
  }

  function updateDropDrag(scene, modifiers) {
    var wheel = files ? files.dropWheel : null
    if (!wheel) return
    var point = dragPoint(scene)
    wheel.updateDrag(point.x, point.y, point.outside, modifiers)
  }

  function endDropDrag() {
    var wheel = files ? files.dropWheel : null
    if (wheel) wheel.endDrag(undefined, undefined, undefined)
  }

  function entryFor(item) {
    if (!item || !item.path || isBinned(item) || !fileActionsFor(item)) return null
    return {
      path: String(item.path),
      name: String(item.name || ""),
      isDir: !!(item.is_dir || item.isDir),
      isSymlink: tree.isLinked(item),
      kind: String(item.kind || ""),
      mime: String(item.mime || "")
    }
  }

  function openMenu(item, rowItem, x, y, mode, keyboard) {
    if (isBinned(item)) {
      if (mode && mode !== "actions") return false
      tree.actionRequested(item)
      return true
    }
    var entry = entryFor(item)
    if (!entry || !files || !context) return false
    var origin = rowItem || tree
    var host = context.hostWindow && context.hostWindow.contentItem ? context.hostWindow.contentItem : tree
    var point = origin.mapToItem(host, Number(x) || 0, Number(y) || 0)
    var placement = { edge: String(context.edge || "left"), keyboard: !!keyboard }
    files.openActionMenu(mode || "actions", context.screen, point.x + (Number(context.surfaceOriginX) || 0), point.y, [entry], placement)
    return true
  }

  function currentLeaf() {
    var current = rowAt(currentIndex)
    return current && current.kind !== "group" ? current.item : null
  }

  function menuCurrent(mode) {
    var item = currentLeaf()
    if (!item) return false
    var rowItem = list.itemAtIndex(currentIndex)
    var rightBlade = context && String(context.edge || "") === "right"
    var x = rightBlade ? -Style.space(2) : (rowItem ? Math.max(1, rowItem.width - Style.space(2)) : tree.width)
    var y = rowItem ? Math.max(1, rowItem.height - Style.space(2)) : 0
    return openMenu(item, rowItem, x, y, mode, true)
  }

  function clipboardCurrent(cut) {
    var entry = entryFor(currentLeaf())
    if (!entry || !files) return false
    return files.copySelection(!!cut, [entry.path])
  }

  function editCurrent() {
    var item = currentLeaf()
    var path = item && !isBinned(item) ? String(editPathFor(item) || "") : ""
    if (!files || !path) return false
    files.openInEditor(path)
    return true
  }

  function touches(response) {
    var value = response && typeof response === "object" ? response : ({})
    var paths = Array.isArray(value.paths) ? value.paths.slice() : []
    var mappings = Array.isArray(value.mappings) ? value.mappings : []
    for (var i = 0; i < mappings.length; i++) paths.push(String(mappings[i].source || ""), String(mappings[i].destination || ""))
    if (value.path) paths.push(String(value.path))
    for (var j = 0; j < items.length; j++) {
      var own = String(items[j] && items[j].path || "")
      if (!own) continue
      for (var k = 0; k < paths.length; k++) {
        var other = String(paths[k] || "")
        if (other && (own === other || PathText.within(own, other) || PathText.within(other, own))) return true
      }
    }
    return false
  }

  Connections {
    target: tree.files ? tree.files.history : null
    function onOperationCompleted(response) { if (tree.touches(response)) tree.changed(response) }
  }

  Connections {
    target: tree.view
    ignoreUnknownSignals: true
    function onNavigationTriggered(key) {
      var actions = {
        search: function() { tree.searchRequested() },
        filter: function() { tree.filterRequested() }
      }
      if (actions[key]) actions[key]()
    }
  }

  function groupKey(path) {
    return JSON.stringify(path)
  }

  function rawMetric(item, key) {
    if (!item || key === "off") return undefined
    if (key === "name") return leafLabel(item)
    if (key === "summary") return leafDetail(item)
    var special = specialMetricValue(item, key)
    if (special !== undefined && special !== null) return special
    var metrics = item.metrics && typeof item.metrics === "object" ? item.metrics : ({})
    return metrics[key]
  }

  function metricTextFor(item, key) {
    var value = rawMetric(item, key)
    var kind = view ? view.kindOf(key) : "text"
    if (key === "off" || kind === "agents") return ""
    if (key === "bytes") return Format.bytes(value)
    if (kind === "number") return Format.compact(value)
    return value === undefined || value === null || value === "" ? "—" : String(value)
  }

  function metricText(item) {
    return metricTextFor(item, metricKey)
  }

  function extraKeys() {
    return view && Array.isArray(view.columns) ? view.columns.slice(1) : []
  }

  function extraTexts(item) {
    var keys = extraKeys()
    var result = []
    for (var i = 0; i < keys.length; i++) result.push(metricTextFor(item, keys[i]))
    return result
  }

  function columnWidths() {
    if (!view || !Array.isArray(view.columns)) return []
    var result = []
    for (var i = 0; i < view.columns.length; i++) result.push(view.columnWidthFor(view.columns[i]))
    return result
  }

  function extraGroupTexts(row) {
    var keys = extraKeys()
    var result = []
    for (var i = 0; i < keys.length; i++) {
      var kind = view ? view.kindOf(keys[i]) : "text"
      if (kind !== "number") {
        result.push("")
        continue
      }
      var total = groupTotalFor(row.path, keys[i])
      result.push(keys[i] === "bytes" ? Format.bytes(total) : Format.compact(total))
    }
    return result
  }

  function maximumMetric() {
    if (metricKind !== "number") return 0
    var maximum = 0
    for (var i = 0; i < visibleItems.length; i++) {
      var value = Number(rawMetric(visibleItems[i], metricKey))
      if (isFinite(value) && value > maximum) maximum = value
    }
    return maximum
  }

  function barFraction(item) {
    if (metricKind !== "number" || metricMaximum <= 0) return -1
    var value = Number(rawMetric(item, metricKey))
    return isFinite(value) && value >= 0 ? value / metricMaximum : -1
  }

  function groupTotalFor(path, key) {
    var total = 0
    for (var i = 0; i < visibleItems.length; i++) {
      var groups = groupsFor(visibleItems[i]) || []
      if (groups.length < path.length || groupKey(groups.slice(0, path.length)) !== groupKey(path)) continue
      var value = Number(rawMetric(visibleItems[i], key))
      if (isFinite(value) && value > 0) total += value
    }
    return total
  }

  function groupTotal(path) {
    return groupTotalFor(path, metricKey)
  }

  function groupText(row) {
    if (metricKind !== "number") return String(row.badge || "")
    var total = groupTotal(row.path)
    return metricKey === "bytes" ? Format.bytes(total) : Format.compact(total)
  }

  function isCollapsed(key) {
    return !searching && collapsed[key] === true
  }

  function toggleGroup(key, recursive) {
    if (!recursive) expansion.stop(false)
    var next = ({})
    for (var existing in collapsed) next[existing] = collapsed[existing]
    next[key] = collapsed[key] !== true
    collapsed = next
  }

  function matches(item) {
    if (querySpec.empty) return true
    return SearchQuery.matches(querySpec, { name: leafLabel(item), text: searchText(item), fields: searchFields(item) })
  }

  function clauseMatches(item, key, clause) {
    var raw = rawMetric(item, key)
    if (raw === undefined || raw === null || raw === "") return false
    if (clause.since !== undefined || clause.until !== undefined) {
      var day = String(raw).slice(0, 10)
      return (clause.since === undefined || day >= String(clause.since))
        && (clause.until === undefined || day <= String(clause.until))
    }
    var number = Number(raw)
    if (!isFinite(number)) return false
    return (clause.min === undefined || number >= Number(clause.min)) && (clause.max === undefined || number <= Number(clause.max))
  }

  function passesFilter(item) {
    for (var key in filter)
      if (!clauseMatches(item, key, filter[key])) return false
    return true
  }

  function filterItems() {
    var result = []
    for (var i = 0; i < items.length; i++)
      if (matches(items[i]) && passesFilter(items[i])) result.push(items[i])
    return result
  }

  function compareBy(left, right, sort) {
    var a = rawMetric(left, sort.key)
    var b = rawMetric(right, sort.key)
    var missingA = a === undefined || a === null || a === ""
    var missingB = b === undefined || b === null || b === ""
    if (missingA || missingB) return missingA === missingB ? 0 : (missingA ? 1 : -1)
    var result = view.kindOf(sort.key) === "number"
      ? Number(a) - Number(b)
      : String(a).localeCompare(String(b), undefined, { sensitivity: "base", numeric: true })
    return sort.desc ? -result : result
  }

  function compareLeaves(left, right) {
    var relevance = SearchQuery.compareRanks(searchRank(left), searchRank(right))
    if (relevance !== 0) return relevance
    var sorts = view ? view.sorts : []
    for (var i = 0; i < sorts.length; i++) {
      var result = compareBy(left, right, sorts[i])
      if (result !== 0) return result
    }
    return 0
  }

  function sortedEntries(entries) {
    if (!searching && (sortKey === "" || !view)) return entries
    var indexed = entries.map(function(item, index) { return { item: item, index: index } })
    indexed.sort(function(left, right) { return compareLeaves(left.item, right.item) || left.index - right.index })
    return indexed.map(function(entry) { return entry.item })
  }

  function searchRank(item) {
    return SearchQuery.rank(querySpec, { name: leafLabel(item), text: searchText(item), fields: searchFields(item) })
  }

  function groupedItems() {
    var order = []
    var buckets = ({})
    var list = visibleItems
    for (var i = 0; i < list.length; i++) {
      var path = groupsFor(list[i]) || []
      var key = groupKey(path)
      if (!buckets[key]) {
        buckets[key] = { path: path, entries: [], index: order.length, rank: null }
        order.push(key)
      }
      buckets[key].entries.push(list[i])
      var rank = searchRank(list[i])
      if (SearchQuery.compareRanks(rank, buckets[key].rank) < 0) buckets[key].rank = rank
    }
    order.sort(function(left, right) {
      var leftBucket = buckets[left]
      var rightBucket = buckets[right]
      var ranked = topGroupRank(leftBucket.path) - topGroupRank(rightBucket.path)
      return ranked !== 0 ? ranked : leftBucket.index - rightBucket.index
    })
    if (searching) order = SearchQuery.rankedGroups(order.map(function(key) { return buckets[key] }))
      .map(function(bucket) { return groupKey(bucket.path) })
    return { order: order, buckets: buckets }
  }

  function topGroupRank(path) {
    var scope = path.length > 0 ? String(path[0]).toUpperCase() : ""
    if (scope === "USER") return 0
    if (scope === "PROJECT") return 1
    return 2
  }

  function pushGroupRows(result, path, previous) {
    var shared = 0
    while (shared < path.length && shared < previous.length && path[shared] === previous[shared]) shared++
    for (var depth = shared; depth < path.length; depth++) {
      var branch = path.slice(0, depth + 1)
      result.push({
        kind: "group", label: path[depth], depth: depth, key: groupKey(branch), path: branch,
        badge: String(groupBadge(branch) || ""), item: null
      })
    }
  }

  function hiddenByAncestor(path) {
    for (var depth = 0; depth < path.length; depth++)
      if (isCollapsed(groupKey(path.slice(0, depth + 1)))) return depth
    return -1
  }

  function buildRows() {
    var grouped = groupedItems()
    var result = []
    var previous = []
    for (var i = 0; i < grouped.order.length; i++) {
      var bucket = grouped.buckets[grouped.order[i]]
      var cut = hiddenByAncestor(bucket.path)
      var visible = cut < 0 ? bucket.path : bucket.path.slice(0, cut + 1)
      pushGroupRows(result, visible, previous)
      previous = visible
      if (cut >= 0) continue
      var entries = sortedEntries(bucket.entries)
      for (var j = 0; j < entries.length; j++)
        appendItemRows(result, entries[j], bucket.path.length, false)
    }
    return result
  }

  function appendItemRows(result, item, depth, child) { ArtifactTreeFolders.appendRows(tree, result, item, depth, child) }

  function rowAt(index) {
    return index >= 0 && index < rows.length ? rows[index] : null
  }

  function rowFields(row) {
    return { rowKind: String(row.kind || ""), rowChild: row.child === true, rowDepth: Number(row.depth) || 0,
             groupKey: String(row.key || ""), groupLabel: String(row.label || "") }
  }

  function syncRows() { KeyedRows.sync(rowModel, rows, rowKey, rowFields) }

  function rowKey(row) {
    if (!row) return ""
    return row.kind === "group" ? "group:" + row.key
      : JSON.stringify(["leaf", String(row.item.sourceId || row.item.id || itemPath(row.item) || row.item.name || ""), row.depth])
  }

  function showCurrent() {
    if (list && currentIndex >= 0 && currentIndex < rows.length) list.positionViewAtIndex(currentIndex, ListView.Contain)
  }

  onCurrentIndexChanged: {
    cursorKey = rowKey(rowAt(currentIndex))
    if (!syncingRows) Qt.callLater(showCurrent)
  }

  Component.onCompleted: { syncRows(); markRefresh.restart() }

  onRowsChanged: {
    anchor.capture()
    syncRows()
    var next = Math.max(0, Math.min(currentIndex, rows.length - 1))
    var kept = false
    if (cursorKey) {
      for (var i = 0; i < rows.length; i++) {
        if (rowKey(rows[i]) === cursorKey) { next = i; kept = true; break }
      }
    }
    syncingRows = true
    currentIndex = next
    syncingRows = false
    cursorKey = rowKey(rowAt(currentIndex))
    if (anchor.pending) anchor.restore()
    if (!kept) Qt.callLater(showCurrent)
    markRefresh.restart()
  }

  ListAnchor {
    id: anchor
    view: list
    indexOf: function(key) {
      for (var i = 0; i < rowModel.count; i++)
        if (rowModel.get(i).rowKey === key) return i
      return -1
    }
    keyAt: function(index) { return index >= 0 && index < rowModel.count ? String(rowModel.get(index).rowKey) : "" }
  }

  Timer {
    id: markRefresh
    interval: 40
    onTriggered: tree.refreshMarks()
  }

  function move(delta) {
    if (rows.length === 0) return
    currentIndex = Math.max(0, Math.min(rows.length - 1, currentIndex + delta))
    list.positionViewAtIndex(currentIndex, ListView.Contain)
  }

  function movePage(direction) {
    var count = Math.max(1, Math.floor(list.height / Math.max(1, Style.space(30)) * 0.8))
    move(direction * count)
  }

  function activateCurrent() {
    var row = rowAt(currentIndex)
    if (!row) return
    if (row.kind === "group") toggleGroup(row.key)
    else activateEntry(row.item)
  }

  function activateEntry(item) { expansion.stop(false); ArtifactTreeFolders.activate(tree, item) }

  function collapseCurrent() { expansion.stop(false); ArtifactTreeFolders.collapse(tree) }

  function expandCurrent() { expansion.stop(false); ArtifactTreeFolders.expand(tree) }

  function setBranchExpanded(expanded, all) {
    var scope = ArtifactTreeFolders.branchScope(tree, all)
    if (!scope) return false
    expansion.stop(true)
    if (expanded) {
      expansionScope = scope
      expansion.start()
    } else {
      if (all) currentIndex = 0
      ArtifactTreeFolders.collapseBranch(tree, scope)
    }
    return true
  }

  function openCurrent() {
    expansion.stop(false)
    var row = rowAt(currentIndex)
    if (!row) return false
    if (row.kind === "group") {
      if (isCollapsed(row.key)) expandCurrent()
      else {
        var child = rowAt(currentIndex + 1)
        if (child && child.depth > row.depth) move(1)
      }
      return true
    }
    if (ArtifactTreeFolders.isFolder(tree, row.item) && files && context)
      files.navigateToLocation(itemPath(row.item), context.screen, "browse")
    else if (isBinned(row.item)) tree.actionRequested(row.item)
    else tree.activated(row.item)
    return true
  }

  function revealCurrent() {
    var row = rowAt(currentIndex)
    if (row && row.kind === "leaf") tree.revealed(row.item)
  }

  function actionCurrent() {
    var row = rowAt(currentIndex)
    if (!row || row.kind !== "leaf" || !tree.rowAction(row.item)) return false
    tree.actionRequested(row.item)
    return true
  }

  function firstLeafIndex() {
    for (var i = 0; i < rows.length; i++)
      if (rows[i].kind === "leaf") return i
    return 0
  }

  onQueryChanged: { treeKeys.reset(); currentIndex = firstLeafIndex() }

  function keyAction(event) { return KeyRouter.artifactAction(event) }

  function runAction(action) {
    var handlers = {
      search: function() { tree.searchRequested() },
      help: function() { if (context && context.hostWindow) context.hostWindow.shortcutsOpen = true },
      filter: function() { tree.filterRequested() },
      sort: function() { if (tree.view) tree.view.toggleSort(tree.metricKey) },
      "focus-next": function() { tree.focusNextRequested() },
      "focus-previous": function() { tree.focusPreviousRequested() },
      dismiss: function() { tree.dismissRequested() },
      next: function() { move(1) },
      previous: function() { move(-1) },
      "page-next": function() { movePage(1) },
      "page-previous": function() { movePage(-1) },
      first: function() { currentIndex = 0 },
      last: function() { currentIndex = Math.max(0, rows.length - 1) },
      collapse: function() { collapseCurrent() },
      expand: function() { expandCurrent() },
      up: function() { ArtifactTreeFolders.parent(tree) },
      "expand-recursive": function() { return setBranchExpanded(true) },
      "collapse-recursive": function() { return setBranchExpanded(false) },
      "expand-all": function() { return setBranchExpanded(true, true) },
      "collapse-all": function() { return setBranchExpanded(false, true) },
      activate: function() { activateCurrent() },
      open: function() { return openCurrent() },
      reveal: function() { revealCurrent() },
      action: function() { return actionCurrent() },
      menu: function() { return menuCurrent("actions") },
      rename: function() { return menuCurrent("rename") },
      "open-with": function() { return menuCurrent("open-with") },
      copy: function() { return clipboardCurrent(false) },
      cut: function() { return clipboardCurrent(true) },
      edit: function() { return editCurrent() }
    }
    var handler = handlers[action]
    if (!handler) return false
    return handler() !== false
  }

  Keys.onPressed: function(event) {
    if (files && files.dropWheel && files.dropWheel.handleDragKey(event)) {
      treeKeys.reset()
      event.accepted = true
      return
    }
    var repeated = actionKeys.isRepeat(event)
    var fallback = keyAction(event)
    var action = treeKeys.action(event, repeated, fallback)
    if (action.indexOf("key-") === 0) { event.accepted = true; return }
    if (action === fallback && typeof keyHandler === "function" && keyHandler(event, repeated)) {
      event.accepted = true
      return
    }
    if (KeyRouter.ignoresAutoRepeat(action, event.key) && repeated) {
      event.accepted = true
      return
    }
    if (runAction(action)) event.accepted = true
  }

  Keys.onReleased: function(event) {
    actionKeys.release(event)
    if (files && files.dropWheel && files.dropWheel.handleDragKeyRelease(event)) event.accepted = true
  }

  ListView {
    id: list
    anchors.fill: parent
    clip: true
    model: ListModel { id: rowModel }
    currentIndex: tree.currentIndex
    boundsBehavior: Flickable.StopAtBounds
    onMovementStarted: anchor.clear()
    onFlickStarted: anchor.clear()

    delegate: PaneRow {
      id: row
      required property int index
      required property string rowKey
      required property string rowKind
      required property bool rowChild
      required property int rowDepth
      required property string groupKey
      required property string groupLabel
      readonly property var entry: tree.rowAt(index) ? tree.rowAt(index).item : null
      readonly property bool isGroup: rowKind === "group"
      readonly property bool folderGroup: isGroup && rowDepth > 0
      readonly property bool isChild: rowChild
      readonly property bool showsAgents: !!entry && !isGroup && !isChild && !tree.isBinned(entry) && tree.metricKind === "agents" && tree.installedAgents.length > 0
      width: list.width
      emphasized: isGroup && !folderGroup
      indent: rowDepth * Style.space(13)
      expanderReserved: !isGroup && (rowDepth >= 2 || isChild)
      glyphStruck: folderGroup && tree.groupGlyphStruck((tree.rowAt(index) || {}).path || []) === true
      expander: folderGroup ? (tree.isCollapsed(groupKey) ? "" : "")
        : (!isGroup && ArtifactTreeFolders.isFolder(tree, entry) ? (ArtifactTreeFolders.expanded(tree, entry) ? "" : "") : "")
      glyph: folderGroup ? (String(tree.groupGlyph((tree.rowAt(index) || {}).path || []) || "") || FileIcons.folderIcon(!tree.isCollapsed(groupKey)))
        : (isGroup ? (tree.isCollapsed(groupKey) ? "›" : "⌄") : (entry ? String(tree.leafGlyph(entry) || "") : ""))
      glyphColor: (folderGroup && !glyphStruck) || (!isGroup && entry && (entry.is_dir || entry.isDir)) ? Color.accent : Color.muted
      label: folderGroup ? groupLabel : (isGroup ? groupLabel.toUpperCase() : (entry ? tree.leafLabel(entry) : ""))
      badge: isGroup ? tree.groupText(tree.rowAt(index)) : (isChild ? "" : tree.metricText(entry))
      extras: isGroup ? tree.extraGroupTexts(tree.rowAt(index)) : (isChild ? [] : tree.extraTexts(entry))
      columnWidths: tree.columnWidths()
      metricRightMargin: tree.view && tree.view.metricRightMargin !== undefined
        ? tree.view.metricRightMargin
        : Style.space(7)
      barFraction: isGroup || isChild ? -1 : tree.barFraction(entry)
      barColumn: tree.metricKind === "number"
      linked: !isGroup && tree.isLinked(entry)
      linkOnRight: !linked
      struck: !isGroup && !isChild && tree.isBinned(entry)
      trailing: showsAgents ? agentStripFor : null
      actions: !isGroup && !isChild && !!tree.rowAction(entry) ? rowActionFor : null
      actionsVisible: rowHover.hovered || (index === tree.currentIndex && tree.activeFocus)
      current: index === tree.currentIndex
      focused: tree.activeFocus
      hovered: rowHover.hovered

      HoverHandler {
        id: rowHover
        blocking: false
      }

      HintTip {
        visible: row.linkHovered && !row.isGroup && tree.isLinked(row.entry)
        anchorItem: row.linkAnchor
        title: "Symlink"
        context: [{ text: tree.linkTarget(row.entry) }]
      }

      DragHandler {
        id: rowDrag
        target: null
        acceptedButtons: Qt.LeftButton
        enabled: !row.isGroup && !!tree.entryFor(row.entry)
        cursorShape: active ? Qt.ClosedHandCursor : Qt.PointingHandCursor
        onActiveChanged: {
          if (active) {
            tree.currentIndex = row.index
            tree.beginDropDrag(row.entry, centroid.scenePosition)
            return
          }
          tree.endDropDrag()
        }
        onCentroidChanged: if (active) tree.updateDropDrag(centroid.scenePosition, centroid.modifiers)
      }

      readonly property Component rowActionFor: Component {
        Item {
          readonly property var spec: tree.rowAction(row.entry) || ({})
          implicitWidth: Style.space(14)
          implicitHeight: Style.space(16)

          Text {
            textFormat: Text.PlainText
            anchors.centerIn: parent
            text: String(spec.glyph || "")
            color: spec.danger ? Color.urgent : Color.accent
            opacity: actionPointer.containsMouse ? 1 : 0.8
            font.family: Style.font.family
            font.pixelSize: Typography.bodySmall
          }

          MouseArea {
            id: actionPointer
            anchors.fill: parent
            anchors.margins: -Style.space(3)
            hoverEnabled: true
            cursorShape: Qt.PointingHandCursor
            onClicked: {
              tree.currentIndex = row.index
              tree.actionRequested(row.entry)
            }
          }

          HintTip {
            visible: actionPointer.containsMouse
            title: String(spec.title || "")
            actions: [{ button: "left", text: String(spec.title || "") }, { shortcut: "d" }]
          }
        }
      }

      readonly property Component agentStripFor: Component {
        AgentStrip {
          installed: tree.installedAgents
          applied: row.entry ? tree.appliedAgents(row.entry) : []
          onToggled: function(agentId, on) { tree.agentToggled(row.entry, agentId, on) }
          onAllRequested: function(on) { tree.agentsAllRequested(row.entry, on) }
        }
      }

      MouseArea {
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.RightButton
        cursorShape: rowDrag.active ? Qt.ClosedHandCursor : Qt.PointingHandCursor
        onClicked: function(mouse) {
          tree.currentIndex = row.index
          tree.forceActiveFocus()
          if (mouse.button === Qt.RightButton) {
            if (!row.isGroup) tree.openMenu(row.entry, row, mouse.x, mouse.y, "actions")
            return
          }
          if (row.isGroup) tree.toggleGroup(row.groupKey)
          else if (ArtifactTreeFolders.isFolder(tree, row.entry)
                   && mouse.x <= Style.space(38) + row.indent) tree.activateEntry(row.entry)
        }
        onDoubleClicked: {
          tree.currentIndex = row.index
          if (!row.isGroup) tree.activateEntry(row.entry)
        }
      }
    }
  }

  ScrollEdgeFade {
    anchors.fill: list
    flickable: list
    surfaceColor: tree.surfaceColor
    z: 2
  }

  MarkedScrollBar {
    id: ruler
    anchors.right: list.right
    anchors.rightMargin: Style.space(2)
    anchors.top: list.top
    anchors.bottom: list.bottom
    flickable: list
    marks: tree.marks
    z: 3
    onHeightChanged: markRefresh.restart()
  }
}
