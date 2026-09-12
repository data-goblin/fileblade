import QtQuick
import "../lib/PathText.js" as PathText
import "../lib/TreeOrder.js" as TreeOrder
import "../lib/StateDocument.js" as StateDocument
import "../lib/GitSummary.js" as GitSummary
import "../lib/FolderPalette.js" as FolderPalette
import Quickshell
import Quickshell.Io
import qs.Commons

Item {
  id: controller

  required property var service
  property bool ready: false
  property bool stateWritable: true
  property bool stateRereadPending: false
  property var themePalette: ({})
  property var userPalette: ({})
  property string userPaletteError: ""
  property string userPaletteRequestId: ""
  property int userPaletteGeneration: 0
  readonly property string userPalettePath: service.bladeHost.configDir + "/colors.json"
  property bool stateReadQueued: false
  property bool themeReadQueued: false
  property string stateReadRequestId: ""
  property int stateReadGeneration: 0
  property string themeReadRequestId: ""
  property int themeReadGeneration: 0
  property string stateWriteRequestId: ""
  property string queuedStateDocument: ""
  property int stateWriteGeneration: 0
  property alias showHidden: persisted.showHidden
  property alias welcomeState: persisted.welcomeState
  property alias searchCaseSensitive: persisted.searchCaseSensitive
  property alias searchRegex: persisted.searchRegex
  property alias searchTreeLayout: persisted.searchTreeLayout
  property alias searchDeep: persisted.searchDeep
  property alias searchHistory: persisted.searchHistory
  property alias rootPath: persisted.rootPath
  property alias rootBackStack: persisted.rootBackStack
  property alias rootForwardStack: persisted.rootForwardStack
  property alias folderColors: persisted.folderColors
  property alias priorityProperty: persisted.priorityProperty
  property alias priorityColumns: persisted.priorityColumns
  property alias gitEnabled: persisted.gitEnabled
  property alias projectContext: persisted.projectContext
  property alias gitStatusDetails: persisted.gitStatusDetails
  property alias gitSummaryFields: persisted.gitSummaryFields
  property alias propertyIcons: persisted.propertyIcons
  property alias confirmTrash: persisted.confirmTrash
  property alias scrollMarks: persisted.scrollMarks
  property alias autoHideSearch: persisted.autoHideSearch
  property alias showSystemVolumes: persisted.showSystemVolumes
  property alias modeBadge: persisted.modeBadge
  property alias dragOut: persisted.dragOut
  property alias folderColorScope: persisted.folderColorScope
  property alias treeSort: persisted.treeSort
  property alias treeFilter: persisted.treeFilter
  property alias favorites: persisted.favorites
  property alias trashLastClearedAt: persisted.trashLastClearedAt
  property alias updateCheckedAt: persisted.updateCheckedAt
  readonly property var favoriteLookup: {
    var lookup = ({})
    for (var i = 0; i < favorites.length; i++) lookup[String(favorites[i].path)] = true
    return lookup
  }

  PersistentProperties {
    id: persisted
    reloadableId: "kurt-filetree-layout"
    property bool hydrated: false
    property bool showHidden: true
    property string welcomeState: ""
    property bool searchCaseSensitive: false
    property bool searchRegex: false
    property bool searchTreeLayout: false
    property bool searchDeep: false
    property var searchHistory: []
    property string rootPath: service.home
    property var rootBackStack: []
    property var rootForwardStack: []
    property var folderColors: ({})
    property string priorityProperty: "modified"
    property var priorityColumns: ["modified"]
    property bool gitEnabled: true
    property bool projectContext: false
    property var gitStatusDetails: []
    property var gitSummaryFields: GitSummary.normalizeFields()
    property bool propertyIcons: true
    property bool confirmTrash: true
    property bool scrollMarks: true
    property bool autoHideSearch: false
    property bool showSystemVolumes: false
    property string modeBadge: "header"
    property string dragOut: "paste"
    property string folderColorScope: "icon"
    property var treeSort: []
    property var treeFilter: ({})
    property var favorites: []
    property double trashLastClearedAt: 0
    property double updateCheckedAt: 0
  }

  function themedFolderColor(name, fallback) {
    var custom = userPalette && userPalette[name]
    if (typeof custom === "string" && custom) return custom
    var value = themePalette && themePalette[name]
    var base = typeof value === "string" && /^#[0-9a-fA-F]{6}$/.test(value) ? value : fallback
    return FolderPalette.distinct(name, base)
  }

  function requestUserPaletteRead() {
    userPaletteFile.reload()
    if (userPaletteRequestId) service.cancelBackendRequest(userPaletteRequestId, userPaletteGeneration, true)
    var current = ++userPaletteGeneration
    userPaletteRequestId = service.backendRequest("read-text", ["--path", userPalettePath, "--limit", "16384"], current, function(response) {
      if (current !== controller.userPaletteGeneration) return
      controller.userPaletteRequestId = ""
      var missing = response && response.ok === false && /\(os error 2\)$/.test(String(response.error || ""))
      try {
        if (!missing && (!response || !response.ok)) throw new Error(String(response && response.error || "Unable to read colors.json"))
        var next = missing ? ({}) : FolderPalette.parseUser(response.text)
        if (JSON.stringify(next) !== JSON.stringify(controller.userPalette)) controller.userPalette = next
        controller.userPaletteError = ""
      } catch (failure) {
        controller.userPaletteError = String(failure).slice(0, 300)
        console.warn("FileBlade colors.json: " + controller.userPaletteError + "; keeping previous colours")
      }
    })
  }

  function loadThemeFolderPalette(raw) {
    var palette = ({})
    var lines = String(raw || "").split("\n")
    for (var i = 0; i < lines.length; i++) {
      var match = lines[i].match(/^\s*([A-Za-z0-9_-]+)\s*=\s*["']?(#[0-9A-Fa-f]{6})/)
      if (match) palette[match[1]] = match[2]
    }
    themePalette = palette
  }

  function normalizeRoot(value) {
    var path = PathText.pathText(value)
    if (!path) return service.home
    if (path === service.trashResource || path === "trash://") return service.trashResource
    if (path === service.recentResource || path === "recent://") return service.recentResource
    if (path === service.drivesResource || path === "drives://") return service.drivesResource
    return PathText.normalize(path, service.home)
  }

  function normalizedFolderColor(value) {
    var color = String(value === undefined || value === null ? "" : value).trim().toLowerCase()
    if (!color || color === "default" || color === "none" || color === "clear") return ""
    for (var i = 0; i < service.folderColorChoices.length; i++) {
      var choice = service.folderColorChoices[i]
      if (color === choice.key || color === String(choice.label).toLowerCase()) return choice.value
    }
    return /^#[0-9a-f]{6}$/.test(color) ? color : null
  }

  function normalizedFolderColorScope(value) {
    var scope = String(value === undefined || value === null ? "" : value).trim().toLowerCase()
    return scope === "name" || scope === "row" ? scope : "icon"
  }

  function normalizedFolderColors(value) {
    var result = ({})
    if (!value || typeof value !== "object" || Array.isArray(value)) return result
    var paths = Object.keys(value)
    for (var i = 0; i < paths.length; i++) {
      var path = normalizeRoot(paths[i])
      var color = normalizedFolderColor(value[paths[i]])
      if (path && color) result[path] = color
    }
    return result
  }

  function normalizedFavorites(value) {
    var result = []
    var seen = ({})
    if (!Array.isArray(value)) return result
    for (var i = 0; i < value.length; i++) appendNormalizedFavorite(result, seen, value[i])
    return result
  }

  function favoriteValue(item, camelName, snakeName, fallback) {
    if (!item || typeof item !== "object") return fallback
    var value = item[camelName] === undefined ? item[snakeName] : item[camelName]
    return value === undefined || value === null ? fallback : value
  }

  function appendNormalizedFavorite(result, seen, item) {
    if (!item) return
    var rawPath = PathText.pathText(typeof item === "string" ? item : favoriteValue(item, "path", "path", ""))
    if (!rawPath) return
    var path = normalizeRoot(rawPath)
    if (!path || path === service.trashResource || path === service.recentResource || path === service.drivesResource || seen[path]) return
    seen[path] = true
    var isDir = !!favoriteValue(item, "isDir", "is_dir", false)
    result.push({
      path: path,
      name: String(favoriteValue(item, "name", "name", service.rootName(path))),
      isDir: isDir,
      isSymlink: !!favoriteValue(item, "isSymlink", "is_symlink", false),
      isGitRepo: !!favoriteValue(item, "isGitRepo", "is_git_repo", false),
      mime: String(favoriteValue(item, "mime", "mime", isDir ? "inode/directory" : "application/octet-stream"))
    })
  }

  function normalizedNavigationStack(value, currentRoot) {
    var result = []
    if (!Array.isArray(value)) return result
    for (var i = 0; i < value.length; i++) {
      var raw = PathText.pathText(value[i])
      if (!raw) continue
      var path = normalizeRoot(raw)
      if (result.length === 0 || result[result.length - 1] !== path) result.push(path)
    }
    if (result.length > 50) result = result.slice(result.length - 50)
    var current = normalizeRoot(currentRoot)
    if (result.length > 0 && result[result.length - 1] === current) result.pop()
    return result
  }

  function pathWithin(path, parent) {
    var target = normalizeRoot(path)
    var root = normalizeRoot(parent)
    var virtual = [service.trashResource, service.recentResource, service.drivesResource]
    if (virtual.indexOf(target) >= 0 || virtual.indexOf(root) >= 0) return target === root
    return PathText.within(target, root)
  }

  function normalizedOperationMappings(response) {
    var operation = String(response && response.operation || "")
    if (["move", "rename"].indexOf(operation) < 0) return []
    var raw = response && Array.isArray(response.mappings) ? response.mappings : []
    var mappings = []
    var seen = ({})
    for (var i = 0; i < raw.length; i++) appendMapping(mappings, seen, raw[i])
    mappings.sort(function(left, right) { return right.source.length - left.source.length })
    return mappings
  }

  function appendMapping(mappings, seen, raw) {
    var rawSource = String(raw && raw.source || "")
    var rawDestination = String(raw && raw.destination || "")
    if (!rawSource || !rawDestination) return
    var source = normalizeRoot(rawSource)
    var destination = normalizeRoot(rawDestination)
    if (!source || !destination || source === destination || seen[source]) return
    seen[source] = true
    mappings.push({ source: source, destination: destination })
  }

  function remappedOperationPath(path, mappings) {
    var raw = String(path || "")
    if (!raw) return ""
    var target = normalizeRoot(raw)
    for (var i = 0; i < mappings.length; i++) {
      var source = mappings[i].source
      if (!pathWithin(target, source)) continue
      return normalizeRoot(PathText.remap(target, source, mappings[i].destination))
    }
    return target
  }

  function remappedPathList(values, mappings) {
    var result = []
    if (!Array.isArray(values)) return result
    for (var i = 0; i < values.length; i++) {
      var mapped = remappedOperationPath(values[i], mappings)
      if (mapped && result.indexOf(mapped) < 0) result.push(mapped)
    }
    return result
  }

  function remapPersistentPathState(mappings) {
    if (!Array.isArray(mappings) || mappings.length === 0) return false
    var previousRoot = rootPath
    remapNavigationState(mappings)
    remapFavorites(mappings)
    remapFolderColors(mappings)
    if (rootPath !== previousRoot) service.recordZoxideVisit(rootPath)
    scheduleSave()
    return true
  }

  function remapNavigationState(mappings) {
    rootPath = remappedOperationPath(rootPath, mappings)
    rootBackStack = normalizedNavigationStack(remappedPathList(rootBackStack, mappings), rootPath)
    rootForwardStack = normalizedNavigationStack(remappedPathList(rootForwardStack, mappings), rootPath)
    service.clipboardPaths = remappedPathList(service.clipboardPaths, mappings)
    service.restoreExpandedPaths = remappedPathList(service.restoreExpandedPaths, mappings)
  }

  function remapFavorites(mappings) {
    var remapped = []
    for (var i = 0; i < favorites.length; i++) {
      var favorite = favorites[i]
      var path = remappedOperationPath(favorite.path, mappings)
      remapped.push({
        path: path,
        name: service.rootName(path),
        isDir: !!favorite.isDir,
        isSymlink: !!favorite.isSymlink,
        isGitRepo: !!favorite.isGitRepo,
        mime: String(favorite.mime || (favorite.isDir ? "inode/directory" : "application/octet-stream"))
      })
    }
    favorites = normalizedFavorites(remapped)
  }

  function remapFolderColors(mappings) {
    var stable = ({})
    var moved = ({})
    var paths = Object.keys(folderColors)
    for (var i = 0; i < paths.length; i++) {
      var oldPath = paths[i]
      var newPath = remappedOperationPath(oldPath, mappings)
      if (newPath === normalizeRoot(oldPath)) stable[newPath] = folderColors[oldPath]
      else moved[newPath] = folderColors[oldPath]
    }
    var movedPaths = Object.keys(moved)
    for (var j = 0; j < movedPaths.length; j++) stable[movedPaths[j]] = moved[movedPaths[j]]
    folderColors = stable
  }

  function pathRemoved(path, removals) {
    for (var i = 0; i < removals.length; i++) if (pathWithin(path, removals[i])) return true
    return false
  }

  function normalizedRemovals(rawRemovals) {
    var removals = []
    for (var i = 0; i < rawRemovals.length; i++) {
      var removal = normalizeRoot(rawRemovals[i])
      if (removal && removals.indexOf(removal) < 0) removals.push(removal)
    }
    return removals
  }

  function prunePersistentPathState(rawRemovals) {
    var removals = normalizedRemovals(rawRemovals)
    if (removals.length === 0) return false
    rootPath = rootAfterRemoval(removals)
    rootBackStack = normalizedNavigationStack(keptPaths(rootBackStack, removals), rootPath)
    rootForwardStack = normalizedNavigationStack(keptPaths(rootForwardStack, removals), rootPath)
    favorites = keptFavorites(removals)
    folderColors = keptFolderColors(removals)
    service.clipboardPaths = keptPaths(service.clipboardPaths, removals)
    service.restoreExpandedPaths = keptPaths(service.restoreExpandedPaths, removals)
    scheduleSave()
    return true
  }

  function rootAfterRemoval(removals) {
    var containing = []
    for (var i = 0; i < removals.length; i++) if (pathWithin(rootPath, removals[i])) containing.push(removals[i])
    if (containing.length === 0) return rootPath
    containing.sort(function(left, right) { return left.length - right.length })
    var next = service.parentDirectory(containing[0])
    while (next !== "/" && pathRemoved(next, removals)) next = service.parentDirectory(next)
    return next
  }

  function keptPaths(values, removals) {
    var kept = []
    for (var i = 0; i < values.length; i++) if (!pathRemoved(values[i], removals)) kept.push(values[i])
    return kept
  }

  function keptFavorites(removals) {
    var kept = []
    for (var i = 0; i < favorites.length; i++) if (!pathRemoved(favorites[i].path, removals)) kept.push(favorites[i])
    return normalizedFavorites(kept)
  }

  function keptFolderColors(removals) {
    var kept = ({})
    var paths = Object.keys(folderColors)
    for (var i = 0; i < paths.length; i++) if (!pathRemoved(paths[i], removals)) kept[paths[i]] = folderColors[paths[i]]
    return kept
  }

  function favoriteIndex(path) {
    var rawPath = PathText.pathText(path)
    if (!rawPath) return -1
    var target = normalizeRoot(rawPath)
    for (var i = 0; i < favorites.length; i++) if (String(favorites[i].path) === target) return i
    return -1
  }

  function isFavorite(path) {
    var rawPath = PathText.pathText(path)
    return rawPath ? !!favoriteLookup[normalizeRoot(rawPath)] : false
  }

  function pinFavorite(path, name, isDir, isSymlink, mime, isGitRepo) {
    var rawPath = PathText.pathText(path)
    if (!rawPath) return false
    var target = normalizeRoot(rawPath)
    if (favoriteIndex(target) >= 0) return false
    var next = favorites.slice()
    next.push({
      path: target,
      name: String(name || service.rootName(target)),
      isDir: !!isDir,
      isSymlink: !!isSymlink,
      isGitRepo: !!isGitRepo,
      mime: String(mime || (isDir ? "inode/directory" : "application/octet-stream"))
    })
    favorites = next
    scheduleSave()
    return true
  }

  function unpinFavorite(path) {
    if (!PathText.pathText(path)) return false
    var index = favoriteIndex(path)
    if (index < 0) return false
    var next = favorites.slice()
    next.splice(index, 1)
    favorites = next
    scheduleSave()
    return true
  }

  function pinFavoritesDocument(text) {
    var decoded = service.decodedJsonDocument(text)
    if (!decoded.ok || !Array.isArray(decoded.value))
      return { ok: false, error: decoded.error || "payload is not an array" }
    var incoming = normalizedFavorites(decoded.value)
    if (incoming.length === 0) return { ok: false, error: "payload contains no valid favorites" }
    var next = favorites.slice()
    var seen = ({})
    for (var i = 0; i < next.length; i++) seen[String(next[i].path)] = true
    var added = appendNewFavorites(next, seen, incoming)
    if (added > 0) {
      favorites = next
      scheduleSave()
    }
    return { ok: true, requested: incoming.length, added: added, unchanged: incoming.length - added, total: favorites.length }
  }

  function appendNewFavorites(target, seen, incoming) {
    var added = 0
    for (var i = 0; i < incoming.length; i++) {
      var favorite = incoming[i]
      if (seen[favorite.path]) continue
      seen[favorite.path] = true
      target.push(favorite)
      added++
    }
    return added
  }

  function unpinFavoritesDocument(text) {
    var decoded = service.decodedJsonDocument(text)
    if (!decoded.ok || !Array.isArray(decoded.value))
      return { ok: false, error: decoded.error || "payload is not an array" }
    var selection = favoriteTargets(decoded.value)
    if (selection.requested === 0) return { ok: false, error: "payload contains no valid paths" }
    var next = []
    var removed = 0
    for (var i = 0; i < favorites.length; i++) {
      if (selection.targets[String(favorites[i].path)]) removed++
      else next.push(favorites[i])
    }
    if (removed > 0) {
      favorites = next
      scheduleSave()
    }
    return {
      ok: true,
      requested: selection.requested,
      removed: removed,
      unchanged: selection.requested - removed,
      total: favorites.length
    }
  }

  function favoriteTargets(values) {
    var targets = ({})
    var requested = 0
    for (var i = 0; i < values.length; i++) {
      var raw = values[i]
      var rawPath = PathText.pathText(raw && typeof raw === "object" ? (raw.path || "") : (raw || ""))
      if (!rawPath) continue
      var path = normalizeRoot(rawPath)
      if (targets[path]) continue
      targets[path] = true
      requested++
    }
    return { targets: targets, requested: requested }
  }

  function folderColor(path) {
    return String(folderColors[normalizeRoot(path)] || "")
  }

  function folderColorLabel(value) {
    var color = normalizedFolderColor(value)
    if (color === null) return "Invalid"
    for (var i = 0; i < service.folderColorChoices.length; i++)
      if (service.folderColorChoices[i].value === color) return service.folderColorChoices[i].label
    return color || "Default (theme)"
  }

  function applyFolderColorChanges(changes) {
    if (!Array.isArray(changes) || changes.length === 0) return false
    var next = Object.assign({}, folderColors)
    var changed = false
    for (var i = 0; i < changes.length; i++) {
      var rawPath = PathText.pathText(changes[i] && changes[i].path || "")
      var color = normalizedFolderColor(changes[i] && changes[i].value)
      if (!rawPath || color === null) continue
      var target = normalizeRoot(rawPath)
      var previous = String(next[target] || "")
      if (previous === color) continue
      if (color) next[target] = color
      else delete next[target]
      changed = true
    }
    if (!changed) return false
    folderColors = next
    scheduleSave()
    return true
  }

  function setFolderColor(path, value) {
    var target = normalizeRoot(path)
    var color = normalizedFolderColor(value)
    if (!target || color === null) return false
    return applyFolderColorChanges([{ path: target, value: color }])
  }

  function setSelectionFolderColor(value, entries) {
    var color = normalizedFolderColor(value)
    var targets = Array.isArray(entries) ? entries : service.selectedEntries
    if (color === null || targets.length === 0) return false
    var changes = []
    for (var i = 0; i < targets.length; i++) {
      if (targets[i].gitDeleted) continue
      var target = normalizeRoot(targets[i].path)
      changes.push({ path: target, value: color })
    }
    return applyFolderColorChanges(changes)
  }

  function defaults() {
    var config = service.pluginConfig()
    return {
      open: service.boolValue(config.startOpen, false),
      sidebarWidth: service.numberValue(config.sidebarWidth, 380, 280, 1600),
      propertiesBladeWidth: service.numberValue(config.propertiesBladeWidth, 360, 280, 1200),
      propertiesPlacement: service.normalizePlacement(config.propertiesPlacement),
      propertiesVerticalFraction: service.numberValue(config.propertiesVerticalFraction, 0.34, 0.18, 0.72),
      showHidden: service.boolValue(config.showHidden, true),
      rootPath: normalizeRoot(config.rootPath || service.home),
      priorityProperty: service.normalizePriorityProperty(config.priorityProperty),
      gitEnabled: service.boolValue(config.gitEnabled, true),
      gitSummaryFields: GitSummary.normalizeFields(config.gitSummaryFields),
      projectContext: service.boolValue(config.projectContext, false),
      propertyIcons: service.boolValue(config.propertyIcons, true),
      confirmTrash: service.boolValue(config.confirmTrash, true),
      scrollMarks: service.boolValue(config.scrollMarks, true),
      autoHideSearch: service.boolValue(config.autoHideSearch, false),
      showSystemVolumes: service.boolValue(config.showSystemVolumes, false),
      modeBadge: service.normalizeModeBadge(config.modeBadge),
      dragOut: service.normalizeDragOut(config.dragOut),
      folderColorScope: normalizedFolderColorScope(config.folderColorScope),
      trashRetentionDays: 0
    }
  }

  function resetSettings() {
    var base = defaults()
    showHidden = base.showHidden
    searchCaseSensitive = false
    searchRegex = false
    searchTreeLayout = false
    searchDeep = false
    priorityColumns = service.normalizePriorityColumns([base.priorityProperty])
    priorityProperty = priorityColumns.length > 0 ? priorityColumns[0] : "none"
    gitEnabled = base.gitEnabled
    gitSummaryFields = base.gitSummaryFields
    projectContext = base.projectContext
    gitStatusDetails = service.gitStatusDetailsFromColumns(priorityColumns)
    propertyIcons = base.propertyIcons
    confirmTrash = base.confirmTrash
    scrollMarks = base.scrollMarks
    autoHideSearch = base.autoHideSearch
    showSystemVolumes = base.showSystemVolumes
    modeBadge = base.modeBadge
    dragOut = base.dragOut
    folderColorScope = base.folderColorScope
    treeSort = TreeOrder.normalizeSorts([])
    treeFilter = TreeOrder.normalizeFilter({})
    service.resetTree()
    scheduleSave()
    return true
  }

  function normalizedTrashRetentionDays(value) {
    var days = Number(value)
    if (!isFinite(days)) return 0
    return Math.max(0, Math.min(3650, Math.round(days)))
  }


  function normalizedTrashTimestamp(value) {
    var timestamp = Number(value)
    return isFinite(timestamp) && timestamp > 0 ? Math.floor(timestamp) : 0
  }

  function markTrashCleared(value) {
    trashLastClearedAt = normalizedTrashTimestamp(value || Date.now())
    scheduleSave()
  }

  function applyState(raw) {
    if (persisted.hydrated) {
      finishHydration({})
      return
    }
    var base = defaults()
    var state = parseState(raw)
    showHidden = service.boolValue(state.showHidden, base.showHidden)
    welcomeState = typeof state.welcomeState === "string" && ["dismissed", "installed"].indexOf(state.welcomeState) >= 0 ? state.welcomeState : ""
    searchCaseSensitive = service.boolValue(state.searchCaseSensitive, false)
    searchRegex = service.boolValue(state.searchRegex, false)
    searchTreeLayout = service.boolValue(state.searchTreeLayout, false)
    searchDeep = service.boolValue(state.searchDeep, false)
    searchHistory = Array.isArray(state.searchHistory) ? state.searchHistory.filter(function(item) { return typeof item === "string" }).slice(0, 50) : []
    rootPath = normalizeRoot(state.rootPath || base.rootPath)
    rootBackStack = normalizedNavigationStack(state.rootBackStack, rootPath)
    rootForwardStack = normalizedNavigationStack(state.rootForwardStack, rootPath)
    folderColors = normalizedFolderColors(state.folderColors)
    priorityColumns = Array.isArray(state.priorityColumns)
      ? service.normalizePriorityColumns(state.priorityColumns)
      : service.normalizePriorityColumns([state.priorityProperty || base.priorityProperty])
    priorityProperty = priorityColumns.length > 0 ? priorityColumns[0] : "none"
    gitEnabled = service.boolValue(state.gitEnabled, base.gitEnabled)
    gitSummaryFields = GitSummary.normalizeFields(state.gitSummaryFields === undefined ? base.gitSummaryFields : state.gitSummaryFields)
    projectContext = service.boolValue(state.projectContext, base.projectContext)
    gitStatusDetails = state.gitStatusDetails === undefined
      ? service.gitStatusDetailsFromColumns(state.priorityColumns)
      : service.normalizeGitStatusDetails(state.gitStatusDetails)
    propertyIcons = service.boolValue(state.propertyIcons, base.propertyIcons)
    confirmTrash = service.boolValue(state.confirmTrash, base.confirmTrash)
    scrollMarks = service.boolValue(state.scrollMarks, base.scrollMarks)
    autoHideSearch = service.boolValue(state.autoHideSearch, base.autoHideSearch)
    showSystemVolumes = service.boolValue(state.showSystemVolumes, base.showSystemVolumes)
    modeBadge = state.modeBadge === undefined ? base.modeBadge : service.normalizeModeBadge(state.modeBadge)
    dragOut = state.dragOut === undefined ? base.dragOut : service.normalizeDragOut(state.dragOut)
    folderColorScope = state.folderColorScope === undefined ? base.folderColorScope : normalizedFolderColorScope(state.folderColorScope)
    treeSort = TreeOrder.normalizeSorts(state.treeSort)
    treeFilter = TreeOrder.normalizeFilter(state.treeFilter)
    favorites = normalizedFavorites(state.favorites)
    trashLastClearedAt = normalizedTrashTimestamp(state.trashLastClearedAt)
    updateCheckedAt = normalizedTrashTimestamp(state.updateCheckedAt)
    persisted.hydrated = true
    finishHydration({
      open: service.boolValue(state.open, base.open),
      sidebarWidth: service.numberValue(state.sidebarWidth, base.sidebarWidth, 280, 1600),
      propertiesBladeWidth: service.numberValue(state.propertiesBladeWidth, base.propertiesBladeWidth, 280, 1600),
      propertiesPlacement: service.normalizePlacement(state.propertiesPlacement || base.propertiesPlacement),
      propertiesVerticalFraction: service.numberValue(state.propertiesVerticalFraction, base.propertiesVerticalFraction, 0.18, 0.72)
    })
  }

  function parseState(raw) {
    var state = null
    try { state = raw && String(raw).trim() ? JSON.parse(String(raw)) : null }
    catch (e) { state = null }
    return state && typeof state === "object" ? state : ({})
  }

  function finishHydration(bladeState) {
    ready = true
    service.resetTree()
    service.scheduleWatcherRestart()
    service.bladeHost.bootstrap(bladeState)
  }

  function document() {
    return {
      version: 12,
      showHidden: showHidden,
      welcomeState: welcomeState,
      searchCaseSensitive: searchCaseSensitive,
      searchRegex: searchRegex,
      searchTreeLayout: searchTreeLayout,
      searchDeep: searchDeep,
      searchHistory: searchHistory,
      rootPath: rootPath,
      rootBackStack: Array.isArray(rootBackStack) ? rootBackStack : [],
      rootForwardStack: Array.isArray(rootForwardStack) ? rootForwardStack : [],
      folderColors: folderColors,
      priorityProperty: priorityProperty,
      priorityColumns: priorityColumns,
      gitEnabled: gitEnabled,
      gitSummaryFields: gitSummaryFields,
      projectContext: projectContext,
      gitStatusDetails: gitStatusDetails,
      propertyIcons: propertyIcons,
      confirmTrash: confirmTrash,
      scrollMarks: scrollMarks,
      autoHideSearch: autoHideSearch,
      showSystemVolumes: showSystemVolumes,
      modeBadge: modeBadge,
      dragOut: dragOut,
      folderColorScope: folderColorScope,
      treeSort: treeSort,
      treeFilter: treeFilter,
      favorites: favorites,
      trashLastClearedAt: trashLastClearedAt,
      updateCheckedAt: updateCheckedAt
    }
  }

  function markUpdateChecked(timestamp) {
    updateCheckedAt = normalizedTrashTimestamp(timestamp)
    scheduleSave()
  }

  function scheduleSave() {
    if (ready && stateWritable) saveTimer.restart()
  }

  function setWelcomeState(value) {
    var next = String(value || "")
    if (["", "dismissed", "installed"].indexOf(next) < 0 || next === welcomeState) return welcomeState
    welcomeState = next
    save()
    return welcomeState
  }

  function save() {
    if (!stateWritable) return
    writeState(JSON.stringify(document(), null, 2) + "\n")
  }

  function writeState(text) {
    if (stateWriteRequestId) {
      queuedStateDocument = text
      return
    }
    stateWriteGeneration++
    var requestGeneration = stateWriteGeneration
    stateWriteRequestId = service.backendRequest("state-write", ["--document", text], requestGeneration, function(response) {
      if (requestGeneration !== controller.stateWriteGeneration) return
      controller.stateWriteRequestId = ""
      if (response && response.ok) stateFileSignal.reload()
      var queued = controller.queuedStateDocument
      controller.queuedStateDocument = ""
      if (queued) Qt.callLater(function() { controller.writeState(queued) })
    })
  }

  function requestStateRead() {
    if (stateReadRequestId) {
      stateReadQueued = true
      return
    }
    stateReadQueued = false
    stateReadGeneration++
    var requestGeneration = stateReadGeneration
    stateReadRequestId = service.backendRequest("state-read", [], requestGeneration, function(response) {
      if (requestGeneration !== controller.stateReadGeneration) return
      controller.stateReadRequestId = ""
      controller.receiveStateRead(response)
      if (controller.stateReadQueued) Qt.callLater(controller.requestStateRead)
    })
  }

  function receiveStateRead(response) {
    if (ready && !stateRereadPending) return
    var plan = StateDocument.hydrationPlan(response)
    if (plan.warning) console.warn("data-goblin.fileblade: " + plan.warning)
    stateWritable = plan.writable
    stateRereadPending = !plan.apply && plan.retry
    if (plan.apply) {
      if (ready) persisted.hydrated = false
      applyState(plan.text)
      return
    }
    if (!ready) applyState("")
  }

  function requestThemeRead() {
    if (themeReadRequestId) {
      themeReadQueued = true
      return
    }
    themeReadQueued = false
    themeReadGeneration++
    var requestGeneration = themeReadGeneration
    themeReadRequestId = service.backendRequest("read-text", ["--path", Color.currentThemePath + "/colors.toml", "--limit", "262144"], requestGeneration, function(response) {
      if (requestGeneration !== controller.themeReadGeneration) return
      controller.themeReadRequestId = ""
      controller.loadThemeFolderPalette(response && response.ok ? response.text : "")
      if (controller.themeReadQueued) Qt.callLater(controller.requestThemeRead)
    })
  }

  FileView {
    id: stateFileSignal
    preload: false
    path: service.statePath
    watchChanges: true
    atomicWrites: true
    printErrors: false
    onFileChanged: { reload(); if (!controller.ready || controller.stateRereadPending) controller.requestStateRead() }
  }

  FileView {
    id: userPaletteFile
    path: controller.userPalettePath
    preload: false
    watchChanges: true
    printErrors: false
    onFileChanged: userPaletteDebounce.restart()
  }
  Timer { id: userPaletteDebounce; interval: 150; onTriggered: controller.requestUserPaletteRead() }

  FileView {
    id: themeFolderPaletteFile
    preload: false
    path: Color.currentThemePath + "/colors.toml"
    watchChanges: true
    printErrors: false
    onPathChanged: controller.requestThemeRead()
    onFileChanged: { reload(); controller.requestThemeRead() }
  }

  Connections {
    target: controller.service
    function onBackendReadyChanged() {
      if (controller.service.backendReady && controller.stateRereadPending) controller.requestStateRead()
    }
  }

  Timer {
    id: saveTimer
    interval: 160
    onTriggered: controller.save()
  }

  Component.onCompleted: {
    requestStateRead()
    requestThemeRead()
    requestUserPaletteRead()
  }
}
