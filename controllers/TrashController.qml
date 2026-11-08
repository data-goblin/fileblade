import QtQuick
import "../lib/PathText.js" as PathText

Item {
  id: controller

  signal operationFinished()

  required property var service
  property alias model: trashModel
  readonly property bool busy: listRequestId !== ""
  readonly property bool operationBusy: operationRequestId !== ""
  property string error: ""
  property string notice: ""
  property string operationLabel: ""
  property var progress: null
  property int count: 0
  property int stores: 0
  property string estimatedSizeText: "0 B"
  property bool truncated: false
  property int unknownSize: 0
  property string selectedId: ""
  property string listRequestId: ""
  property int listGeneration: 0
  property bool listDirty: false
  property string operationRequestId: ""
  property int operationGeneration: 0
  property string watchRequestId: ""
  property int watchGeneration: 0
  property string watchFingerprint: ""
  property var watchPaths: []
  property int watchFailures: 0
  property bool retentionQuiet: false
  property int noticeDuration: 3000
  property var presented: []
  property double nextCleanupAt: 0
  readonly property string lastClearedText: formatTimestamp(service.trashLastClearedAt, "Never")
  readonly property string nextCleanupText: {
    if (service.trashCleanupConsent !== true || service.trashRetentionDays <= 0) return "Off"
    if (nextCleanupAt <= 0) return "Nothing scheduled"
    if (nextCleanupAt <= Date.now()) return "Due now"
    return formatTimestamp(nextCleanupAt, "Nothing scheduled")
  }

  ListModel { id: trashModel }

  function formatTimestamp(value, fallback) {
    var timestamp = Number(value)
    if (!isFinite(timestamp) || timestamp <= 0) return String(fallback || "Never")
    return Qt.formatDateTime(new Date(timestamp), "yyyy-MM-dd HH:mm")
  }

  function clearNotice() {
    noticeExpiry.stop()
    notice = ""
  }

  function showNotice(value) {
    notice = String(value || "")
    noticeExpiry.stop()
    if (notice !== "") noticeExpiry.start()
  }

  function cleanupDeadline() {
    var days = Number(service.trashRetentionDays) || 0
    if (service.trashCleanupConsent !== true || days <= 0) return 0
    var earliest = 0
    for (var index = 0; index < trashModel.count; index++) {
      var deletedAt = Date.parse(String(trashModel.get(index).deletedAt || ""))
      if (!isFinite(deletedAt)) continue
      var deadline = deletedAt + days * 86400000
      if (earliest <= 0 || deadline < earliest) earliest = deadline
    }
    return earliest
  }

  Component.onCompleted: {
    refresh()
    if (service.stateReady) retentionSchedule.restart()
  }

  function refresh() {
    if (listRequestId) {
      listDirty = true
      return listRequestId
    }
    listDirty = false
    error = ""
    listGeneration++
    var requestGeneration = listGeneration
    listRequestId = service.backendRequest("trash-list", ["--limit", "1000"], requestGeneration, function(response) {
      if (requestGeneration !== controller.listGeneration) return
      controller.listRequestId = ""
      controller.applyList(response || {})
      if (controller.listDirty) Qt.callLater(controller.refresh)
    }, null, 30000)
    return listRequestId
  }

  function applyList(response) {
    var entries = Array.isArray(response.entries) ? response.entries : []
    var rows = []
    for (var index = 0; index < entries.length; index++) rows.push(row(entries[index]))
    syncRows(rows)
    count = Math.max(entries.length, Number(response.count) || 0)
    nextCleanupAt = cleanupDeadline()
    stores = Math.max(0, Number(response.stores) || 0)
    estimatedSizeText = String(response.estimated_size_text || "0 B")
    unknownSize = Math.max(0, Number(response.unknown_size) || 0)
    truncated = !!response.truncated
    var errors = Array.isArray(response.errors) ? response.errors : []
    error = response.ok === false
      ? String(response.error || "Unable to read Trash")
      : (errors.length > 0 ? String(errors[0]) : "")
    if (selectedId && indexOfId(selectedId) < 0) selectedId = ""
    setWatchPaths(Array.isArray(response.watch_paths) ? response.watch_paths : [])
  }

  function syncRows(rows) {
    if (presented.length !== trashModel.count) presented = []
    var mirror = presented.slice()
    for (var index = 0; index < rows.length; index++) {
      var key = String(rows[index].entryId || "")
      var json = JSON.stringify(rows[index])
      var currentKey = index < trashModel.count
        ? String(index < mirror.length ? mirror[index].key : trashModel.get(index).entryId || "")
        : ""
      if (index < trashModel.count && currentKey === key) {
        if (index >= mirror.length || mirror[index].json !== json) trashModel.set(index, rows[index])
        mirror[index] = { key: key, json: json }
        continue
      }
      var found = -1
      for (var probe = index + 1; probe < trashModel.count; probe++) {
        var probeKey = String(probe < mirror.length ? mirror[probe].key : trashModel.get(probe).entryId || "")
        if (probeKey === key) {
          found = probe
          break
        }
      }
      if (found >= 0) {
        trashModel.move(found, index, 1)
        var moved = mirror.splice(found, 1)[0]
        mirror.splice(index, 0, moved)
        if (!moved || moved.json !== json) trashModel.set(index, rows[index])
      } else {
        trashModel.insert(index, rows[index])
        mirror.splice(index, 0, null)
      }
      mirror[index] = { key: key, json: json }
    }
    if (trashModel.count > rows.length) {
      trashModel.remove(rows.length, trashModel.count - rows.length)
      mirror.length = rows.length
    }
    presented = mirror
  }

  function row(entry) {
    var rawSize = entry.size === undefined || entry.size === null ? -1 : Number(entry.size)
    return {
      entryId: String(entry.id || ""),
      source: String(entry.source || "desktop"),
      module: String(entry.module || ""),
      artifactId: String(entry.artifact_id || ""),
      resource: String(entry.resource || ""),
      name: String(entry.name || ""),
      originalPath: String(entry.original_path || ""),
      originalParent: String(entry.original_parent || ""),
      deletedAt: String(entry.deleted_at || ""),
      size: isFinite(rawSize) ? rawSize : -1,
      sizeText: String(entry.size_text || "—"),
      kind: String(entry.kind || "File"),
      mime: String(entry.mime || "application/octet-stream"),
      isDir: !!entry.is_dir,
      isLink: !!entry.is_link,
      canRestore: !!entry.can_restore,
      canRestoreTo: entry.can_restore_to !== false,
      canDelete: entry.can_delete !== false,
      requiresModuleRestore: !!entry.requires_module_restore,
      emergency: !!entry.emergency,
      sourceMount: String(entry.source_mount || ""),
      store: String(entry.store || "")
    }
  }

  function indexOfId(id) {
    var wanted = String(id || "")
    for (var index = 0; index < trashModel.count; index++)
      if (String(trashModel.get(index).entryId) === wanted) return index
    return -1
  }

  function entry(id) {
    var index = indexOfId(id)
    return index < 0 ? null : trashModel.get(index)
  }

  function setWatchPaths(paths) {
    var next = []
    for (var index = 0; index < paths.length; index++) {
      var path = String(paths[index] || "")
      if (path && next.indexOf(path) < 0) next.push(path)
    }
    next.sort()
    var fingerprint = JSON.stringify(next)
    watchPaths = next
    if (fingerprint === watchFingerprint && watchRequestId) return
    watchFingerprint = fingerprint
    restartWatch()
  }

  function restartWatch() {
    watchGeneration++
    if (watchRequestId) {
      service.cancelBackendRequest(watchRequestId, watchGeneration - 1)
      watchRequestId = ""
    }
    watchRestart.stop()
    if (watchPaths.length === 0) return
    var requestGeneration = watchGeneration
    watchRequestId = service.backendSubscribe(watchPaths, requestGeneration, function(event) {
      if (requestGeneration !== controller.watchGeneration || !controller.listingEvent(event)) return
      trashRefresh.restart()
    }, function() {
      if (requestGeneration !== controller.watchGeneration) return
      if (controller.watchFailures > 0) controller.refresh()
      controller.watchFailures = 0
    }, function(response) {
      if (requestGeneration !== controller.watchGeneration) return
      controller.watchRequestId = ""
      if (response && response.cancelled) return
      controller.watchFailures++
      watchRestart.interval = Math.min(30000, 1000 * Math.pow(2, controller.watchFailures - 1))
      watchRestart.restart()
    })
  }

  function listingEvent(event) {
    var name = String(event && event.name || "")
    if (/\.lock$/.test(name)) return false
    var events = event && Array.isArray(event.events) ? event.events : []
    for (var index = 0; index < events.length; index++)
      if (events[index] !== "attrib") return true
    return events.length === 0
  }

  function restore(id, destination, recreateParent) {
    var item = entry(id)
    if (item && item.source === "satellite") {
      if (String(destination || "").trim() || recreateParent) {
        error = "Satellite Trash entries restore to their original location"
        return "unsupported"
      }
      var helperArguments = service.artifactActions.restoreArguments(item.module)
      if (item.requiresModuleRestore && helperArguments.length === 0) {
        error = "Open the “" + item.module + "” satellite to restore this item"
        return "module-required"
      }
      return run("bin-restore", ["--module", item.module, "--id", item.artifactId].concat(helperArguments), "Restoring")
    }
    var arguments = ["--id", String(id || "")]
    var target = PathText.pathText(destination)
    if (target) arguments.push("--destination", target)
    if (recreateParent) arguments.push("--recreate-parent")
    return run("trash-restore", arguments, "Restoring")
  }

  function deletePermanently(id) {
    var item = entry(id)
    if (item && item.source === "satellite")
      return run("bin-purge", ["--module", item.module, "--id", item.artifactId], "Deleting permanently")
    return run("trash-delete", ["--id", String(id || "")], "Deleting permanently")
  }

  function empty() {
    return run("trash-empty", [], "Emptying Trash")
  }

  function pruneExpired() {
    if (!service.stateReady || service.trashCleanupConsent !== true || service.trashRetentionDays <= 0) return "disabled"
    if (operationRequestId) {
      retentionSchedule.interval = 60000
      retentionSchedule.restart()
      return "busy"
    }
    retentionSchedule.interval = 1500
    return run("trash-prune", ["--days", String(service.trashRetentionDays)], "Cleaning expired Trash", true)
  }

  function run(command, arguments, label, quiet) {
    if (operationRequestId) return "busy"
    operationLabel = String(label || "Trash operation")
    error = ""
    clearNotice()
    progress = null
    retentionQuiet = !!quiet
    operationGeneration++
    var requestGeneration = operationGeneration
    operationRequestId = service.backendRequest(command, arguments, requestGeneration, function(response) {
      if (requestGeneration !== controller.operationGeneration) return
      controller.operationRequestId = ""
      controller.progress = null
      if (response && response.ok) {
        var changed = Math.max(0, Number(response.completed) || 0)
        if (command === "trash-empty" || (command === "trash-prune" && changed > 0))
          service.markTrashCleared(Date.now())
        if (command === "trash-restore" || command === "bin-restore")
          controller.showNotice("Restored")
        else if (!controller.retentionQuiet || changed > 0)
          controller.showNotice(controller.operationLabel + (controller.retentionQuiet ? ": " + changed + " removed" : " complete"))
      }
      else controller.error = String(response && response.error || controller.operationLabel + " failed")
      controller.retentionQuiet = false
      controller.operationLabel = ""
      controller.refresh()
      service.history.refreshJournal()
      controller.operationFinished()
    }, function(value) {
      if (requestGeneration === controller.operationGeneration) controller.progress = value
    }, 900000, { untimed: true })
    return operationRequestId
  }

  function cancelOperation() {
    if (!operationRequestId) return false
    if (operationRequestId.indexOf("artifact-restore-") === 0) return false
    return service.cancelBackendRequest(operationRequestId, operationGeneration)
  }

  Timer {
    id: trashRefresh
    interval: 160
    onTriggered: controller.refresh()
  }

  Timer {
    id: noticeExpiry
    interval: controller.noticeDuration
    onTriggered: controller.notice = ""
  }


  Timer {
    id: retentionSchedule
    interval: 1500
    onTriggered: controller.pruneExpired()
  }

  Timer {
    interval: 3600000
    repeat: true
    running: service.stateReady && service.trashCleanupConsent === true && service.trashRetentionDays > 0
    onTriggered: controller.pruneExpired()
  }

  Connections {
    target: service.history
    function onOperationCompleted(response) {
      if (response && ["trash", "undo", "redo"].indexOf(String(response.operation || "")) >= 0)
        controller.refresh()
    }
  }

  Connections {
    target: service
    function onStateReadyChanged() {
      if (service.stateReady && service.trashCleanupConsent === true && service.trashRetentionDays > 0) retentionSchedule.restart()
    }
    function onTrashRetentionDaysChanged() {
      controller.nextCleanupAt = controller.cleanupDeadline()
      retentionSchedule.stop()
      if (service.stateReady && service.trashCleanupConsent === true && service.trashRetentionDays > 0) retentionSchedule.restart()
    }
  }

  Timer {
    id: watchRestart
    interval: 1000
    onTriggered: controller.restartWatch()
  }
}
