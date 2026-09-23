import QtQuick

Item {
  id: lane
  visible: false

  required property var owner
  required property string scope
  property var items: []
  property string projectRoot: ""
  property string loadError: ""
  property string watchProblem: ""
  property bool watchLimited: false
  property bool truncated: false
  property bool busy: false
  property int generation: 0
  property var scan: null
  property bool refreshQueued: false
  property bool loaded: false
  property bool stopping: false
  property var watch: null
  property int watchGeneration: 0
  property string watchFingerprint: ""
  readonly property string watchError: watchProblem || (watchLimited ? "Some sources exceed the watch limit; refresh to check them" : "")

  function refresh(retryWatch) {
    if (!owner.ready) return
    if (retryWatch === true && watchProblem) stopWatch()
    refreshQueued = true
    busy = true
    debounce.interval = loaded ? 50 : 0
    if (!debounce.running) debounce.start()
  }

  function invalidate() {
    generation++
    items = []
    projectRoot = ""
    loadError = ""
    loaded = false
    truncated = false
    suspendScan()
    stopWatch()
    refresh()
  }

  function suspend() {
    generation++
    suspendScan(owner.stopping)
    stopWatch(owner.stopping)
  }

  function shutdown() {
    stopping = true
    suspendScan(true)
    stopWatch(true)
  }

  function suspendScan(dispose) {
    debounce.stop()
    refreshQueued = false
    busy = false
    if (scan) scan.files.cancelBackendRequest(scan.id, scan.generation, dispose)
  }

  function startScan() {
    if (!owner.ready || scan || owner.applying || !refreshQueued) return
    refreshQueued = false
    var request = { id: "", generation: generation, files: owner.files }
    scan = request
    var args = ["--project", owner.anchorPath, "--json", "--scope", scope].concat(owner.projectArguments, owner.scanArguments)
    request.id = owner.files.backendRequest("helper-read", owner.argumentsFor("list", args), request.generation, function(response) {
      if (lane.stopping || lane.scan !== request) return
      lane.scan = null
      if (lane.owner.ready && request.generation === lane.generation) lane.acceptScan(response)
      lane.busy = lane.owner.ready && lane.refreshQueued
      if (lane.refreshQueued) debounce.restart()
      else lane.owner.queueCounts()
    }, null, 35000)
  }

  function acceptScan(response) {
    var key = owner.itemsKey
    if (!response || response.ok !== true || response.schemaVersion !== 1 || !Array.isArray(response[key])
        || (owner.healthBasis && response.healthBasis !== owner.healthBasis)) {
      loadError = String(response && (response.error || response.message) || "Discovery returned no readable payload").slice(0, 200)
      items = []
      return
    }
    loadError = ""
    loaded = true
    projectRoot = String(response.project || "")
    truncated = response.truncated === true || response[key].length > owner.maximumItems
    var rows = []
    for (var i = 0; i < response[key].length && i < owner.maximumItems; i++) {
      var row = response[key][i]
      if (!row || typeof row !== "object" || Array.isArray(row)) continue
      row.metrics = owner.boundedMetrics(row.metrics)
      rows.push(row)
    }
    items = rows
    startWatch(response.watchPaths, response.watchTruncated === true)
    if (typeof owner.startUsageWatch === "function") owner.startUsageWatch(response.usageWatchPaths)
  }

  function startWatch(raw, capped) {
    var paths = Array.isArray(raw) ? raw.filter(function(path) { return typeof path === "string" && path !== "" }).slice(0, 512) : []
    var fingerprint = JSON.stringify(paths)
    var limited = capped || (Array.isArray(raw) && raw.length > 512)
    if (watch && fingerprint === watchFingerprint) {
      watchLimited = limited
      return
    }
    stopWatch()
    watchLimited = limited
    if (!owner.ready || paths.length === 0) return
    watchFingerprint = fingerprint
    var request = { id: "", generation: ++watchGeneration, files: owner.files }
    watch = request
    request.id = owner.files.backendSubscribe(paths, request.generation, function(event) {
      if (lane.watch !== request || !lane.owner.ready) return
      if (event && (event.overflow || (event.events || []).some(function(name) {
        return name === "delete_self" || name === "move_self" || name === "unmount" || name === "ignored"
      }))) lane.stopWatch()
      lane.refresh()
    }, function(response) {
      if (lane.watch !== request || !lane.owner.ready) return
      lane.watchFailures = 0
      lane.watchProblem = response && Array.isArray(response.skipped) && response.skipped.length
        ? "Some sources could not be watched; refresh to retry" : ""
      lane.refresh()
    }, function(response) {
      if (lane.watch !== request) return
      lane.watch = null
      lane.watchFingerprint = ""
      if (!lane.owner.ready || (response && response.cancelled)) return
      lane.watchProblem = "Watch stopped; retrying"
      lane.watchFailures++
      laneRetry.interval = Math.min(60000, 2000 * Math.pow(2, Math.min(5, lane.watchFailures)))
      laneRetry.restart()
    })
  }

  property int watchFailures: 0

  Timer {
    id: laneRetry
    repeat: false
    onTriggered: {
      if (!lane.owner || !lane.owner.ready || lane.watch) return
      lane.refresh()
    }
  }

  function stopWatch(dispose) {
    var request = watch
    watch = null
    watchFingerprint = ""
    watchProblem = ""
    watchLimited = false
    if (request) request.files.cancelBackendRequest(request.id, request.generation, dispose)
  }

  Timer { id: debounce; interval: 0; onTriggered: lane.startScan() }
}
