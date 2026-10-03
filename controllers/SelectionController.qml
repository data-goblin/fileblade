import QtQuick
import "../lib/PathText.js" as PathText

Item {
  id: controller

  required property var service
  property string selectedPath: ""
  property var selectedPaths: []
  property var selectedPathLookup: ({})
  property var selectedEntries: []
  property string anchorPath: ""
  property string importError: ""
  signal chosen()
  readonly property int selectedCount: selectedPaths.length
  readonly property string rootPath: service.rootPath
  readonly property var treeModel: service.treeModel
  readonly property var searchModel: service.searchModel
  readonly property bool pickerActive: service.pickerActive
  readonly property string pickerMode: service.pickerMode
  readonly property bool pickerSaveValidationBusy: service.pickerSaveValidationBusy
  readonly property bool pickerMultiple: service.pickerMultiple
  readonly property int selectedFolderCount: service.selectedFolderCount
  readonly property string selectionFolderColor: service.selectionFolderColor
  readonly property bool open: service.open
  readonly property string searchQuery: service.searchQuery
  readonly property bool quickNavActive: service.quickNavActive
  readonly property bool searchBusy: service.searchBusy
  readonly property string searchBackend: service.searchBackend
  readonly property string searchFilterSummary: service.searchFilterSummary
  readonly property int searchGitRepositoryCount: service.searchGitRepositoryCount
  readonly property string searchError: service.searchError

  function normalizeRoot(path) { return service.normalizeRoot(path) }
  function indexOfTreePath(path) { return service.indexOfTreePath(path) }
  function pickerAllowsEntry(path, isDir, mime) { return service.pickerAllowsEntry(path, isDir, mime) }
  function clearPickerOverwriteConfirmation() { service.clearPickerOverwriteConfirmation() }
  function requestStat(path) { service.requestStat(path) }
  function folderColor(path) { return service.folderColor(path) }
  function isFavorite(path) { return service.isFavorite(path) }

  function objectText(value, name, fallback) {
    var text = String(value && value[name] || "")
    return text || String(fallback || "")
  }

  function firstText(values) {
    for (var i = 0; i < values.length; i++) {
      var text = String(values[i] || "")
      if (text) return text
    }
    return ""
  }

  function finiteNumber(value, fallback) {
    var number = Number(value)
    return isFinite(number) ? number : fallback
  }

  function makeRow(entry, depth) {
    var git = service.gitEnabled
    return {
      name: objectText(entry, "name", ""),
      path: objectText(entry, "path", ""),
      isDir: !!entry.is_dir,
      isSymlink: !!entry.is_symlink,
      isGitRepo: git && !!entry.is_git_repo,
      gitDeleted: git && !!entry.is_deleted,
      gitIgnored: git && !!entry.git_ignored,
      gitRepoRoot: git ? objectText(entry, "git_repo_root", "") : "",
      gitStatus: git ? objectText(entry, "git_status", "") : "",
      gitStatusLabel: git ? objectText(entry, "git_status_label", "") : "",
      gitIndexStatus: git ? objectText(entry, "git_index_status", "") : "",
      gitWorktreeStatus: git ? objectText(entry, "git_worktree_status", "") : "",
      gitOriginalPath: git ? objectText(entry, "git_original_path", "") : "",
      gitModifiedCount: git ? Math.max(0, finiteNumber(entry.git_modified_count, 0)) : 0,
      gitDeletedCount: git ? Math.max(0, finiteNumber(entry.git_deleted_count, 0)) : 0,
      gitNewCount: git ? Math.max(0, finiteNumber(entry.git_new_count, 0)) : 0,
      gitUntrackedCount: git ? Math.max(0, finiteNumber(entry.git_untracked_count, 0)) : 0,
      gitSummary: git && entry.git_summary ? JSON.stringify(entry.git_summary) : "",
      gitRepoName: git ? objectText(entry, "git_repo_name", "") : "",
      gitBranch: git ? objectText(entry, "git_branch", "") : "",
      gitWorktree: git ? objectText(entry, "git_worktree", "") : "",
      size: Number(entry.size === undefined ? -1 : entry.size),
      sizeText: objectText(entry, "size_text", "—"),
      modified: objectText(entry, "modified", ""),
      created: objectText(entry, "created", ""),
      statFingerprint: objectText(entry, "stat_fingerprint", ""),
      kind: objectText(entry, "kind", entry.is_dir ? "Directory" : "File"),
      mime: objectText(entry, "mime", entry.is_dir ? "inode/directory" : "application/octet-stream"),
      relative: objectText(entry, "relative", ""),
      nameSpans: objectText(entry, "name_spans", ""),
      relativeSpans: objectText(entry, "relative_spans", ""),
      depth: Math.max(0, finiteNumber(depth, 0)),
      expanded: false,
      loaded: false,
      loading: false,
      error: "",
      windowLoaded: 0,
      windowTotal: 0
    }
  }

  function rootName(path) {
    if (String(path) === service.home) return "home"
    if (String(path) === service.trashResource) return "Trash"
    if (String(path) === service.recentResource) return "Recent"
    if (String(path) === service.drivesResource) return "Drives"
    return PathText.name(path)
  }

  function entrySnapshot(row) {
    return {
      path: objectText(row, "path", ""),
      name: objectText(row, "name", rootName(row.path)),
      isDir: !!row.isDir,
      isSymlink: !!row.isSymlink,
      isGitRepo: !!row.isGitRepo,
      gitDeleted: !!row.gitDeleted,
      gitIgnored: !!row.gitIgnored,
      gitRepoRoot: objectText(row, "gitRepoRoot", ""),
      gitStatus: objectText(row, "gitStatus", ""),
      gitStatusLabel: objectText(row, "gitStatusLabel", ""),
      gitIndexStatus: objectText(row, "gitIndexStatus", ""),
      gitWorktreeStatus: objectText(row, "gitWorktreeStatus", ""),
      gitOriginalPath: objectText(row, "gitOriginalPath", ""),
      gitModifiedCount: Math.max(0, finiteNumber(row.gitModifiedCount, 0)),
      gitDeletedCount: Math.max(0, finiteNumber(row.gitDeletedCount, 0)),
      gitNewCount: Math.max(0, finiteNumber(row.gitNewCount, 0)),
      gitUntrackedCount: Math.max(0, finiteNumber(row.gitUntrackedCount, 0)),
      gitSummary: objectText(row, "gitSummary", ""),
      gitRepoName: objectText(row, "gitRepoName", ""),
      gitBranch: objectText(row, "gitBranch", ""),
      gitWorktree: objectText(row, "gitWorktree", ""),
      size: Number(row.size === undefined ? -1 : row.size),
      sizeText: objectText(row, "sizeText", "—"),
      modified: objectText(row, "modified", ""),
      created: objectText(row, "created", ""),
      statFingerprint: objectText(row, "statFingerprint", ""),
      kind: objectText(row, "kind", row.isDir ? "Directory" : "File"),
      mime: objectText(row, "mime", row.isDir ? "inode/directory" : "application/octet-stream")
    }
  }

  function isSelected(path) {
    return !!selectedPathLookup[String(path || "")]
  }

  function humanSize(size) {
    var value = Number(size)
    if (!isFinite(value) || value < 0) return "—"
    var units = ["B", "KB", "MB", "GB", "TB", "PB"]
    var index = 0
    while (value >= 1024 && index < units.length - 1) {
      value /= 1024
      index++
    }
    return index === 0 ? Math.round(value) + " " + units[index] : value.toFixed(1) + " " + units[index]
  }

  function currentMetadata(path) {
    var current = service.selectedMetadata
    return current && objectText(current, "path", "") === path ? current : null
  }

  function selectedMetadataDocument(entry, current, fingerprint) {
    var snapshot = entrySnapshot(entry)
    return {
      path: snapshot.path,
      name: snapshot.name,
      is_dir: snapshot.isDir,
      is_symlink: snapshot.isSymlink,
      kind: snapshot.kind,
      mime: firstText([objectText(current, "mime", ""), snapshot.mime]),
      size: snapshot.size,
      size_text: snapshot.sizeText,
      modified: firstText([snapshot.modified, objectText(current, "modified", "")]),
      created: firstText([snapshot.created, objectText(current, "created", "")]),
      stat_fingerprint: fingerprint,
      is_deleted: snapshot.gitDeleted,
      git_ignored: snapshot.gitIgnored,
      git_repo_root: snapshot.gitRepoRoot,
      git_status: snapshot.gitStatus,
      git_status_label: snapshot.gitStatusLabel,
      git_index_status: snapshot.gitIndexStatus,
      git_worktree_status: snapshot.gitWorktreeStatus,
      git_original_path: snapshot.gitOriginalPath,
      git_modified_count: snapshot.gitModifiedCount,
      git_deleted_count: snapshot.gitDeletedCount,
      git_new_count: snapshot.gitNewCount,
      git_untracked_count: snapshot.gitUntrackedCount,
      git_summary: snapshot.gitSummary ? JSON.parse(snapshot.gitSummary) : null,
      git_repo_name: snapshot.gitRepoName,
      git_branch: snapshot.gitBranch,
      git_worktree: snapshot.gitWorktree
    }
  }

  function setPrimaryEntry(entry) {
    if (!entry || !entry.path) {
      selectedPath = ""
      service.metadataControllerApi.reset()
      return
    }
    var path = String(entry.path)
    var current = currentMetadata(path)
    var fingerprint = firstText([entry.statFingerprint, objectText(current, "stat_fingerprint", "")])
    var unchanged = !!current && fingerprint === service.selectedMetadataFingerprint
    selectedPath = path
    service.selectedMetadataFingerprint = fingerprint
    service.selectedMetadata = Object.assign({}, current || ({}), selectedMetadataDocument(entry, current, fingerprint))
    service.metadataError = ""
    if (entry.gitDeleted) {
      service.metadataBusy = false
      return
    }
    var lookupPending = service.metadataControllerApi.lookupPending(path)
    if (unchanged && (service.selectedMetadata.permissions !== undefined || lookupPending)) return
    requestStat(selectedPath)
  }

  function dedupedSelection(entries) {
    var unique = []
    var paths = []
    var lookup = ({})
    for (var i = 0; i < entries.length; i++) {
      var entry = entrySnapshot(entries[i])
      if (!entry.path || lookup[entry.path]) continue
      if (pickerActive && !pickerAllowsEntry(entry.path, entry.isDir, entry.mime)) continue
      lookup[entry.path] = paths.length + 1
      paths.push(entry.path)
      unique.push(entry)
      if (pickerActive && !pickerMultiple) break
    }
    return { entries: unique, paths: paths, lookup: lookup }
  }

  function applySelection(entries, primaryEntry, anchorPath) {
    if (pickerActive && pickerMode === "save") {
      clearPickerOverwriteConfirmation()
      if (!pickerSaveValidationBusy) service.operationError = ""
    }
    var selected = dedupedSelection(entries)
    selectedEntries = selected.entries
    selectedPaths = selected.paths
    selectedPathLookup = selected.lookup
    controller.anchorPath = String(anchorPath || (primaryEntry ? primaryEntry.path : ""))
    var primary = primaryEntry && !!selected.lookup[String(primaryEntry.path || "")]
      ? entrySnapshot(primaryEntry)
      : (selected.entries.length > 0 ? selected.entries[selected.entries.length - 1] : null)
    setPrimaryEntry(primary)
  }

  function toggleSelectedEntry(current) {
    var toggled = selectedEntries.slice()
    var existing = Number(selectedPathLookup[current.path] || 0) - 1
    if (existing >= 0) toggled.splice(existing, 1)
    else toggled.push(current)
    applySelection(toggled, existing >= 0 ? null : current, current.path)
  }

  function modelAnchorIndex(model, fallback) {
    for (var i = 0; i < model.count; i++)
      if (String(model.get(i).path) === controller.anchorPath) return i
    return fallback
  }

  function selectModelRange(model, index, current, additive) {
    var anchorIndex = modelAnchorIndex(model, index)
    var ranged = additive ? selectedEntries.slice() : []
    var start = Math.min(anchorIndex, index)
    var end = Math.max(anchorIndex, index)
    for (var rowIndex = start; rowIndex <= end; rowIndex++) ranged.push(entrySnapshot(model.get(rowIndex)))
    applySelection(ranged, current, controller.anchorPath || current.path)
  }

  function selectModelIndex(model, index, mode) {
    if (!model || index < 0 || index >= model.count) return
    var current = entrySnapshot(model.get(index))
    if (pickerActive && !pickerAllowsEntry(current.path, current.isDir, current.mime)) return
    if (pickerActive && pickerMode === "save" && !current.isDir) service.pickerFileName = current.name
    var selectionMode = String(mode || "replace")
    if (pickerActive && !pickerMultiple) selectionMode = "replace"

    if (selectionMode === "toggle") toggleSelectedEntry(current)
    else if (selectionMode === "range" || selectionMode === "add-range") selectModelRange(model, index, current, selectionMode === "add-range")
    else applySelection([current], current, current.path)
    chosen()
  }

  function clearSelection() {
    applySelection([], null, "")
  }

  function remapPaths(mappings) {
    var primaryPath = service.remappedOperationPath(selectedPath, mappings)
    var changed = false
    var primary = null
    var entries = selectedEntries.map(function(entry) {
      var path = service.remappedOperationPath(entry.path, mappings)
      var next = entry
      if (path !== entry.path) {
        changed = true
        next = Object.assign({}, entry, { path: path, name: rootName(path), statFingerprint: "" })
      }
      if (path === primaryPath) primary = next
      return next
    })
    if (changed) applySelection(entries, primary, service.remappedOperationPath(anchorPath, mappings))
  }

  function entryForKnownPath(path) {
    var target = normalizeRoot(path)
    var treeIndex = indexOfTreePath(target)
    if (treeIndex >= 0) return entrySnapshot(treeModel.get(treeIndex))
    for (var rowIndex = 0; rowIndex < searchModel.count; rowIndex++)
      if (String(searchModel.get(rowIndex).path) === target) return entrySnapshot(searchModel.get(rowIndex))
    var favoritesModel = service.favoritesModel
    for (var favoriteIndex = 0; favoriteIndex < favoritesModel.count; favoriteIndex++)
      if (String(favoritesModel.get(favoriteIndex).path) === target) return entrySnapshot(favoritesModel.get(favoriteIndex))
    var recentModel = service.recentModel
    for (var recentIndex = 0; recentIndex < recentModel.count; recentIndex++)
      if (String(recentModel.get(recentIndex).path) === target) return entrySnapshot(recentModel.get(recentIndex))
    return null
  }

  function decodedJsonDocument(text) {
    var source = String(text || "")
    if (source.indexOf("base64:") === 0) {
      try { source = Qt.atob(source.slice(7)) }
      catch (decodeError) { return { ok: false, error: "base64: " + decodeError, value: null } }
    }
    try { return { ok: true, error: "", value: JSON.parse(source) } }
    catch (parseError) { return { ok: false, error: "json: " + parseError, value: null } }
  }

  function importedValue(item, camelName, snakeName) {
    return item[camelName] === undefined ? item[snakeName] : item[camelName]
  }

  function importedEntry(item) {
    if (!item || typeof item !== "object" || !item.path) return null
    var isDir = !!importedValue(item, "isDir", "is_dir")
    var size = importedValue(item, "size", "size")
    return {
      path: normalizeRoot(item.path),
      name: objectText(item, "name", rootName(item.path)),
      isDir: isDir,
      isSymlink: !!importedValue(item, "isSymlink", "is_symlink"),
      size: Number(size === undefined ? -1 : size),
      sizeText: firstText([item.sizeText, item.size_text, "—"]),
      modified: objectText(item, "modified", ""),
      created: objectText(item, "created", ""),
      statFingerprint: firstText([item.statFingerprint, item.stat_fingerprint]),
      kind: objectText(item, "kind", isDir ? "Directory" : "File"),
      mime: objectText(item, "mime", isDir ? "inode/directory" : "application/octet-stream")
    }
  }

  function selectEntriesDocument(text) {
    importError = ""
    var decoded = decodedJsonDocument(text)
    if (!decoded.ok) {
      importError = decoded.error
      return false
    }
    var raw = decoded.value
    if (!Array.isArray(raw) || raw.length === 0) {
      importError = "payload is not a non-empty array"
      return false
    }
    var entries = []
    for (var i = 0; i < raw.length; i++) {
      var entry = importedEntry(raw[i])
      if (entry) entries.push(entry)
    }
    if (entries.length === 0) {
      importError = "payload contains no path entries"
      return false
    }
    applySelection(entries, entries[entries.length - 1], entries[0].path)
    chosen()
    return true
  }

  function selectionDocument() {
    var entries = []
    for (var i = 0; i < selectedEntries.length; i++) {
      var entry = entrySnapshot(selectedEntries[i])
      entry.folderColor = folderColor(entry.path)
      entries.push(entry)
    }
    return {
      count: selectedCount,
      paths: selectedPaths,
      primaryPath: selectedPath,
      primary: service.selectedMetadata,
      entries: entries,
      folderCount: selectedFolderCount,
      folderColor: selectionFolderColor,
      metadataBusy: service.metadataBusy,
      metadataError: service.metadataError
    }
  }

  function boundedDocumentLimit(value) {
    var numeric = Math.floor(Number(value))
    if (!isFinite(numeric) || numeric <= 0) numeric = 500
    return Math.max(1, Math.min(2000, numeric))
  }

  function modelEntryDocument(row, includeTreeState) {
    var entry = entrySnapshot(row)
    entry.modified = String(row.modified || "")
    entry.created = String(row.created || "")
    entry.relative = String(row.relative || "")
    entry.relativeSpans = String(row.relativeSpans || "")
    entry.selected = isSelected(entry.path)
    entry.favorite = isFavorite(entry.path)
    entry.folderColor = entry.isDir ? folderColor(entry.path) : ""
    if (includeTreeState) {
      entry.depth = Math.max(0, Number(row.depth) || 0)
      entry.expanded = !!row.expanded
      entry.loaded = !!row.loaded
      entry.loading = !!row.loading
      entry.error = String(row.error || "")
    }
    return entry
  }

  readonly property int documentByteBudget: 64000

  function modelDocument(model, limit, includeTreeState) {
    var maximum = boundedDocumentLimit(limit)
    var count = model ? model.count : 0
    var entries = []
    var bytes = 0
    for (var i = 0; i < count && entries.length < maximum; i++) {
      if (String(model.get(i).kind) === "More") continue
      var entry = modelEntryDocument(model.get(i), includeTreeState)
      bytes += JSON.stringify(entry).length + 1
      if (bytes > documentByteBudget && entries.length > 0) break
      entries.push(entry)
    }
    return {
      rootPath: rootPath,
      count: count,
      returned: entries.length,
      truncated: entries.length < count,
      entries: entries
    }
  }

  function treeDocument(limit) {
    var document = modelDocument(treeModel, limit, true)
    document.open = open
    return document
  }

  function searchResultsDocument(limit) {
    var document = modelDocument(searchModel, limit, false)
    var staged = service.searchStagedRows
    if (Array.isArray(staged) && staged.length > document.count) {
      var maximum = boundedDocumentLimit(limit)
      var entries = []
      var bytes = 0
      for (var i = 0; i < staged.length && i < maximum; i++) {
        var entry = modelEntryDocument(staged[i], false)
        bytes += JSON.stringify(entry).length + 1
        if (bytes > documentByteBudget && entries.length > 0) break
        entries.push(entry)
      }
      document.count = staged.length
      document.returned = entries.length
      document.truncated = entries.length < staged.length
      document.entries = entries
    }
    document.query = searchQuery
    document.quickNav = quickNavActive
    document.busy = searchBusy
    document.backend = searchBackend
    document.filters = searchFilterSummary
    document.gitRepositories = searchGitRepositoryCount
    document.error = searchError
    return document
  }

  function parentDirectory(path) {
    var target = String(path || "")
    if (target === service.trashResource) return service.trashResource
    if (target === service.recentResource) return service.recentResource
    if (target === service.drivesResource) return service.drivesResource
    return PathText.parent(target)
  }

  function selectionDestination() {
    if (selectedEntries.length === 1 && selectedEntries[0].isDir) return selectedEntries[0].path
    if (selectedPath) return parentDirectory(selectedPath)
    return rootPath
  }

  function selectPath(path, isDir, name, kind, sizeText, mime, size) {
    var entry = {
      path: String(path || ""),
      name: String(name || rootName(path)),
      isDir: !!isDir,
      isSymlink: false,
      size: Number(size === undefined ? -1 : size),
      sizeText: String(sizeText || "—"),
      kind: String(kind || (isDir ? "Directory" : "File")),
      mime: String(mime || (isDir ? "inode/directory" : "application/octet-stream"))
    }
    applySelection([entry], entry, entry.path)
    chosen()
  }

}
