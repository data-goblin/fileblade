import QtQuick
import "../lib/PathText.js" as PathText
import "../lib/TreeOrder.js" as TreeOrder
import "../lib/Highlight.js" as Highlight
import "../lib/SearchQuery.js" as SearchQuery

Item {
  id: root

  required property var service
  required property var model
  required property var bladeHost

  visible: false

  property string query: ""
  property bool busy: false
  property string error: ""
  property string backend: "nucleo"
  property string filterSummary: ""
  property int gitRepositoryCount: 0
  property int walked: 0
  property int indexed: 0
  property bool partial: false
  property bool truncated: false
  property bool sessionFresh: true
  property string listFile: ""
  property string listTitle: ""
  readonly property bool listActive: listFile !== ""
  property int historyIndex: -1
  property string deepOverride: ""
  readonly property bool queryNeedsBackend: /(^|\s)(content|grep|c|scope):/i.test(query)
  readonly property bool deep: listActive || queryNeedsBackend || (deepOverride !== "" ? deepOverride === "deep" : service.searchDeep)
  property bool quickNavActive: false
  property string quickNavChannel: "folders"
  property string quickNavHome: "folders"
  property int quickNavSelectionRevision: 0
  property string quickNavPresentedQuery: ""
  property string quickNavMonitor: ""
  property var quickNavTargetScreen: null
  property string activeQuery: ""
  property string activeMode: ""
  property var activeResponse: null
  property bool queued: false
  property int lastExitCode: -1
  property int lastPayloadCount: -1
  property string lastStderr: ""
  property int cancellationCount: 0
  property var visitQueue: []
  property string activeVisit: ""
  property string activeSearchRequestId: ""
  property int searchGeneration: 0
  property string activeVisitRequestId: ""
  property int visitGeneration: 0
  property var stagedRows: []
  readonly property int pageSize: 48
  property int loadedLimit: 48
  readonly property int resultCount: stagedRows.length
  readonly property bool spinning: (busy || (partial && !quickNavActive)) && (quickNavActive ? quickNavChannel === "files" : deep)
  property string spinnerGlyph: ""
  property int spinnerIndex: 0
  readonly property var spinnerFrames: ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"]

  function clearRows() {
    quickNavPresentedQuery = ""
    model.clear()
    presented = []
    stagedRows = []
    loadedLimit = pageSize
  }

  function presentRows(rows, all) {
    stagedRows = rows
    syncRows(all ? rows : rows.slice(0, loadedLimit))
    var selectionQuery = quickNavChannel + "\u0000" + query
    if (quickNavActive && quickNavPresentedQuery !== selectionQuery) {
      quickNavPresentedQuery = selectionQuery
      quickNavSelectionRevision++
    }
  }

  function loadMore() {
    if (syncing) {
      moreRequested = true
      return
    }
    if (model.count >= stagedRows.length) return
    loadedLimit = model.count + pageSize
    syncRows(stagedRows.slice(0, loadedLimit))
  }

  function loadAll() {
    if (syncing) {
      moreRequested = true
      return
    }
    if (model.count >= stagedRows.length) return
    loadedLimit = stagedRows.length
    syncRows(stagedRows)
  }

  onSpinningChanged: {
    spinnerIndex = 0
    spinnerGlyph = spinning ? spinnerFrames[0] : ""
  }

  Timer {
    interval: 80
    repeat: true
    running: root.spinning
    onTriggered: {
      root.spinnerIndex = (root.spinnerIndex + 1) % root.spinnerFrames.length
      root.spinnerGlyph = root.spinnerFrames[root.spinnerIndex]
    }
  }

  function startQuickNav(targetScreen, channel) {
    quickNavTargetScreen = targetScreen || null
    quickNavMonitor = bladeHost.focusedMonitorName
    quickNavHome = String(channel || "folders")
    quickNavChannel = quickNavHome
    quickNavActive = true
    query = ""
    clearRows()
    error = ""
    backend = "zoxide"
    gitRepositoryCount = 0
    if (!service.open) service.setOpen(true)
    debounce.stop()
    runSearch()
    focusTimer.restart()
  }

  function setQuickNavChannel(channel) {
    if (!quickNavActive || quickNavChannel === String(channel)) return
    quickNavChannel = String(channel)
    query = ""
    clearRows()
    runSearch()
  }

  function rowKey(row) {
    return String(row.path || "") + "\u0000" + String(row.relative || "") + "\u0000" + String(row.kind || "")
  }

  property bool syncing: false
  property bool moreRequested: false

  function syncRows(rows) {
    syncing = true
    try {
      syncRowsNow(rows)
    } finally {
      syncing = false
    }
    if (moreRequested) {
      moreRequested = false
      Qt.callLater(loadMore)
    }
  }

  property var presented: []

  function syncRowsNow(rows) {
    if (presented.length !== model.count) presented = []
    var mirror = presented.slice()
    for (var index = 0; index < rows.length; index++) {
      var wanted = rowKey(rows[index])
      var json = JSON.stringify(rows[index])
      if (index < model.count && (index < mirror.length ? mirror[index].key : rowKey(model.get(index))) === wanted) {
        if (index >= mirror.length || mirror[index].json !== json) {
          model.set(index, rows[index])
          mirror[index] = { key: wanted, json: json }
        }
        continue
      }
      var found = -1
      for (var probe = index + 1; probe < model.count; probe++) {
        if ((probe < mirror.length ? mirror[probe].key : rowKey(model.get(probe))) === wanted) {
          found = probe
          break
        }
      }
      if (found >= 0) {
        model.remove(index, found - index)
        mirror.splice(index, found - index)
        if (index >= mirror.length || mirror[index].json !== json) model.set(index, rows[index])
      } else {
        model.insert(index, rows[index])
        mirror.splice(index, 0, null)
      }
      mirror[index] = { key: wanted, json: json }
    }
    if (model.count > rows.length) {
      model.remove(rows.length, model.count - rows.length)
      mirror.length = rows.length
    }
    presented = mirror
  }

  function localRows(value) {
    var provider = service.channelProvider(quickNavChannel)
    var rows = provider ? provider.rows(value) : []
    var next = []
    for (var index = 0; index < rows.length && index < 50; index++) {
      var row = rows[index]
      next.push(service.makeRow({
        name: String(row.name || ""),
        path: String(row.path || ("channel:" + quickNavChannel + ":" + index)),
        relative: String(row.hint || ""),
        is_dir: !!row.isDir,
        kind: row.path ? "" : "Action",
        name_spans: Highlight.serializeSpans(Highlight.subsequenceSpans(row.name, value))
      }, 0))
    }
    presentRows(next, true)
    busy = false
    error = ""
  }

  function recall(delta) {
    var history = Array.isArray(service.searchHistory) ? service.searchHistory : []
    var next = historyIndex + delta
    if (next < -1 || next >= history.length) return null
    historyIndex = next
    return next < 0 ? "" : history[next]
  }

  function startList(file) {
    stopQuickNav()
    listFile = String(file || "")
    listTitle = ""
    query = ""
    if (!service.open) service.setOpen(true)
    runSearch()
  }

  function stopList() {
    if (!listActive) return
    listFile = ""
    listTitle = ""
    cancelSearch()
    if (query.trim() === "") {
      clearRows()
      busy = false
    } else debounce.restart()
  }

  function restart() {
    if (quickNavActive) runSearch()
    else if (!deep && query.trim() !== "") shallowDebounce.restart()
    else if (query.trim() !== "" || listActive) debounce.restart()
  }

  function shallowRecord(row, relative) {
    var name = String(row.name || "")
    var dot = name.lastIndexOf(".")
    var kinds = [row.isDir ? "dir" : "file"]
    if (row.isSymlink) kinds.push("link")
    if (row.isGitRepo) kinds.push("repo")
    var mime = String(row.mime || "")
    var prefixes = ["image/", "text/", "video/", "audio/"]
    for (var i = 0; i < prefixes.length; i++) if (mime.indexOf(prefixes[i]) === 0) kinds.push(prefixes[i].slice(0, -1))
    var folders = []
    var parts = relative.split("/")
    for (var j = 1; j < parts.length; j++) folders.push(parts.slice(0, j).join("/"))
    return { name: name, text: relative, fields: { type: kinds, format: [dot > 0 ? name.slice(dot + 1).toLowerCase() : ""], mime: [mime.toLowerCase()], "in": folders } }
  }

  function applyShallow() {
    cancelSearch()
    var value = query.trim()
    var spec = SearchQuery.parse(value, ["type", "format", "mime", "in"], { caseSensitive: service.searchCaseSensitive, regex: service.searchRegex })
    var tree = service.treeModel
    var root = service.rootPath
    var sourceRows = []
    for (var index = 0; index < tree.count; index++) sourceRows.push(tree.get(index))
    var ranked = SearchQuery.rankedTree(sourceRows, spec, function(row) {
      return shallowRecord(row, PathText.relative(row.path, root))
    })
    var next = []
    var words = spec.terms.filter(function(term) { return !term.negate && !term.pattern }).map(function(term) { return term.text }).join(" ")
    for (var out = 0; out < ranked.rows.length; out++) {
      var source = ranked.rows[out]
      var entry = service.entrySnapshot(source)
      entry.relative = PathText.relative(source.path, root)
      entry.name_spans = Highlight.serializeSpans(Highlight.subsequenceSpans(source.name, words))
      entry.relative_spans = ""
      entry.git_status = source.gitStatus
      entry.git_status_label = source.gitStatusLabel
      entry.git_repo_root = source.gitRepoRoot
      entry.git_ignored = source.gitIgnored
      entry.size_text = source.sizeText
      entry.size = source.size
      entry.modified = source.modified
      entry.created = source.created
      entry.is_dir = source.isDir
      entry.is_symlink = source.isSymlink
      entry.is_git_repo = source.isGitRepo
      next.push(service.makeRow(entry, Number(source.depth) || 0))
    }
    presentRows(next, false)
    backend = "tree"
    walked = tree.count
    indexed = ranked.matched
    partial = false
    truncated = false
    filterSummary = spec.invalid || ""
    gitRepositoryCount = 0
    error = spec.invalid || ""
    busy = false
  }

  function stopQuickNav() {
    focusTimer.stop()
    quickNavTargetScreen = null
    quickNavActive = false
    query = ""
    clearRows()
    busy = false
    error = ""
    gitRepositoryCount = 0
    cancelSearch()
  }

  function yieldFocus() { focusTimer.stop(); quickNavTargetScreen = null }

  function searchRequest(mode, value) {
    if (mode === "zoxide" && quickNavChannel === "files") {
      var fileArguments = ["--root", service.rootPath, "--query", value, "--limit", "50", "--fresh"]
      if (service.showHidden) fileArguments.push("--show-hidden")
      return { command: "search", arguments: fileArguments }
    }
    if (mode === "zoxide" && quickNavChannel === "recent") {
      var recentArguments = ["--query", value, "--limit", "50"]
      if (service.showHidden) recentArguments.push("--show-hidden")
      return { command: "frecency-list", arguments: recentArguments }
    }
    if (mode === "zoxide") {
      var folderArguments = ["--root", service.home, "--query", value, "--exclude", service.rootPath, "--limit", "50"]
      if (service.showHidden) folderArguments.push("--show-hidden")
      return { command: "quicknav", arguments: folderArguments }
    }
    var arguments = ["--root", service.rootPath, "--query", value, "--limit", "200"]
    if (service.showHidden) arguments.push("--show-hidden")
    if (sessionFresh) arguments.push("--fresh")
    if (service.searchCaseSensitive) arguments.push("--case-sensitive")
    if (service.searchRegex) arguments.push("--regex")
    if (listActive) arguments.push("--list", listFile)
    if (service.searchTreeLayout) arguments.push("--tree")
    if (!service.gitEnabled) arguments.push("--no-git")
    var repositories = service.gitEnabled ? Object.keys(service.gitRepoDirectories) : []
    var added = 0
    for (var index = 0; index < repositories.length && added < 32; index++) {
      if (!service.pathWithin(repositories[index], service.rootPath)) continue
      arguments.push("--repository", repositories[index])
      added++
    }
    return { command: "search", arguments: arguments }
  }

  function cancelSearch() {
    if (!activeSearchRequestId) return
    cancellationCount++
    service.cancelBackendRequest(activeSearchRequestId, searchGeneration)
    searchGeneration++
    activeSearchRequestId = ""
  }

  function runSearch() {
    var value = query.trim()
    var mode = quickNavActive ? "zoxide" : "filesystem"
    if (mode === "filesystem" && !value && !listActive) return
    if (activeSearchRequestId) cancelSearch()
    if (mode === "zoxide" && service.channelProvider(quickNavChannel)) {
      localRows(value)
      return
    }
    activeQuery = value
    activeMode = mode
    activeResponse = null
    if (mode === "filesystem" && backend === "zoxide") backend = "nucleo"
    lastExitCode = -1
    lastPayloadCount = -1
    lastStderr = ""
    busy = true
    error = ""
    gitRepositoryCount = 0
    var request = searchRequest(mode, value)
    searchGeneration++
    var requestGeneration = searchGeneration
    activeSearchRequestId = service.backendRequest(request.command, request.arguments, requestGeneration, function(response) {
      if (requestGeneration !== root.searchGeneration) return
      root.activeSearchRequestId = ""
      root.activeResponse = response
      root.lastPayloadCount = response && Array.isArray(response.entries) ? response.entries.length : -1
      root.finishSearch(0)
    }, function(payload) {
      if (requestGeneration !== root.searchGeneration || root.quickNavActive) return
      root.applyRows(payload)
      root.busy = true
    }, 10000)
    if (mode === "filesystem") sessionFresh = false
  }

  function applyRows(response) {
    var entries = service.searchTreeLayout ? (response.entries || []) : TreeOrder.arrange(response.entries, service.treeSort, service.treeFilter)
    var next = []
    for (var index = 0; index < entries.length; index++) next.push(service.makeRow(entries[index], Number(entries[index].depth) || 0))
    presentRows(next, false)
    backend = String(response.backend || "nucleo")
    walked = Math.max(0, Number(response.walked) || 0)
    indexed = Math.max(0, Number(response.indexed) || 0)
    partial = !!response.partial
    truncated = !!response.truncated
  }

  function applyResponse(response) {
    if (quickNavActive) {
      var loaded = Array.isArray(response.entries) ? response.entries : []
      var loadedRows = []
      for (var loadedIndex = 0; loadedIndex < loaded.length; loadedIndex++) loadedRows.push(service.makeRow(loaded[loadedIndex], 0))
      presentRows(loadedRows, true)
      backend = "zoxide"
      filterSummary = ""
      gitRepositoryCount = 0
      error = response.ok ? "" : String(response.error || "Quick navigation failed")
      busy = false
      return
    }
    applyRows(response)
    listTitle = response.list ? String(response.title || "list") : ""
    filterSummary = String(response.filters || "")
    gitRepositoryCount = service.gitEnabled ? Math.max(0, Number(response.git_repositories) || 0) : 0
    error = response.ok ? "" : String(response.error || "Search failed")
    busy = false
    if (partial && response.ok) continueTimer.restart()
  }

  function rerunSearch() {
    if (quickNavActive || query.trim() !== "") runSearch()
  }

  function finishSearch(exitCode) {
    lastExitCode = exitCode
    var response = activeResponse || { ok: false, error: "Search backend exited with " + exitCode, entries: [] }
    var currentMode = quickNavActive ? "zoxide" : "filesystem"
    var wanted = query.trim()
    if (activeMode === currentMode && activeQuery === wanted) {
      applyResponse(response)
      if (currentMode === "filesystem" && response.ok && wanted) service.rememberSearch(wanted)
    }
    var rerun = queued || activeMode !== currentMode || activeQuery !== wanted
    activeQuery = ""
    activeMode = ""
    activeResponse = null
    queued = false
    if (rerun && (quickNavActive || listActive || query.trim() !== "")) Qt.callLater(runSearch)
  }

  function recordVisit(path) {
    var resolved = service.normalizeRoot(path)
    if (resolved.indexOf("://") >= 0) return
    if (resolved === activeVisit || visitQueue.indexOf(resolved) >= 0) return
    visitQueue = visitQueue.concat([resolved])
    startNextVisit()
  }

  function startNextVisit() {
    if (activeVisitRequestId || visitQueue.length === 0) return
    activeVisit = visitQueue[0]
    visitQueue = visitQueue.slice(1)
    visitGeneration++
    var requestGeneration = visitGeneration
    activeVisitRequestId = service.backendRequest("visit", ["--path", activeVisit], requestGeneration, function(response) {
      if (requestGeneration !== root.visitGeneration) return
      root.activeVisitRequestId = ""
      root.activeVisit = ""
      Qt.callLater(root.startNextVisit)
    })
  }

  onQueryChanged: {
    loadedLimit = pageSize
    if (query.trim() === "") historyIndex = -1
    if (quickNavActive) {
      quickDebounce.restart()
      return
    }
    backend = deep ? "nucleo" : "tree"
    if (listActive) {
      busy = true
      debounce.restart()
      return
    }
    if (query.trim() === "") {
      deepOverride = ""
      debounce.stop()
      shallowDebounce.stop()
      continueTimer.stop()
      queued = false
      cancelSearch()
      clearRows()
      busy = false
      error = ""
      filterSummary = ""
      gitRepositoryCount = 0
      partial = false
      sessionFresh = true
    } else if (!deep) {
      error = ""
      shallowDebounce.restart()
    } else {
      busy = true
      error = ""
      debounce.restart()
    }
  }

  Connections {
    target: root.service
    function onTreeStructureRevisionChanged() {
      if (!root.deep && !root.quickNavActive && root.query.trim() !== "") shallowDebounce.restart()
    }
  }

  onQuickNavActiveChanged: {
    if (!quickNavActive && query.trim() === "") {
      clearRows()
      busy = false
      error = ""
      gitRepositoryCount = 0
    }
  }

  Timer {
    id: debounce
    interval: 180
    onTriggered: root.runSearch()
  }

  Timer {
    id: shallowDebounce
    interval: 40
    onTriggered: if (!root.deep && !root.quickNavActive && root.query.trim() !== "") root.applyShallow()
  }

  Timer {
    id: quickDebounce
    interval: 60
    onTriggered: if (root.quickNavActive) root.runSearch()
  }

  Timer {
    id: continueTimer
    interval: 600
    onTriggered: if (root.partial && !root.quickNavActive && root.query.trim() !== "" && !root.activeSearchRequestId) root.runSearch()
  }

  Timer {
    id: focusTimer
    interval: 220
    onTriggered: {
      if (!service.open || !root.quickNavActive) return
      if (root.quickNavMonitor !== bladeHost.focusedMonitorName) { root.quickNavTargetScreen = null; return }
      var screen = root.quickNavTargetScreen
      if (!screen) screen = service.referenceScreen(null)
      bladeHost.focusModule("files", screen, "quicknav")
      root.quickNavTargetScreen = null
    }
  }

}
