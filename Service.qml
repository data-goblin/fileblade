import QtQuick
import Quickshell
import Quickshell.Io
import "blades"
import "controllers"
import "lib/PathText.js" as PathText
import "lib/GitSummary.js" as GitSummary

Item {
  id: service

  property var shell: null
  property var manifest: null
  property var pluginRegistry: null

  property alias pluginWatcherExcludePattern: pluginWatcherController.excludePattern
  property alias pluginWatcherFiltered: pluginWatcherController.filtered
  property alias pluginWatcherStopPending: pluginWatcherController.stopPending
  property alias pluginWatcherHandoffCount: pluginWatcherController.handoffCount
  property alias desiredPluginWatcherCommand: pluginWatcherController.desiredCommand
  property double serviceGeneration: Date.now()

  readonly property string home: Quickshell.env("HOME") || "/"
  readonly property string stateHome: Quickshell.env("XDG_STATE_HOME") || (home + "/.local/state")
  readonly property string stateDir: stateHome + "/omarchy/fileblade"
  readonly property string statePath: stateDir + "/state.json"
  readonly property string fallbackPluginDir: decodeURIComponent(Qt.resolvedUrl(".").toString().replace(/^file:\/\//, "").replace(/\/$/, ""))
  readonly property string pluginDir: manifest && manifest.__sourceDir ? String(manifest.__sourceDir) : fallbackPluginDir
  readonly property string cliPath: pluginDir + "/fileblade"
  readonly property string trashResource: "trash:///"
  readonly property alias artifactActions: artifactActions
  readonly property bool trashMode: normalizeRoot(rootPath) === trashResource
  readonly property string recentResource: "recent:///"
  readonly property bool recentMode: normalizeRoot(rootPath) === recentResource
  readonly property string drivesResource: "drives:///"
  readonly property bool drivesMode: normalizeRoot(rootPath) === drivesResource
  readonly property bool backendReady: backendClient.ready
  readonly property bool backendStalled: backendClient.stalled
  readonly property bool backendVersionSkew: backendClient.versionSkew
  readonly property string backendVersion: backendClient.backendVersion
  readonly property var backendLog: backendClient.backendLog
  function retryBackend() { backendClient.retry() }
  function startExtensionInstall() { backendClient.startExtensionInstall() }
  readonly property var backendLimits: backendClient.limits
  readonly property string backendError: backendClient.lastError

  BackendClient {
    id: backendClient
    cliPath: service.cliPath
    expectedVersion: service.manifest && service.manifest.version ? String(service.manifest.version) : ""
  }

  ArtifactActionController { id: artifactActions; service: service }
  ExtensionCatalog {
    id: extensionCatalog
    service: service
    watchPaths: [service.home + "/.config/omarchy", service.home + "/.config/omarchy/plugins"]
    onRefreshed: bladeHost.registry.rescan()
  }

  Connections {
    target: service
    function onBackendReadyChanged() {
      if (!service.backendReady) return
      extensionCatalog.refresh()
      extensionCatalog.watch()
    }
    function onPluginRegistryChanged() { if (service.backendReady) extensionCatalog.refresh() }
  }

  Connections {
    target: bladeHost
    function onBladeOpened(edge) { if (service.backendReady) extensionCatalog.refreshIfStale() }
  }
  ExtensionProviders {
    id: extensionProviders
    providers: extensionCatalog.providers
    disclosed: service.pluginRegistry && service.pluginRegistry.installedPlugins ? service.pluginRegistry.installedPlugins : ({})
    files: service
    inventoryUrl: service.pluginDir ? "file://" + service.pluginDir + "/ui/ArtifactInventory.qml" : ""
  }
  readonly property alias extensionCatalog: extensionCatalog
  KeybindingsController { id: keybindings; service: service }
  PreferencesController { id: preferencesController; service: service }
  FileView {
    id: preferencesWatch
    path: bladeHost.configDir + "/settings.json"
    preload: false
    watchChanges: true
    printErrors: false
    onFileChanged: { reload(); preferencesController.refreshSoon() }
  }
  property alias preferences: preferencesController
  readonly property bool agentManagementEnabled: preferencesController.agentManagement
  readonly property alias keybindings: keybindings
  WelcomeController { id: welcomeController; service: service }
  readonly property alias welcome: welcomeController

  function backendRequest(name, arguments, generation, callback, progress, deadlineMs, options) {
    return backendClient.request(name, arguments, generation, callback, progress, deadlineMs, options)
  }

  function cancelBackendRequest(id, generation, discardCallbacks) {
    return backendClient.cancel(id, generation, discardCallbacks)
  }
  function backendSubscribe(paths, generation, eventCallback, readyCallback, closedCallback) {
    return backendClient.subscribe(paths, generation, eventCallback, readyCallback, closedCallback)
  }

  function backendSubscribeTopic(topic, paths, generation, eventCallback, readyCallback, closedCallback) {
    return backendClient.subscribeTopic(topic, paths, generation, eventCallback, readyCallback, closedCallback)
  }

  function backendCommand(name) {
    return [cliPath, "_backend", String(name)]
  }

  function moduleDirs(id, callback) { return bladeHost.dirs.ensure(id, callback) }
  readonly property var services: {
    var map = ({ files: service, actions: actionController })
    var supplied = extensionProviders.services
    var ids = supplied ? Object.keys(supplied) : []
    for (var i = 0; i < ids.length; i++) if (!map[ids[i]]) map[ids[i]] = supplied[ids[i]]
    return map
  }

  property alias stateReady: stateController.ready
  property alias bladeHost: bladeHost
  readonly property bool open: bladeHost.anyOpen
  readonly property var installedAgents: agentsController.installedAgents
  readonly property string projectRoot: projectController.projectRoot
  readonly property string projectMarker: projectController.projectMarker
  readonly property string contextPath: projectController.contextPath
  readonly property int sidebarWidth: bladeHost.bladeWidth("left")
  readonly property int propertiesBladeWidth: bladeHost.bladeWidth("right")
  readonly property string propertiesPlacement: bladeHost.propertiesPlacement()
  readonly property string monitorMode: bladeHost.monitorMode
  readonly property bool settingsOpen: bladeHost.settingsOpen
  readonly property string focusedBlade: bladeHost.focusedEdge
  property alias showHidden: stateController.showHidden
  property alias welcomeState: stateController.welcomeState
  function setWelcomeState(value) { return stateController.setWelcomeState(value) }
  property alias searchCaseSensitive: stateController.searchCaseSensitive
  property alias searchRegex: stateController.searchRegex
  property alias searchTreeLayout: stateController.searchTreeLayout
  property alias searchDeep: stateController.searchDeep
  property alias searchDeepActive: searchController.deep
  property alias searchHistory: stateController.searchHistory
  property alias treeSort: stateController.treeSort
  property alias treeFilter: stateController.treeFilter
  property alias rootPath: stateController.rootPath
  property alias rootBackStack: stateController.rootBackStack
  property alias rootForwardStack: stateController.rootForwardStack
  property alias folderColors: stateController.folderColors
  property alias priorityProperty: stateController.priorityProperty
  property alias priorityColumns: stateController.priorityColumns
  property alias gitEnabled: stateController.gitEnabled
  property alias projectContext: stateController.projectContext
  property alias gitStatusDetails: stateController.gitStatusDetails
  property alias gitSummaryFields: stateController.gitSummaryFields
  readonly property var gitSummaryChoices: GitSummary.choices
  property alias propertyIcons: stateController.propertyIcons
  property alias confirmTrash: stateController.confirmTrash
  property alias scrollMarks: stateController.scrollMarks
  property alias autoHideSearch: stateController.autoHideSearch
  property alias showSystemVolumes: stateController.showSystemVolumes
  property alias modeBadge: stateController.modeBadge
  property alias dragOut: stateController.dragOut
  property string editorMode: "NORMAL"
  readonly property color editorModeColor: editorMode === "VISUAL" ? themedFolderColor("magenta", "#c678dd") : (editorMode === "INSERT" ? themedFolderColor("green", "#98c379") : themedFolderColor("blue", "#61afef"))
  property alias folderColorScope: stateController.folderColorScope
  property alias favorites: stateController.favorites
  property alias trashCleanupConsent: preferencesController.trashCleanupConsent
  property alias trashRetentionDays: preferencesController.trashRetentionDays
  property alias trashLastClearedAt: stateController.trashLastClearedAt
  property alias updateCheckedAt: stateController.updateCheckedAt
  property alias favoritePathLookup: stateController.favoriteLookup
  readonly property bool canGoUp: normalizeRoot(rootPath) !== "/" && !trashMode && !recentMode && !drivesMode
  readonly property bool canGoBack: Array.isArray(rootBackStack) && rootBackStack.length > 0
  readonly property string backDestination: canGoBack ? String(rootBackStack[rootBackStack.length - 1]) : ""
  readonly property bool canGoForward: Array.isArray(rootForwardStack) && rootForwardStack.length > 0
  readonly property string forwardDestination: canGoForward ? String(rootForwardStack[rootForwardStack.length - 1]) : ""

  BladeHost {
    id: bladeHost
    shell: service.shell
    pluginRegistry: service.pluginRegistry
    catalogProviders: extensionCatalog.providers
    providerErrors: extensionProviders.errors
    pluginDir: service.pluginDir
    config: service.pluginConfig()
    services: service.services
    updates: updateController
    onAnyOpenChanged: if (anyOpen) updateController.checkIfStale()
    onLayoutApplied: {
      if (bladeHost.pendingOpenEdges && bladeHost.pendingOpenEdges.left) navigationController.focusAfterOpen()
    }
  }

  signal locationValidationFinished(var targetScreen, bool success, string path, string error, string monitor)
  signal trashConfirmationRequested(var paths)
  property var pendingTrashPaths: []
  property int trashConfirmationSerial: 0
  function resolveTrashConfirmation(confirm) {
    var paths = Array.isArray(pendingTrashPaths) ? pendingTrashPaths.slice() : []
    pendingTrashPaths = []
    if (confirm && paths.length > 0) operationController.trashSelection(paths)
    return paths.length
  }
  signal treeRowsReplacing()

  property alias treeModel: treeController.model
  readonly property alias treeExpansionError: treeController.expansionError
  property alias favoritesModel: favoritesController.model
  readonly property alias drivesController: drivesController
  property alias searchModel: searchModel
  property alias applicationModel: applicationController.model
  property alias selectedPath: selectionController.selectedPath
  property alias selectedPaths: selectionController.selectedPaths
  property alias selectedPathLookup: selectionController.selectedPathLookup
  property alias selectedEntries: selectionController.selectedEntries
  property alias selectionAnchorPath: selectionController.anchorPath
  property alias selectedMetadata: metadataController.selected
  property alias selectedMetadataFingerprint: metadataController.fingerprint
  property alias metadataBusy: metadataController.busy
  property alias metadataError: metadataController.error
  property alias statCancellationCount: metadataController.cancellationCount
  property alias metadataControllerApi: metadataController
  property alias selectionImportError: selectionController.importError
  readonly property int selectedCount: selectedPaths.length
  readonly property int selectedTotalSize: {
    var total = 0
    for (var i = 0; i < selectedEntries.length; i++) {
      var size = Number(selectedEntries[i].size)
      if (size >= 0 && !selectedEntries[i].isDir) total += size
    }
    return total
  }
  readonly property bool selectionHasDeleted: {
    for (var i = 0; i < selectedEntries.length; i++) if (selectedEntries[i].gitDeleted) return true
    return false
  }

  property alias clipboardPaths: operationController.clipboardPaths
  property alias clipboardMode: operationController.clipboardMode
  readonly property bool clipboardReady: clipboardPaths.length > 0
  property alias externalClipboardPaths: operationController.externalPaths
  property alias externalClipboardMode: operationController.externalMode
  property alias externalClipboardMime: operationController.externalMime
  property alias externalClipboardBusy: operationController.externalBusy
  property alias externalClipboardError: operationController.externalError
  property alias pendingExternalPasteDestination: operationController.pendingExternalDestination
  property alias pendingExternalPasteOperationId: operationController.pendingExternalOperationId
  property alias externalClipboardResponse: operationController.externalResponse
  property alias externalClipboardDiscardResult: operationController.discardExternalResult
  readonly property bool externalClipboardReady: externalClipboardPaths.length > 0
  readonly property bool pasteReady: clipboardReady || externalClipboardReady
  readonly property int pasteCount: clipboardReady ? clipboardPaths.length : externalClipboardPaths.length

  property alias actionMenuOpen: actionMenuController.open
  property alias actionMenuMode: actionMenuController.mode
  property alias actionMenuPath: actionMenuController.path
  property alias actionMenuEntry: actionMenuController.entry
  property alias actionMenuPaths: actionMenuController.paths
  property alias actionMenuEntries: actionMenuController.entries
  property alias actionInput: actionMenuController.input
  property alias actionMenuScreen: actionMenuController.screen
  property alias actionMenuX: actionMenuController.menuX
  property alias actionMenuY: actionMenuController.menuY
  property alias actionMenuOpenLeft: actionMenuController.openLeft
  property alias rememberApplicationDefault: actionMenuController.rememberDefault
  property alias applicationsMime: applicationController.mime
  property alias applicationsPath: applicationController.path
  property alias applicationsBusy: applicationController.busy
  property alias applicationsError: applicationController.error
  property alias applicationsLoaded: applicationController.loaded
  property alias activeApplicationsPath: applicationController.activePath
  property alias pendingApplicationsPath: applicationController.pendingPath
  property alias pendingApplicationsMime: applicationController.pendingMime
  property alias applicationLookupCount: applicationController.lookupCount
  property alias applicationCancellationCount: applicationController.cancellationCount
  property alias applicationCacheHitCount: applicationController.cacheHitCount
  property alias applicationMimeHintCount: applicationController.mimeHintCount

  property alias operationBusy: operationController.busy
  property alias operationLabel: operationController.label
  property alias operationError: operationController.error
  property alias operationNotice: operationController.notice
  property alias operationQueue: operationController.queue
  property alias activeOperation: operationController.active
  property alias activeOperationResponse: operationController.activeResponse
  readonly property var history: operationController
  property alias activeOperationProgress: operationController.progress
  property alias operationCancelRequested: operationController.cancelRequested
  property alias operationSerial: operationController.serial
  property alias operationResults: operationController.results
  readonly property int operationPendingCount: operationQueue.length + (operationBusy ? 1 : 0)
    + (pendingExternalPasteOperationId ? 1 : 0)
  readonly property string activeOperationId: operationController.activeId
  readonly property bool operationCancellable: operationBusy && activeOperation
    ? !!activeOperation.cancellable && !operationCancelRequested
    : false

  property alias recentModel: recentController.model
  property alias recentBusy: recentController.busy
  property alias recentError: recentController.error
  property alias trashModel: trashController.model
  property alias trashBusy: trashController.busy
  property alias trashOperationBusy: trashController.operationBusy
  property alias trashError: trashController.error
  property alias trashNotice: trashController.notice
  property alias trashOperationLabel: trashController.operationLabel
  property alias trashProgress: trashController.progress
  property alias trashCount: trashController.count
  property alias trashStores: trashController.stores
  property alias trashEstimatedSizeText: trashController.estimatedSizeText
  property alias trashUnknownSize: trashController.unknownSize
  property alias trashTruncated: trashController.truncated
  property alias trashSelectedId: trashController.selectedId
  property alias trashLastClearedText: trashController.lastClearedText
  property alias trashNextCleanupText: trashController.nextCleanupText

  property alias dropWheel: dropWheelController
  property alias dropWheelOpen: dropWheelController.wheelOpen

  property alias launchBusy: launchController.busy
  property alias launchStatus: launchController.status
  property alias launchError: launchController.error
  property alias lastLaunchedPath: launchController.lastPath
  property alias lastLaunchedAddress: launchController.lastAddress

  property alias pickerActive: pickerController.active
  property alias pickerRequestId: pickerController.requestId
  property alias pickerMode: pickerController.mode
  property alias pickerTitle: pickerController.title
  property alias pickerMultiple: pickerController.multiple
  property alias pickerExtensions: pickerController.extensions
  property alias pickerSuggestedName: pickerController.suggestedName
  property alias pickerFileName: pickerController.fileName
  property alias pickerSaveValidationBusy: pickerController.saveValidationBusy
  property alias pickerPendingSavePath: pickerController.pendingSavePath
  property alias pickerOverwriteArmed: pickerController.overwriteArmed
  property alias pickerOverwritePath: pickerController.overwritePath
  property alias pickerControllerApi: pickerController

  property alias searchQuery: searchController.query
  property alias searchBusy: searchController.busy
  property alias searchResultCount: searchController.resultCount
  property alias searchStagedRows: searchController.stagedRows
  property alias searchSpinner: searchController.spinnerGlyph
  property alias searchError: searchController.error
  property alias searchBackend: searchController.backend
  property alias searchFilterSummary: searchController.filterSummary
  property alias searchGitRepositoryCount: searchController.gitRepositoryCount
  property alias operationActor: operationController.actor
  property alias searchListActive: searchController.listActive
  property alias searchListTitle: searchController.listTitle
  property alias searchWalked: searchController.walked
  property alias searchIndexed: searchController.indexed
  property alias searchPartial: searchController.partial
  property alias searchTruncated: searchController.truncated
  property alias quickNavActive: searchController.quickNavActive
  property alias quickNavChannel: searchController.quickNavChannel
  property alias quickNavHome: searchController.quickNavHome
  property var channelProviders: ({})
  property alias activeSearchQuery: searchController.activeQuery
  property alias activeSearchMode: searchController.activeMode
  property alias lastSearchExitCode: searchController.lastExitCode
  property alias lastSearchPayloadCount: searchController.lastPayloadCount
  property alias lastSearchStderr: searchController.lastStderr
  property alias searchCancellationCount: searchController.cancellationCount
  property alias treeQueue: treeController.treeQueue
  property alias directoryBatchLimit: treeController.directoryBatchLimit
  property alias activeTreePaths: treeController.activeTreePaths
  property alias activeTreeResponse: treeController.activeTreeResponse
  property alias treeReadProcessCount: treeController.treeReadProcessCount
  property alias lastTreeBatchSize: treeController.lastTreeBatchSize
  property alias restoreExpandedPaths: treeController.restoreExpandedPaths
  property alias watcherRestartPending: watchController.watcherRestartPending
  property alias watcherRunning: watchController.watcherRunning
  property alias activeWatcherFingerprint: watchController.activeWatcherFingerprint
  property alias activeWatcherPathCount: watchController.activeWatcherPathCount
  property alias watcherStartCount: watchController.watcherStartCount
  property alias watcherRestartCount: watchController.watcherRestartCount
  property alias watcherRestartSkippedCount: watchController.watcherRestartSkippedCount
  property alias pendingWatchDirectories: watchController.pendingWatchDirectories
  property alias watchRefreshQueue: watchController.watchRefreshQueue
  property alias activeWatchRefreshPaths: watchController.activeWatchRefreshPaths
  property alias activeWatchRefreshResponse: watchController.activeWatchRefreshResponse
  property alias watchReadProcessCount: watchController.watchReadProcessCount
  property alias lastWatchBatchSize: watchController.lastWatchBatchSize
  property alias gitRepoDirectories: treeController.gitRepoDirectories
  property alias gitMetadataPathLimit: treeController.gitMetadataPathLimit
  property alias gitMetadataBusy: treeController.gitMetadataBusy
  property alias gitMetadataQueued: treeController.gitMetadataQueued
  property alias queuedGitMetadataReason: treeController.queuedGitMetadataReason
  property alias activeGitMetadataPaths: treeController.activeGitMetadataPaths
  property alias activeGitMetadataResponse: treeController.activeGitMetadataResponse
  property alias activeGitMetadataGeneration: treeController.activeGitMetadataGeneration
  property alias gitMetadataRefreshCount: treeController.gitMetadataRefreshCount
  property alias gitMetadataNoopCount: treeController.gitMetadataNoopCount
  property alias lastGitMetadataPathCount: treeController.lastGitMetadataPathCount
  property alias lastGitMetadataRepositoryCount: treeController.lastGitMetadataRepositoryCount
  property alias lastGitMetadataReason: treeController.lastGitMetadataReason
  property alias lastGitMetadataFingerprint: treeController.lastGitMetadataFingerprint
  property alias gitMetadataError: treeController.gitMetadataError
  property alias filesystemRefreshCount: watchController.filesystemRefreshCount
  property alias lastFilesystemEventPath: watchController.lastFilesystemEventPath
  property alias treeGeneration: treeController.treeGeneration
  property alias treeStructureRevision: treeController.treeStructureRevision
  property alias treeRowsRevision: treeController.treeRowsRevision
  readonly property alias treeLoading: treeController.treeLoading
  property alias treePathIndexRevision: treeController.treePathIndexRevision
  property alias treePathIndex: treeController.treePathIndex
  property alias locationValidationBusy: locationController.busy
  property alias locationValidationPath: locationController.path
  property alias locationValidationError: locationController.error
  property alias activeLocationPath: locationController.activePath
  property alias activeLocationMode: locationController.activeMode
  property alias activeLocationOrigin: locationController.activeOrigin
  property alias pendingLocationPath: locationController.pendingPath
  property alias pendingLocationMode: locationController.pendingMode
  property alias activeLocationTargetScreen: locationController.activeTargetScreen
  property alias pendingLocationTargetScreen: locationController.pendingTargetScreen
  property alias activeLocationResponse: locationController.activeResponse
  property alias locationValidationCount: locationController.validationCount
  property alias locationCancellationCount: locationController.cancellationCount
  property alias historyCurrentPruneCount: locationController.historyPruneCount
  property alias rootRecoveryOrigin: locationController.recoveryOrigin
  property alias rootRecoveryNotice: locationController.recoveryNotice
  property alias lastMissingRoot: locationController.lastMissingRoot
  property alias lastRecoveredRoot: locationController.lastRecoveredRoot
  property alias rootRecoveryCount: locationController.recoveryCount
  property alias themeFolderPalette: stateController.themePalette
  readonly property bool searching: !quickNavActive && (searchQuery.trim() !== "" || searchListActive)
  readonly property int gitStatusPollIntervalMs: {
    var configured = pluginConfig().gitStatusPollIntervalMs
    if (configured !== undefined && Number(configured) === 0) return 0
    return Math.round(numberValue(configured, 5000, 1000, 60000))
  }

  readonly property int selectedFolderCount: {
    var count = 0
    for (var i = 0; i < selectedEntries.length; i++) if (selectedEntries[i].isDir) count++
    return count
  }
  readonly property int colorableSelectionCount: {
    var count = 0
    for (var i = 0; i < selectedEntries.length; i++) if (!selectedEntries[i].gitDeleted) count++
    return count
  }
  readonly property var folderColorChoices: [
    { key: "default", label: "Default (theme)", value: "" },
    { key: "red", label: "Red", value: themedFolderColor("red", "#e06c75") },
    { key: "orange", label: "Orange", value: themedFolderColor("orange", "#d19a66") },
    { key: "yellow", label: "Yellow", value: themedFolderColor("yellow", "#e5c07b") },
    { key: "green", label: "Green", value: themedFolderColor("green", "#98c379") },
    { key: "cyan", label: "Cyan", value: themedFolderColor("cyan", "#56b6c2") },
    { key: "blue", label: "Blue", value: themedFolderColor("blue", "#61afef") },
    { key: "magenta", label: "Magenta", value: themedFolderColor("magenta", "#c678dd") },
    { key: "muted", label: "Muted", value: themedFolderColor("muted", "#abb2bf") }
  ]
  readonly property var priorityPropertyChoices: configController.priorityPropertyChoices
  readonly property var gitStatusDetailChoices: configController.gitStatusDetailChoices
  readonly property string selectionFolderColor: {
    var value = null
    for (var i = 0; i < selectedEntries.length; i++) {
      if (selectedEntries[i].gitDeleted) continue
      var current = folderColor(selectedEntries[i].path)
      if (value === null) value = current
      else if (value !== current) return "mixed"
    }
    return value === null ? "" : value
  }

  ListModel { id: searchModel }

  AgentsController {
    id: agentsController
    service: service
  }

  ProjectController {
    id: projectController
    service: service
  }

  UpdateController {
    id: updateController
    service: service
    host: bladeHost
  }

  SearchController {
    id: searchController
    service: service
    model: searchModel
    bladeHost: bladeHost
  }

  PickerController {
    id: pickerController
    service: service
  }

  DropWheelController {
    id: dropWheelController
    service: service
    modifierName: String(service.pluginConfig().dropModifier || "space").toLowerCase()
    systemDragOut: service.dragOut === "system"
  }

  LaunchController {
    id: launchController
    service: service
  }

  MetadataController {
    id: metadataController
    service: service
  }

  ApplicationController {
    id: applicationController
    service: service
  }

  OperationController {
    id: operationController
    service: service
  }

  TrashController {
    id: trashController
    service: service
  }

  StateController {
    id: stateController
    service: service
  }

  TreeController {
    id: treeController
    service: service
  }
  FavoritesController {
    id: favoritesController
    service: service
  }

  DrivesController {
    id: drivesController
    service: service
    showSystemVolumes: stateController.showSystemVolumes
  }

  RecentController {
    id: recentController
    service: service
  }

  WatchController {
    id: watchController
    service: service
  }

  LocationController {
    id: locationController
    service: service
  }

  SelectionController {
    id: selectionController
    service: service
  }

  PluginWatcherController {
    id: pluginWatcherController
    service: service
  }

  ActionMenuController {
    id: actionMenuController
    service: service
  }

  ActionController {
    id: actionController
    service: service
    catalogProviders: extensionCatalog.providers
  }

  ConfigController {
    id: configController
    service: service
  }

  NavigationController {
    id: navigationController
    service: service
  }

  function themedFolderColor(name, fallback) {
    return stateController.themedFolderColor(name, fallback)
  }

  function loadThemeFolderPalette(raw) {
    stateController.loadThemeFolderPalette(raw)
  }

  function pluginConfig() { return configController.pluginConfig() }
  function boolValue(value, fallback) { return configController.boolValue(value, fallback) }
  function numberValue(value, fallback, minimum, maximum) { return configController.numberValue(value, fallback, minimum, maximum) }
  function normalizePlacement(value) { return configController.normalizePlacement(value) }
  function normalizeModeBadge(value) { return configController.normalizeModeBadge(value) }
  function normalizeDragOut(value) { return configController.normalizeDragOut(value) }
  function normalizeMonitorMode(value) { return configController.normalizeMonitorMode(value) }
  function normalizePriorityProperty(value) { return configController.normalizePriorityProperty(value) }
  function priorityPropertyLabel(value) { return configController.priorityPropertyLabel(value, false) }
  function fileTypeLabel(name, isDir, isSymlink, kind, mime) { return configController.fileTypeLabel(name, isDir, isSymlink, kind, mime) }
  function priorityValue(name, isDir, isSymlink, sizeText, kind, mime, modified, created) { return configController.priorityValue(name, isDir, isSymlink, sizeText, kind, mime, modified, created) }
  function priorityValueFor(key, name, isDir, isSymlink, sizeText, kind, mime, modified, created) { return configController.priorityValueFor(key, name, isDir, isSymlink, sizeText, kind, mime, modified, created) }
  function normalizePriorityColumns(value) { return configController.normalizePriorityColumns(value) }

  function normalizeRoot(value) {
    return stateController.normalizeRoot(value)
  }

  function markUpdateChecked(timestamp) {
    return stateController.markUpdateChecked(timestamp)
  }

  function normalizeGitStatusDetails(value) { return configController.normalizeGitStatusDetails(value) }
  function gitStatusDetailsFromColumns(value) { return configController.gitStatusDetailsFromColumns(value) }

  function normalizedFolderColor(value) {
    return stateController.normalizedFolderColor(value)
  }

  function normalizedFolderColors(value) {
    return stateController.normalizedFolderColors(value)
  }

  function normalizedFavorites(value) {
    return stateController.normalizedFavorites(value)
  }

  function normalizedNavigationStack(value, currentRoot) {
    return stateController.normalizedNavigationStack(value, currentRoot)
  }

  function pathWithin(path, parent) {
    return stateController.pathWithin(path, parent)
  }

  function normalizedOperationMappings(response) {
    return stateController.normalizedOperationMappings(response)
  }

  function remappedOperationPath(path, mappings) {
    return stateController.remappedOperationPath(path, mappings)
  }

  function remappedPathList(values, mappings) {
    return stateController.remappedPathList(values, mappings)
  }

  function remapPersistentPathState(mappings) {
    return stateController.remapPersistentPathState(mappings)
  }

  function pathRemoved(path, removals) {
    return stateController.pathRemoved(path, removals)
  }

  function prunePersistentPathState(rawRemovals) {
    return stateController.prunePersistentPathState(rawRemovals)
  }

  function favoriteIndex(path) {
    return stateController.favoriteIndex(path)
  }

  function isFavorite(path) {
    return stateController.isFavorite(path)
  }

  function pinFavorite(path, name, isDir, isSymlink, mime, isGitRepo) {
    return stateController.pinFavorite(path, name, isDir, isSymlink, mime, isGitRepo)
  }

  function unpinFavorite(path) {
    return stateController.unpinFavorite(path)
  }

  function pinFavoritesDocument(text) {
    return stateController.pinFavoritesDocument(text)
  }

  function unpinFavoritesDocument(text) {
    return stateController.unpinFavoritesDocument(text)
  }

  function toggleFavorite(path, name, isDir, isSymlink, mime, isGitRepo) {
    return isFavorite(path)
      ? unpinFavorite(path)
      : pinFavorite(path, name, isDir, isSymlink, mime, isGitRepo)
  }

  function activateFavorite(entry, targetScreen) {
    if (!entry || !entry.path) return
    if (entry.isDir) navigateToLocation(entry.path, targetScreen, "favorite")
    else openDefault(entry.path, targetScreen, false)
  }

  function gitStatusColor(status) {
    return configController.gitStatusColor(status)
  }

  function folderColor(path) {
    return stateController.folderColor(path)
  }

  function folderColorLabel(value) {
    return stateController.folderColorLabel(value)
  }

  function setFolderColor(path, value) {
    return operationController.setFolderColor(path, value)
  }

  function setSelectionFolderColor(value, entries) {
    return operationController.setSelectionFolderColor(value, entries)
  }

  function applyFolderColorChanges(changes) {
    return stateController.applyFolderColorChanges(changes)
  }

  function defaults() {
    return stateController.defaults()
  }

  function resetSettings() {
    return stateController.resetSettings()
  }

  function applyState(raw) {
    stateController.applyState(raw)
  }

  function scheduleStateSave() {
    stateController.scheduleSave()
  }

  function setTrashRetentionDays(value, consent) {
    return preferencesController.setTrashRetentionDays(value, consent)
  }

  function markTrashCleared(value) {
    stateController.markTrashCleared(value)
  }

  function restartRootRecoveryNotice() {
    locationController.restartRecoveryNotice()
  }

  function stopRootRecoveryNotice() {
    locationController.stopRecoveryNotice()
  }

  function panelActiveFor(panelScreen) { return navigationController.panelActiveFor(panelScreen) }
  function setOpen(value) { navigationController.setOpen(value) }
  function toggleOpen() { navigationController.toggleOpen() }
  function setSidebarWidth(value, screenWidth, persist) { navigationController.setSidebarWidth(value, screenWidth, persist) }
  function setPropertiesBladeWidth(value, screenWidth, persist) { navigationController.setPropertiesBladeWidth(value, screenWidth, persist) }
  function setPropertiesPlacement(value) { navigationController.setPropertiesPlacement(value) }
  function setPriorityProperty(value) { return navigationController.setPriorityProperty(value) }
  function setPriorityColumns(value) { return navigationController.setPriorityColumns(value) }
  function setGitStatusDetails(value) { return navigationController.setGitStatusDetails(value) }
  function setGitSummaryFields(value) {
    gitSummaryFields = GitSummary.normalizeFields(value)
    scheduleStateSave()
    return gitSummaryFields
  }
  function setTreeOrder(sort, filter) { return navigationController.setTreeOrder(sort, filter) }
  function rerunSearch() { searchController.rerunSearch() }
  function loadMoreSearchRows() { searchController.loadMore() }
  function loadAllSearchRows() { searchController.loadAll() }
  function cyclePriorityProperty() { return navigationController.cyclePriorityProperty() }
  function setSettingsOpen(value, edge) { navigationController.setSettingsOpen(value, edge) }
  function toggleSettings(edge) { return navigationController.toggleSettings(edge) }
  function focusTree(targetScreen, later) { return later ? navigationController.focusAfterOpen(targetScreen) : navigationController.focusTree(targetScreen) }
  function focusSearch(targetScreen) { return navigationController.focusSearch(targetScreen) }
  function focusLocation(targetScreen) { return navigationController.focusLocation(targetScreen) }
  function focusProperties(targetScreen) { return navigationController.focusProperties(targetScreen) }
  function clearLocationValidationError() { locationController.clearError() }
  function navigateToLocation(path, targetScreen, mode) { return locationController.navigate(path, targetScreen, mode) }
  function cancelLocationValidation(clearError) { locationController.cancel(clearError) }
  function discardUnavailableHistoryDestination(mode, path, origin) { return locationController.discardUnavailableHistoryDestination(mode, path, origin) }
  function discardCurrentHistoryDestinations(mode) { return locationController.discardCurrentHistoryDestinations(mode) }
  function startQuickNav(targetScreen, channel) {
    searchController.startQuickNav(targetScreen, channel)
  }

  function setQuickNavChannel(channel) {
    searchController.setQuickNavChannel(channel)
  }

  function registerChannel(id, provider) {
    var key = String(id || "")
    if (!key || !provider || typeof provider.rows !== "function") return false
    var next = Object.assign({}, channelProviders)
    next[key] = provider
    channelProviders = next
    return true
  }

  function channelProvider(id) {
    return channelProviders[String(id || "")] || null
  }

  function channelForPrefix(prefix) {
    var ids = Object.keys(channelProviders)
    for (var index = 0; index < ids.length; index++)
      if (String(channelProviders[ids[index]].prefix || "") === String(prefix)) return ids[index]
    return ""
  }

  function activateChannelRow(id, index) {
    var provider = channelProvider(id)
    if (!provider) return false
    var rows = provider.rows(searchQuery.trim())
    var row = rows[index]
    if (!row) return false
    if (typeof row.activate === "function") row.activate()
    else if (typeof provider.activate === "function") provider.activate(row)
    return true
  }

  function quickActions() {
    return [
      { name: "Undo", hint: "u", activate: function() { history.undoOperation(false, false) } },
      { name: "Redo", hint: "Ctrl+R", activate: function() { history.redoOperation(false, false) } },
      { name: "Paste here", hint: "p", activate: function() { pasteInto("") } },
      { name: "New file", hint: "a", activate: function() { bladeHost.focusModule("files", null, "tree"); createEntry("untitled.md", false, "") } },
      { name: "New folder", hint: "Ctrl+Shift+N", activate: function() { createEntry("new-folder", true, "") } },
      { name: showHidden ? "Hide hidden files" : "Show hidden files", hint: ". / Shift+H", activate: function() { toggleHidden() } },
      { name: searchTreeLayout ? "Results as a list" : "Results as a tree", hint: "Ctrl+B", activate: function() { setSearchLayout(!searchTreeLayout) } },
      { name: "Refresh", hint: "Shift+R", activate: function() { refreshTree() } },
      { name: "Open Trash", hint: "trash:///", activate: function() { navigateToLocation(trashResource, null, "browse") } },
      { name: "Open recent files", hint: "recent:///", activate: function() { navigateToLocation(recentResource, null, "browse") } },
      { name: "Go home", hint: "Alt+Home", activate: function() { navigateToLocation(home, null, "browse") } },
      { name: "Clear search", hint: "Esc", activate: function() { stopList(); searchQuery = "" } }
    ]
  }

  Component.onCompleted: registerChannel("actions", {
    prefix: ">",
    title: "ACTIONS",
    note: "command palette",
    placeholder: "Action…",
    rows: function(query) {
      var words = String(query || "").toLowerCase().split(/\s+/).filter(function(word) { return word !== "" })
      return quickActions().filter(function(row) {
        var haystack = (row.name + " " + row.hint).toLowerCase()
        return words.every(function(word) {
          var position = 0
          for (var i = 0; i < word.length; i++) {
            position = haystack.indexOf(word[i], position)
            if (position < 0) return false
            position++
          }
          return true
        })
      })
    }
  })

  function stopQuickNav() {
    searchController.stopQuickNav()
  }

  function startList(file) {
    searchController.startList(file)
  }

  function stopList() {
    searchController.stopList()
  }

  function recordZoxideVisit(path) {
    searchController.recordVisit(path)
  }

  function refreshTrash() { return trashController.refresh() }
  function restoreTrashEntry(id, destination, recreateParent) { return trashController.restore(id, destination, recreateParent) }
  function trashDeletePermanently(id) { return trashController.deletePermanently(id) }
  function emptyTrash() { return trashController.empty() }
  function restartSearch() {
    searchController.restart()
  }

  function rememberSearch(query) {
    var text = String(query || "").trim()
    if (!text) return
    var next = [text].concat(searchHistory.filter(function(item) { return item !== text })).slice(0, 50)
    searchHistory = next
    scheduleStateSave()
  }

  function recallSearch(delta) {
    return searchController.recall(delta)
  }

  function setSearchDeep(deep) {
    if (searchDeep === !!deep) return false
    searchDeep = !!deep
    scheduleStateSave()
    searchController.restart()
    return true
  }
  function toggleSearchDeep() { return setSearchDeep(!searchDeep) }
  function searchWith(query, mode) {
    searchController.deepOverride = mode === "deep" || mode === "shallow" ? mode : ""
    quickNavActive = false
    searchQuery = String(query || "")
  }

  function setSearchLayout(tree) {
    if (searchTreeLayout === !!tree) return false
    searchTreeLayout = !!tree
    scheduleStateSave()
    searchController.restart()
    return true
  }

  function setSearchOptions(caseSensitive, regex) {
    var nextCase = !!caseSensitive
    var nextRegex = !!regex
    if (searchCaseSensitive === nextCase && searchRegex === nextRegex) return false
    searchCaseSensitive = nextCase
    searchRegex = nextRegex
    scheduleStateSave()
    searchController.restart()
    return true
  }

  function setRootPath(value, rememberHistory, preserveForward) { navigationController.setRootPath(value, rememberHistory, preserveForward) }
  function goUp() { return navigationController.goUp() }
  function goHome() { return navigationController.goHome() }
  function goBack(targetScreen) { return navigationController.goBack(targetScreen) }
  function goForward(targetScreen) { return navigationController.goForward(targetScreen) }
  function setShowHidden(value) { return navigationController.setShowHidden(value) }
  function toggleHidden() { return setShowHidden(!showHidden) }
  function setGitEnabled(value) {
    var desired = !!value
    if (gitEnabled === desired) return gitEnabled
    gitEnabled = desired
    scheduleStateSave()
    treeController.resetGitIntegration()
    return gitEnabled
  }
  function setProjectContext(value) { projectContext = !!value; scheduleStateSave(); return projectContext }
  function setPropertyIcons(value) {
    propertyIcons = !!value
    scheduleStateSave()
    return propertyIcons
  }
  function setFolderColorScope(value) {
    folderColorScope = stateController.normalizedFolderColorScope(value)
    scheduleStateSave()
    return folderColorScope
  }

  function setConfirmTrash(value) {
    confirmTrash = !!value
    scheduleStateSave()
    return confirmTrash
  }

  function setScrollMarks(value) {
    scrollMarks = !!value
    scheduleStateSave()
    return scrollMarks
  }

  function setAutoHideSearch(value) {
    autoHideSearch = !!value
    scheduleStateSave()
    return autoHideSearch
  }

  function setShowSystemVolumes(value) {
    showSystemVolumes = !!value
    scheduleStateSave()
    return showSystemVolumes
  }

  function setModeBadge(value) {
    modeBadge = normalizeModeBadge(value)
    scheduleStateSave()
    return modeBadge
  }

  function setDragOut(value) {
    dragOut = normalizeDragOut(value)
    scheduleStateSave()
    return dragOut
  }

  function makeRow(entry, depth) { return selectionController.makeRow(entry, depth) }
  function rootName(path) { return selectionController.rootName(path) }
  function entrySnapshot(row) { return selectionController.entrySnapshot(row) }
  function isSelected(path) { return selectionController.isSelected(path) }
  function humanSize(size) { return selectionController.humanSize(size) }
  function setPrimaryEntry(entry) { selectionController.setPrimaryEntry(entry) }
  function applySelection(entries, primaryEntry, anchorPath) { selectionController.applySelection(entries, primaryEntry, anchorPath) }
  function remapSelection(mappings) { selectionController.remapPaths(mappings) }
  function selectModelIndex(model, index, mode) { selectionController.selectModelIndex(model, index, mode) }
  function clearSelection() { selectionController.clearSelection() }
  function entryForKnownPath(path) { return selectionController.entryForKnownPath(path) }
  function decodedJsonDocument(text) { return selectionController.decodedJsonDocument(text) }
  function selectEntriesDocument(text) { return selectionController.selectEntriesDocument(text) }
  function selectionDocument() { return selectionController.selectionDocument() }
  function boundedDocumentLimit(value) { return selectionController.boundedDocumentLimit(value) }
  function modelEntryDocument(row, includeTreeState) { return selectionController.modelEntryDocument(row, includeTreeState) }
  function modelDocument(model, limit, includeTreeState) { return selectionController.modelDocument(model, limit, includeTreeState) }
  function treeDocument(limit) { return selectionController.treeDocument(limit) }
  function searchResultsDocument(limit) { return selectionController.searchResultsDocument(limit) }
  function parentDirectory(path) { return selectionController.parentDirectory(path) }
  function selectionDestination() { return selectionController.selectionDestination() }

  function expandedPaths() { return treeController.expandedPaths() }
  function listingOrderArguments(limit) { return treeController.listingOrderArguments(limit) }
  function windowLimitFor(paths) { return treeController.windowLimitFor(paths) }
  function loadMoreChildren(path) { return treeController.loadMoreChildren(path) }
  function resetTree(preserveExpansion, pathMappings, removedPaths) { treeController.resetTree(preserveExpansion, pathMappings, removedPaths) }
  function markTreeStructureChanged() { treeController.markTreeStructureChanged() }
  function rebuildTreePathIndex() { treeController.rebuildTreePathIndex() }
  function indexOfTreePath(path) { return treeController.indexOfTreePath(path) }
  function removeDescendants(parentIndex) { treeController.removeDescendants(parentIndex) }
  function enqueueChildren(path) { treeController.enqueueChildren(path) }
  function startNextTreeRequest() { treeController.startNextTreeRequest() }
  function registerGitRepository(repoRoot, gitDir) { treeController.registerGitRepository(repoRoot, gitDir) }
  function clearModelGitMetadata(model, rowIndex) { treeController.clearModelGitMetadata(model, rowIndex) }
  function clearDirectoryGitMetadata(parentIndex) { treeController.clearDirectoryGitMetadata(parentIndex) }
  function applyModelGitMetadata(model, rowIndex, path, response, registerRepository) { treeController.applyModelGitMetadata(model, rowIndex, path, response, registerRepository) }
  function applyDirectoryGitMetadata(parentIndex, path, response, registerRepository) { treeController.applyDirectoryGitMetadata(parentIndex, path, response, registerRepository) }
  function applySearchGitMetadata(rowIndex, path, response) { treeController.applySearchGitMetadata(rowIndex, path, response) }
  function directoryResponseForPath(payload, path, exitCode) { return treeController.directoryResponseForPath(payload, path, exitCode) }
  function navigationStackWithoutMissingRoot(values, missingRoot, currentRoot) { return treeController.navigationStackWithoutMissingRoot(values, missingRoot, currentRoot) }
  function recoverMissingRoot(path) { return treeController.recoverMissingRoot(path) }
  function completeMissingRootRecovery() { treeController.completeMissingRootRecovery() }
  function abortMissingRootRecovery() { treeController.abortMissingRootRecovery() }
  function applyTreeResponse(path, response) { treeController.applyTreeResponse(path, response) }
  function finishTreeRequest(exitCode) { treeController.finishTreeRequest(exitCode) }
  function toggleDirectory(index) { treeController.toggleDirectory(index) }
  function setDirectoryExpanded(path, expanded) { return treeController.setDirectoryExpanded(path, expanded) }
  function setBranchExpanded(path, expanded) { return treeController.setBranchExpanded(path, expanded) }
  function refreshTree(pathMappings, removedPaths) { treeController.refreshTree(pathMappings, removedPaths) }
  function refreshRecent() { recentController.refresh() }
  function recordFrecencyVisit(arguments) { recentController.recordVisit(arguments) }
  function treeRowSnapshot(row) { return treeController.treeRowSnapshot(row) }
  function refreshSelectionFromVisibleModels() { treeController.refreshSelectionFromVisibleModels() }
  function reconcileDirectory(path, response) { treeController.reconcileDirectory(path, response) }
  function visibleGitMetadataPaths() { return treeController.visibleGitMetadataPaths() }
  function scheduleGitMetadataRefresh(reason, repoRoot) { treeController.scheduleGitMetadataRefresh(reason, repoRoot) }
  function requestVisibleGitMetadataRefresh(reason) { return treeController.requestVisibleGitMetadataRefresh(reason) }
  function finishGitMetadataRefresh(exitCode) { treeController.finishGitMetadataRefresh(exitCode) }
  function receiveFilesystemEvent(data) { watchController.receiveFilesystemEvent(data) }
  function flushFilesystemEvents() { watchController.flushFilesystemEvents() }
  function startNextWatchRefresh() { watchController.startNextWatchRefresh() }
  function watchedDirectories() { return watchController.watchedDirectories() }
  function watcherFingerprint(paths) { return watchController.watcherFingerprint(paths) }
  function scheduleWatcherRestart() { watchController.scheduleWatcherRestart() }
  function startWatcher() { watchController.startWatcher() }

  function selectPath(path, isDir, name, kind, sizeText, mime, size) {
    selectionController.selectPath(path, isDir, name, kind, sizeText, mime, size)
  }

  function requestStat(path) {
    metadataController.request(path)
  }

  function fileUrl(path) {
    return PathText.fileUrl(path)
  }

  function enqueueLaunch(path, mode, desktopId, targetScreen, directoryHint) {
    return launchController.enqueue(path, mode, desktopId, 0, targetScreen, directoryHint)
  }

  function yieldFocusForExternalLaunch() {
    closeActionMenu()
    if (dropWheelOpen) dropWheelController.keyboardFocusReleased = true
    navigationController.yieldFocus()
    searchController.yieldFocus()
    return bladeHost.yieldFocus()
  }

  function openInEditor(path) { return enqueueLaunch(path, "editor", "") }

  function preferredScreen() { return bladeHost.preferredScreen() }
  function referenceScreen(candidate) { return bladeHost.referenceScreen(candidate) }
  function defaultOpenScreen(targetScreen) { return targetScreen || (actionMenuOpen && actionMenuScreen ? actionMenuScreen : bladeHost.referenceScreen(null)) }

  function openDefault(path, targetScreen, directoryHint) {
    return enqueueLaunch(path, "default", "", defaultOpenScreen(targetScreen), directoryHint)
  }

  function openAtLine(path, line) {
    return launchController.enqueue(path, "editor", "", line)
  }

  function openWithApplication(desktopId, path) {
    var target = String(path || selectedPath)
    var desktop = String(desktopId || "")
    if (!target || !desktop) return
    if (rememberApplicationDefault && applicationsMime) {
      applicationsLoaded = false
      enqueueOperation("Set default application", backendCommand("set-default").concat([
        "--mime", applicationsMime,
        "--desktop-id", desktop
      ]), false, false, false)
    }
    enqueueLaunch(target, "application", desktop)
    closeActionMenu()
  }

  function revealInFileManager(path, isDir, targetScreen) {
    var target = String(path || selectedPath)
    if (!target) return
    if (isDir) return openDefault(target, targetScreen, true)
    return enqueueLaunch(target, "reveal", "")
  }

  function actionMenuVisibleFor(targetScreen) { return actionMenuController.visibleFor(targetScreen) }
  function openActionMenu(mode, targetScreen, x, y, entries, placement) { actionMenuController.show(mode, targetScreen, x, y, entries, placement) }
  function closeActionMenu() { actionMenuController.close() }

  function cancelApplicationLookup() {
    applicationController.cancel()
  }

  function loadApplications(path) {
    applicationController.load(path)
  }

  function selectionUris(paths) {
    return operationController.selectionUris(paths)
  }

  function copySelection(cut, paths) {
    return operationController.copySelection(cut, paths)
  }

  function extractArchives(paths) {
    return operationController.extractArchives(paths)
  }

  function copyPaths(paths) {
    return operationController.copyPaths(paths)
  }

  function clearFileClipboard() {
    return operationController.clearClipboard()
  }

  function operationCommand(kind, sources, destination) {
    return operationController.command(kind, sources, destination)
  }

  function enqueuePaste(paths, mode, destination, external, requestId) {
    return operationController.enqueuePaste(paths, mode, destination, external, requestId)
  }

  function pasteInto(destination) {
    return operationController.pasteInto(destination)
  }

  function requestExternalClipboard(destination, pasteAfterProbe) {
    operationController.requestExternalClipboard(destination, pasteAfterProbe)
  }

  function moveSelectionTo(destination, copyInstead, paths) {
    return operationController.moveSelectionTo(destination, copyInstead, paths)
  }

  function renameSelection(name, path) {
    return operationController.renameSelection(name, path)
  }

  function createEntry(name, directory, parent) {
    return operationController.createEntry(name, directory, parent)
  }

  function trashSelection(paths) {
    return operationController.trashSelection(paths)
  }

  function requestTrash(paths) {
    return operationController.requestTrash(paths)
  }

  function enqueueOperation(label, command, clearClipboardAfter, refreshAfter, clearSelectionAfter, clearExternalAfter, requestId) {
    return operationController.enqueue(label, command, clearClipboardAfter, refreshAfter, clearSelectionAfter, clearExternalAfter, requestId)
  }

  function operationResult(requestId) {
    return operationController.result(requestId)
  }

  function cancelOperation(requestId) {
    return operationController.cancel(requestId)
  }

  function pickerAllowsEntry(path, isDir, mime) {
    return pickerController.allowsEntry(path, isDir, mime)
  }

  function beginPicker(optionsText) {
    return pickerController.begin(optionsText)
  }

  function clearPickerOverwriteConfirmation() {
    pickerController.clearOverwriteConfirmation()
  }

  function confirmPicker() {
    return pickerController.confirm()
  }

  function cancelPicker() {
    pickerController.cancel()
  }

  function activatePickerEntry(path, isDir, name) {
    return pickerController.activateEntry(path, isDir, name)
  }

  function pickerResult(requestId) {
    return pickerController.result(requestId)
  }

  FileTreeIpc {
    id: fileTreeIpc
    service: service
    bladeHost: bladeHost
    watchController: watchController
  }

  BladeScreens {
    host: bladeHost
  }
}
