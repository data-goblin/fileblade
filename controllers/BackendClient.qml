import QtQuick
import Quickshell
import Quickshell.Io
import "../lib/RequestQueue.js" as RequestQueue

Item {
  id: root

  required property string cliPath
  property string expectedVersion: ""
  property bool ready: false
  property bool desiredRunning: true
  property bool stalled: false
  property int serial: 0
  property int restartDelay: 80
  property int immediateExits: 0
  property double startedAt: 0
  property int inFlight: 0
  property var pending: ({})
  property var queued: []
  property var waiting: []
  property var backendLog: []
  property string lastError: ""
  property string lastStderr: ""
  property string backendVersion: ""
  property var limits: ({})
  property var paths: ({})
  readonly property bool nativeAuthority: String(Quickshell.env("FILEBLADE_NATIVE_STATE_ROOT") || "") !== ""
  signal operationAccepted(string requestId, string generation, string operationId)
  signal operationUpdated(string operationId, var frame)
  readonly property bool versionSkew: expectedVersion !== "" && backendVersion !== "" && expectedVersion !== backendVersion
  readonly property int protocolVersion: 1
  readonly property int expiryGraceMs: 2000
  readonly property int immediateExitMs: 1000
  readonly property int immediateExitLimit: 5
  readonly property int backendLogLines: 50

  function key(id, generation) {
    return String(id) + "\u0000" + String(generation)
  }

  function interactiveFrame(frame) {
    var entry = pending[key(frame.id, frame.generation)]
    return !!entry && !!entry.interactive
  }

  function canTransmit(frame) {
    return RequestQueue.canTransmit(interactiveFrame(frame), inFlight, limits)
  }

  function request(command, arguments, generation, callback, progress, deadlineMs, options) {
    serial++
    var id = "qml-" + Date.now() + "-" + serial
    var currentGeneration = generation === undefined || generation === null ? 0 : generation
    var budget = Math.max(1, Number(deadlineMs) || 15000)
    var untimed = !!(options && options.untimed)
    var frame = {
      v: protocolVersion,
      type: "request",
      id: id,
      generation: currentGeneration,
      command: String(command),
      arguments: Array.isArray(arguments) ? arguments.map(function(value) { return String(value) }) : [],
      deadline_ms: budget
    }
    if (options && options.input !== undefined) frame.input = String(options.input)
    pending[key(id, currentGeneration)] = {
      kind: "request",
      sent: false,
      callback: typeof callback === "function" ? callback : null,
      progress: typeof progress === "function" ? progress : null,
      interactive: RequestQueue.interactive(command, options),
      budget: budget,
      expiresAt: untimed ? 0 : Date.now() + budget + expiryGraceMs
    }
    dispatch(frame)
    armExpiry()
    return id
  }

  function subscribe(paths, generation, eventCallback, readyCallback, closedCallback, includeWrites) {
    return subscribeTopic("filesystem", paths, generation, eventCallback, readyCallback, closedCallback, includeWrites)
  }

  function operation(operationId, action, generation, callback) {
    serial++
    var id = "qml-operation-" + Date.now() + "-" + serial
    var currentGeneration = generation === undefined || generation === null ? 0 : generation
    pending[key(id, currentGeneration)] = {
      kind: "operation",
      sent: false,
      callback: typeof callback === "function" ? callback : null,
      budget: 15000,
      expiresAt: Date.now() + 15000 + expiryGraceMs
    }
    dispatch({ v: protocolVersion, type: "operation", id: id, generation: currentGeneration, op: String(operationId), action: String(action) })
    armExpiry()
    return id
  }

  function subscribeTopic(topic, paths, generation, eventCallback, readyCallback, closedCallback, includeWrites) {
    serial++
    var id = "qml-watch-" + Date.now() + "-" + serial
    var currentGeneration = generation === undefined || generation === null ? 0 : generation
    var frame = {
      v: protocolVersion,
      type: "subscribe",
      id: id,
      generation: currentGeneration,
      topic: String(topic || "filesystem"),
      paths: Array.isArray(paths) ? paths.map(function(value) { return String(value) }) : []
    }
    if (includeWrites === true) frame.includeWrites = true
    pending[key(id, currentGeneration)] = {
      kind: "subscribe",
      sent: false,
      callback: typeof closedCallback === "function" ? closedCallback : null,
      progress: null,
      event: typeof eventCallback === "function" ? eventCallback : null,
      subscribed: typeof readyCallback === "function" ? readyCallback : null
    }
    dispatch(frame)
    return id
  }

  function dispatch(frame) {
    if (!ready) {
      queued.push(frame)
      ensureRunning()
      return
    }
    if (frame.type === "request" && (waiting.length > 0 || !canTransmit(frame))) {
      waiting.push(frame)
      drain()
      return
    }
    transmit(frame)
  }

  function transmit(frame) {
    var entry = pending[key(frame.id, frame.generation)]
    if (!entry) return
    if (frame.type === "request") inFlight++
    entry.sent = true
    send(frame)
  }

  function drain() {
    var active = []
    for (var queuedIndex = 0; queuedIndex < waiting.length; queuedIndex++) {
      var queuedFrame = waiting[queuedIndex]
      if (pending[key(queuedFrame.id, queuedFrame.generation)]) active.push(queuedFrame)
    }
    waiting = active
    while (ready && waiting.length > 0) {
      var priorities = waiting.map(function(frame) { return interactiveFrame(frame) })
      var index = RequestQueue.nextWaitingIndex(priorities, inFlight, limits)
      if (index < 0) return
      transmit(waiting.splice(index, 1)[0])
    }
  }

  function armExpiry() {
    var soonest = 0
    var keys = Object.keys(pending)
    for (var index = 0; index < keys.length; index++) {
      var expiresAt = pending[keys[index]].expiresAt || 0
      if (expiresAt > 0 && (soonest === 0 || expiresAt < soonest)) soonest = expiresAt
    }
    if (soonest === 0) {
      expiryTimer.stop()
      return
    }
    expiryTimer.interval = Math.max(1, soonest - Date.now())
    expiryTimer.restart()
  }

  function expirePending() {
    var now = Date.now()
    var keys = Object.keys(pending)
    for (var index = 0; index < keys.length; index++) {
      var request = pending[keys[index]]
      if (!request.expiresAt || request.expiresAt > now) continue
      var parts = keys[index].split("\u0000")
      if (ready && request.sent) send({ v: protocolVersion, type: "cancel", id: parts[0], generation: parts[1] })
      complete(keys[index], {
        ok: false,
        deadline_exceeded: true,
        error: "No answer from the backend within " + Math.ceil(request.budget / 1000) + " s"
      })
    }
    armExpiry()
  }

  function removeFrame(list, requestKey) {
    var remaining = []
    for (var index = 0; index < list.length; index++) {
      var frame = list[index]
      if (key(frame.id, frame.generation) !== requestKey) remaining.push(frame)
    }
    return remaining
  }

  function cancel(id, generation, discardCallbacks) {
    var currentGeneration = generation === undefined || generation === null ? 0 : generation
    var requestKey = key(id, currentGeneration)
    var entry = pending[requestKey]
    if (!entry) return false
    if (discardCallbacks) {
      entry.callback = null
      entry.progress = null
      entry.event = null
      entry.subscribed = null
    }
    if (entry.sent && ready) {
      if (nativeAuthority && entry.kind === "request") {
        if (discardCallbacks && entry.operationId) return true
        if (!discardCallbacks) entry.cancelRequested = true
        if (entry.operationId && !discardCallbacks) {
          send({ v: protocolVersion, type: "cancel", op: entry.operationId })
          return true
        }
      }
      send({ v: protocolVersion, type: "cancel", id: String(id), generation: currentGeneration })
      return true
    }
    queued = removeFrame(queued, requestKey)
    waiting = removeFrame(waiting, requestKey)
    complete(requestKey, { ok: false, cancelled: true, error: "request cancelled" })
    return true
  }

  function send(frame) {
    if (!backend.running) return false
    backend.write(JSON.stringify(frame) + "\n")
    return true
  }

  function begin() {
    ready = false
    startedAt = Date.now()
    send({ v: protocolVersion, type: "hello", view: nativeAuthority })
  }

  function receiveStderr(data) {
    var line = String(data).trim()
    if (line === "") return
    lastStderr = line
    var log = backendLog.slice()
    log.push(line)
    if (log.length > backendLogLines) log = log.slice(log.length - backendLogLines)
    backendLog = log
  }

  function receive(data) {
    var frame
    try { frame = JSON.parse(String(data)) }
    catch (error) {
      receiveStderr("malformed frame: " + String(data).slice(0, 200))
      return
    }
    if (Number(frame.v) !== protocolVersion) {
      lastError = "Backend protocol version mismatch"
      backend.running = false
      return
    }
    if (frame.type === "hello") {
      if (!frame.ok) {
        lastError = String(frame.error || "Backend handshake failed")
        backend.running = false
        return
      }
      ready = true
      lastError = ""
      restartDelay = 80
      immediateExits = 0
      stalled = false
      inFlight = 0
      limits = frame.limits && typeof frame.limits === "object" ? frame.limits : ({})
      paths = frame.paths && typeof frame.paths === "object" ? frame.paths : ({})
      backendVersion = String(frame.version || "")
      if (versionSkew) console.warn("data-goblin.fileblade: backend " + backendVersion + " does not match plugin " + expectedVersion + "; update or reinstall the plugin")
      var recovered = frame.recovered
      if (recovered && Array.isArray(recovered.restored) && recovered.restored.length > 0)
        console.warn("data-goblin.fileblade: restored " + recovered.restored.length + " staged item(s) after an interrupted operation: " + recovered.restored.join(", "))
      if (recovered && Array.isArray(recovered.conflicts) && recovered.conflicts.length > 0)
        console.warn("data-goblin.fileblade: " + recovered.conflicts.length + " staged item(s) could not be restored because the original name is taken; see fileblade doctor")
      var frames = queued
      queued = []
      for (var index = 0; index < frames.length; index++) dispatch(frames[index])
      return
    }
    if (frame.op && (frame.type === "progress" || frame.type === "response"))
      operationUpdated(String(frame.op), frame)
    var requestKey = key(frame.id, frame.generation)
    var request = pending[requestKey]
    if (!request) return
    if (frame.type === "accepted") {
      request.operationId = String(frame.op)
      request.expiresAt = 0
      armExpiry()
      operationAccepted(String(frame.id), String(frame.generation), request.operationId)
      if (request.cancelRequested) send({ v: protocolVersion, type: "cancel", op: request.operationId })
      return
    }
    if (frame.type === "progress") {
      if (request.expiresAt) {
        request.expiresAt = Date.now() + request.budget + expiryGraceMs
        armExpiry()
      }
      if (request.progress) request.progress(frame.payload || {})
      return
    }
    if (frame.type === "subscribed") {
      if (request.subscribed) request.subscribed(frame)
      return
    }
    if (frame.type === "event") {
      if (request.event) request.event(frame)
      return
    }
    if (frame.type === "response" || frame.type === "operation") {
      var response = frame.ok ? frame.payload : Object.assign({}, frame.payload || {}, {
        ok: false,
        cancelled: !!frame.cancelled,
        deadline_exceeded: !!frame.deadline_exceeded,
        error: String(frame.error || "Backend request failed"),
        error_id: String(frame.error_id || "")
      })
      complete(requestKey, response)
      if (frame.type === "response" && frame.op)
        send({ v: protocolVersion, type: "operation", action: "fetch", op: String(frame.op), id: String(frame.id) + "-fetch", generation: frame.generation })
      return
    }
    if (frame.type === "error") complete(requestKey, {
      ok: false,
      error: String(frame.error || "Backend protocol error")
    })
  }

  function complete(requestKey, response) {
    var request = pending[requestKey]
    if (!request) return
    delete pending[requestKey]
    if (request.kind === "request" && request.sent) inFlight = Math.max(0, inFlight - 1)
    if (request.expiresAt) armExpiry()
    if (request.callback) request.callback(response)
    drain()
  }

  function failAll(message) {
    var requests = pending
    pending = ({})
    queued = []
    waiting = []
    inFlight = 0
    expiryTimer.stop()
    var keys = Object.keys(requests)
    for (var index = 0; index < keys.length; index++) {
      var request = requests[keys[index]]
      if (request.callback) request.callback({ ok: false, detached: !!request.operationId, operationId: request.operationId || "", error: message })
    }
  }

  function ensureRunning() {
    if (!desiredRunning || stalled || backend.running || restartTimer.running) return
    backend.running = true
  }

  function retry() {
    stalled = false
    immediateExits = 0
    restartDelay = 80
    ensureRunning()
  }

  function stop() {
    if (!desiredRunning) return
    desiredRunning = false
    restartTimer.stop()
    if (backend.running) {
      var owner = String(backend.processId)
      backend.running = false
      if (!nativeAuthority) Quickshell.execDetached([root.cliPath, "_backend", "dim-windows", "--state", "off", "--after-exit", owner])
    }
    failAll("Backend stopped")
  }

  Component.onCompleted: ensureRunning()
  Component.onDestruction: stop()

  Timer {
    id: expiryTimer
    onTriggered: root.expirePending()
  }

  Timer {
    id: restartTimer
    interval: root.restartDelay
    onTriggered: root.ensureRunning()
  }

  Process {
    id: backend
    command: [root.cliPath, "serve", "--max-concurrency", "16"]
    stdinEnabled: true
    stdout: SplitParser { onRead: data => root.receive(data) }
    stderr: SplitParser { onRead: data => root.receiveStderr(data) }
    onStarted: root.begin()
    onExited: function(exitCode) {
      root.ready = false
      if (!root.desiredRunning) return
      var detail = root.lastStderr ? ": " + root.lastStderr : ""
      var message = root.lastError || ("Backend exited with " + exitCode + detail)
      root.lastError = message
      root.failAll(message)
      if (Date.now() - root.startedAt < root.immediateExitMs) root.immediateExits++
      else root.immediateExits = 0
      if (root.immediateExits >= root.immediateExitLimit) {
        root.stalled = true
        root.lastError = message + " (gave up after " + root.immediateExits + " immediate exits; run fileblade doctor)"
        console.warn("data-goblin.fileblade: " + root.lastError)
        return
      }
      root.restartDelay = Math.min(4000, Math.max(80, root.restartDelay * 2))
      restartTimer.restart()
    }
  }
}
