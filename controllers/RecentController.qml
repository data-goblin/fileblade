import QtQuick

Item {
  id: controller

  required property var service
  property alias model: recentModel
  property bool busy: false
  property string error: ""
  property int generation: 0
  property string activeRequestId: ""
  readonly property int limit: 200

  ListModel { id: recentModel }

  function refresh() {
    if (!service.recentMode) return
    if (activeRequestId) {
      service.cancelBackendRequest(activeRequestId, generation)
      activeRequestId = ""
    }
    busy = true
    generation++
    var requestGeneration = generation
    var arguments = ["--limit", String(limit), "--query", ""]
    if (service.showHidden) arguments.push("--show-hidden")
    activeRequestId = service.backendRequest("frecency-list", arguments, requestGeneration, function(response) {
      if (requestGeneration !== controller.generation) return
      controller.activeRequestId = ""
      controller.finish(response || { ok: false, error: "Recent files request failed", entries: [] })
    })
  }

  function finish(response) {
    var entries = Array.isArray(response.entries) ? response.entries : []
    recentModel.clear()
    for (var index = 0; index < entries.length; index++) recentModel.append(service.makeRow(entries[index], 0))
    error = response.ok ? "" : String(response.error || "Unable to read recent files")
    busy = false
    service.scheduleGitMetadataRefresh("recent")
  }

  function recordVisit(arguments) {
    service.backendRequest("frecency-visit", arguments, 0, function() { controller.refresh() })
  }

  Connections {
    target: service
    function onShowHiddenChanged() { controller.refresh() }
    function onRecentModeChanged() {
      if (service.recentMode) controller.refresh()
      else recentModel.clear()
    }
  }
}
