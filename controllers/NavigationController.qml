import QtQuick
import "../lib/TreeOrder.js" as TreeOrder

Item {
  id: controller

  required property var service

  function panelActiveFor(panelScreen) {
    return service.bladeHost.panelActiveFor(panelScreen)
  }

  function setOpen(value) {
    var desired = !!value
    if (!desired && service.pickerActive) {
      var requestId = service.pickerRequestId
      service.pickerControllerApi.setResult(requestId, { status: "cancelled", mode: service.pickerMode, paths: [] })
      service.pickerActive = false
      service.pickerRequestId = ""
    }
    service.bladeHost.setAllOpen(desired)
    if (!desired) {
      service.closeActionMenu()
      service.cancelLocationValidation()
    } else {
      var files = service.bladeHost.findModule("files")
      focusAfterOpen(files ? service.bladeHost.preferredScreen(files.edge) : null)
    }
  }

  function toggleOpen() { setOpen(!service.open) }
  function setSidebarWidth(value, screenWidth, persist) { service.bladeHost.setWidth("left", value, screenWidth, persist) }
  function setPropertiesBladeWidth(value, screenWidth, persist) { service.bladeHost.setWidth("right", value, screenWidth, persist) }

  function setPropertiesPlacement(value) {
    service.bladeHost.applyPlacement(value)
    service.bladeHost.setSettingsOpen(false)
  }

  function setPriorityColumns(value) {
    var next = service.normalizePriorityColumns(value)
    var unchanged = JSON.stringify(next) === JSON.stringify(service.priorityColumns)
    var createdChanged = (next.indexOf("created") >= 0) !== (service.priorityColumns.indexOf("created") >= 0)
    service.priorityColumns = next
    service.priorityProperty = next.length > 0 ? next[0] : "none"
    if (!unchanged) {
      service.scheduleStateSave()
      if (createdChanged) service.refreshTree()
    }
    return service.priorityColumns
  }

  function setPriorityProperty(value) {
    var wanted = service.normalizePriorityProperty(value)
    var rest = service.priorityColumns.slice(1).filter(function(key) { return key !== wanted })
    setPriorityColumns(wanted === "none" ? rest : [wanted].concat(rest))
    return service.priorityProperty
  }

  function setGitStatusDetails(value) {
    var next = service.normalizeGitStatusDetails(value)
    if (JSON.stringify(next) === JSON.stringify(service.gitStatusDetails)) return service.gitStatusDetails
    service.gitStatusDetails = next
    service.scheduleStateSave()
    return service.gitStatusDetails
  }

  function setTreeOrder(sort, filter) {
    var nextSort = TreeOrder.normalizeSorts(sort)
    var nextFilter = TreeOrder.normalizeFilter(filter)
    var unchanged = JSON.stringify(nextSort) === JSON.stringify(service.treeSort)
      && JSON.stringify(nextFilter) === JSON.stringify(service.treeFilter)
    if (unchanged) return false
    service.treeSort = nextSort
    service.treeFilter = nextFilter
    service.scheduleStateSave()
    service.refreshTree()
    service.rerunSearch()
    return true
  }

  function cyclePriorityProperty() {
    var properties = ["none", "size", "type", "modified", "created", "repo", "branch", "worktree"]
    var index = properties.indexOf(service.priorityProperty)
    return setPriorityProperty(properties[(index + 1) % properties.length])
  }

  function setSettingsOpen(value, edge) { service.bladeHost.setSettingsOpen(value, edge) }
  function toggleSettings(edge) { return service.bladeHost.toggleSettings(edge) }
  function focusTree(screen) { return service.bladeHost.focusModule("files", screen, "tree") }
  function focusSearch(screen) { return service.bladeHost.focusModule("files", screen, "search") }

  function focusLocation(screen) {
    service.locationValidationError = ""
    if (!service.open) setOpen(true)
    return service.bladeHost.focusModule("files", screen, "location")
  }

  function focusProperties(screen) { return service.bladeHost.focusModule("properties", screen, "") }

  function setRootPath(value, rememberHistory, preserveForward) {
    var next = service.normalizeRoot(value)
    if (service.rootRecoveryOrigin || service.rootRecoveryNotice) service.abortMissingRootRecovery()
    if (next === service.rootPath && service.treeModel.count > 0) return
    rememberRootHistory(next, rememberHistory)
    if (!preserveForward) service.rootForwardStack = []
    service.rootPath = next
    service.quickNavActive = false
    service.searchQuery = ""
    service.searchModel.clear()
    service.searchGitRepositoryCount = 0
    service.clearSelection()
    service.resetTree()
    service.recordZoxideVisit(service.rootPath)
    service.scheduleStateSave()
  }

  function rememberRootHistory(next, rememberHistory) {
    var previous = service.normalizeRoot(service.rootPath)
    if (rememberHistory === false || previous === next) return
    var history = Array.isArray(service.rootBackStack) ? service.rootBackStack.slice() : []
    if (history.length === 0 || history[history.length - 1] !== previous) history.push(previous)
    service.rootBackStack = history.length > 50 ? history.slice(history.length - 50) : history
  }

  function goUp() {
    if (!service.canGoUp) return service.rootPath
    setRootPath(service.parentDirectory(service.rootPath))
    return service.rootPath
  }

  function goHome() {
    setRootPath(service.home)
    return service.rootPath
  }

  function goBack(screen) {
    service.discardCurrentHistoryDestinations("back")
    if (!service.canGoBack) return service.rootPath
    return service.navigateToLocation(service.backDestination, screen, "back")
  }

  function goForward(screen) {
    service.discardCurrentHistoryDestinations("forward")
    if (!service.canGoForward) return service.rootPath
    return service.navigateToLocation(service.forwardDestination, screen, "forward")
  }

  function setShowHidden(value) {
    var desired = !!value
    if (service.showHidden === desired) return service.showHidden
    service.showHidden = desired
    service.resetTree(true)
    if (service.quickNavActive || service.searchQuery.trim() !== "") service.restartSearch()
    service.scheduleStateSave()
    return service.showHidden
  }

  property string focusMonitor: ""
  property var focusTarget: null
  function focusAfterOpen(targetScreen) {
    focusTarget = targetScreen || service.preferredScreen() || null
    focusMonitor = service.bladeHost.focusedMonitorName
    focusTimer.restart()
  }
  function yieldFocus() { focusTimer.stop() }

  Timer {
    id: focusTimer
    interval: 120
    onTriggered: {
      if (!service.open || service.actionMenuOpen) return
      if (controller.focusMonitor !== service.bladeHost.focusedMonitorName) { controller.focusTarget = null; return }
      var screen = controller.focusTarget
      controller.focusTarget = null
      if (screen) controller.focusTree(screen)
    }
  }
}
