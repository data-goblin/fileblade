import QtQuick
import "../lib/PathText.js" as PathText

Item {
  id: controller

  required property var service
  property bool watcherRunning: false
  property bool refreshBusy: false
  property string watcherRequestId: ""
  property int watcherGeneration: 0
  property string refreshRequestId: ""
  property int refreshGeneration: 0
  property bool watcherRestartPending: false
  property string activeWatcherFingerprint: ""
  property int activeWatcherPathCount: 0
  property int watcherStartCount: 0
  property int watcherRestartCount: 0
  property int watcherFailureCount: 0
  property string watcherError: ""
  property int watcherRestartSkippedCount: 0
  property var pendingWatchDirectories: []
  property var watchRefreshQueue: []
  property var activeWatchRefreshPaths: []
  property var activeWatchRefreshResponse: null
  property int watchReadProcessCount: 0
  property int lastWatchBatchSize: 0
  property int filesystemRefreshCount: 0
  property string lastFilesystemEventPath: ""
  property bool indexDirty: false
  property int gitDirectoryEventsIgnored: 0
  readonly property var gitDecorationEntries: [
    "index", "HEAD", "packed-refs", "config", "refs", "logs",
    "MERGE_HEAD", "REBASE_HEAD", "CHERRY_PICK_HEAD", "REVERT_HEAD", "BISECT_LOG"
  ]
  readonly property var gitWatchSubdirectories: [
    "refs/heads", "refs/remotes", "refs/tags", "logs/refs/heads", "logs/refs/remotes"
  ]
  readonly property bool stateReady: service.stateReady
  readonly property bool open: service.open
  readonly property string rootPath: service.rootPath
  readonly property bool showHidden: service.showHidden
  readonly property string searchQuery: service.searchQuery
  readonly property int directoryBatchLimit: service.directoryBatchLimit
  readonly property var treeModel: service.treeModel
  readonly property var gitRepoDirectories: service.gitRepoDirectories

  function parentDirectory(path) { return service.parentDirectory(path) }
  function indexOfTreePath(path) { return service.indexOfTreePath(path) }
  function scheduleGitMetadataRefresh(reason, repoRoot) { service.scheduleGitMetadataRefresh(reason, repoRoot) }

  function pathInside(path, directory) {
    return PathText.within(path, directory)
  }

  function appendPendingDirectory(path) {
    if (!path || pendingWatchDirectories.indexOf(path) >= 0) return false
    pendingWatchDirectories = pendingWatchDirectories.concat([path])
    return true
  }

  function gitDirectoryEntryMatters(changedPath, gitDirectory) {
    if (changedPath === gitDirectory) return true
    var relative = PathText.relative(changedPath, gitDirectory)
    if (relative === "") return true
    if (relative.length > 5 && relative.slice(-5) === ".lock") return false
    if (relative.indexOf("/") >= 0) return true
    return gitDecorationEntries.indexOf(relative) >= 0
  }

  function filesystemEventContext(changedPath) {
    var repoRoots = Object.keys(gitRepoDirectories)
    var worktreeRoot = ""
    for (var index = 0; index < repoRoots.length; index++) {
      var repoRoot = repoRoots[index]
      var gitDir = String(gitRepoDirectories[repoRoot] || "")
      if (gitDir && pathInside(changedPath, gitDir))
        return {
          gitDirectory: true,
          repoRoot: repoRoot,
          relevant: true,
          matters: gitDirectoryEntryMatters(changedPath, gitDir)
        }
      if (pathInside(changedPath, repoRoot) && repoRoot.length > worktreeRoot.length)
        worktreeRoot = repoRoot
    }
    return {
      gitDirectory: false,
      repoRoot: worktreeRoot,
      relevant: worktreeRoot !== "",
      matters: true
    }
  }

  function enqueueExpandedRepositoryRows(repoRoot) {
    for (var rowIndex = 0; rowIndex < treeModel.count; rowIndex++) {
      var row = treeModel.get(rowIndex)
      var rowPath = String(row.path || "")
      if (row.isDir && row.expanded && pathInside(rowPath, repoRoot)) appendPendingDirectory(rowPath)
    }
  }

  function receiveFilesystemEvent(event) {
    if (!event || event.overflow) {
      lastFilesystemEventPath = ""
      indexDirty = true
      service.refreshTree()
      scheduleGitMetadataRefresh("watch-overflow", "")
      watchRefreshTimer.restart()
      return
    }
    var changedPath = String(event.path || event.root || "")
    if (!changedPath) return
    var context = filesystemEventContext(changedPath)
    if (context.gitDirectory && context.matters) {
      enqueueExpandedRepositoryRows(context.repoRoot)
      lastFilesystemEventPath = changedPath
      scheduleGitMetadataRefresh("git-event", context.repoRoot)
      watchRefreshTimer.restart()
      return
    }
    if (context.gitDirectory) gitDirectoryEventsIgnored++
    var events = Array.isArray(event.events) ? event.events : []
    var selfGone = String(event.name || "") === "" && changedPath === String(event.root || "")
      && (events.indexOf("delete_self") >= 0 || events.indexOf("move_self") >= 0)
    var directory = selfGone ? changedPath : parentDirectory(changedPath)
    lastFilesystemEventPath = changedPath
    if (!context.gitDirectory) indexDirty = true
    var index = indexOfTreePath(directory)
    if (index >= 0 && treeModel.get(index).isDir && treeModel.get(index).expanded) appendPendingDirectory(directory)
    if (context.relevant && !context.gitDirectory)
      scheduleGitMetadataRefresh("filesystem-event", context.repoRoot)
    watchRefreshTimer.restart()
  }

  function flushFilesystemEvents() {
    var pending = pendingWatchDirectories.slice()
    pendingWatchDirectories = []
    for (var i = 0; i < pending.length; i++) {
      if (watchRefreshQueue.indexOf(pending[i]) < 0 && activeWatchRefreshPaths.indexOf(pending[i]) < 0)
        watchRefreshQueue = watchRefreshQueue.concat([pending[i]])
    }
    if (pending.length > 0) filesystemRefreshCount++
    if (indexDirty) service.backendRequest("index-invalidate", [], 0, function() {
      if (controller.searchQuery.trim() !== "") service.restartSearch()
    })
    indexDirty = false
    startNextWatchRefresh()
  }

  function startNextWatchRefresh() {
    if (refreshBusy || watchRefreshQueue.length === 0) return
    var paths = []
    while (watchRefreshQueue.length > 0 && paths.length < directoryBatchLimit) {
      var next = watchRefreshQueue[0]
      watchRefreshQueue = watchRefreshQueue.slice(1)
      var index = indexOfTreePath(next)
      if (index >= 0 && treeModel.get(index).expanded && paths.indexOf(next) < 0)
        paths.push(next)
    }
    if (paths.length === 0) {
      Qt.callLater(startNextWatchRefresh)
      return
    }
    activeWatchRefreshPaths = paths
    activeWatchRefreshResponse = null
    lastWatchBatchSize = paths.length
    watchReadProcessCount++
    var arguments = []
    for (var pathIndex = 0; pathIndex < paths.length; pathIndex++)
      arguments.push("--path", paths[pathIndex])
    if (showHidden) arguments.push("--show-hidden")
    arguments = arguments.concat(service.listingOrderArguments(service.windowLimitFor(paths)))
    refreshBusy = true
    refreshGeneration++
    var requestGeneration = refreshGeneration
    refreshRequestId = service.backendRequest("children-batch", arguments, requestGeneration, function(response) {
      if (requestGeneration !== controller.refreshGeneration) return
      controller.refreshRequestId = ""
      controller.refreshBusy = false
      controller.activeWatchRefreshResponse = response
      controller.finishWatchRefresh(0)
    })
  }

  function finishWatchRefresh(exitCode) {
    var paths = activeWatchRefreshPaths.slice()
    for (var i = 0; i < paths.length; i++) {
      var response = service.directoryResponseForPath(activeWatchRefreshResponse, paths[i], exitCode)
      service.reconcileDirectory(paths[i], response)
    }
    activeWatchRefreshPaths = []
    activeWatchRefreshResponse = null
    scheduleWatcherRestart()
    Qt.callLater(startNextWatchRefresh)
  }

  function watchPathLimit() {
    var limits = service.backendLimits || ({})
    var limit = Number(limits.watch_paths)
    return isFinite(limit) && limit > 0 ? Math.floor(limit) : 512
  }

  function watchedDirectories() {
    if (service.trashMode || service.recentMode || service.drivesMode || PathText.isRemote(rootPath)) return []
    var limit = watchPathLimit()
    var result = [rootPath]
    for (var i = 0; i < treeModel.count && result.length < limit; i++) {
      var row = treeModel.get(i)
      var path = String(row.path || "")
      if (row.isDir && row.expanded && path && result.indexOf(path) < 0) result.push(path)
    }
    var repoRoots = Object.keys(gitRepoDirectories)
    for (var repoIndex = 0; repoIndex < repoRoots.length && result.length < limit; repoIndex++) {
      var gitDir = String(gitRepoDirectories[repoRoots[repoIndex]] || "")
      if (!gitDir) continue
      if (result.indexOf(gitDir) < 0) result.push(gitDir)
      for (var subIndex = 0; subIndex < gitWatchSubdirectories.length && result.length < limit; subIndex++) {
        var subdirectory = gitDir + "/" + gitWatchSubdirectories[subIndex]
        if (result.indexOf(subdirectory) < 0) result.push(subdirectory)
      }
    }
    return result
  }

  function watcherRestartDelay() {
    return Math.min(30000, 180 * Math.pow(2, Math.min(8, watcherFailureCount)))
  }

  function watcherFingerprint(paths) {
    return JSON.stringify(Array.isArray(paths) ? paths : [])
  }

  function scheduleWatcherRestart() {
    if (!stateReady) return
    if (open && watcherRunning
        && watcherFingerprint(watchedDirectories()) === activeWatcherFingerprint) {
      watcherRestartSkippedCount++
      return
    }
    watcherRestartTimer.restart()
  }

  function startWatcher() {
    if (!open || !stateReady) return
    var paths = watchedDirectories()
    if (paths.length === 0) {
      watcherRestartPending = false
      activeWatcherFingerprint = ""
      activeWatcherPathCount = 0
      watcherRunning = false
      watcherRequestId = ""
      return
    }
    watcherRestartPending = false
    activeWatcherFingerprint = watcherFingerprint(paths)
    activeWatcherPathCount = paths.length
    watcherStartCount++
    watcherGeneration++
    var requestGeneration = watcherGeneration
    watcherRunning = true
    watcherRequestId = service.backendSubscribe(paths, requestGeneration, function(event) {
      if (requestGeneration !== controller.watcherGeneration) return
      controller.receiveFilesystemEvent(event)
    }, function(response) {
      if (requestGeneration !== controller.watcherGeneration) return
      controller.watcherRunning = true
      controller.watcherFailureCount = 0
      controller.watcherError = ""
      var skipped = response && Array.isArray(response.skipped) ? response.skipped : []
      if (skipped.length > 0) console.warn("data-goblin.fileblade: not watching " + skipped.length + " path(s): " + skipped.map(function(entry) { return String(entry.path) + " (" + String(entry.error) + ")" }).join(", "))
    }, function(response) {
      if (requestGeneration !== controller.watcherGeneration) return
      controller.watcherRequestId = ""
      controller.watcherRunning = false
      var failed = !(response && (response.ok || response.cancelled))
      if (failed) {
        controller.watcherFailureCount++
        controller.watcherError = String(response && response.error || "filesystem watcher stopped")
        console.warn("data-goblin.fileblade: filesystem watcher failed (" + controller.watcherFailureCount + "): " + controller.watcherError + "; retry in " + controller.watcherRestartDelay() + " ms")
      }
      if (controller.watcherRestartPending && controller.open && !failed) Qt.callLater(controller.startWatcher)
      else if (controller.open) {
        watcherRestartTimer.interval = failed ? controller.watcherRestartDelay() : 180
        watcherRestartTimer.restart()
      }
      else {
        controller.activeWatcherFingerprint = ""
        controller.activeWatcherPathCount = 0
      }
    })
  }

  function stopWatcher(restart) {
    watcherRestartPending = !!restart
    if (!watcherRequestId) {
      watcherRunning = false
      if (restart && open) Qt.callLater(startWatcher)
      return
    }
    service.cancelBackendRequest(watcherRequestId, watcherGeneration)
  }

  Timer {
    id: watcherRestartTimer
    interval: 180
    onTriggered: {
      watcherRestartTimer.interval = 180
      if (!service.open) {
        service.watcherRestartPending = false
        if (controller.watcherRunning) controller.stopWatcher(false)
      } else if (controller.watcherRunning) {
        if (service.watcherFingerprint(service.watchedDirectories()) === service.activeWatcherFingerprint) {
          service.watcherRestartSkippedCount++
          return
        }
        service.watcherRestartCount++
        service.watcherRestartPending = true
        controller.stopWatcher(true)
      } else {
        service.startWatcher()
      }
    }
  }

  Timer {
    id: watchRefreshTimer
    interval: 220
    onTriggered: service.flushFilesystemEvents()
  }

}
