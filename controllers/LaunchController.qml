import QtQuick

Item {
  id: root

  required property var service

  visible: false

  property bool busy: false
  property string status: ""
  property string error: ""
  property string lastPath: ""
  property string lastAddress: ""
  property var queue: []
  property var activeLaunch: null
  property string activeRequestId: ""
  property int generation: 0

  function enqueue(path, mode, desktopId, line, targetScreen, directoryHint) {
    var target = String(path || service.selectedPath)
    if (!target) return false
    queue = queue.concat([{
      path: target,
      mode: String(mode || "default"),
      desktopId: String(desktopId || ""),
      line: Math.max(0, Number(line) || 0),
      targetScreen: targetScreen || null,
      directory: typeof directoryHint === "boolean" ? directoryHint : null
    }])
    error = ""
    startNext()
    return true
  }

  function openUrl(url, targetScreen) {
    var target = String(url || "")
    if (target.length > 2048 || !/^https?:\/\/[^\/?#@\s]+([\/?#]\S*)?$/i.test(target)) {
      error = "Only http and https links open from FileBlade"
      return false
    }
    return enqueue(target, "url", "", 0, targetScreen, false)
  }

  function startNext() {
    if (activeRequestId || queue.length === 0) return
    activeLaunch = queue[0]
    queue = queue.slice(1)
    busy = true
    if (activeLaunch.mode === "default" && activeLaunch.directory === true) {
      openDirectory()
      return
    }
    if (activeLaunch.mode === "default" && activeLaunch.directory === null) {
      probeDefault()
      return
    }
    launchExternal()
  }

  function probeDefault() {
    status = "Checking " + service.rootName(activeLaunch.path) + "…"
    error = ""
    generation++
    var requestGeneration = generation
    activeRequestId = service.backendRequest("stat-batch", ["--path", activeLaunch.path], requestGeneration, function(result) {
      if (requestGeneration !== root.generation) return
      root.activeRequestId = ""
      var entries = result && Array.isArray(result.entries) ? result.entries : []
      if (!result || !result.ok || entries.length === 0) {
        root.finish({ ok: false, error: String(result && result.error || "Unable to inspect path") })
      } else if (entries[0].is_dir) {
        root.openDirectory()
      } else {
        root.launchExternal()
      }
    })
  }

  function openDirectory() {
    var target = activeLaunch.path
    var targetScreen = activeLaunch.targetScreen
    lastPath = target
    lastAddress = ""
    status = "Opened in FileBlade"
    error = ""
    if (!service.open) service.setOpen(true)
    service.navigateToLocation(target, targetScreen, "browse")
    busy = false
    activeLaunch = null
    Qt.callLater(startNext)
  }

  function launchExternal() {
    status = activeLaunch.mode === "url" ? "Opening link…" : "Opening " + service.rootName(activeLaunch.path) + "…"
    error = ""
    service.yieldFocusForExternalLaunch()
    var arguments = ["--path", activeLaunch.path, "--mode", activeLaunch.mode]
    if (activeLaunch.desktopId) arguments.push("--desktop-id", activeLaunch.desktopId)
    if (activeLaunch.line > 0) arguments.push("--line", String(activeLaunch.line))
    generation++
    var requestGeneration = generation
    activeRequestId = service.backendRequest("launch", arguments, requestGeneration, function(result) {
      if (requestGeneration !== root.generation) return
      root.activeRequestId = ""
      root.finish(result)
    }, null, 15000)
  }

  function finish(result) {
    result = result || { ok: false, error: "Launcher failed" }
    lastPath = activeLaunch ? activeLaunch.path : ""
    lastAddress = String(result.address || "")
    if (result.ok) {
      status = result.placed ? "Opened on the far left" : "Opened"
      error = String(result.placement_error || "")
      if (result.mode !== "url") service.recordFrecencyVisit(["--path", lastPath])
    } else {
      status = ""
      error = String(result.error || "Unable to open path")
    }
    busy = false
    activeLaunch = null
    Qt.callLater(startNext)
  }
}
