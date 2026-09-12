import QtQuick
import "../lib/PathText.js" as PathText

Item {
  id: controller

  required property var service
  property var clipboardPaths: []
  property string clipboardMode: "copy"
  property var externalPaths: []
  property string externalMode: "copy"
  property string externalMime: ""
  property bool externalBusy: false
  property string externalError: ""
  property string pendingExternalDestination: ""
  property string pendingExternalOperationId: ""
  property var externalResponse: null
  property bool discardExternalResult: false
  property bool busy: false
  property string label: ""
  property string error: ""
  property string notice: ""
  property var queue: []
  property var active: null
  property var activeResponse: null
  property var progress: null
  property bool cancelRequested: false
  property int serial: 0
  property var results: []
  property var journalUndo: []
  property var journalRedo: []
  property bool journalDirty: false
  property string refusedUndoId: ""
  property string refusedRedoId: ""
  property string externalRequestId: ""
  property int externalGeneration: 0
  property string journalRequestId: ""
  property int journalGeneration: 0
  property string activeBackendRequestId: ""
  readonly property string activeId: active ? String(active.id || "") : ""
  readonly property int undoCount: journalUndo.length
  readonly property int redoCount: journalRedo.length
  readonly property string undoLabel: journalUndo.length > 0 ? String(journalUndo[0].label || "") : ""
  readonly property string redoLabel: journalRedo.length > 0 ? String(journalRedo[0].label || "") : ""
  readonly property var journaledKinds: ["copy", "move", "rename", "create", "color", "trash"]
  readonly property var historyKinds: ["undo", "redo"]

  signal operationCompleted(var response)

  Component.onCompleted: refreshJournal()

  onNoticeChanged: {
    if (notice && !busy) noticeClearTimer.restart()
    else noticeClearTimer.stop()
  }

  onBusyChanged: {
    if (busy) noticeClearTimer.stop()
    else if (notice) noticeClearTimer.restart()
  }

  Timer {
    id: noticeClearTimer
    interval: 5000
    onTriggered: if (!controller.busy) controller.notice = ""
  }

  function selectionUris(paths) {
    var sources = Array.isArray(paths) ? paths : service.selectedPaths
    var uris = []
    for (var i = 0; i < sources.length; i++) uris.push(service.fileUrl(sources[i]))
    return uris.length === 0 ? "" : uris.join("\r\n") + "\r\n"
  }

  function copySelection(cut, paths) {
    var sources = Array.isArray(paths) ? paths : service.selectedPaths
    if (sources.length === 0) return false
    clipboardPaths = sources.slice()
    clipboardMode = cut ? "cut" : "copy"
    externalPaths = []
    externalMode = "copy"
    externalMime = ""
    externalError = ""
    notice = (cut ? "Cut " : "Copied ") + sources.length + (sources.length === 1 ? " item" : " items")
    var arguments = []
    for (var index = 0; index < sources.length; index++) arguments.push("--path", String(sources[index]))
    if (cut) arguments.push("--cut")
    service.backendRequest("clipboard-write", arguments, Date.now(), function(response) {
      if (!response || !response.ok) controller.error = String(response && response.error || "Unable to update the file clipboard")
    })
    service.closeActionMenu()
    return true
  }

  function extractArchives(paths) {
    var targets = Array.isArray(paths) && paths.length > 0 ? paths : service.selectedPaths
    for (var index = 0; index < targets.length; index++)
      enqueue("Extract", service.backendCommand("archive-extract").concat(["--path", String(targets[index])]), false, true, false, false)
    service.closeActionMenu()
    return targets.length > 0
  }

  function copyPaths(paths) {
    var sources = Array.isArray(paths) ? paths : service.selectedPaths
    if (sources.length === 0) return false
    var arguments = []
    for (var index = 0; index < sources.length; index++) arguments.push("--path", String(sources[index]))
    service.backendRequest("clipboard-text", arguments, Date.now(), function(response) {
      if (response && response.ok) controller.notice = sources.length === 1 ? "Copied path" : "Copied " + sources.length + " paths"
      else controller.error = String(response && response.error || "Unable to copy the path")
    })
    service.closeActionMenu()
    return true
  }

  function clearClipboard() {
    var pendingId = pendingExternalOperationId
    if (externalRequestId) {
      discardExternalResult = true
      service.cancelBackendRequest(externalRequestId, externalGeneration)
      externalGeneration++
      externalRequestId = ""
    }
    clipboardPaths = []
    clipboardMode = "copy"
    clearExternal()
    externalBusy = false
    externalError = ""
    pendingExternalDestination = ""
    pendingExternalOperationId = ""
    if (pendingId) rememberResult({ id: pendingId, label: "Paste" }, {
      ok: false,
      error: "Paste cancelled because the file clipboard was cleared"
    })
    notice = "File clipboard cleared"
    return true
  }

  function clearExternal() {
    externalPaths = []
    externalMode = "copy"
    externalMime = ""
  }

  function command(kind, sources, destination) {
    var value = service.backendCommand(kind).concat(["--destination", destination])
    for (var i = 0; i < sources.length; i++) value.push("--source", String(sources[i]))
    return value
  }

  function enqueuePaste(paths, mode, destination, external, requestId) {
    var target = String(destination || service.selectionDestination())
    var moving = mode === "cut"
    var id = enqueue(
      moving ? "Move" : "Copy",
      command(moving ? "move" : "copy", paths, target),
      !external && moving,
      true,
      true,
      external && moving,
      requestId
    )
    service.closeActionMenu()
    return id
  }

  function pasteInto(destination) {
    var target = String(destination || service.selectionDestination())
    if (clipboardPaths.length > 0) return enqueuePaste(clipboardPaths, clipboardMode, target, false, "")
    if (pendingExternalOperationId) return pendingExternalOperationId
    notice = "Reading file clipboard…"
    pendingExternalOperationId = nextId()
    requestExternalClipboard(target, true)
    return pendingExternalOperationId
  }

  function requestExternalClipboard(destination, pasteAfterProbe) {
    if (pasteAfterProbe) pendingExternalDestination = String(destination || service.selectionDestination())
    if (externalRequestId) return
    discardExternalResult = false
    externalBusy = true
    externalError = ""
    externalResponse = null
    externalGeneration++
    var requestGeneration = externalGeneration
    externalRequestId = service.backendRequest("clipboard", ["--limit", "512"], requestGeneration, function(response) {
      if (requestGeneration !== controller.externalGeneration) return
      controller.externalRequestId = ""
      controller.externalResponse = response
      controller.finishExternal(0)
    })
  }

  function finishExternal(exitCode) {
    if (discardExternalResult) {
      discardExternalResult = false
      externalBusy = false
      externalResponse = null
      return
    }
    var pendingId = pendingExternalOperationId
    if (clipboardPaths.length > 0) {
      finishExternalWithInternal(pendingId)
      return
    }
    var response = externalResponse || { ok: false, paths: [], error: "Clipboard backend exited with " + exitCode }
    externalPaths = response.ok && Array.isArray(response.paths) ? response.paths : []
    externalMode = String(response.mode || "copy") === "cut" ? "cut" : "copy"
    externalMime = String(response.mime || "")
    externalError = response.ok ? "" : String(response.error || "Unable to read clipboard")
    externalBusy = false
    externalResponse = null
    finishExternalPaste(pendingId)
  }

  function finishExternalWithInternal(pendingId) {
    clearExternal()
    externalBusy = false
    externalResponse = null
    var destination = pendingExternalDestination
    pendingExternalDestination = ""
    pendingExternalOperationId = ""
    if (pendingId && destination) enqueuePaste(clipboardPaths, clipboardMode, destination, false, pendingId)
  }

  function finishExternalPaste(pendingId) {
    var destination = pendingExternalDestination
    pendingExternalDestination = ""
    pendingExternalOperationId = ""
    if (!destination) return
    if (externalPaths.length === 0) {
      notice = ""
      error = externalError || "Clipboard does not contain local files"
      if (pendingId) rememberResult({ id: pendingId, label: "Paste" }, { ok: false, error: error })
      return
    }
    notice = ""
    enqueuePaste(externalPaths, externalMode, destination, true, pendingId)
  }
  function moveSelectionTo(destination, copyInstead, paths) {
    var sources = Array.isArray(paths) ? paths.slice() : service.selectedPaths
    if (sources.length === 0) return ""
    var copying = !!copyInstead
    return enqueue(copying ? "Copy" : "Move", command(copying ? "copy" : "move", sources, destination), false)
  }

  function renameSelection(name, path) {
    var target = String(path || (service.selectedPaths.length === 1 ? service.selectedPaths[0] : ""))
    if (!target) return ""
    var command = service.backendCommand("rename").concat(["--path", target, "--name", String(name || "")])
    if (PathText.nameNeedsEscaping(target)) command.push("--name-escaped")
    var id = enqueue("Rename", command, false, true, false)
    service.closeActionMenu()
    return id
  }

  function setFolderColor(path, value) {
    var target = PathText.pathText(path)
    return target ? setSelectionFolderColor(value, [{ path: target }]) : false
  }

  function setSelectionFolderColor(value, entries) {
    var color = service.normalizedFolderColor(value)
    var targets = Array.isArray(entries) ? entries : service.selectedEntries
    if (color === null || targets.length === 0) return false
    var seen = ({})
    var changes = []
    for (var i = 0; i < targets.length; i++) {
      var entry = targets[i]
      if (entry && typeof entry === "object" && entry.gitDeleted) continue
      var rawPath = PathText.pathText(entry && typeof entry === "object" ? (entry.path || "") : (entry || ""))
      if (!rawPath) continue
      var target = service.normalizeRoot(rawPath)
      if (seen[target]) continue
      seen[target] = true
      var before = service.folderColor(target)
      if (before === color) continue
      changes.push({ path: target, before: before, after: color })
    }
    if (changes.length === 0) return Object.keys(seen).length > 0
    var arguments = []
    for (var index = 0; index < changes.length; index++) {
      var change = changes[index]
      arguments.push("--path", change.path, "--before", change.before, "--after", change.after)
    }
    var verb = color ? "Color" : "Reset color"
    var operationLabel = changes.length === 1
      ? verb + " " + service.rootName(changes[0].path)
      : verb + " " + changes.length + " items"
    return !!enqueue(operationLabel, service.backendCommand("color").concat(arguments), false, false, false, false)
  }

  function createEntry(name, directory, parent) {
    var target = String(parent || service.selectionDestination())
    var id = enqueue(directory ? "New folder" : "New file", service.backendCommand("create").concat([
      "--parent", target,
      "--name", String(name || "")
    ]).concat(directory ? ["--directory"] : []), false)
    service.closeActionMenu()
    return id
  }

  function requestTrash(paths) {
    var targets = Array.isArray(paths) && paths.length > 0 ? paths : service.selectedPaths
    if (targets.length === 0) return ""
    if (!service.confirmTrash) return trashSelection(targets)
    service.closeActionMenu()
    service.pendingTrashPaths = targets.slice()
    service.trashConfirmationSerial++
    service.trashConfirmationRequested(targets.slice())
    return "confirm"
  }

  function trashSelection(paths) {
    var targets = Array.isArray(paths) ? paths : service.selectedPaths
    if (targets.length === 0) return ""
    var value = service.backendCommand("trash")
    for (var i = 0; i < targets.length; i++) value.push("--path", targets[i])
    var id = enqueue("Move to Trash", value, false)
    service.closeActionMenu()
    return id
  }

  function nextId() {
    serial++
    return "op-" + Date.now() + "-" + serial
  }

  function refreshJournal() {
    if (journalRequestId) {
      journalDirty = true
      return
    }
    journalDirty = false
    journalResponse = null
    journalGeneration++
    var requestGeneration = journalGeneration
    journalRequestId = service.backendRequest("journal", ["--limit", "20"], requestGeneration, function(response) {
      if (requestGeneration !== controller.journalGeneration) return
      controller.journalRequestId = ""
      controller.journalResponse = response
      controller.finishJournal()
    })
  }

  property var journalResponse: null

  function finishJournal() {
    var response = journalResponse || ({})
    journalUndo = Array.isArray(response.undo) ? response.undo : []
    journalRedo = Array.isArray(response.redo) ? response.redo : []
    if (refusedUndoId && (journalUndo.length === 0 || String(journalUndo[0].id) !== refusedUndoId)) refusedUndoId = ""
    if (refusedRedoId && (journalRedo.length === 0 || String(journalRedo[0].id) !== refusedRedoId)) refusedRedoId = ""
    journalResponse = null
    if (journalDirty) Qt.callLater(controller.refreshJournal)
  }

  function undoOperation(drop, force) { return stepHistory("undo", !!drop, !!force) }

  function redoOperation(drop, force) { return stepHistory("redo", !!drop, !!force) }

  function historyInFlight() {
    if (active && historyKinds.indexOf(String(active.kind || "")) >= 0) return true
    for (var i = 0; i < queue.length; i++) if (historyKinds.indexOf(String(queue[i].kind || "")) >= 0) return true
    return false
  }

  function ageText(seconds) {
    var value = Math.max(0, Number(seconds) || 0)
    if (value < 600) return ""
    if (value < 3600) return " (" + Math.round(value / 60) + " min ago)"
    if (value < 86400) return " (" + Math.round(value / 3600) + " h ago)"
    return " (" + Math.round(value / 86400) + " d ago)"
  }

  function skipRefused() {
    if (refusedUndoId) return stepHistory("undo", true, false)
    if (refusedRedoId) return stepHistory("redo", true, false)
    notice = "No refused entry to skip"
    return ""
  }

  function stepHistory(direction, drop, force) {
    var undoing = direction === "undo"
    var stack = undoing ? journalUndo : journalRedo
    if (stack.length === 0) {
      notice = undoing ? "Nothing to undo" : "Nothing to redo"
      return ""
    }
    if (historyInFlight()) {
      notice = "Wait for the current undo or redo to finish"
      return "busy"
    }
    var top = stack[0]
    var refused = undoing ? refusedUndoId : refusedRedoId
    if (drop && refused !== String(top.id || "")) {
      notice = "Only a refused entry can be skipped"
      return "not-refused"
    }
    var verb = drop ? "Skip " : (undoing ? "Undo " : "Redo ")
    var operationLabel = verb + String(top.label || "") + ageText(top.ageSeconds)
    var flags = (drop ? ["--drop"] : []).concat(force && !drop ? ["--force"] : [])
    var id = enqueue(operationLabel, service.backendCommand(direction).concat(flags), false, true, true, false, "")
    service.closeActionMenu()
    return id
  }

  function blockedEntryId(response) {
    var entry = response.entry || ({})
    var entryId = String(entry.id || "")
    return response.ok || response.empty ? "" : entryId
  }

  function historyOutcome(completed, response) {
    var direction = String(completed ? completed.kind : "")
    if (historyKinds.indexOf(direction) < 0) return
    var blocked = blockedEntryId(response)
    if (direction === "undo") refusedUndoId = blocked
    else refusedRedoId = blocked
    if (blocked) error = String(response.error || "") + ". Fix it and retry, or press U to skip this entry"
    else if (String(response.operation || "") === "drop") notice = "Skipped " + String(response.entry.label || "entry")
    else if (Array.isArray(response.unverified) && response.unverified.length > 0)
      error = "Trashed " + response.unverified.length + " large folder(s) without a full change check; restore from Trash if needed"
  }

  property string actor: "ui"

  function enqueue(operationLabel, operationCommand, clearClipboardAfter, refreshAfter, clearSelectionAfter, clearExternalAfter, requestId) {
    var id = String(requestId || nextId())
    operationCommand = operationCommand.concat(["--actor", actor])
    var backendOperation = Array.isArray(operationCommand) && operationCommand.length > 2
      && String(operationCommand[0]) === service.cliPath && String(operationCommand[1]) === "_backend"
      ? String(operationCommand[2]) : ""
    var journaled = journaledKinds.indexOf(backendOperation) >= 0
    queue = queue.concat([{
      id: id,
      label: String(operationLabel || "File operation"),
      command: journaled ? operationCommand.concat(["--journal-id", id]) : operationCommand,
      arguments: (journaled ? operationCommand.concat(["--journal-id", id]) : operationCommand).slice(3),
      kind: backendOperation,
      cancellable: ["copy", "move"].indexOf(backendOperation) >= 0,
      clearClipboard: !!clearClipboardAfter,
      clearExternal: !!clearExternalAfter,
      refresh: refreshAfter === undefined ? true : !!refreshAfter,
      clearSelection: clearSelectionAfter === undefined ? true : !!clearSelectionAfter
    }])
    startNext()
    return id
  }

  function applyPathEffects(response) {
    var valid = response && typeof response === "object" ? response : ({})
    var operation = String(valid.operation || "")
    var mappings = service.normalizedOperationMappings(response)
    var paths = Array.isArray(valid.paths) ? valid.paths : []
    var colors = Array.isArray(valid.colors) ? valid.colors : []
    var removals = []
    if (mappings.length > 0) service.remapPersistentPathState(mappings)
    if (operation === "rename" && valid.ok) service.remapSelection(mappings)
    var colorsChanged = valid.ok && colors.length > 0 ? service.applyFolderColorChanges(colors) : false
    if (operation === "trash" && valid.ok && paths.length > 0) {
      removals = paths.slice()
      service.prunePersistentPathState(removals)
    }
    var changed = colorsChanged || mappings.length > 0 || removals.length > 0 || paths.length > 0 || !!(valid.ok && String(valid.path || ""))
    return { changed: changed, mappings: mappings, removals: removals }
  }

  function rememberResult(operation, response) {
    if (!operation || !operation.id) return
    var next = results.slice()
    next.push({
      id: String(operation.id),
      status: response && response.cancelled ? "cancelled" : (response && response.ok ? "succeeded" : "failed"),
      label: String(operation.label || "File operation"),
      error: response && response.ok ? "" : String(response && response.error || "File operation failed"),
      result: response || ({ ok: false, error: "Missing operation response" })
    })
    results = next.length > 64 ? next.slice(next.length - 64) : next
  }

  function result(requestId) {
    var id = String(requestId || "")
    if (!id) return { status: "unknown", id: id }
    if (active && String(active.id || "") === id) return activeResult(id)
    if (pendingExternalOperationId === id) return { status: "running", id: id, label: "Reading file clipboard" }
    var queued = queuedResult(id)
    if (queued) return queued
    var completed = takeCompletedResult(id)
    return completed || { status: "unknown", id: id }
  }

  function activeResult(id) {
    return {
      status: cancelRequested ? "cancelling" : "running",
      id: id,
      label: String(active.label || "File operation"),
      cancellable: !!active.cancellable,
      progress: progress || ({})
    }
  }

  function queuedResult(id) {
    for (var i = 0; i < queue.length; i++) {
      if (String(queue[i].id || "") !== id) continue
      return { status: "queued", id: id, label: String(queue[i].label || "File operation"), cancellable: true }
    }
    return null
  }

  function takeCompletedResult(id) {
    for (var i = 0; i < results.length; i++) {
      if (String(results[i].id || "") !== id) continue
      var value = results[i]
      var remaining = results.slice()
      remaining.splice(i, 1)
      results = remaining
      return value
    }
    return null
  }

  function cancel(requestId) {
    var id = String(requestId || activeId)
    if (!id) return "unknown"
    if (pendingExternalOperationId === id) return cancelPendingPaste(id)
    var queued = cancelQueued(id)
    if (queued) return queued
    if (active && String(active.id || "") === id) return cancelActive()
    return completedStatus(id)
  }

  function cancelPendingPaste(id) {
    if (externalRequestId) {
      discardExternalResult = true
      service.cancelBackendRequest(externalRequestId, externalGeneration)
      externalGeneration++
      externalRequestId = ""
    }
    pendingExternalDestination = ""
    pendingExternalOperationId = ""
    externalBusy = false
    rememberResult({ id: id, label: "Paste" }, {
      ok: false,
      cancelled: true,
      operation: "clipboard",
      error: "Paste cancelled before the file transfer started"
    })
    notice = "Paste cancelled"
    return "cancelled"
  }

  function cancelQueued(id) {
    for (var i = 0; i < queue.length; i++) {
      var queued = queue[i]
      if (String(queued.id || "") !== id) continue
      var remaining = queue.slice()
      remaining.splice(i, 1)
      queue = remaining
      rememberResult(queued, {
        ok: false,
        cancelled: true,
        operation: String(queued.kind || ""),
        error: "Cancelled before the operation started"
      })
      notice = String(queued.label || "File operation") + " cancelled"
      return "cancelled"
    }
    return ""
  }

  function cancelActive() {
    if (!active.cancellable) return "not-cancellable"
    if (cancelRequested) return "cancelling"
    cancelRequested = true
    notice = "Stopping " + String(active.label || "file operation").toLowerCase() + "…"
    if (activeBackendRequestId) service.cancelBackendRequest(activeBackendRequestId, String(active.id || ""))
    return "cancelling"
  }

  function completedStatus(id) {
    for (var i = 0; i < results.length; i++)
      if (String(results[i].id || "") === id) return "complete"
    return "unknown"
  }

  function applyProgress(response) {
    var previous = progress || ({ paths: [], mappings: [] })
    var paths = Array.isArray(previous.paths) ? previous.paths.slice() : []
    var mappings = Array.isArray(previous.mappings) ? previous.mappings.slice() : []
    appendCompletedPath(paths, response)
    appendCompletedMapping(mappings, response)
    progress = {
      operation: String(response.operation || previous.operation || ""),
      phase: String(response.phase || ""),
      index: Math.max(0, Number(response.index) || 0),
      total: Math.max(0, Number(response.total) || 0),
      completed: Math.max(0, Number(response.completed) || 0),
      source: String(response.source || ""),
      target: String(response.target || ""),
      paths: paths,
      mappings: mappings
    }
    if (!cancelRequested) updateProgressNotice(response)
  }

  function appendCompletedPath(paths, response) {
    var path = String(response.path || "")
    if (response.phase === "completed" && path && paths.indexOf(path) < 0) paths.push(path)
  }

  function appendCompletedMapping(mappings, response) {
    var mapping = response.mapping
    if (response.phase !== "completed" || !mapping || !mapping.source || !mapping.destination) return
    for (var i = 0; i < mappings.length; i++)
      if (String(mappings[i].source) === String(mapping.source)) return
    mappings.push({ source: String(mapping.source), destination: String(mapping.destination) })
  }

  function updateProgressNotice(response) {
    var verb = String(response.operation || "") === "move" ? "Moving" : "Copying"
    var position = progress.total > 0 ? progress.index + " of " + progress.total : ""
    var name = service.rootName(progress.source)
    notice = verb + (position ? " " + position : "") + (name ? " — " + name : "")
  }

  function startNext() {
    if (activeBackendRequestId || queue.length === 0) return
    active = queue[0]
    queue = queue.slice(1)
    activeResponse = null
    progress = {
      operation: String(active.kind || ""),
      phase: "queued",
      index: 0,
      total: 0,
      completed: 0,
      source: "",
      target: "",
      paths: [],
      mappings: []
    }
    cancelRequested = false
    busy = true
    label = active.label
    error = ""
    notice = label + "…"
    var requestGeneration = String(active.id || "")
    activeBackendRequestId = service.backendRequest(active.kind, active.arguments, requestGeneration, function(response) {
      if (!controller.active || requestGeneration !== String(controller.active.id || "")) return
      controller.activeBackendRequestId = ""
      controller.activeResponse = response
      controller.finish(0)
    }, function(update) {
      if (!controller.active || requestGeneration !== String(controller.active.id || "")) return
      controller.applyProgress(update)
    }, 900000, { untimed: true })
  }

  function finish(exitCode) {
    var completed = active
    var response = completionResponse(exitCode, completed)
    var effects = applyPathEffects(response)
    if (response.cancelled) finishCancelled(response, effects)
    else if (response.ok) finishSucceeded(effects)
    else finishFailed(response, effects)
    rememberResult(completed, response)
    historyOutcome(completed, response)
    refreshJournal()
    operationCompleted(response)
    busy = false
    label = ""
    active = null
    activeBackendRequestId = ""
    activeResponse = null
    progress = null
    cancelRequested = false
    Qt.callLater(controller.startNext)
  }

  function completionResponse(exitCode, completed) {
    if (activeResponse) return activeResponse
    if (!cancelRequested) return { ok: false, error: "File operation exited with " + exitCode }
    var value = progress || ({})
    return {
      ok: false,
      cancelled: true,
      operation: String(value.operation || (completed && completed.kind) || ""),
      paths: Array.isArray(value.paths) ? value.paths : [],
      mappings: Array.isArray(value.mappings) ? value.mappings : [],
      partial_target: String(value.phase === "starting" ? value.target || "" : ""),
      error: "Operation cancelled; the item in progress may have left a partial destination"
    }
  }

  function finishCancelled(response, effects) {
    notice = label + " stopped"
    error = response.partial_target ? "Check for a partial item at " + String(response.partial_target) : ""
    clearAfterOperation()
    service.clearSelection()
    refreshAfterOperation(effects)
  }

  function finishSucceeded(effects) {
    notice = label + " complete"
    error = ""
    clearAfterOperation()
    if (active && active.clearSelection) service.clearSelection()
    refreshAfterOperation(effects)
  }

  function finishFailed(response, effects) {
    notice = ""
    error = String(response.error || "File operation failed")
    if (!effects.changed) return
    service.clearSelection()
    refreshAfterOperation(effects)
  }

  function clearAfterOperation() {
    if (active && active.clearClipboard) {
      clipboardPaths = []
      clipboardMode = "copy"
    }
    if (active && active.clearExternal) clearExternal()
  }

  function refreshAfterOperation(effects) {
    if (active && active.refresh) service.refreshTree(effects.mappings, effects.removals)
  }

}
