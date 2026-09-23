import QtQuick

Item {
  id: inventory
  visible: false

  property var files: null
  property string providerId: ""
  property string providerRoot: ""
  property string helperId: "inventory"
  property string itemsKey: "items"
  property string healthBasis: ""
  property bool exactProject: true
  property var scanArguments: []
  property int maximumItems: 1000
  property var observers: []
  readonly property string anchorPath: files ? String(files.contextPath !== undefined
    ? files.contextPath : (files.projectRoot || files.selectedPath || files.rootPath || "")) : ""
  readonly property var projectArguments: exactProject && files && (files.contextPath !== undefined || files.projectRoot) ? ["--exact"] : []
  readonly property bool ready: !stopping && observers.length > 0 && !!files && providerId !== "" && providerRoot !== ""
  readonly property var lanes: [projectLane, userLane]
  property var items: []
  property string itemsFingerprint: ""
  property bool overflow: false
  readonly property string projectRoot: projectLane.projectRoot
  readonly property string loadError: projectLane.loadError || userLane.loadError
  readonly property string watchError: projectLane.watchError || userLane.watchError
  readonly property bool truncated: overflow || projectLane.truncated || userLane.truncated
  readonly property bool busy: projectLane.busy || userLane.busy
  property string applyError: ""
  readonly property bool applying: mutation !== null
  property int generation: 0
  property var mutation: null
  property bool stopping: false
  property string activityMethod: ""
  property string usageCountsMethod: ""
  property bool usageCountsItems: true
  property var usageCountsArguments: function(inventory) { return [] }
  property var countsRequest: null
  property var usageCounts: ({})
  property bool countsQueued: false
  property bool usagePending: false
  property var activityArguments: function(inventory) { return [] }
  property var activity: null
  property string activityError: ""
  property var activityRequest: null
  property int activityGeneration: 0
  property bool activityQueued: false
  property var activityObservers: []
  readonly property bool activityEnabled: activityMethod !== "" && activityObservers.length > 0
  property var usageWatch: null
  property int usageWatchGeneration: 0
  property string usageWatchFingerprint: ""
  property var usageWatchPaths: []
  readonly property int usageChangeDelayMs: 100
  readonly property int usagePollIntervalMs: 60000

  signal mutationFinished(string method, var response, string project)

  InventoryLane { id: projectLane; owner: inventory; scope: "project"; onItemsChanged: inventory.publish() }
  InventoryLane { id: userLane; owner: inventory; scope: "user"; onItemsChanged: inventory.publish() }

  function attach(context) {
    if (!context || observers.indexOf(context) >= 0) return
    if (!files) files = context.service("files")
    observers = observers.concat([context])
  }

  function detach(context) {
    observers = observers.filter(function(value) { return value !== context })
  }

  function observeActivity(observer, enabled) {
    activityObservers = activityObservers.filter(function(value) { return value !== observer })
    if (enabled) activityObservers = activityObservers.concat([observer])
  }

  function argumentsFor(method, arguments) {
    return ["--provider", providerId, "--plugin-dir", ["fileblade.core.skills", "fileblade.core.memory", "fileblade.core.hooks", "fileblade.core.mcp"].indexOf(providerId) >= 0 ? "" : providerRoot,
            "--helper", helperId, "--method", method, "--arguments", JSON.stringify(arguments)]
  }

  function refresh(retryWatch) {
    projectLane.refresh(retryWatch)
    userLane.refresh(retryWatch)
    requestActivity()
  }

  function refreshUsage() {
    if (usageCountsMethod === "") {
      refresh()
      return
    }
    requestActivity()
    requestCounts()
  }

  function queueCounts() {
    if (!ready || usageCountsMethod === "") return
    countsQueued = true
    if (!usageChange.running) usageChange.start()
  }

  function requestCounts() {
    if (!ready || usageCountsMethod === "") return
    countsQueued = true
    if (countsRequest || applying || busy) return
    countsQueued = false
    var stubs = items.map(function(row) { return { id: String(row.id || ""), name: String(row.name || ""), source: String(row.source || "") } })
    var request = { id: "", generation: generation, files: files }
    countsRequest = request
    request.id = files.backendRequest("helper-read", argumentsFor(usageCountsMethod,
      usageCountsArguments(inventory).concat(usageCountsItems ? ["--items", JSON.stringify(stubs)] : [])), request.generation, function(response) {
      if (inventory.stopping || inventory.countsRequest !== request) return
      inventory.countsRequest = null
      if (!inventory.ready || request.generation !== inventory.generation) return
      inventory.acceptCounts(response)
      if (inventory.countsQueued) inventory.queueCounts()
      inventory.requestActivity()
    }, null, 20000)
  }

  function acceptCounts(response) {
    if (!response || response.ok !== true || !response.counts || typeof response.counts !== "object") return
    usageCounts = response.counts
    usagePending = response.usageIngestPending === true
    publish()
    if (Array.isArray(response.usageWatchPaths)) startUsageWatch(response.usageWatchPaths)
    if (response.usageIngestPending === true) queueCounts()
  }

  function suspendCounts(dispose) {
    var request = countsRequest
    countsRequest = null
    countsQueued = false
    usageChange.stop()
    if (request) request.files.cancelBackendRequest(request.id, request.generation, dispose)
  }

  function requestActivity() {
    if (!ready || !activityEnabled) return
    activityQueued = true
    activityDebounce.restart()
  }

  function startActivity() {
    if (!ready || !activityEnabled || activityRequest || applying || !activityQueued) return
    if (usageCountsMethod !== "" && (busy || countsRequest)) return
    activityQueued = false
    var request = { id: "", generation: activityGeneration, files: files }
    activityRequest = request
    request.id = files.backendRequest("helper-read", argumentsFor(activityMethod,
      activityArguments(inventory).concat(usageCountsMethod !== "" ? ["--no-ingest"] : [])), request.generation, function(response) {
      if (inventory.stopping || inventory.activityRequest !== request) return
      inventory.activityRequest = null
      if (inventory.ready && request.generation === inventory.activityGeneration) {
        if (inventory.usageCountsMethod !== "" && response && response.ok === true)
          response = Object.assign({}, response, { ingestPending: inventory.usagePending })
        inventory.acceptActivity(response)
      }
      if (inventory.activityQueued) activityDebounce.restart()
    }, null, 35000)
  }

  function acceptActivity(response) {
    if (!response || response.ok !== true || response.schemaVersion !== 1 || !Array.isArray(response.days)) {
      activityError = String(response && (response.error || response.message) || "Activity returned no readable payload").slice(0, 200)
      return
    }
    activityError = ""
    var wasPending = activity && activity.ingestPending === true
    if (JSON.stringify(activity) !== JSON.stringify(response)) activity = response
    if (response.ingestPending === true && usageCountsMethod === "") activityRetry.restart()
    else if (wasPending && usageCountsMethod === "") {
      projectLane.refresh()
      userLane.refresh()
    }
  }

  function startUsageWatch(raw) {
    var paths = Array.isArray(raw) ? raw.filter(function(path) { return typeof path === "string" && path !== "" }).slice(0, 128) : []
    var fingerprint = JSON.stringify(paths)
    if (usageWatch && fingerprint === usageWatchFingerprint) return
    stopUsageWatch()
    usageWatchPaths = paths
    if (!ready || paths.length === 0) return
    usageWatchFingerprint = fingerprint
    var request = { id: "", generation: ++usageWatchGeneration, files: files }
    usageWatch = request
    request.id = files.backendSubscribe(paths, request.generation, function(event) {
      if (inventory.usageWatch !== request || !inventory.ready) return
      if (event && (event.overflow || (event.events || []).some(function(name) {
        return name === "delete_self" || name === "move_self" || name === "unmount" || name === "ignored"
      }))) inventory.stopUsageWatch()
      if (!usageChange.running) usageChange.start()
    }, function(response) {
      if (inventory.usageWatch === request && inventory.ready) inventory.queueCounts()
    }, function(response) {
      if (inventory.usageWatch !== request) return
      inventory.usageWatch = null
      inventory.usageWatchFingerprint = ""
    }, true)
  }

  function stopUsageWatch(dispose) {
    var request = usageWatch
    usageWatch = null
    usageWatchFingerprint = ""
    usageChange.stop()
    if (request) request.files.cancelBackendRequest(request.id, request.generation, dispose)
  }

  function suspendActivity(dispose) {
    activityGeneration++
    activityDebounce.stop()
    activityRetry.stop()
    activityQueued = false
    if (activityRequest) activityRequest.files.cancelBackendRequest(activityRequest.id, activityRequest.generation, dispose)
  }

  function startScan() {
    projectLane.startScan()
    userLane.startScan()
  }

  function publish() {
    var rows = projectLane.items.concat(userLane.items)
    overflow = rows.length > maximumItems
    rows = rows.slice(0, maximumItems)
    rows = rows.map(function(row) {
      var values = usageCounts[String(row.id || "")]
      if (!values) return row
      var merged = Object.assign({}, row, values)
      merged.metrics = boundedMetrics(Object.assign({}, row.metrics || {}, values))
      return merged
    })
    var fingerprint = JSON.stringify(rows)
    if (fingerprint === itemsFingerprint) return
    itemsFingerprint = fingerprint
    items = rows
  }

  function boundedMetrics(raw) {
    if (!raw || typeof raw !== "object" || Array.isArray(raw)) return ({})
    var result = ({})
    for (var date of ["updated", "created"]) result[date] = String(raw[date] || "").slice(0, 32)
    for (var count of ["bytes", "characters", "words", "tokens", "fileTokens",
                       "uses", "usesAgent", "usesUser", "usesScheduled", "failed"]) {
      var value = raw[count], number = Number(value)
      result[count] = value === null || value === undefined || !isFinite(number) ? null : Math.max(0, number)
    }
    return result
  }

  function failureMessage(response) {
    var message = response && (response.error || response.message)
    var results = response && Array.isArray(response.results) ? response.results : []
    for (var i = 0; !message && i < results.length; i++)
      if (results[i] && results[i].ok === false) message = results[i].message
    return String(message || "Change refused").slice(0, 200)
  }

  function mutate(method, arguments, input, callback) {
    if (!ready || applying || !Array.isArray(arguments)) return false
    applyError = ""
    var request = { id: "", generation: generation, files: files, project: anchorPath, method: method }
    mutation = request
    generation++
    for (var lane of lanes) {
      lane.generation++
      lane.suspendScan()
    }
    suspendActivity()
    suspendCounts()
    request.id = files.backendRequest("helper-write", argumentsFor(method, arguments.slice()), request.generation, function(response) {
      if (inventory.stopping || inventory.mutation !== request) return
      inventory.mutation = null
      if (response && response.ok === true && response.schemaVersion !== 1)
        response = { ok: false, error: "Change returned no readable payload" }
      if (request.project === inventory.anchorPath && (!response || response.ok !== true))
        inventory.applyError = inventory.failureMessage(response)
      inventory.refresh()
      inventory.mutationFinished(request.method, response, request.project)
      if (typeof callback === "function") callback(response)
    }, null, 35000, { input: input === undefined ? "" : String(input), untimed: true })
    return true
  }

  onAnchorPathChanged: {
    generation++
    applyError = ""
    projectLane.invalidate()
    suspendActivity()
    suspendCounts()
    activity = null
    activityError = ""
    requestActivity()
  }
  onActivityEnabledChanged: {
    if (activityEnabled) requestActivity()
    else suspendActivity()
  }
  onReadyChanged: {
    if (ready) refresh()
    else {
      for (var lane of lanes) lane.suspend()
      suspendActivity(stopping)
      suspendCounts(stopping)
      stopUsageWatch(stopping)
    }
  }
  Component.onDestruction: {
    stopping = true
    for (var lane of lanes) lane.shutdown()
    suspendActivity(true)
    suspendCounts(true)
    stopUsageWatch(true)
    if (mutation) mutation.files.cancelBackendRequest(mutation.id, mutation.generation, true)
  }

  Timer { id: activityDebounce; interval: 0; onTriggered: inventory.startActivity() }
  Timer { id: usageChange; interval: inventory.usageChangeDelayMs; onTriggered: inventory.refreshUsage() }
  Timer {
    id: usagePoll
    interval: inventory.usagePollIntervalMs
    repeat: true
    running: inventory.ready && inventory.usageWatchPaths.length > 0
    onTriggered: inventory.refreshUsage()
  }
  Timer { id: activityRetry; interval: 500; onTriggered: inventory.requestActivity() }
}
