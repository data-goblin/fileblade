import QtQuick
import "../lib/PathText.js" as PathText
import "../lib/TreeOrder.js" as TreeOrder
import "../lib/KeyedRows.js" as KeyedRows

Item {
  id: controller

  required property var service
  property alias model: treeModel
  readonly property bool stateReady: service.stateReady
  readonly property bool open: service.open
  readonly property string rootPath: service.rootPath
  readonly property bool remotePath: PathText.isRemote(rootPath)
  function remoteLocation() { return remotePath ? service.drivesController.descriptorForPath(rootPath) : null }
  property string treeLocationGeneration: ""
  readonly property bool showHidden: service.showHidden
  readonly property bool gitEnabled: service.gitEnabled
  readonly property bool quickNavActive: service.quickNavActive
  readonly property string searchQuery: service.searchQuery
  readonly property var searchModel: service.searchModel
  readonly property var favoritesModel: service.favoritesModel
  readonly property var recentModel: service.recentModel
  readonly property string selectedPath: service.selectedPath
  readonly property var selectedPaths: service.selectedPaths
  readonly property var selectedEntries: service.selectedEntries
  readonly property string selectionAnchorPath: service.selectionAnchorPath

  ListModel {
    id: treeModel
    property var folderCountCache: null
  }

  property string expansionRoot: ""
  property bool expansionIncludesIgnored: false
  readonly property alias expansionError: expansion.error
  TreeExpansion {
    id: expansion
    active: controller.open && controller.stateReady
    loading: controller.treeLoading
    revision: controller.treeStructureRevision
    rowCount: treeModel.count
    nextEntry: controller.nextBranchEntry
    expandEntry: function(entry) {
      if (entry.more) controller.loadMoreChildren(entry.path)
      else controller.toggleDirectory(controller.indexOfTreePath(entry.path), true)
    }
  }

  function nextBranchEntry() {
    var start = indexOfTreePath(expansionRoot)
    if (start < 0) return null
    var depth = Number(treeModel.get(start).depth)
    for (var i = start; i < treeModel.count; i++) {
      var row = treeModel.get(i)
      if (i > start && Number(row.depth) <= depth) break
      if (i > start && row.isDir && (row.isSymlink || (row.gitIgnored && !expansionIncludesIgnored))) {
        i = directoryEndIndex(i, Number(row.depth)) - 1
        continue
      }
      if (row.isDir && !row.gitDeleted && !row.expanded)
        return { path: String(row.path), depth: Number(row.depth) - depth }
      if (row.kind === "More") {
        var parent = String(row.path).slice(0, -5)
        var parentIndex = indexOfTreePath(parent)
        if (pendingWindowPaths.indexOf(parent) < 0 && parentIndex >= 0 && !treeModel.get(parentIndex).error)
          return { path: parent, depth: Number(row.depth) - depth, more: true }
      }
    }
    return null
  }

  function setBranchExpanded(path, expanded) {
    var index = indexOfTreePath(path)
    if (index < 0) return false
    var row = treeModel.get(index)
    if (!row.isDir || row.gitDeleted) return false
    expansion.stop(true)
    if (expanded) {
      expansionRoot = String(row.path)
      expansionIncludesIgnored = row.gitIgnored === true
      expansion.start()
    } else {
      restoreExpandedPaths = restoreExpandedPaths.filter(function(value) { return !pathWithin(value, path) })
      treeQueue = treeQueue.filter(function(value) { return !pathWithin(value.path, path) })
      if (row.expanded) toggleDirectory(index)
    }
    return true
  }

  function rootName(path) { return service.rootName(path) }
  function normalizeRoot(path) { return service.normalizeRoot(path) }
  function parentDirectory(path) { return service.parentDirectory(path) }
  function remappedPathList(paths, mappings) { return service.remappedPathList(paths, mappings) }
  function pathRemoved(path, removals) { return service.pathRemoved(path, removals) }
  function pathWithin(path, parent) { return service.pathWithin(path, parent) }
  function normalizedNavigationStack(paths, root) { return service.normalizedNavigationStack(paths, root) }
  function remappedOperationPath(path, mappings) { return service.remappedOperationPath(path, mappings) }
  function makeRow(entry, depth) { return service.makeRow(entry, depth) }
  function clearSelection() { service.clearSelection() }
  function scheduleStateSave() { service.scheduleStateSave() }
  function recordZoxideVisit(path) { service.recordZoxideVisit(path) }
  function entryForKnownPath(path) { return service.entryForKnownPath(path) }
  function entrySnapshot(row) { return service.entrySnapshot(row) }
  function applySelection(entries, primary, anchor) { service.applySelection(entries, primary, anchor) }
  function scheduleWatcherRestart() { service.scheduleWatcherRestart() }

  property var treeQueue: []
  readonly property int windowPage: 400
  property var pendingWindowPaths: []
  readonly property int directoryBatchLimit: 32
  property var activeTreePaths: []
  property var activeTreeResponse: null
  property string activeTreeRequestId: ""
  property int activeTreeGeneration: -1
  property int treeReadProcessCount: 0
  property int lastTreeBatchSize: 0
  property bool refreshGitOnNextTreeRequest: false
  property var restoreExpandedPaths: []
  property var gitRepoDirectories: ({})
  readonly property int gitMetadataPathLimit: 1000
  property bool gitMetadataBusy: false
  property bool gitMetadataQueued: false
  property string queuedGitMetadataReason: ""
  property var activeGitMetadataPaths: []
  property var activeGitMetadataResponse: null
  property int activeGitMetadataGeneration: -1
  property int gitMetadataRefreshCount: 0
  property int gitMetadataNoopCount: 0
  property var dirtyGitRepositories: ({})
  property var gitMetadataRowFingerprints: ({})
  property var lastGitMetadataPayload: null
  property bool gitMetadataRepaintPending: false
  property bool activeGitMetadataScoped: false
  property bool gitMetadataFullSweepPending: true
  property int gitMetadataScopedRefreshCount: 0
  property int gitStatusPollBackoff: 1
  readonly property int gitStatusPollBackoffLimit: 4
  property int lastGitMetadataPathCount: 0
  property int lastGitMetadataRepositoryCount: 0
  property string lastGitMetadataReason: ""
  property string lastGitMetadataFingerprint: ""
  property string gitMetadataError: ""
  property string activeGitMetadataRequestId: ""
  property string activeGitMetadataRequestGeneration: ""
  property int treeGeneration: 0
  property int treeStructureRevision: 0
  property int treeRowsRevision: 0
  readonly property bool treeLoading: activeTreeRequestId !== "" || treeQueue.length > 0 || pendingWindowPaths.length > 0
  property int treePathIndexRevision: -1
  property var treePathIndex: ({})

  function expandedPaths() {
    var paths = []
    for (var i = 0; i < treeModel.count; i++) {
      var row = treeModel.get(i)
      if (row.isDir && row.expanded && Number(row.depth) > 0) paths.push(String(row.path))
    }
    return paths
  }

  function resetTree(preserveExpansion, pathMappings, removedPaths) {
    if (!stateReady) return
    expansion.stop(true)
    var preservedPaths = preserveExpansion
      ? remappedPathList(expandedPaths(), Array.isArray(pathMappings) ? pathMappings : [])
      : []
    if (Array.isArray(removedPaths) && removedPaths.length > 0)
      preservedPaths = preservedPaths.filter(function(path) { return !pathRemoved(path, removedPaths) })
    restoreExpandedPaths = preservedPaths
    if (activeTreeRequestId) {
      service.cancelBackendRequest(activeTreeRequestId, activeTreeGeneration)
      activeTreeRequestId = ""
    }
    if (activeGitMetadataRequestId) {
      service.cancelBackendRequest(activeGitMetadataRequestId, activeGitMetadataRequestGeneration)
      activeGitMetadataRequestId = ""
    }
    treeGeneration++
    treeLocationGeneration = remoteLocation() ? String(remoteLocation().session_generation) : ""
    activeTreeGeneration = -1
    activeTreePaths = []
    activeTreeResponse = null
    activeGitMetadataRequestGeneration = ""
    activeGitMetadataPaths = []
    activeGitMetadataResponse = null
    activeGitMetadataGeneration = -1
    gitMetadataBusy = false
    treeQueue = []
    gitMetadataQueued = false
    queuedGitMetadataReason = ""
    lastGitMetadataFingerprint = ""
    gitMetadataRowFingerprints = ({})
    gitMetadataError = ""
    gitRepoDirectories = ({})
    dirtyGitRepositories = ({})
    gitMetadataFullSweepPending = true
    service.treeRowsReplacing()
    treeModel.clear()
    if (service.trashMode) {
      markTreeStructureChanged()
      service.refreshTrash()
      scheduleWatcherRestart()
      return
    }
    if (service.recentMode) {
      markTreeStructureChanged()
      service.refreshRecent()
      scheduleWatcherRestart()
      return
    }
    if (service.drivesMode) {
      markTreeStructureChanged()
      scheduleWatcherRestart()
      return
    }
    treeModel.append({
      name: rootName(rootPath), path: rootPath, isDir: true, isSymlink: false,
      isGitRepo: false, gitDeleted: false, gitIgnored: false, gitRepoRoot: "", gitStatus: "", gitStatusLabel: "",
      gitIndexStatus: "", gitWorktreeStatus: "", gitOriginalPath: "", gitRepoName: "", gitBranch: "", gitWorktree: "",
      gitModifiedCount: 0, gitDeletedCount: 0, gitNewCount: 0,
      gitUntrackedCount: 0,
      gitSummary: "",
      size: -1, sizeText: "—", modified: "", created: "", statFingerprint: "", kind: "Directory", mime: "inode/directory", relative: "",
      nameSpans: "", relativeSpans: "",
      depth: 0, expanded: true, loaded: false, loading: true, error: "",
      windowLoaded: 0, windowTotal: 0
    })
    markTreeStructureChanged()
    enqueueChildren(rootPath)
    scheduleWatcherRestart()
  }

  function markTreeStructureChanged() {
    treeStructureRevision++
    scheduleGitMetadataRepaint()
  }

  function scheduleGitMetadataRepaint() {
    if (!lastGitMetadataPayload || gitMetadataRepaintPending) return
    gitMetadataRepaintPending = true
    Qt.callLater(repaintGitMetadata)
  }

  function repaintGitMetadata() {
    gitMetadataRepaintPending = false
    var payload = lastGitMetadataPayload
    if (!payload || !gitEnabled) return
    applyGitMetadataResults(payload.response, payload.paths, payload.results, payload.exitCode)
  }

  function rebuildTreePathIndex() {
    var next = ({})
    for (var i = 0; i < treeModel.count; i++)
      next[String(treeModel.get(i).path)] = i + 1
    treePathIndex = next
    treePathIndexRevision = treeStructureRevision
  }

  function indexOfTreePath(path) {
    if (treePathIndexRevision !== treeStructureRevision) rebuildTreePathIndex()
    return Number(treePathIndex[String(path)] || 0) - 1
  }

  function removeDescendants(parentIndex) {
    if (parentIndex < 0 || parentIndex >= treeModel.count) return
    var parentDepth = Number(treeModel.get(parentIndex).depth)
    var count = 0
    for (var i = parentIndex + 1; i < treeModel.count; i++) {
      if (Number(treeModel.get(i).depth) <= parentDepth) break
      count++
    }
    if (count > 0) {
      treeModel.remove(parentIndex + 1, count)
      markTreeStructureChanged()
    }
  }

  function enqueueChildren(path) {
    treeQueue = treeQueue.concat([{ path: String(path) }])
    startNextTreeRequest()
  }

  function startNextTreeRequest() {
    if (activeTreeRequestId || treeQueue.length === 0) return
    var queued = treeQueue.slice(0, remotePath ? 1 : directoryBatchLimit)
    treeQueue = treeQueue.slice(queued.length)
    var paths = []
    for (var i = 0; i < queued.length; i++) {
      var path = String(queued[i].path || "")
      if (path && paths.indexOf(path) < 0) paths.push(path)
    }
    if (paths.length === 0) {
      Qt.callLater(startNextTreeRequest)
      return
    }
    activeTreePaths = paths
    activeTreeResponse = null
    lastTreeBatchSize = paths.length
    treeReadProcessCount++
    var arguments = []
    for (var pathIndex = 0; pathIndex < paths.length; pathIndex++)
      arguments.push("--path", paths[pathIndex])
    if (showHidden) arguments.push("--show-hidden")
    arguments = arguments.concat(service.listingOrderArguments(service.windowLimitFor(paths)))
    if (refreshGitOnNextTreeRequest) {
      arguments.push("--fresh-git")
      refreshGitOnNextTreeRequest = false
    }
    activeTreeGeneration = treeGeneration
    var requestGeneration = activeTreeGeneration
    if (remotePath) {
      arguments = remoteArguments(paths[0], 0, service.windowLimitFor(paths))
      if (!arguments) {
        activeTreeResponse = { ok: false, error: "Remote location is unavailable; reconnect it in Drives" }
        finishTreeRequest(0)
        return
      }
    }
    activeTreeRequestId = service.backendRequest(remotePath ? "list" : "children-batch", arguments, requestGeneration, function(response) {
      if (requestGeneration !== controller.activeTreeGeneration) return
      controller.activeTreeRequestId = ""
      if (controller.remotePath && response) response.path = paths[0]
      controller.activeTreeResponse = response
      controller.finishTreeRequest(0)
    })
  }

  function remoteArguments(path, start, count) {
    var peer = remoteLocation()
    var relative = peer ? service.drivesController.relativePath(peer, path) : null
    if (!peer || relative === null) return null
    var arguments = ["--location", peer.id, "--generation", peer.session_generation, "--path", relative,
      "--start", String(start), "--count", String(count)].concat(listingOrderArguments(count).slice(2))
    if (gitEnabled) arguments.push("--no-git")
    if (showHidden) arguments.push("--show-hidden")
    return arguments
  }

  Connections {
    target: controller.service.drivesController || null
    function onPeerLocationsChanged() {
      if (!controller.remotePath || !controller.stateReady) return
      var peer = controller.service.drivesController.descriptorForPath(controller.rootPath)
      if (String(peer && peer.session_generation || "") === controller.treeLocationGeneration) return
      controller.clearSelection()
      controller.resetTree()
    }
  }

  function registerGitRepository(repoRoot, gitDir) {
    if (!gitEnabled) return
    var root = normalizeRoot(repoRoot)
    var directory = normalizeRoot(gitDir)
    if (!root || !directory || gitRepoDirectories[root] === directory) return
    var next = ({})
    var roots = Object.keys(gitRepoDirectories)
    for (var i = 0; i < roots.length; i++) next[roots[i]] = gitRepoDirectories[roots[i]]
    next[root] = directory
    gitRepoDirectories = next
  }

  function clearModelGitMetadata(model, rowIndex) {
    if (!model || rowIndex < 0 || rowIndex >= model.count) return
    model.setProperty(rowIndex, "isGitRepo", false)
    model.setProperty(rowIndex, "gitRepoRoot", "")
    model.setProperty(rowIndex, "gitStatus", "")
    model.setProperty(rowIndex, "gitStatusLabel", "")
    model.setProperty(rowIndex, "gitIndexStatus", "")
    model.setProperty(rowIndex, "gitWorktreeStatus", "")
    model.setProperty(rowIndex, "gitOriginalPath", "")
    model.setProperty(rowIndex, "gitModifiedCount", 0)
    model.setProperty(rowIndex, "gitDeletedCount", 0)
    model.setProperty(rowIndex, "gitNewCount", 0)
    model.setProperty(rowIndex, "gitUntrackedCount", 0)
    model.setProperty(rowIndex, "gitSummary", "")
    model.setProperty(rowIndex, "gitRepoName", "")
    model.setProperty(rowIndex, "gitBranch", "")
    model.setProperty(rowIndex, "gitWorktree", "")
    model.setProperty(rowIndex, "gitDeleted", false)
    model.setProperty(rowIndex, "gitIgnored", false)
  }

  function clearDirectoryGitMetadata(parentIndex) {
    clearModelGitMetadata(treeModel, parentIndex)
  }

  function clearModelGitMetadataRows(model) {
    if (!model) return
    for (var rowIndex = 0; rowIndex < model.count; rowIndex++)
      clearModelGitMetadata(model, rowIndex)
  }

  function resetGitIntegration() {
    gitMetadataRefreshTimer.stop()
    clearModelGitMetadataRows(treeModel)
    clearModelGitMetadataRows(searchModel)
    clearModelGitMetadataRows(favoritesModel)
    clearModelGitMetadataRows(recentModel)
    service.searchGitRepositoryCount = 0
    refreshSelectionFromVisibleModels()
    if (!stateReady) return
    refreshGitOnNextTreeRequest = true
    resetTree(true)
    if (searchQuery.trim() !== "") service.restartSearch()
  }

  function applyModelGitMetadata(model, rowIndex, path, response, registerRepository) {
    if (!model || rowIndex < 0 || rowIndex >= model.count || !response) return
    if (!gitEnabled) {
      clearModelGitMetadata(model, rowIndex)
      return
    }
    if (!response.git) {
      if (response.ok) clearModelGitMetadata(model, rowIndex)
      return
    }
    var git = response.git
    var repoRoot = normalizeRoot(git.root)
    if (!repoRoot) {
      if (response.ok) clearModelGitMetadata(model, rowIndex)
      return
    }
    if (registerRepository !== false) registerGitRepository(repoRoot, git.git_dir)
    if (git.ok === false) {
      model.setProperty(rowIndex, "gitSummary", normalizeRoot(path) === repoRoot && git.summary ? JSON.stringify(git.summary) : "")
      return
    }
    setModelGitMetadata(model, rowIndex, path, repoRoot, git)
  }

  function setModelGitMetadata(model, rowIndex, path, repoRoot, git) {
    model.setProperty(rowIndex, "isGitRepo", !!model.get(rowIndex).isDir && normalizeRoot(path) === repoRoot)
    model.setProperty(rowIndex, "gitRepoRoot", repoRoot)
    model.setProperty(rowIndex, "gitStatus", String(git.status || ""))
    model.setProperty(rowIndex, "gitStatusLabel", String(git.status_label || ""))
    model.setProperty(rowIndex, "gitIndexStatus", String(git.index_status || ""))
    model.setProperty(rowIndex, "gitWorktreeStatus", String(git.worktree_status || ""))
    model.setProperty(rowIndex, "gitOriginalPath", String(git.original_path || ""))
    model.setProperty(rowIndex, "gitModifiedCount", Math.max(0, Number(git.modified_count) || 0))
    model.setProperty(rowIndex, "gitDeletedCount", Math.max(0, Number(git.deleted_count) || 0))
    model.setProperty(rowIndex, "gitNewCount", Math.max(0, Number(git.new_count) || 0))
    model.setProperty(rowIndex, "gitUntrackedCount", Math.max(0, Number(git.untracked_count) || 0))
    model.setProperty(rowIndex, "gitSummary", normalizeRoot(path) === repoRoot && git.summary ? JSON.stringify(git.summary) : "")
    model.setProperty(rowIndex, "gitRepoName", String(git.name || ""))
    model.setProperty(rowIndex, "gitBranch", String(git.branch || ""))
    model.setProperty(rowIndex, "gitWorktree", String(git.worktree || ""))
    model.setProperty(rowIndex, "gitDeleted", !!git.deleted)
    model.setProperty(rowIndex, "gitIgnored", !!git.ignored)
  }

  function applyDirectoryGitMetadata(parentIndex, path, response, registerRepository) {
    if (remotePath) return
    applyModelGitMetadata(treeModel, parentIndex, path, response, registerRepository)
  }

  function applySearchGitMetadata(rowIndex, path, response) {
    if (rowIndex < 0 || rowIndex >= searchModel.count || !response) return
    var wasDeleted = !!searchModel.get(rowIndex).gitDeleted
    applyModelGitMetadata(searchModel, rowIndex, path, response, false)
    var isDeleted = !!searchModel.get(rowIndex).gitDeleted
    if ((response.exists === false && !isDeleted) || wasDeleted !== isDeleted) searchDebounce.restart()
  }

  function directoryResponseForPath(payload, path, exitCode) {
    var results = payload && Array.isArray(payload.results) ? payload.results : []
    var target = normalizeRoot(path)
    for (var i = 0; i < results.length; i++) {
      if (normalizeRoot(results[i].path) === target) return results[i]
    }
    if (payload && payload.path && normalizeRoot(payload.path) === target) return payload
    return {
      ok: false,
      path: target,
      error: String(payload && payload.error || "Filesystem backend exited with " + exitCode)
    }
  }

  function navigationStackWithoutMissingRoot(values, missingRoot, currentRoot) {
    var kept = []
    if (Array.isArray(values)) {
      for (var i = 0; i < values.length; i++)
        if (!pathWithin(values[i], missingRoot)) kept.push(values[i])
    }
    return normalizedNavigationStack(kept, currentRoot)
  }

  function recoverMissingRoot(path) {
    var missing = normalizeRoot(path)
    if (normalizeRoot(rootPath) !== missing || missing === "/") return false
    var fallback = parentDirectory(missing)
    if (!service.rootRecoveryOrigin) {
      service.rootRecoveryOrigin = missing
      service.lastMissingRoot = missing
      service.rootRecoveryCount++
    }
    service.stopRootRecoveryNotice()
    service.rootRecoveryNotice = "Finding the nearest available parent of " + service.rootRecoveryOrigin + "…"
    service.rootPath = fallback
    service.rootBackStack = navigationStackWithoutMissingRoot(service.rootBackStack, missing, fallback)
    service.rootForwardStack = navigationStackWithoutMissingRoot(service.rootForwardStack, missing, fallback)
    service.quickNavActive = false
    service.searchQuery = ""
    searchModel.clear()
    service.searchGitRepositoryCount = 0
    clearSelection()
    Qt.callLater(function() {
      if (service.rootRecoveryOrigin && normalizeRoot(service.rootPath) === fallback)
        service.resetTree()
    })
    scheduleStateSave()
    return true
  }

  function completeMissingRootRecovery() {
    if (!service.rootRecoveryOrigin) return
    service.rootRecoveryOrigin = ""
    service.lastRecoveredRoot = normalizeRoot(rootPath)
    service.rootRecoveryNotice = "Folder disappeared — moved up to " + service.lastRecoveredRoot
    service.restartRootRecoveryNotice()
    recordZoxideVisit(service.lastRecoveredRoot)
    scheduleStateSave()
  }

  function abortMissingRootRecovery() {
    service.rootRecoveryOrigin = ""
    service.rootRecoveryNotice = ""
    service.stopRootRecoveryNotice()
  }

  function applyTreeError(parentIndex, path, response) {
    if (remotePath && response.error_id === "stale-location" && remoteLocation()) {
      service.drivesController.invalidatePeer(remoteLocation().id, remoteLocation().session_generation)
      return
    }
    if (parentIndex === 0 && !!response.missing && recoverMissingRoot(path)) return
    if (parentIndex === 0 && service.rootRecoveryOrigin) abortMissingRootRecovery()
    treeModel.setProperty(parentIndex, "loaded", true)
    treeModel.setProperty(parentIndex, "error", String(response.error || "Unable to read directory"))
  }

  function insertTreeEntries(parentIndex, rawEntries, windowed) {
    var entries = windowed ? (Array.isArray(rawEntries) ? rawEntries : []) : TreeOrder.arrange(rawEntries, service.treeSort, service.treeFilter)
    var depth = Number(treeModel.get(parentIndex).depth) + 1
    var directoriesToRestore = []
    for (var i = 0; i < entries.length; i++) {
      var inserted = makeRow(entries[i], depth)
      if (inserted.isDir && restoreExpandedPaths.indexOf(inserted.path) >= 0) {
        inserted.expanded = true
        inserted.loading = true
        directoriesToRestore.push(inserted.path)
      }
      treeModel.insert(parentIndex + 1 + i, inserted)
    }
    return directoriesToRestore
  }

  function restoreTreeDirectories(paths) {
    for (var i = 0; i < paths.length; i++) treeQueue = treeQueue.concat([{ path: paths[i] }])
  }

  function listingOrderArguments(limit) {
    var sorts = TreeOrder.normalizeSorts(service.treeSort)
    var key = sorts.length > 0 ? String(sorts[0].key) : "name"
    var arguments = ["--limit", String(Math.max(1, limit)), "--sort", key]
    if (sorts.length > 0 && sorts[0].desc) arguments.push("--desc")
    var filter = TreeOrder.normalizeFilter(service.treeFilter)
    if (Object.keys(filter).length > 0) arguments.push("--filter", JSON.stringify(filter))
    if (service.priorityColumns.indexOf("created") >= 0 || key === "created" || filter.created !== undefined)
      arguments.push("--include-created")
    if (!gitEnabled) arguments.push("--no-git")
    return arguments
  }

  function moreRowPath(path) {
    return String(path) + " more"
  }

  function windowLimitFor(paths) {
    var limit = windowPage
    for (var i = 0; i < paths.length; i++) {
      var moreIndex = indexOfTreePath(moreRowPath(paths[i]))
      if (moreIndex >= 0) limit = Math.max(limit, Number(treeModel.get(moreIndex).windowLoaded) || 0)
    }
    return limit
  }

  function moreRow(path, depth, loaded, total) {
    var row = makeRow({ name: (total - loaded) + " more", path: moreRowPath(path), kind: "More", mime: "" }, depth)
    row.windowLoaded = loaded
    row.windowTotal = total
    return row
  }

  function settleWindowedDirectory(parentIndex, response, insertedCount) {
    var path = String(treeModel.get(parentIndex).path)
    var depth = Number(treeModel.get(parentIndex).depth) + 1
    var moreIndex = indexOfTreePath(moreRowPath(path))
    if (moreIndex >= 0) {
      treeModel.remove(moreIndex, 1)
      markTreeStructureChanged()
    }
    if (!response.windowed || !response.truncated) return
    var loaded = (Number(response.start) || 0) + insertedCount
    var total = Number(response.total) || loaded
    if (loaded >= total) return
    var endIndex = directoryEndIndex(parentIndex, depth - 1)
    treeModel.insert(endIndex, moreRow(path, depth, loaded, total))
    markTreeStructureChanged()
  }

  function loadMoreChildren(path) {
    var target = normalizeRoot(path)
    var moreIndex = indexOfTreePath(moreRowPath(target))
    if (moreIndex < 0 || pendingWindowPaths.indexOf(target) >= 0) return false
    var loaded = Number(treeModel.get(moreIndex).windowLoaded) || 0
    pendingWindowPaths = pendingWindowPaths.concat([target])
    var arguments = ["--path", target, "--start", String(loaded), "--count", String(windowPage)]
    if (showHidden) arguments.push("--show-hidden")
    arguments = arguments.concat(listingOrderArguments(windowPage).slice(2))
    var requestGeneration = treeGeneration
    if (remotePath) {
      arguments = remoteArguments(target, loaded, windowPage)
      if (!arguments) {
        pendingWindowPaths = pendingWindowPaths.filter(function(item) { return item !== target })
        return false
      }
    }
    service.backendRequest(remotePath ? "list" : "children-window", arguments, requestGeneration, function(response) {
      controller.pendingWindowPaths = controller.pendingWindowPaths.filter(function(item) { return item !== target })
      if (requestGeneration !== controller.treeGeneration) return
      controller.applyWindowResponse(target, response || { ok: false })
    })
    return true
  }

  function applyWindowResponse(path, response) {
    var parentIndex = indexOfTreePath(path)
    if (parentIndex < 0 || !treeModel.get(parentIndex).expanded) return
    var moreIndex = indexOfTreePath(moreRowPath(path))
    if (moreIndex < 0) return
    if (!response.ok) {
      if (remotePath && response.error_id === "stale-location" && remoteLocation()) {
        service.drivesController.invalidatePeer(remoteLocation().id, remoteLocation().session_generation)
        return
      }
      treeModel.setProperty(parentIndex, "error", String(response.error || "Unable to read more entries"))
      return
    }
    var entries = Array.isArray(response.entries) ? response.entries : []
    var depth = Number(treeModel.get(parentIndex).depth) + 1
    var directoriesToRestore = []
    for (var i = 0; i < entries.length; i++) {
      var inserted = makeRow(entries[i], depth)
      if (inserted.isDir && restoreExpandedPaths.indexOf(inserted.path) >= 0) {
        inserted.expanded = true
        inserted.loading = true
        directoriesToRestore.push(inserted.path)
      }
      treeModel.insert(moreIndex + i, inserted)
    }
    markTreeStructureChanged()
    settleWindowedDirectory(parentIndex, response, entries.length)
    restoreTreeDirectories(directoriesToRestore)
    startNextTreeRequest()
  }

  function applyTreeResponse(path, response) {
    var parentIndex = indexOfTreePath(path)
    if (parentIndex < 0) return
    applyDirectoryGitMetadata(parentIndex, path, response)
    treeModel.setProperty(parentIndex, "loading", false)
    if (!response.ok) return applyTreeError(parentIndex, path, response)
    if (!treeModel.get(parentIndex).expanded) return
    removeDescendants(parentIndex)
    var entries = Array.isArray(response.entries) ? response.entries : []
    restoreTreeDirectories(insertTreeEntries(parentIndex, entries, !!response.windowed))
    if (entries.length > 0) markTreeStructureChanged()
    settleWindowedDirectory(parentIndex, response, entries.length)
    treeModel.setProperty(parentIndex, "loaded", true)
    treeModel.setProperty(parentIndex, "error", response.truncated && !response.windowed ? "Showing the first " + String(response.limit || entries.length) + " entries" : "")
    if (parentIndex === 0) completeMissingRootRecovery()
  }

  function finishTreeRequest(exitCode) {
    var paths = activeTreePaths.slice()
    var payload = activeTreeResponse
    for (var i = 0; i < paths.length; i++)
      applyTreeResponse(paths[i], directoryResponseForPath(payload, paths[i], exitCode))
    activeTreePaths = []
    activeTreeResponse = null
    activeTreeGeneration = -1
    scheduleWatcherRestart()
    Qt.callLater(startNextTreeRequest)
  }

  function toggleDirectory(index, recursive) {
    if (index < 0 || index >= treeModel.count) return
    if (!recursive) expansion.stop(false)
    var row = treeModel.get(index)
    if (!row.isDir) {
      service.openDefault(row.path, undefined, false)
      return
    }
    if (row.expanded) {
      treeModel.setProperty(index, "expanded", false)
      treeModel.setProperty(index, "loaded", false)
      treeModel.setProperty(index, "loading", false)
      removeDescendants(index)
      scheduleWatcherRestart()
    } else {
      treeModel.setProperty(index, "expanded", true)
      treeModel.setProperty(index, "loading", true)
      treeModel.setProperty(index, "error", "")
      enqueueChildren(row.path)
      scheduleWatcherRestart()
    }
  }

  function setDirectoryExpanded(path, expanded) {
    var index = indexOfTreePath(path)
    if (index < 0) return "not-visible"
    var row = treeModel.get(index)
    if (!row.isDir) return "not-directory"
    var desired = !!expanded
    if (!!row.expanded !== desired) toggleDirectory(index)
    return desired ? "expanded" : "collapsed"
  }

  function refreshTree(pathMappings, removedPaths) {
    refreshGitOnNextTreeRequest = true
    resetTree(true, pathMappings, removedPaths)
    if (searchQuery.trim() !== "") service.restartSearch()
  }

  function metadataField(value, name) {
    return String(value && value[name] || "")
  }

  function metadataText(value, name, fallback) {
    return metadataField(value, name) || fallback
  }

  function metadataNumber(value, fallback) {
    var number = Number(value)
    return isFinite(number) ? number : fallback
  }

  function treeRowSnapshot(row) {
    return {
      name: metadataField(row, "name"),
      path: metadataField(row, "path"),
      isDir: !!row.isDir,
      isSymlink: !!row.isSymlink,
      isGitRepo: !!row.isGitRepo,
      gitDeleted: !!row.gitDeleted,
      gitIgnored: !!row.gitIgnored,
      gitRepoRoot: metadataField(row, "gitRepoRoot"),
      gitStatus: metadataField(row, "gitStatus"),
      gitStatusLabel: metadataField(row, "gitStatusLabel"),
      gitIndexStatus: metadataField(row, "gitIndexStatus"),
      gitWorktreeStatus: metadataField(row, "gitWorktreeStatus"),
      gitOriginalPath: metadataField(row, "gitOriginalPath"),
      gitModifiedCount: Math.max(0, metadataNumber(row.gitModifiedCount, 0)),
      gitDeletedCount: Math.max(0, metadataNumber(row.gitDeletedCount, 0)),
      gitNewCount: Math.max(0, metadataNumber(row.gitNewCount, 0)),
      gitUntrackedCount: Math.max(0, metadataNumber(row.gitUntrackedCount, 0)),
      gitSummary: metadataField(row, "gitSummary"),
      gitRepoName: metadataField(row, "gitRepoName"),
      gitBranch: metadataField(row, "gitBranch"),
      gitWorktree: metadataField(row, "gitWorktree"),
      size: Number(row.size === undefined ? -1 : row.size),
      sizeText: metadataText(row, "sizeText", "—"),
      modified: metadataField(row, "modified"),
      created: metadataField(row, "created"),
      statFingerprint: metadataField(row, "statFingerprint"),
      kind: metadataText(row, "kind", row.isDir ? "Directory" : "File"),
      mime: metadataText(row, "mime", row.isDir ? "inode/directory" : "application/octet-stream"),
      relative: metadataField(row, "relative"),
      nameSpans: metadataField(row, "nameSpans"),
      relativeSpans: metadataField(row, "relativeSpans"),
      depth: Math.max(0, metadataNumber(row.depth, 0)),
      expanded: !!row.expanded,
      loaded: !!row.loaded,
      loading: !!row.loading,
      error: metadataField(row, "error"),
      windowLoaded: Math.max(0, metadataNumber(row.windowLoaded, 0)),
      windowTotal: Math.max(0, metadataNumber(row.windowTotal, 0))
    }
  }

  function refreshSelectionFromVisibleModels() {
    if (selectedPaths.length === 0) return
    var previousPrimary = selectedPath
    var previousAnchor = selectionAnchorPath
    var entries = []
    for (var i = 0; i < selectedPaths.length; i++) {
      var entry = entryForKnownPath(selectedPaths[i])
      if (entry) entries.push(entry)
    }
    if (JSON.stringify(entries) === JSON.stringify(selectedEntries)) return
    var primary = entryForKnownPath(previousPrimary)
    applySelection(entries, primary, previousAnchor)
  }

  function directoryEndIndex(parentIndex, parentDepth) {
    var endIndex = parentIndex + 1
    while (endIndex < treeModel.count && Number(treeModel.get(endIndex).depth) > parentDepth) endIndex++
    return endIndex
  }

  function directChildSnapshots(parentIndex, endIndex, parentDepth) {
    var rows = []
    for (var currentIndex = parentIndex + 1; currentIndex < endIndex; currentIndex++)
      if (Number(treeModel.get(currentIndex).depth) === parentDepth + 1)
        rows.push(entrySnapshot(treeModel.get(currentIndex)))
    return rows
  }

  function directorySegments(parentIndex, endIndex, parentDepth) {
    var segments = ({})
    var cursor = parentIndex + 1
    while (cursor < endIndex) {
      var row = treeModel.get(cursor)
      if (Number(row.depth) !== parentDepth + 1) {
        cursor++
        continue
      }
      var segmentEnd = cursor + 1
      while (segmentEnd < endIndex && Number(treeModel.get(segmentEnd).depth) > parentDepth + 1) segmentEnd++
      var segment = []
      for (var segmentIndex = cursor; segmentIndex < segmentEnd; segmentIndex++)
        segment.push(treeRowSnapshot(treeModel.get(segmentIndex)))
      segments[String(row.path)] = segment
      cursor = segmentEnd
    }
    return segments
  }

  function retainedDirectoryRows(incoming, incomingRows, segments) {
    var replacement = []
    for (var incomingIndex = 0; incomingIndex < incoming.length; incomingIndex++) {
      var nextRow = incomingRows[incomingIndex]
      var retained = segments[nextRow.path]
      if (retained && retained.length > 0) {
        nextRow.expanded = retained[0].expanded
        nextRow.loaded = retained[0].loaded
        nextRow.loading = retained[0].loading
        nextRow.error = retained[0].error
      }
      replacement.push(nextRow)
      if (retained) {
        for (var retainedIndex = 1; retainedIndex < retained.length; retainedIndex++)
          replacement.push(retained[retainedIndex])
      }
    }
    return replacement
  }

  function replaceDirectoryRows(parentIndex, endIndex, replacement) {
    service.treeRowsReplacing()
    var stats = KeyedRows.syncSegment(treeModel, parentIndex + 1, endIndex - parentIndex - 1, replacement, "path")
    if (stats.structural) markTreeStructureChanged()
    if (stats.updated > 0) treeRowsRevision++
  }

  function reconcileDirectory(path, response) {
    var parentIndex = indexOfTreePath(path)
    if (parentIndex < 0) return
    if (!response || !response.ok) {
      if (parentIndex === 0 && response && !!response.missing && recoverMissingRoot(path)) return
      treeModel.setProperty(parentIndex, "error", metadataText(response, "error", "Unable to refresh directory"))
      return
    }
    applyDirectoryGitMetadata(parentIndex, path, response)
    var parentDepth = Number(treeModel.get(parentIndex).depth)
    var endIndex = directoryEndIndex(parentIndex, parentDepth)
    var windowed = !!response.windowed
    var incoming = windowed
      ? (Array.isArray(response.entries) ? response.entries : [])
      : TreeOrder.arrange(response.entries, service.treeSort, service.treeFilter)
    var incomingRows = incoming.map(function(entry) { return makeRow(entry, parentDepth + 1) })
    var currentRows = directChildSnapshots(parentIndex, endIndex, parentDepth).filter(function(row) { return row.kind !== "More" })
    if (JSON.stringify(currentRows) === JSON.stringify(incomingRows.map(entrySnapshot))) {
      refreshSelectionFromVisibleModels()
      return
    }
    var segments = directorySegments(parentIndex, endIndex, parentDepth)
    var replacement = retainedDirectoryRows(incoming, incomingRows, segments)
    replaceDirectoryRows(parentIndex, endIndex, replacement)
    settleWindowedDirectory(parentIndex, response, incoming.length)
    treeModel.setProperty(parentIndex, "loaded", true)
    treeModel.setProperty(parentIndex, "loading", false)
    treeModel.setProperty(parentIndex, "error", "")
    refreshSelectionFromVisibleModels()
  }

  function appendUniquePath(paths, path) {
    var candidate = String(path || "")
    if (!candidate || paths.indexOf(candidate) >= 0) return false
    paths.push(candidate)
    return true
  }

  function repositoryFallbackPath(repoRoot) {
    var fallback = ""
    for (var seedIndex = 0; seedIndex < treeModel.count; seedIndex++) {
      var seedRow = treeModel.get(seedIndex)
      if (!seedRow.isDir) continue
      var seedPath = metadataField(seedRow, "path")
      var seedRepo = metadataField(seedRow, "gitRepoRoot")
      if (seedPath === repoRoot) return seedPath
      if (!fallback && (seedRepo === repoRoot || pathWithin(seedPath, repoRoot))) fallback = seedPath
    }
    return fallback
  }

  function appendRepositoryFallbacks(paths, repoRoots) {
    for (var repoIndex = 0; repoIndex < repoRoots.length; repoIndex++) {
      appendUniquePath(paths, repositoryFallbackPath(repoRoots[repoIndex]))
    }
  }

  function appendDiscoveredRepositoryPaths(paths) {
    for (var candidateIndex = 0; candidateIndex < treeModel.count && paths.length < gitMetadataPathLimit; candidateIndex++) {
      var candidate = treeModel.get(candidateIndex)
      if (candidate.isDir && candidate.isGitRepo) appendUniquePath(paths, metadataField(candidate, "path"))
    }
  }

  function pathWithinRepositories(path, repoRoots) {
    for (var index = 0; index < repoRoots.length; index++)
      if (pathWithin(path, repoRoots[index])) return true
    return false
  }

  function searchRowHasGit(row, repoRoots) {
    return metadataField(row, "gitRepoRoot") !== "" || pathWithinRepositories(metadataField(row, "path"), repoRoots)
  }

  function appendSearchGitPaths(paths, repoRoots) {
    if (!quickNavActive && searchQuery.trim() !== "") {
      for (var searchIndex = 0; searchIndex < searchModel.count && paths.length < gitMetadataPathLimit; searchIndex++) {
        var searchRow = searchModel.get(searchIndex)
        if (searchRowHasGit(searchRow, repoRoots)) appendUniquePath(paths, metadataField(searchRow, "path"))
      }
    }
  }

  function treeRowHasGit(row, repoRoots) {
    var repoRoot = metadataField(row, "gitRepoRoot")
    return !!gitRepoDirectories[repoRoot] || pathWithinRepositories(metadataField(row, "path"), repoRoots)
  }

  function appendRecentGitPaths(paths) {
    for (var rowIndex = 0; rowIndex < recentModel.count && paths.length < gitMetadataPathLimit; rowIndex++)
      appendUniquePath(paths, metadataField(recentModel.get(rowIndex), "path"))
  }

  function appendFavoriteGitPaths(paths) {
    for (var rowIndex = 0; rowIndex < favoritesModel.count && paths.length < gitMetadataPathLimit; rowIndex++)
      appendUniquePath(paths, metadataField(favoritesModel.get(rowIndex), "path"))
  }

  function appendTreeGitPaths(paths, repoRoots) {
    for (var rowIndex = 0; rowIndex < treeModel.count && paths.length < gitMetadataPathLimit; rowIndex++) {
      var row = treeModel.get(rowIndex)
      if (treeRowHasGit(row, repoRoots)) appendUniquePath(paths, metadataField(row, "path"))
    }
  }

  function visibleGitMetadataPaths() {
    if (!gitEnabled) return []
    var repoRoots = Object.keys(gitRepoDirectories)
    var paths = []
    appendRepositoryFallbacks(paths, repoRoots)
    appendDiscoveredRepositoryPaths(paths)
    appendSearchGitPaths(paths, repoRoots)
    appendTreeGitPaths(paths, repoRoots)
    appendFavoriteGitPaths(paths)
    appendRecentGitPaths(paths)
    return paths.slice(0, gitMetadataPathLimit)
  }

  function markGitRepositoryDirty(repoRoot) {
    var root = normalizeRoot(String(repoRoot || ""))
    if (!root) {
      gitMetadataFullSweepPending = true
      return
    }
    if (gitMetadataFullSweepPending || dirtyGitRepositories[root]) return
    var next = ({})
    var keys = Object.keys(dirtyGitRepositories)
    for (var index = 0; index < keys.length; index++) next[keys[index]] = true
    next[root] = true
    dirtyGitRepositories = next
  }

  function gitMetadataScopableReason(reason) {
    var value = String(reason || "")
    return value === "git-event" || value === "filesystem-event"
  }

  function scopedGitMetadataPaths(scopeRoots) {
    var paths = visibleGitMetadataPaths()
    if (!scopeRoots || scopeRoots.length === 0) return paths
    var scoped = []
    for (var pathIndex = 0; pathIndex < paths.length; pathIndex++) {
      var path = paths[pathIndex]
      for (var rootIndex = 0; rootIndex < scopeRoots.length; rootIndex++) {
        var root = scopeRoots[rootIndex]
        if (path === root || pathWithin(path, root)) {
          scoped.push(path)
          break
        }
      }
    }
    return scoped
  }

  function scheduleGitMetadataRefresh(reason, repoRoot) {
    if (!gitEnabled || !open) return
    if (repoRoot === undefined) gitMetadataFullSweepPending = true
    else markGitRepositoryDirty(repoRoot)
    queuedGitMetadataReason = String(reason || "event")
    gitMetadataRefreshTimer.restart()
  }

  function requestVisibleGitMetadataRefresh(reason) {
    if (remotePath) return "inactive"
    if (!gitEnabled || !open || !stateReady) return "inactive"
    if (gitMetadataBusy) {
      gitMetadataQueued = true
      queuedGitMetadataReason = String(reason || "queued")
      return "queued"
    }
    var scopeRoots = gitMetadataFullSweepPending || !gitMetadataScopableReason(reason)
      ? []
      : Object.keys(dirtyGitRepositories)
    var scoped = scopeRoots.length > 0
    var paths = scopedGitMetadataPaths(scopeRoots)
    if (paths.length === 0) {
      gitMetadataFullSweepPending = false
      dirtyGitRepositories = ({})
      return "no-visible-paths"
    }
    gitMetadataFullSweepPending = false
    dirtyGitRepositories = ({})
    if (scoped) gitMetadataScopedRefreshCount++
    activeGitMetadataScoped = scoped
    activeGitMetadataPaths = paths
    activeGitMetadataResponse = null
    activeGitMetadataGeneration = treeGeneration
    lastGitMetadataReason = String(reason || "manual")
    gitMetadataBusy = true
    gitMetadataError = ""
    var arguments = []
    for (var pathIndex = 0; pathIndex < paths.length; pathIndex++)
      arguments.push("--path", paths[pathIndex])
    activeGitMetadataRequestGeneration = String(treeGeneration) + "-git-" + String(gitMetadataRefreshCount + 1)
    var requestGeneration = activeGitMetadataRequestGeneration
    activeGitMetadataRequestId = service.backendRequest("git-metadata-batch", arguments, requestGeneration, function(response) {
      if (requestGeneration !== controller.activeGitMetadataRequestGeneration) return
      controller.activeGitMetadataRequestId = ""
      controller.activeGitMetadataResponse = response
      controller.finishGitMetadataRefresh(0)
    }, null, 30000)
    return "started"
  }

  function gitMetadataFingerprintRow(result) {
    var git = result && result.git
    return [
      metadataField(result, "path"),
      result && result.exists === false ? "missing" : "exists",
      metadataField(git, "root"),
      metadataField(git, "status"),
      metadataField(git, "index_status"),
      metadataField(git, "worktree_status"),
      metadataField(git, "original_path"),
      git && git.deleted ? "deleted" : "present",
      git && git.ignored ? "ignored" : "tracked",
      metadataField(git, "error")
    ].join("\u001f")
  }

  function gitMetadataSnapshot(results) {
    var repositories = ({})
    var rows = ({})
    var fingerprintRows = []
    var errors = []
    for (var index = 0; index < results.length; index++) {
      var result = results[index]
      var git = result && result.git
      if (git && git.root && git.git_dir) {
        repositories[normalizeRoot(git.root)] = normalizeRoot(git.git_dir)
        if (git.error) errors.push(String(git.error))
      }
      var row = gitMetadataFingerprintRow(result)
      rows[metadataField(result, "path")] = row
      fingerprintRows.push(row)
    }
    return {
      repositories: repositories,
      rows: rows,
      fingerprint: fingerprintRows.join("\u001e"),
      error: errors.length > 0 ? errors[0] : ""
    }
  }

  function gitMetadataRowsChanged(rows, scoped) {
    var keys = Object.keys(rows)
    for (var index = 0; index < keys.length; index++)
      if (gitMetadataRowFingerprints[keys[index]] !== rows[keys[index]]) return true
    if (!scoped && Object.keys(gitMetadataRowFingerprints).length !== keys.length) return true
    return false
  }

  function storeGitMetadataRows(rows, scoped) {
    var next = ({})
    if (scoped) {
      var existing = Object.keys(gitMetadataRowFingerprints)
      for (var index = 0; index < existing.length; index++)
        next[existing[index]] = gitMetadataRowFingerprints[existing[index]]
    }
    var keys = Object.keys(rows)
    for (var rowIndex = 0; rowIndex < keys.length; rowIndex++) next[keys[rowIndex]] = rows[keys[rowIndex]]
    gitMetadataRowFingerprints = next
  }

  function mergedRepositoryDirectories(repositories) {
    var next = ({})
    var existing = Object.keys(gitRepoDirectories)
    for (var index = 0; index < existing.length; index++)
      next[existing[index]] = gitRepoDirectories[existing[index]]
    var found = Object.keys(repositories)
    for (var foundIndex = 0; foundIndex < found.length; foundIndex++)
      next[found[foundIndex]] = repositories[found[foundIndex]]
    return next
  }

  function gitMetadataResponses(results) {
    var responses = ({})
    for (var index = 0; index < results.length; index++) {
      var path = metadataField(results[index], "path")
      if (path) responses[normalizeRoot(path)] = results[index]
    }
    return responses
  }

  function applyGitMetadataResults(response, paths, results, exitCode) {
    var responses = gitMetadataResponses(results)
    for (var pathIndex = 0; pathIndex < paths.length; pathIndex++) {
      var path = paths[pathIndex]
      var rowIndex = indexOfTreePath(path)
      var pathResponse = responses[normalizeRoot(path)] || directoryResponseForPath(response, path, exitCode)
      if (rowIndex >= 0) applyDirectoryGitMetadata(rowIndex, path, pathResponse, false)
    }
    for (var searchIndex = 0; searchIndex < searchModel.count; searchIndex++) {
      var searchPath = String(searchModel.get(searchIndex).path || "")
      var searchResponse = responses[normalizeRoot(searchPath)]
      if (searchResponse) applySearchGitMetadata(searchIndex, searchPath, searchResponse)
    }
    for (var favoriteIndex = 0; favoriteIndex < favoritesModel.count; favoriteIndex++) {
      var favoritePath = String(favoritesModel.get(favoriteIndex).path || "")
      var favoriteResponse = responses[normalizeRoot(favoritePath)]
      if (favoriteResponse) applyModelGitMetadata(favoritesModel, favoriteIndex, favoritePath, favoriteResponse, false)
    }
    for (var recentIndex = 0; recentIndex < recentModel.count; recentIndex++) {
      var recentPath = String(recentModel.get(recentIndex).path || "")
      var recentResponse = responses[normalizeRoot(recentPath)]
      if (recentResponse) applyModelGitMetadata(recentModel, recentIndex, recentPath, recentResponse, false)
    }
  }

  function applySuccessfulGitMetadata(response, paths, results, exitCode) {
    var scoped = activeGitMetadataScoped
    var snapshot = gitMetadataSnapshot(results)
    var previousRepositories = JSON.stringify(gitRepoDirectories)
    if (!scoped && results.length > 0)
      lastGitMetadataPayload = { response: response, paths: paths, results: results, exitCode: exitCode }
    if (gitMetadataRowsChanged(snapshot.rows, scoped)) {
      applyGitMetadataResults(response, paths, results, exitCode)
      storeGitMetadataRows(snapshot.rows, scoped)
      lastGitMetadataFingerprint = snapshot.fingerprint
      refreshSelectionFromVisibleModels()
      gitStatusPollBackoff = 1
    } else {
      gitMetadataNoopCount++
      if (lastGitMetadataReason === "poll")
        gitStatusPollBackoff = Math.min(gitStatusPollBackoffLimit, gitStatusPollBackoff * 2)
    }
    var nextRepositories = scoped ? mergedRepositoryDirectories(snapshot.repositories) : snapshot.repositories
    var repositoriesChanged = JSON.stringify(nextRepositories) !== previousRepositories
    if (repositoriesChanged) gitRepoDirectories = nextRepositories
    lastGitMetadataRepositoryCount = Object.keys(nextRepositories).length
    if (!scoped || snapshot.error) gitMetadataError = snapshot.error
    if (repositoriesChanged) scheduleWatcherRestart()
  }

  function finishGitMetadataRefresh(exitCode) {
    var response = activeGitMetadataResponse || {
      ok: false,
      error: "Git metadata backend exited with " + exitCode,
      results: []
    }
    var paths = activeGitMetadataPaths.slice()
    var generation = activeGitMetadataGeneration
    var rerun = gitMetadataQueued
    var rerunReason = queuedGitMetadataReason || "queued"
    activeGitMetadataPaths = []
    activeGitMetadataResponse = null
    activeGitMetadataGeneration = -1
    activeGitMetadataRequestGeneration = ""
    gitMetadataQueued = false
    queuedGitMetadataReason = ""
    gitMetadataBusy = false
    if (generation !== treeGeneration) return

    gitMetadataRefreshCount++
    lastGitMetadataPathCount = paths.length
    var results = response && Array.isArray(response.results) ? response.results : []
    if (!response.ok) {
      gitMetadataError = String(response.error || "Unable to refresh Git metadata")
    } else {
      applySuccessfulGitMetadata(response, paths, results, exitCode)
    }
    if (rerun && gitEnabled && open) {
      queuedGitMetadataReason = rerunReason
      gitMetadataRefreshTimer.restart()
    }
  }

  Timer {
    id: gitMetadataRefreshTimer
    interval: 160
    onTriggered: service.requestVisibleGitMetadataRefresh(service.queuedGitMetadataReason || "event")
  }

  Timer {
    id: gitStatusPollTimer
    interval: Math.max(1000, service.gitStatusPollIntervalMs || 5000)
      * controller.gitStatusPollBackoff * (service.watcherRunning ? 6 : 1)
    repeat: true
    running: service.gitEnabled && service.open && service.stateReady && service.gitStatusPollIntervalMs > 0
    onTriggered: service.requestVisibleGitMetadataRefresh("poll")
  }

  onOpenChanged: {
    scheduleWatcherRestart()
    gitStatusPollBackoff = 1
    if (gitEnabled && open) scheduleGitMetadataRefresh("open")
  }

  onRootPathChanged: { expansion.stop(true); gitStatusPollBackoff = 1 }
  onTreeStructureRevisionChanged: gitStatusPollBackoff = 1

}
