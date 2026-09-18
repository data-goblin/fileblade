import QtQuick
import qs.Commons
import "KeyMap.js" as KeyMap
import "../../../theme"

FocusScope {
  id: module

  property var context: null

  readonly property string title: "Skills"
  readonly property string status: statusText()
  readonly property var metricOptions: context ? context.metrics.options(["off", "agents", { key: "uses", label: "Uses", kind: "number" }, { key: "usesAgent", label: "Uses (agent)", shortLabel: "USES (AGENT)", kind: "number" }, { key: "usesUser", label: "Uses (user)", shortLabel: "USES (USER)", kind: "number" }, "updated", "created", { key: "tokens", label: "Tokens (descriptions)" }, { key: "fileTokens", label: "Tokens (SKILL.md)", shortLabel: "TOKENS (SKILL.MD)", kind: "number" }, "characters", "words", "bytes", "summary"]) : []
  readonly property var view: viewLoader.item
  readonly property var files: context ? context.service("files") : null
  readonly property var shortcuts: files && files.keybindings ? [files.keybindings.treeShortcuts] : []
  readonly property var installedAgents: files && Array.isArray(files.installedAgents) ? files.installedAgents : []
  readonly property var provider: context ? context.providerService : null
  readonly property var inventory: provider ? provider.inventory : null
  readonly property string anchorPath: inventory ? inventory.anchorPath : ""
  readonly property var projectArguments: inventory ? inventory.projectArguments : []
  readonly property bool suspended: !context || context.collapsed === true || context.bladeOpen === false
  readonly property color paneBackground: Qt.lighter(Color.bar.background, 1.035)

  readonly property var items: inventory ? inventory.items.map(skillRow).filter(Boolean) : []
  readonly property string loadError: viewError || (inventory ? inventory.loadError : (provider ? provider.error : ""))
  readonly property bool busy: inventory ? inventory.busy : false
  readonly property bool truncated: inventory ? inventory.truncated : false
  property string viewError: ""
  property string binError: ""
  property var attachedProvider: null
  property var attachedContext: null

  property string query: ""
  property string dayFilter: ""
  property string dayLabel: ""
  property var dayIds: null
  property int dayGeneration: 0
  property string dayRequestId: ""
  property string selectedSkillId: ""
  readonly property var selectedSkill: items.find(function(item) { return item.id === selectedSkillId }) || null
  readonly property string skillHistoryItems: selectedSkill ? JSON.stringify(skillStubs([selectedSkill])) : ""
  property var skillHistory: null
  property var skillHistoryRequest: null
  property string skillHistoryError: ""
  property bool caseSensitive: false
  property bool regex: false
  readonly property bool applying: inventory ? inventory.applying : false
  readonly property string applyError: binError || (inventory ? inventory.applyError || inventory.watchError : "") || (bin.item ? bin.item.listingError : "") || (tree.item ? tree.item.folderError : "")

  function takeFocus(part) {
    if (String(part || "") === "search") openSearch()
    else focusTree()
  }

  function specialMetricValue(entry, key) {
    if (key === "agents" || (entry.metrics && key in entry.metrics)) return undefined
    return originBadge(entry) || undefined
  }

  function appliedAgents(entry) {
    return Array.isArray(entry.agents) ? entry.agents : []
  }

  function applyAgents(entry, agentIds, on) {
    if (!inventory || !entry || applying || !Array.isArray(agentIds) || agentIds.length === 0) return
    var command = ["--json", "--project", anchorPath, "--id", String(entry.id), "--state", on ? "on" : "off"].concat(projectArguments)
    for (var i = 0; i < agentIds.length; i++) command.push("--agent", String(agentIds[i]))
    inventory.mutate("apply", command)
  }

  function toggleAgent(entry, agentId, on) {
    applyAgents(entry, [agentId], on)
  }

  function toggleAllAgents(entry, on) {
    applyAgents(entry, installedAgents, on)
  }

  function focusTree() {
    if (tree.item) tree.item.forceActiveFocus()
    else module.forceActiveFocus()
  }

  function openSearch() {
    if (search.item) search.item.reveal()
  }

  function closeSearch() {
    query = ""
    focusTree()
  }

  function refresh() {
    viewError = ""
    binError = ""
    if (inventory) inventory.applyError = ""
    if (inventory && !suspended) inventory.refresh(true)
    if (bin.item) bin.item.refresh()
    if (tree.item) tree.item.refreshFolders()
  }

  function rescan() {
    if (inventory && !suspended) inventory.refresh()
    if (bin.item) bin.item.refresh()
  }

  function cancelDayRequest() {
    dayGeneration++
    if (dayRequestId && files) files.cancelBackendRequest(dayRequestId, dayGeneration - 1, true)
    dayRequestId = ""
  }

  function clearDay() {
    cancelDayRequest()
    dayFilter = ""
    dayLabel = ""
    dayIds = null
  }

  function toggleDay(key, label) {
    if (!files || !inventory) return
    if (dayFilter === key) {
      clearDay()
      focusTree()
      return
    }
    cancelDayRequest()
    var generation = dayGeneration
    var requested = String(key)
    dayFilter = requested
    dayLabel = String(label || requested)
    binError = ""
    dayRequestId = files.backendRequest("helper-read", inventory.argumentsFor("usage-day",
      ["--json", "--day", requested, "--items", JSON.stringify(skillStubs(items))]), generation, function(response) {
      if (generation !== module.dayGeneration) return
      module.dayRequestId = ""
      if (!response || response.ok !== true || !Array.isArray(response.items)) {
        module.clearDay()
        module.binError = String(response && response.error || "Day filter unavailable").slice(0, 200)
        return
      }
      module.dayIds = response.items.map(function(entry) { return String(entry.id || "") })
      module.focusTree()
    }, null, 15000)
  }

  function skillStubs(rows) {
    return rows.map(function(item) { return { id: String(item.id || ""), name: String(item.name || ""), source: String(item.source || "") } })
  }

  function selectSkill(entry) {
    var path = entry && entry.kind !== "bin" ? String(entry.path || "") : ""
    var selected = items.find(function(item) { return path === item.path || path.indexOf(item.path + "/") === 0 })
    selectedSkillId = selected ? selected.id : ""
  }

  function cancelSkillHistory() {
    historyDebounce.stop()
    var request = skillHistoryRequest
    skillHistoryRequest = null
    if (request) request.files.cancelBackendRequest(request.id, 0, true)
  }

  function refreshSkillHistory() {
    cancelSkillHistory()
    skillHistory = null
    skillHistoryError = ""
    if (heatmap.active && inventory && files && skillHistoryItems !== "") historyDebounce.restart()
  }

  function requestSkillHistory() {
    if (!heatmap.active || !inventory || !files || skillHistoryItems === "") return
    var request = { id: "", files: files }
    skillHistoryRequest = request
    request.id = files.backendRequest("helper-read", inventory.argumentsFor("usage",
      ["--json", "--items", skillHistoryItems]), 0, function(response) {
      if (module.skillHistoryRequest !== request) return
      module.skillHistoryRequest = null
      if (!response || response.ok !== true || response.schemaVersion !== 1 || !Array.isArray(response.days)) {
        module.skillHistoryError = String(response && response.error || "Skill activity unavailable").slice(0, 200)
        return
      }
      module.skillHistory = response
    }, null, 15000)
  }

  onSkillHistoryItemsChanged: refreshSkillHistory()
  onAnchorPathChanged: { clearDay(); selectedSkillId = ""; refreshSkillHistory() }
  Timer { id: historyDebounce; interval: 120; onTriggered: module.requestSkillHistory() }
  Connections {
    target: module.inventory
    function onActivityChanged() { module.refreshSkillHistory() }
  }

  function livePath(item) {
    if (!item) return ""
    return String(item.kind === "bin" && item.realpath ? item.realpath : item.path || "")
  }

  function syncProvider() {
    var next = suspended ? null : provider
    if (next === attachedProvider && context === attachedContext) return
    if (attachedProvider) attachedProvider.detach(attachedContext)
    attachedProvider = next
    attachedContext = context
    if (next) next.attach(context)
  }

  function skillDirectoryPath(path) {
    var value = String(path || "")
    return value.slice(-9) === "/SKILL.md" ? value.slice(0, -9) : value
  }

  function skillDescriptorPath(item) {
    if (!item) return ""
    var descriptor = String(item.kind === "bin" ? "" : item.descriptorPath || "")
    if (descriptor) return descriptor
    var path = livePath(item)
    if (path.slice(-9) === "/SKILL.md") return path
    return path.slice(-1) === "/" ? path + "SKILL.md" : path + "/SKILL.md"
  }

  function skillRow(raw) {
    var descriptorPath = String(raw.path || "")
    var path = skillDirectoryPath(descriptorPath)
    if (!context || !context.paths.canonical(descriptorPath) || !path) return null
    return Object.assign({}, raw, {
      path: path,
      isDir: path !== descriptorPath,
      skillRoot: true,
      descriptorPath: descriptorPath,
      linkTarget: skillDirectoryPath(String(raw.link_target || ""))
    })
  }

  function binItem(entry) {
    var path = String(entry.path || "")
    var scope = String(entry.scope || "")
    if (path === "" || (scope !== "user" && scope !== "project")) return null
    var cut = path.lastIndexOf("/")
    var folder = cut > 0 && path.substring(cut + 1) === "SKILL.md" ? path.substring(0, cut) : path
    return { id: String(entry.id || ""), name: String(entry.name || ""), kind: "skill", scope: String(entry.scope || ""),
             detail: String(entry.detail || ""), path: folder, isDir: true, paths: [folder],
             skillRoot: true, linkTarget: String(entry.linkTarget || entry.link_target || entry.realpath || ""),
             position: Math.max(0, allRows().indexOf(entry)), groups: groupPath(entry),
             metrics: entry.metrics && typeof entry.metrics === "object" ? entry.metrics : ({}) }
  }

  function allRows() {
    var binned = bin.item ? bin.item.rows : []
    return bin.item ? bin.item.mergeRows(items, binned, function(entry) { return module.groupPath(entry) }) : items
  }

  function scopeGroup(item) {
    var scope = String(item.scope || "")
    if (scope === "project") return "Project"
    if (scope === "extension") return "Extension"
    if (scope === "managed") return "Managed"
    if (scope === "system") return "System"
    return "User"
  }

  function pluginGroup(item) {
    var source = String(item.source || "")
    if (source.indexOf("plugin:") === 0) return source.substring(7)
    if (source === "synced") return "Synced"
    return "No Plugin"
  }

  function pluginParts(item) {
    var source = String(item.source || "")
    if (source.indexOf("plugin:") !== 0) return null
    var key = source.substring(7)
    var at = key.lastIndexOf("@")
    return { marketplace: at > 0 ? key.substring(at + 1) : "", plugin: at > 0 ? key.substring(0, at) : key }
  }

  function groupPath(item) {
    var scope = scopeGroup(item)
    var parts = pluginParts(item)
    if (parts) return [scope, parts.marketplace || "No marketplace", parts.plugin]
    if (String(item.source || "") === "synced") return [scope, "No marketplace", "Synced"]
    return [scope, "No marketplace", "No plugin"]
  }

  readonly property var marketplaces: items.reduce(function(found, item) {
    var parts = pluginParts(item)
    if (parts && parts.marketplace && found.indexOf(parts.marketplace) < 0) found.push(parts.marketplace)
    return found
  }, [])

  function groupGlyph(path) {
    if (path.length === 3) return path[2] === "Synced" ? "" : "󰚥"
    if (path.length === 2) return "󰏗"
    return ""
  }

  function groupGlyphStruck(path) {
    return (path.length === 2 && path[1] === "No marketplace") || (path.length === 3 && path[2] === "No plugin")
  }

  function originBadge(item) {
    if (item.enabled === false) return "off"
    var agents = Array.isArray(item.agents) ? item.agents : []
    return agents.length > 1 ? agents.length + " agents" : ""
  }

  function searchText(entry) {
    var agents = Array.isArray(entry.agents) ? entry.agents.join(" ") : ""
    return String(entry.name || "") + " " + String(entry.detail || "") + " " + String(entry.path || "")
      + " " + String(entry.source || "") + " " + String(entry.scope || "") + " " + agents
  }

  function binnedSkill(item) {
    return !!item && item.kind === "bin" && String(item.originKind || "") === "skill"
  }

  function openItem(item) {
    if (!item || !files) return
    if (item.isDir || binnedSkill(item)) files.navigateToLocation(livePath(item), context.screen, "browse")
    else files.openDefault(livePath(item), context.screen, false)
  }

  function openDescriptor(item) {
    if (item && files) files.openDefault(skillDescriptorPath(item), context.screen, false)
  }

  function dropSpec(entry) {
    if (!entry || !entry.path || (entry.skillRoot !== true && !binnedSkill(entry))) return null
    var applicable = entry.kind !== "bin" && (String(entry.scope || "") === "user" || String(entry.scope || "") === "project")
    var actions = [
      { id: "skill-open", label: "Open SKILL.md", glyph: "󰍔", key: "o", description: "Open the skill in the editor",
        run: function() { module.openDescriptor(entry) } },
      { id: "skill-reveal", label: "Reveal", glyph: "", key: "r", description: "Show the skill folder in Files",
        run: function() { module.revealItem(entry) } }
    ]
    if (applicable && module.installedAgents.length > 0) actions.unshift({
      id: "skill-apply", label: "Apply to agents", glyph: "󰚩", key: "a", description: "Link this skill for every installed agent",
      run: function() { module.toggleAllAgents(entry, true) }
    })
    return { glyph: "", actions: actions, includeDefaults: true }
  }

  function revealItem(item) {
    if (item && files) files.navigateToLocation(livePath(item), context.screen, "browse")
  }

  function statusText() {
    if (module.loadError) return "error"
    if (module.applying) return "Applying…"
    if (module.applyError) return module.applyError
    if (module.dayRequestId) return "Filtering " + module.dayLabel + "…"
    if (module.dayFilter) return module.dayLabel + ": " + (module.dayIds ? module.dayIds.length : 0) + " used  ·  Esc clears"
    if (heatmap.active && module.skillHistoryError) return "Activity: " + module.skillHistoryError
    if (heatmap.active && inventory && inventory.activityError) return "Activity: " + inventory.activityError
    if (heatmap.active && inventory && inventory.activity && inventory.activity.ingestPending) return "Reading activity…"
    if (module.busy) return "Scanning…"
    if (tree.item && tree.item.searching) return tree.item.visibleItems.length + " of " + module.items.length
    if (module.truncated) return module.items.length + " skills (capped)"
    return module.items.length + " skills"
  }

  onProviderChanged: syncProvider()
  onContextChanged: syncProvider()
  onItemsChanged: if (bin.item) bin.item.refresh()
  onSuspendedChanged: {
    syncProvider()
    if (suspended) cancelDayRequest()
  }
  Component.onCompleted: syncProvider()
  Component.onDestruction: {
    cancelDayRequest()
    cancelSkillHistory()
    if (attachedProvider) attachedProvider.detach(attachedContext)
  }

  Keys.onPressed: function(event) {
    if (event.key === Qt.Key_Escape && module.dayFilter !== "") {
      module.clearDay()
      event.accepted = true
      return
    }
    if (KeyMap.resolve(event.key, event.modifiers, Qt) !== "rescan") return
    module.refresh()
    event.accepted = true
  }

  Loader {
    id: viewLoader
    width: 0
    height: 0
    source: module.context ? module.context.ui.url("PaneView") : ""
    onLoaded: {
      item.defaultMetric = "uses"
      item.activityOption = true
      item.options = module.metricOptions
      item.context = module.context
    }
  }

  Rectangle {
    anchors.fill: parent
    color: module.paneBackground
  }

  Loader {
    id: header
    anchors.top: heatmap.bottom
    anchors.topMargin: heatmap.height > 0 || search.height > 0 ? Style.space(4) : 0
    anchors.left: parent.left
    anchors.right: parent.right
    source: module.context ? module.context.ui.url("PaneHeader") : ""
    onLoaded: {
      item.context = module.context
      item.title = "SKILLS"
      item.tabIndex = Qt.binding(function() { return module.context.tabIndex })
      item.reservedLeft = Qt.binding(function() { return module.context.cornerReserveLeft })
      item.reservedRight = Qt.binding(function() { return module.context.cornerReserveRight })
      item.highlighted = Qt.binding(function() { return module.activeFocus })
      item.status = Qt.binding(function() { return module.status })
      item.view = Qt.binding(function() { return module.view })
    }
  }

  Loader {
    id: search
    anchors.top: parent.top
    anchors.topMargin: height > 0 ? Style.space(6) : 0
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.leftMargin: Style.space(7)
    anchors.rightMargin: Style.space(7)
    active: !module.suspended
    height: item && item.visible ? Style.space(32) : 0
    source: module.context ? module.context.ui.url("PaneSearchField") : ""
    onLoaded: {
      item.prompt = "Filter skills…"
      item.context = Qt.binding(function() { return module.context })
      item.text = Qt.binding(function() { return module.query })
      item.showOptions = true
      item.caseSensitive = module.caseSensitive
      item.regex = module.regex
      item.tabTarget = Qt.binding(function() { return heatmap.item && heatmap.item.visible ? heatmap.item : tree.item })
      item.optionsToggled.connect(function(nextCase, nextRegex) {
        module.caseSensitive = nextCase
        module.regex = nextRegex
      })
      item.textChanged.connect(function() { module.query = item.text })
      item.accepted.connect(function() { module.focusTree() })
      item.dismissed.connect(function() { module.closeSearch() })
      item.advanced.connect(function() { module.focusTree() })
    }
  }

  Loader {
    id: heatmap
    onActiveChanged: {
      module.refreshSkillHistory()
      if (!active && !module.suspended) module.focusTree()
    }
    anchors.top: search.bottom
    anchors.topMargin: height > 0 ? Style.space(4) : 0
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.leftMargin: Style.space(7)
    anchors.rightMargin: Style.space(7)
    active: !module.suspended && !!module.view && module.view.activity && module.height >= Style.space(300)
    height: item && item.visible ? item.implicitHeight : 0
    source: module.context ? module.context.ui.url("UsageHeatmap") : ""
    onLoaded: {
      item.unitLabel = "skill uses"
      item.inventory = Qt.binding(function() { return module.inventory })
      item.selectedDay = Qt.binding(function() { return module.dayFilter })
      item.payload = Qt.binding(function() {
        if (!module.selectedSkill) return module.inventory ? module.inventory.activity : null
        return module.skillHistory || (module.inventory && module.inventory.activity ? Object.assign({}, module.inventory.activity, { days: [] }) : null)
      })
      item.dismissed.connect(function() { module.focusTree() })
      item.previousRequested.connect(function() { module.openSearch() })
      item.dayActivated.connect(function(day, key) { module.toggleDay(key, item.dateLabel(day)) })
    }
  }

  Loader {
    id: bin
    anchors.fill: parent
    z: 60
    readonly property bool wanted: !module.suspended && !!module.files
    onWantedChanged: sync()
    Component.onCompleted: sync()
    function sync() {
      if (wanted) setSource(module.context.ui.url("ArtifactBin"), { service: module.files })
      else source = ""
    }
    onLoaded: {
      item.module = "skills"
      item.context = Qt.binding(function() { return module.context })
      item.describe = function(entry) { return module.binItem(entry) }
      item.changed.connect(function() {
        module.binError = item.error
        if (tree.item) tree.item.forceActiveFocus()
        module.rescan()
      })
    }
  }

  Loader {
    id: tree
    anchors.top: header.bottom
    anchors.topMargin: Style.space(4)
    anchors.bottom: parent.bottom
    anchors.left: parent.left
    anchors.right: parent.right
    active: !module.suspended
    visible: active
    source: module.context ? module.context.ui.url("ArtifactTree") : ""
    onStatusChanged: {
      if (status === Loader.Error) module.viewError = "the Fileblade host did not supply ArtifactTree"
    }
    onLoaded: {
      item.context = Qt.binding(function() { return module.context })
      item.expandableItems = true
      item.loadFolderChildren = true
      item.editPathFor = function(entry) {
        return entry && entry.skillRoot ? module.skillDescriptorPath(entry) : String(entry && entry.path || "")
      }
      item.changed.connect(function() { module.rescan() })
      item.selectionChanged.connect(function(entry) { module.selectSkill(entry) })
      item.items = Qt.binding(function() { return module.allRows() })
      item.query = Qt.binding(function() { return module.query })
      item.idFilter = Qt.binding(function() { return module.dayIds })
      item.caseSensitive = Qt.binding(function() { return module.caseSensitive })
      item.regex = Qt.binding(function() { return module.regex })
      item.view = Qt.binding(function() { return module.view })
      item.installedAgents = Qt.binding(function() { return module.installedAgents })
      item.appliedAgents = function(entry) { return module.appliedAgents(entry) }
      item.specialMetricValue = function(entry, key) { return module.specialMetricValue(entry, key) }
      item.surfaceColor = Qt.binding(function() { return module.paneBackground })
      item.groupsFor = function(entry) {
        var binned = bin.item ? bin.item.groupFor(entry) : null
        return binned ? binned : module.groupPath(entry)
      }
      item.groupGlyph = function(path) { return module.groupGlyph(path) }
      item.groupGlyphStruck = function(path) { return module.groupGlyphStruck(path) }
      item.rowAction = function(entry) { return bin.item ? bin.item.rowAction(entry) : null }
      item.dropSpec = function(entry) { return module.dropSpec(entry) }
      item.actionRequested.connect(function(entry) { if (bin.item) bin.item.ask(entry) })
      item.groupBadge = function(path) { return "" }
      item.leafDetail = function(entry) {
        if (bin.item && bin.item.isBinned(entry)) return String(entry.detail || "") + bin.item.stateSuffix(entry)
        return String(entry.detail || "") || module.originBadge(entry)
      }
      item.searchText = function(entry) { return module.searchText(entry) }
      item.filterKeys = ["agent", "scope", "source", "plugin"]
      item.searchFields = function(entry) { return ({ agent: entry.agents, scope: entry.scope, source: entry.source, plugin: module.pluginGroup(entry) }) }
      item.activated.connect(function(entry) { module.openItem(entry) })
      item.revealed.connect(function(entry) { module.revealItem(entry) })
      item.searchRequested.connect(function() { module.openSearch() })
      item.filterRequested.connect(function() { if (header.item) header.item.openFilter() })
      item.agentToggled.connect(function(entry, agentId, on) { module.toggleAgent(entry, agentId, on) })
      item.agentsAllRequested.connect(function(entry, on) { module.toggleAllAgents(entry, on) })
      item.focusNextRequested.connect(function() { module.context.focusNext() })
      item.focusPreviousRequested.connect(function() { module.context.focusPrevious() })
      item.dismissRequested.connect(function() { module.context.closeBlade() })
    }
  }

  Text {
    textFormat: Text.PlainText
    anchors.centerIn: parent
    width: Math.max(0, parent.width - Style.space(40))
    visible: !module.suspended && (module.loadError !== "" || module.items.length === 0
      || (tree.item && tree.item.rows.length === 0))
    horizontalAlignment: Text.AlignHCenter
    wrapMode: Text.WordWrap
    text: module.loadError ? module.loadError
      : (module.busy ? "Scanning skills…"
      : (module.query ? "No match for the current filter" : "No skills found"))
    color: module.loadError ? Color.urgent : Color.muted
    font.family: Style.font.family
    font.pixelSize: Typography.body
  }
}
