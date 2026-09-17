import QtQuick
import Quickshell
import Quickshell.Io
import qs.Commons
import "../theme"

QtObject {
  id: root
  required property var service
  required property var bladeHost
  required property var watchController

  function publicBlade(blade) {
    var source = blade && typeof blade === "object" ? blade : ({})
    var slots = []
    var sourceSlots = Array.isArray(source.slots) ? source.slots : []
    for (var index = 0; index < sourceSlots.length; index++) {
      var sourceSlot = sourceSlots[index] || ({})
      var modules = []
      var sourceModules = Array.isArray(sourceSlot.modules) ? sourceSlot.modules : []
      for (var tab = 0; tab < sourceModules.length; tab++)
        modules.push({ module: String((sourceModules[tab] || ({})).module || "") })
      slots.push({
        id: String(sourceSlot.id || ""),
        fraction: Number(sourceSlot.fraction),
        active: Number(sourceSlot.active) || 0,
        collapsed: !!sourceSlot.collapsed,
        modules: modules
      })
    }
    return {
      open: !!source.open,
      width: Number(source.width) || 0,
      mode: String(source.mode || "docked"),
      slots: slots
    }
  }

  function publicBladesDocument() {
    var layout = bladeHost.layoutDocument()
    var blades = layout && layout.blades ? layout.blades : ({})
    return {
      version: Number(layout && layout.version) || 1,
      monitorMode: String(layout && layout.monitorMode || "active"),
      monitorLock: String(layout && layout.monitorLock || ""),
      animations: !!(layout && layout.animations),
      fontScale: Typography.clamp(layout && layout.fontScale),
      blades: { left: publicBlade(blades.left), right: publicBlade(blades.right) }
    }
  }

  function asCli(run) {
    service.operationActor = "cli"
    try { return run() } finally { service.operationActor = "ui" }
  }

  function actionsController() {
    return service.services && service.services.actions ? service.services.actions : null
  }

  function optionalIndex(value) {
    var parsed = Number(String(value || "").trim())
    return isFinite(parsed) && parsed >= 0 && String(value).trim() !== "" ? Math.floor(parsed) : -1
  }

  property IpcHandler handler: IpcHandler {
    target: "data-goblin.fileblade.control"

  function toggle(): string {
    service.toggleOpen()
    return service.open ? "open" : "closed"
  }

  function open(): string {
    service.setOpen(true)
    return "open"
  }

  function close(): string {
    service.setOpen(false)
    return "closed"
  }

  function refresh(): string {
    service.refreshTree()
    return "ok"
  }

  function refreshGit(): string {
    return service.requestVisibleGitMetadataRefresh("ipc")
  }

  function setRoot(path: string): string {
    return service.navigateToLocation(path, service.preferredScreen(), "browse")
  }

  function up(): string {
    return service.goUp()
  }

  function home(): string {
    return service.goHome()
  }

  function back(): string {
    return service.goBack()
  }

  function forward(): string {
    return service.goForward()
  }

  function setSidebarWidth(width: string): string {
    var reference = service.referenceScreen(null)
    var screenWidth = reference ? reference.width : 0
    service.setSidebarWidth(Number(width), screenWidth, true)
    return String(service.sidebarWidth)
  }

  function setPropertiesBladeWidth(width: string): string {
    var reference = service.referenceScreen(null)
    var screenWidth = reference ? reference.width : 0
    service.setPropertiesBladeWidth(Number(width), screenWidth, true)
    return String(service.propertiesBladeWidth)
  }

  function status(): string {
    return JSON.stringify({
      open: service.open,
      rootPath: service.rootPath,
      contextPath: service.contextPath,
      projectContext: service.projectContext,
      projectRoot: service.projectRoot,
      projectMarker: service.projectMarker,
      trashMode: service.trashMode,
      recentMode: service.recentMode,
      recentCount: service.recentModel.count,
      drivesMode: service.drivesMode,
      drivesCount: service.drivesController.volumeCount,
      modeBadge: service.modeBadge,
      editorMode: service.editorMode,
      trashCount: service.trashCount,
      trashSelectedId: service.trashSelectedId,
      trashBusy: service.trashBusy,
      trashOperationBusy: service.trashOperationBusy,
      trashOperationLabel: service.trashOperationLabel,
      trashError: service.trashError,
      trashLastClearedAt: service.trashLastClearedAt,
      trashLastClearedText: service.trashLastClearedText,
      trashNextCleanupText: service.trashNextCleanupText,
      canGoUp: service.canGoUp,
      canGoBack: service.canGoBack,
      backDestination: service.backDestination,
      canGoForward: service.canGoForward,
      forwardDestination: service.forwardDestination,
      rootBackStack: Array.isArray(service.rootBackStack) ? service.rootBackStack : [],
      rootForwardStack: Array.isArray(service.rootForwardStack) ? service.rootForwardStack : [],
      locationValidationBusy: service.locationValidationBusy,
      locationValidationPath: service.locationValidationPath,
      locationValidationError: service.locationValidationError,
      activeLocationPath: service.activeLocationPath,
      activeLocationMode: service.activeLocationMode,
      activeLocationOrigin: service.activeLocationOrigin,
      pendingLocationPath: service.pendingLocationPath,
      pendingLocationMode: service.pendingLocationMode,
      locationValidationCount: service.locationValidationCount,
      locationCancellationCount: service.locationCancellationCount,
      historyCurrentPruneCount: service.historyCurrentPruneCount,
      rootRecoveryOrigin: service.rootRecoveryOrigin,
      rootRecoveryNotice: service.rootRecoveryNotice,
      lastMissingRoot: service.lastMissingRoot,
      lastRecoveredRoot: service.lastRecoveredRoot,
      rootRecoveryCount: service.rootRecoveryCount,
      sidebarWidth: service.sidebarWidth,
      propertiesBladeWidth: service.propertiesBladeWidth,
      propertiesPlacement: service.propertiesPlacement,
      welcomeState: service.welcomeState,
      welcomeInstalling: service.welcome.installing,
      welcomeInstalledCount: service.welcome.installed,
      welcomeError: service.welcome.error,
      priorityProperty: service.priorityProperty,
      priorityPropertyLabel: service.priorityPropertyLabel(service.priorityProperty),
      priorityColumns: service.priorityColumns,
      gitStatusDetails: service.gitStatusDetails,
      gitSummaryFields: service.gitSummaryFields,
      treeSort: service.treeSort,
      settingsOpen: service.settingsOpen,
      settingsEdge: bladeHost.settingsEdge,
      focusedBlade: bladeHost.focusedEdge,
      frameWidth: bladeHost.frameWidth,
      bladeAnimations: bladeHost.animateBlades,
      fontScale: bladeHost.fontScale,
      focusRestoreAddress: bladeHost.restoreFocusAddress,
      focusRestoreClass: bladeHost.restoreFocusClass,
      focusRestoreCount: bladeHost.focusRestoreCount,
      lastFocusRestore: bladeHost.lastFocusRestore,
      bladeModes: { left: bladeHost.bladeMode("left"), right: bladeHost.bladeMode("right") },
      bladeWindowAddresses: bladeHost.windowAddresses,
      bladeWindowTitles: { left: bladeHost.windowTitle("left"), right: bladeHost.windowTitle("right") },
      lastWindowPlacement: bladeHost.lastWindowPlacement,
      lastFocusDirection: bladeHost.lastFocusDirection,
      focusDirectionCount: bladeHost.focusDirectionCount,
      dragActive: bladeHost.dragActive,
      dropEdge: bladeHost.dropEdge,
      dropIndex: bladeHost.dropIndex,
      bladeModules: bladeHost.registry.order,
      bladeLayoutPath: bladeHost.layoutPath,
      showHidden: service.showHidden,
      scrollMarks: service.scrollMarks,
      autoHideSearch: service.autoHideSearch,
      gitEnabled: service.gitEnabled,
      trashRetentionDays: service.trashRetentionDays,
      monitorMode: service.monitorMode,
      monitorLock: bladeHost.monitorLock,
      focusedMonitor: bladeHost.focusedMonitorName,
      bladeScreens: { left: bladeHost.bladeScreenName("left"), right: bladeHost.bladeScreenName("right") },
      pendingTrashCount: Array.isArray(service.pendingTrashPaths) ? service.pendingTrashPaths.length : 0,
      serviceGeneration: service.serviceGeneration,
      pluginWatcherFiltered: service.pluginWatcherFiltered,
      pluginWatcherStopPending: service.pluginWatcherStopPending,
      pluginWatcherHandoffCount: service.pluginWatcherHandoffCount,
      pluginWatcherPattern: service.pluginWatcherExcludePattern,
      pluginWatcherRunning: !!(service.pluginRegistry && service.pluginRegistry.localPluginWatcher
        && service.pluginRegistry.localPluginWatcher.running),
      pluginWatcherCommand: service.pluginRegistry && service.pluginRegistry.localPluginWatcher
        ? service.pluginRegistry.localPluginWatcher.command.join(" ") : "",
      pluginWatcherApplyPending: service.desiredPluginWatcherCommand.length > 0,
      treeEntries: service.treeModel.count,
      treeReadProcessCount: service.treeReadProcessCount,
      lastTreeBatchSize: service.lastTreeBatchSize,
      filesystemRefreshCount: service.filesystemRefreshCount,
      lastFilesystemEventPath: service.lastFilesystemEventPath,
      watcherRunning: watchController.watcherRunning,
      watcherRestartPending: service.watcherRestartPending,
      watchedDirectoryCount: service.activeWatcherPathCount,
      watcherStartCount: service.watcherStartCount,
      watcherRestartCount: service.watcherRestartCount,
      watcherRestartSkippedCount: service.watcherRestartSkippedCount,
      watchRefreshBusy: watchController.refreshBusy || service.watchRefreshQueue.length > 0,
      watchReadProcessCount: service.watchReadProcessCount,
      lastWatchBatchSize: service.lastWatchBatchSize,
      gitRepositoryCount: service.gitEnabled ? Object.keys(service.gitRepoDirectories).length : 0,
      gitStatusPollIntervalMs: service.gitStatusPollIntervalMs,
      gitMetadataBusy: service.gitMetadataBusy,
      gitMetadataQueued: service.gitMetadataQueued,
      gitMetadataRefreshCount: service.gitMetadataRefreshCount,
      gitMetadataNoopCount: service.gitMetadataNoopCount,
      gitMetadataScopedRefreshCount: service.gitMetadataScopedRefreshCount,
      gitDirectoryEventsIgnored: service.gitDirectoryEventsIgnored,
      gitStatusPollBackoff: service.gitStatusPollBackoff,
      lastGitMetadataPathCount: service.lastGitMetadataPathCount,
      lastGitMetadataRepositoryCount: service.lastGitMetadataRepositoryCount,
      lastGitMetadataReason: service.lastGitMetadataReason,
      gitMetadataError: service.gitMetadataError,
      searchQuery: service.searchQuery,
      searchCaseSensitive: service.searchCaseSensitive,
      searchRegex: service.searchRegex,
      quickNavActive: service.quickNavActive,
      searchBackend: service.searchBackend,
      searchTreeLayout: service.searchTreeLayout,
      keybindingsPath: service.keybindings.path,
      keybindingsError: service.keybindings.error,
      searchFilterSummary: service.searchFilterSummary,
      searchGitRepositories: service.searchGitRepositoryCount,
      searchBusy: service.searchBusy,
      searchResults: service.searchModel.count,
      activeSearchQuery: service.activeSearchQuery,
      lastSearchExitCode: service.lastSearchExitCode,
      lastSearchPayloadCount: service.lastSearchPayloadCount,
      lastSearchStderr: service.lastSearchStderr,
      searchCancellationCount: service.searchCancellationCount,
      selectedPath: service.selectedPath,
      selectedPaths: service.selectedPaths,
      selectedCount: service.selectedCount,
      selectedKind: service.selectedMetadata ? String(service.selectedMetadata.kind || "") : "",
      selectedFolderCount: service.selectedFolderCount,
      selectionFolderColor: service.selectionFolderColor,
      folderColorCount: Object.keys(service.folderColors).length,
      folderColorChoices: service.folderColorChoices,
      themeFolderPalette: service.themeFolderPalette,
      favoriteCount: service.favorites.length,
      applicationsPath: service.applicationsPath,
      activeApplicationsPath: service.activeApplicationsPath,
      pendingApplicationsPath: service.pendingApplicationsPath,
      applicationsMime: service.applicationsMime,
      applicationsBusy: service.applicationsBusy,
      applicationsLoaded: service.applicationsLoaded,
      applicationsCount: service.applicationModel.count,
      applicationsError: service.applicationsError,
      applicationLookupCount: service.applicationLookupCount,
      applicationCancellationCount: service.applicationCancellationCount,
      applicationCacheHitCount: service.applicationCacheHitCount,
      applicationMimeHintCount: service.applicationMimeHintCount,
      actionMenuOpen: service.actionMenuOpen,
      actionMenuMode: service.actionMenuMode,
      actionMenuPath: service.actionMenuPath,
      actionMenuPaths: service.actionMenuPaths,
      dropWheel: {
        open: service.dropWheel.wheelOpen, dragging: service.dropWheel.dragActive,
        fromDrag: service.dropWheel.wheelFromDrag, loading: service.dropWheel.loading,
        x: service.dropWheel.wheelX, y: service.dropWheel.wheelY,
        count: service.dropWheel.count, highlighted: service.dropWheel.highlighted,
        outerFocus: service.dropWheel.outerFocus, outerHighlighted: service.dropWheel.outerHighlighted,
        error: service.dropWheel.error, target: service.dropWheel.targetLabel,
        actions: service.dropWheel.ringItems.map(function(item) {
          return { id: item.id, key: item.key, label: item.label,
            placements: (item.placements || []).map(function(child) { return { id: child.id, key: child.key, label: child.label } }) }
        })
      },
      clipboardCount: service.clipboardPaths.length,
      clipboardMode: service.clipboardMode,
      externalClipboardCount: service.externalClipboardPaths.length,
      externalClipboardMode: service.externalClipboardMode,
      externalClipboardMime: service.externalClipboardMime,
      externalClipboardBusy: service.externalClipboardBusy,
      externalClipboardError: service.externalClipboardError,
      pasteCount: service.pasteCount,
      operationBusy: service.operationBusy,
      operationLabel: service.operationLabel,
      operationNotice: service.operationNotice,
      operationError: service.operationError,
      operationPendingCount: service.operationPendingCount,
      activeOperationId: service.activeOperationId,
      undoCount: service.history.undoCount,
      undoLabel: service.history.undoLabel,
      redoCount: service.history.redoCount,
      redoLabel: service.history.redoLabel,
      operationCancellable: service.operationCancellable,
      operationCancelRequested: service.operationCancelRequested,
      operationProgress: service.activeOperationProgress || ({}),
      operationResultCount: service.operationResults.length,
      pendingExternalPasteOperationId: service.pendingExternalPasteOperationId,
      launchBusy: service.launchBusy,
      launchStatus: service.launchStatus,
      launchError: service.launchError,
      lastLaunchedPath: service.lastLaunchedPath,
      lastLaunchedAddress: service.lastLaunchedAddress,
      pickerActive: service.pickerActive,
      pickerRequestId: service.pickerRequestId,
      pickerMode: service.pickerMode,
      pickerSaveValidationBusy: service.pickerSaveValidationBusy,
      pickerPendingSavePath: service.pickerPendingSavePath,
      pickerOverwriteArmed: service.pickerOverwriteArmed,
      pickerOverwritePath: service.pickerOverwritePath,
      metadataBusy: service.metadataBusy,
      statCancellationCount: service.statCancellationCount,
      selectionImportError: service.selectionImportError,
      searchError: service.searchError,
      metadataError: service.metadataError
    })
  }

  function tree(limit: string): string {
    return JSON.stringify(service.treeDocument(limit))
  }

  function searchResults(limit: string): string {
    return JSON.stringify(service.searchResultsDocument(limit))
  }

  function search(query: string): string {
    service.quickNavActive = false
    service.searchQuery = query
    return "ok"
  }

  function setSearchDeep(deep: string): string {
    service.setSearchDeep(String(deep).toLowerCase() === "true")
    return service.searchDeep ? "deep" : "shallow"
  }

  function searchWith(query: string, mode: string): string {
    service.searchWith(query, mode)
    return "ok"
  }

  function setSearchLayout(tree: string): string {
    service.setSearchLayout(String(tree).toLowerCase() === "true")
    return service.searchTreeLayout ? "tree" : "list"
  }

  function reloadKeybindings(): string { service.keybindings.reload(); return "queued" }

  function setSearchOptions(caseSensitive: string, regex: string): string {
    service.setSearchOptions(String(caseSensitive).toLowerCase() === "true", String(regex).toLowerCase() === "true")
    return "ok"
  }

  function clearSearch(): string {
    service.stopQuickNav()
    service.stopList()
    service.searchQuery = ""
    return "ok"
  }

  function showList(file: string): string {
    service.startList(file)
    return "ok"
  }

  function quickNav(): string {
    service.startQuickNav(null, "folders")
    return "ok"
  }

  function quickNavChannel(channel: string): string {
    service.startQuickNav(null, String(channel || "folders"))
    return "ok"
  }

  function setPriorityProperty(property: string): string {
    return service.setPriorityProperty(property)
  }

  function setPriorityColumns(columns: string): string {
    return service.setPriorityColumns(columns).join(",")
  }

  function setGitStatusDetails(details: string): string {
    return service.setGitStatusDetails(details).join(",")
  }

  function setGitSummaryFields(fields: string): string {
    return service.setGitSummaryFields(fields).join(",")
  }

  function cyclePriorityProperty(): string {
    return service.cyclePriorityProperty()
  }

  function setShowHidden(visible: string): string {
    service.setShowHidden(String(visible).toLowerCase() === "true")
    return service.showHidden ? "shown" : "hidden"
  }

  function toggleHidden(): string {
    service.toggleHidden()
    return service.showHidden ? "shown" : "hidden"
  }

  function setScrollMarks(enabled: string): string {
    service.setScrollMarks(String(enabled).toLowerCase() === "true")
    return service.scrollMarks ? "shown" : "hidden"
  }

  function select(path: string): string {
    var known = service.entryForKnownPath(path)
    if (known) {
      service.applySelection([known], known, known.path)
      return "ok"
    }
    service.selectPath(path, false, service.rootName(path), "", "")
    return "unverified"
  }

  function setAutoHideSearch(value: string): string {
    return service.setAutoHideSearch(value === "true") ? "hidden-until-focused" : "always-visible"
  }

  function selectEntries(entries: string): string {
    return service.selectEntriesDocument(entries) ? "ok" : "invalid-entries"
  }

  function selection(): string {
    return JSON.stringify(service.selectionDocument())
  }

  function showActions(): string {
    if (service.selectedCount === 0) return "no-selection"
    service.setOpen(true)
    var target = service.referenceScreen(null)
    service.openActionMenu("actions", target, service.sidebarWidth - Style.space(8), Style.space(70))
    return "open"
  }

  function showOpenWith(): string {
    if (service.selectedCount !== 1)
      return service.selectedCount === 0 ? "no-selection" : "multiple-selection"
    var entry = service.selectedEntries[0]
    if (!entry || entry.isDir || entry.gitDeleted) return "not-a-file"
    service.setOpen(true)
    var target = service.referenceScreen(null)
    service.openActionMenu("open-with", target, service.sidebarWidth - Style.space(8), Style.space(70))
    return "open"
  }

  function hideActions(): string {
    service.closeActionMenu()
    return "closed"
  }

  function showDropWheel(x: string, y: string): string {
    if (service.selectedCount === 0) return "no-selection"
    var pointX = Number(x)
    var pointY = Number(y)
    if (!isFinite(pointX) || !isFinite(pointY)) return "invalid-point"
    var target = service.dropWheel.screenAt(pointX, pointY)
    if (!target) return "off-screen"
    var localX = pointX - (target ? Number(target.x) || 0 : 0)
    var localY = pointY - (target ? Number(target.y) || 0 : 0)
    return service.dropWheel.openForSelection(target, localX, localY) ? "open" : "no-selection"
  }

  function hideDropWheel(): string {
    service.dropWheel.close()
    return "closed"
  }

  function copySelection(cut: string): string {
    if (!service.copySelection(String(cut).toLowerCase() === "true")) return "no-selection"
    return String(cut).toLowerCase() === "true" ? "cut" : "copied"
  }

  function copyPaths(): string {
    if (!service.copyPaths()) return "no-selection"
    return "copied"
  }

  function clearClipboard(): string {
    service.clearFileClipboard()
    return "cleared"
  }

  function paste(destination: string): string {
    return root.asCli(function() { return service.pasteInto(String(destination || "")) })
  }

  function moveSelectionTo(destination: string, copyInstead: string): string {
    if (service.selectedCount === 0) return "no-selection"
    if (!String(destination || "").trim()) return "invalid-destination"
    return root.asCli(function() { return service.moveSelectionTo(destination, String(copyInstead).toLowerCase() === "true") })
  }

  function renameSelection(name: string): string {
    if (service.selectedCount !== 1) return service.selectedCount === 0 ? "no-selection" : "multiple-selection"
    if (!String(name || "").trim()) return "invalid-name"
    return root.asCli(function() { return service.renameSelection(name) })
  }

  function createEntry(name: string, directory: string, parent: string): string {
    if (!String(name || "").trim()) return "invalid-name"
    return root.asCli(function() { return service.createEntry(name, String(directory).toLowerCase() === "true", String(parent || "")) })
  }

  function trashSelection(): string {
    if (service.selectedCount === 0) return "no-selection"
    return root.asCli(function() { return service.trashSelection() })
  }

  function operationResult(requestId: string): string {
    return JSON.stringify(service.operationResult(requestId))
  }

  function undo(drop: string, force: string): string {
    return root.asCli(function() { return service.history.undoOperation(String(drop).toLowerCase() === "true", String(force).toLowerCase() === "true") || "nothing-to-undo" })
  }

  function redo(drop: string, force: string): string {
    return root.asCli(function() { return service.history.redoOperation(String(drop).toLowerCase() === "true", String(force).toLowerCase() === "true") || "nothing-to-redo" })
  }

  function history(): string {
    return JSON.stringify({ undo: service.history.journalUndo, redo: service.history.journalRedo })
  }

  function refreshHistory(): string {
    service.history.refreshJournal()
    return "refreshing"
  }

  function cancelOperation(requestId: string): string {
    return service.cancelOperation(requestId)
  }

  function openPath(path: string): string {
    return service.openDefault(path) ? "queued" : "invalid-path"
  }

  function openSelection(): string {
    if (service.selectedCount !== 1) return service.selectedCount === 0 ? "no-selection" : "multiple-selection"
    return service.openDefault(service.selectedPath) ? "queued" : "invalid-path"
  }

  function editPath(path: string): string {
    return service.enqueueLaunch(path, "editor", "") ? "queued" : "invalid-path"
  }

  function revealPath(path: string): string {
    return service.enqueueLaunch(path, "reveal", "") ? "queued" : "invalid-path"
  }

  function openWithPath(path: string, desktopId: string): string {
    if (!String(path || "").trim()) return "invalid-path"
    if (!String(desktopId || "").trim()) return "invalid-application"
    return service.enqueueLaunch(path, "application", desktopId) ? "queued" : "invalid-path"
  }

  function colorSelection(color: string): string {
    if (service.colorableSelectionCount === 0) return "no-selection"
    return service.setSelectionFolderColor(color) ? service.folderColorLabel(color) : "invalid-color"
  }

  function setFolderColor(path: string, color: string): string {
    return service.setFolderColor(path, color) ? service.folderColorLabel(color) : "invalid-color"
  }

  function setFolderColorScope(scope: string): string {
    return service.setFolderColorScope(scope)
  }

  function clearFolderColor(path: string): string {
    service.setFolderColor(path, "")
    return "Default (theme)"
  }

  function folderColor(path: string): string {
    return JSON.stringify({
      path: service.normalizeRoot(path),
      color: service.folderColor(path),
      label: service.folderColorLabel(service.folderColor(path))
    })
  }

  function favorites(): string {
    return JSON.stringify(service.favorites)
  }

  function pin(path: string, name: string, isDir: string, isSymlink: string, mime: string, isGitRepo: string): string {
    var added = service.pinFavorite(
      path,
      name,
      String(isDir).toLowerCase() === "true",
      String(isSymlink).toLowerCase() === "true",
      mime,
      String(isGitRepo).toLowerCase() === "true"
    )
    return added || service.isFavorite(path) ? "pinned" : "invalid-path"
  }

  function pinMany(entries: string): string {
    return JSON.stringify(service.pinFavoritesDocument(entries))
  }

  function unpin(path: string): string {
    return service.unpinFavorite(path) ? "unpinned" : "not-pinned"
  }

  function unpinMany(paths: string): string {
    return JSON.stringify(service.unpinFavoritesDocument(paths))
  }

  function toggleFavorite(path: string): string {
    var entry = service.entryForKnownPath(path)
    if (!entry || entry.gitDeleted) return "not-visible"
    service.toggleFavorite(entry.path, entry.name, entry.isDir, entry.isSymlink, entry.mime, entry.isGitRepo)
    return service.isFavorite(entry.path) ? "pinned" : "unpinned"
  }

  function pick(options: string): string {
    return service.beginPicker(options)
  }

  function pickerResult(requestId: string): string {
    return JSON.stringify(service.pickerResult(requestId))
  }

  function confirmPick(): string {
    if (service.confirmPicker()) return "accepted"
    if (service.pickerSaveValidationBusy) return "checking"
    if (service.pickerOverwriteArmed) return "confirm-overwrite"
    return "incomplete"
  }

  function cancelPick(): string {
    if (!service.pickerActive) return "inactive"
    service.cancelPicker()
    return "cancelled"
  }

  function setPlacement(placement: string): string {
    service.setPropertiesPlacement(placement)
    return service.propertiesPlacement
  }

  function setModeBadge(placement: string): string {
    return service.setModeBadge(placement)
  }

  function focusBlade(edge: string): string {
    return bladeHost.focusBlade(edge, bladeHost.preferredScreen(edge), -1, "", true) ? "focused" : "no-screen"
  }

  function toggleBladeFocus(edge: string): string {
    return bladeHost.toggleBladeFocus(edge, bladeHost.preferredScreen(edge))
  }

  function focusLeft(): string {
    return bladeHost.toggleBladeFocus("left", bladeHost.preferredScreen("left"))
  }

  function focusRight(): string {
    return bladeHost.toggleBladeFocus("right", bladeHost.preferredScreen("right"))
  }

  function openBlade(edge: string): string {
    bladeHost.setOpen(edge, true, true)
    return "open"
  }

  function setWelcomeState(value: string): string {
    return String(service.setWelcomeState(value))
  }

  function welcomeInstall(): string {
    return service.welcome.install() ? "started" : "busy"
  }

  function welcomeDismiss(): string {
    return service.welcome.dismiss() ? "dismissed" : "busy"
  }

  function resetBladeLayout(): string {
    return bladeHost.resetLayout() ? "reset" : "unchanged"
  }

  function revertDefaults(): string {
    return bladeHost.revertDefaults() ? "reverted" : "unchanged"
  }

  function closeBlade(edge: string): string {
    bladeHost.setOpen(edge, false, true)
    return "closed"
  }

  function toggleBlade(edge: string): string {
    return bladeHost.toggleOpen(edge) ? "open" : "closed"
  }

  function setBladeWidth(edge: string, width: string): string {
    var reference = service.referenceScreen(null)
    var screenWidth = reference ? reference.width : 0
    return String(bladeHost.setWidth(edge, Number(width), screenWidth, true))
  }

  function blades(): string {
    return JSON.stringify(root.publicBladesDocument())
  }

  function setBladeSlots(edge: string, slots: string): string {
    var decoded = service.decodedJsonDocument(slots)
    if (!decoded.ok || !Array.isArray(decoded.value)) return "invalid-slots"
    bladeHost.setSlots(edge, decoded.value)
    return JSON.stringify(bladeHost.bladeFor(edge))
  }

  function setSlotModule(edge: string, index: string, module: string): string {
    return bladeHost.setSlotModule(edge, Number(index), module) ? "ok" : "invalid-slot"
  }

  function addBladeModule(edge: string, module: string): string {
    return bladeHost.addSlot(edge, module, -1) ? "ok" : "invalid-module"
  }

  function removeBladeSlot(edge: string, index: string): string {
    return bladeHost.removeSlot(edge, Number(index)) ? "ok" : "invalid-slot"
  }

  function setBladeSlotCollapsed(edge: string, index: string, collapsed: string): string {
    var value = String(collapsed).toLowerCase()
    return bladeHost.setSlotCollapsed(edge, Number(index), value === "true" || value === "on" || value === "1")
      ? "ok"
      : "invalid-slot"
  }

  function toggleBladeSlotCollapsed(edge: string, index: string): string {
    return bladeHost.toggleSlotCollapsed(edge, Number(index)) ? "ok" : "invalid-slot"
  }

  function moveBladeModule(module: string, edge: string, index: string): string {
    return bladeHost.moveModule(module, edge, Number(index)) ? "ok" : "invalid-module"
  }

  function bladeModules(): string {
    return bladeHost.registry.ipcDocument(function(id) { return bladeHost.findModule(id) })
  }

  function rescanBladeModules(): string {
    bladeHost.registry.rescan()
    return "ok"
  }

  function actions(): string {
    var runner = root.actionsController()
    return JSON.stringify(runner ? runner.document() : { ok: false, error: "script actions are unavailable" })
  }

  function actionResult(requestId: string): string {
    var runner = root.actionsController()
    return JSON.stringify(runner ? runner.result(requestId) : { status: "unknown", id: String(requestId || "") })
  }

  function runAction(key: string, pathsJson: string, yes: string): string {
    var runner = root.actionsController()
    if (!runner) return JSON.stringify({ ok: false, error: "script actions are unavailable" })
    return JSON.stringify(runner.runFromIpc(key, pathsJson, String(yes).toLowerCase() === "true"))
  }

  function setBladeAnimations(enabled: string): string {
    var value = String(enabled).toLowerCase()
    if (value === "toggle") return bladeHost.setAnimateBlades(!bladeHost.animateBlades) ? "on" : "off"
    return bladeHost.setAnimateBlades(value === "true" || value === "on" || value === "1") ? "on" : "off"
  }

  function toggleBladeSettings(edge: string): string {
    return bladeHost.toggleSettings(edge) ? "open" : "closed"
  }

  function undockBlade(edge: string): string {
    return bladeHost.undock(edge)
  }

  function dockBlade(edge: string): string {
    return bladeHost.dock(edge)
  }

  function toggleBladeDock(edge: string): string {
    return bladeHost.toggleDock(edge)
  }

  function releaseBladeFocus(): string {
    return bladeHost.releaseFocus("") ? "released" : "none"
  }

  function focusDirection(direction: string): string {
    return bladeHost.focusDirection(direction)
  }

  function windowClose(): string {
    return bladeHost.windowClose()
  }

  function windowToggle(): string {
    return bladeHost.windowToggle()
  }

  function windowResize(deltaX: string, deltaY: string): string {
    return bladeHost.windowResize(Number(deltaX), Number(deltaY))
  }

  function windowSwap(direction: string): string {
    return bladeHost.windowSwap(direction)
  }

  function setMonitorMode(mode: string, monitor: string): string {
    return String(bladeHost.setMonitorMode(mode, monitor))
  }

  function focusBladeOn(edge: string, monitor: string): string {
    var screen = bladeHost.screenNamed(monitor)
    if (String(monitor || "") !== "" && !screen) return "unknown-monitor"
    return bladeHost.focusBlade(edge, screen, -1, "", true) ? "focused" : "no-screen"
  }

  function moveBladeSlot(sourceEdge: string, sourceIndex: string, targetEdge: string, targetIndex: string, tabIndex: string): string {
    return bladeHost.moveSlotTo(sourceEdge, Number(sourceIndex), targetEdge, Number(targetIndex), root.optionalIndex(tabIndex)) ? "ok" : "invalid-slot"
  }

  function tabBladeSlot(sourceEdge: string, sourceIndex: string, targetEdge: string, targetSlot: string, tabIndex: string, insertAt: string): string {
    return bladeHost.moveTabInto(sourceEdge, Number(sourceIndex), root.optionalIndex(tabIndex), targetEdge, Number(targetSlot), root.optionalIndex(insertAt)) ? "ok" : "invalid-slot"
  }

  function setBladeTab(edge: string, slotIndex: string, tabIndex: string): string {
    return bladeHost.setSlotTab(edge, Number(slotIndex), Number(tabIndex)) ? "ok" : "invalid-tab"
  }

  function cycleBladeTab(edge: string, slotIndex: string, delta: string): string {
    return bladeHost.cycleSlotTab(edge, Number(slotIndex), Number(delta)) ? "ok" : "no-tabs"
  }

  function removeBladeTab(edge: string, slotIndex: string, tabIndex: string): string {
    return bladeHost.removeTab(edge, Number(slotIndex), Number(tabIndex)) ? "ok" : "invalid-tab"
  }

  function focusTree(): string {
    service.focusTree(null)
    return "ok"
  }

  function focusProperties(): string {
    service.focusProperties(null)
    return "ok"
  }

  function focusSearch(): string {
    service.focusSearch(null)
    return "ok"
  }

  function focusLocation(): string {
    service.focusLocation(null)
    return "ok"
  }

  function navigate(path: string): string {
    var target = service.preferredScreen()
    return service.navigateToLocation(path, target, "browse")
  }

  function clearLocationError(): string {
    service.clearLocationValidationError()
    return "cleared"
  }

  function toggleSettings(): string {
    service.toggleSettings()
    return service.settingsOpen ? "open" : "closed"
  }

  function togglePath(path: string): string {
    var index = service.indexOfTreePath(path)
    if (index < 0) return "not-visible"
    if (!service.treeModel.get(index).isDir) return "not-directory"
    service.toggleDirectory(index)
    return service.treeModel.get(index).expanded ? "expanded" : "collapsed"
  }

  function expandPath(path: string): string {
    return service.setDirectoryExpanded(path, true)
  }

  function collapsePath(path: string): string {
    return service.setDirectoryExpanded(path, false)
  }
  }

  property IpcHandler readHandler: IpcHandler {
    target: "data-goblin.fileblade"

    function status(): string { return root.handler.status() }
    function tree(limit: string): string { return root.handler.tree(limit) }
    function searchResults(limit: string): string { return root.handler.searchResults(limit) }
    function selection(): string { return root.handler.selection() }
    function operationResult(requestId: string): string { return root.handler.operationResult(requestId) }
    function history(): string { return root.handler.history() }
    function folderColor(path: string): string { return root.handler.folderColor(path) }
    function favorites(): string { return root.handler.favorites() }
    function pickerResult(requestId: string): string { return root.handler.pickerResult(requestId) }
    function blades(): string { return root.handler.blades() }
    function bladeModules(): string { return root.handler.bladeModules() }
    function actions(): string { return root.handler.actions() }
    function actionResult(requestId: string): string { return root.handler.actionResult(requestId) }
  }
}
