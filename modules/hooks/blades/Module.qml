import QtQuick
import qs.Commons
import "KeyPlan.js" as KeyPlan
import "../../../theme"
import qs.Commons as Commons

FocusScope {
  id: module

  property var context: null

  readonly property string title: "Hooks"
  readonly property var metricOptions: context ? context.metrics.options(["off", "agents", "status", "type", "updated", "created", "summary"]) : []
  readonly property var view: viewLoader.item
  readonly property var files: context ? context.service("files") : null
  readonly property var shortcuts: files && files.keybindings ? [files.keybindings.treeShortcuts] : []
  readonly property var installedAgents: files && Array.isArray(files.installedAgents) ? files.installedAgents : []
  readonly property var provider: context ? context.providerService : null
  readonly property var inventory: provider ? provider.inventory : null
  readonly property string anchorPath: inventory ? inventory.anchorPath : ""
  readonly property var projectArguments: inventory ? inventory.projectArguments : []
  readonly property bool collapsed: !!context && context.collapsed === true
  readonly property bool active: !!context && context.bladeOpen !== false && !collapsed
  readonly property color paneBackground: Qt.lighter(Commons.Color.background, 1.035)
  readonly property string status: statusText()

  readonly property var items: inventory ? inventory.items : []
  readonly property string projectRoot: inventory ? inventory.projectRoot : ""
  readonly property string loadError: inventory ? inventory.loadError : (provider ? provider.error : "")
  readonly property bool busy: inventory ? inventory.busy : false
  property string query: ""
  property bool caseSensitive: false
  property bool regex: false
  readonly property bool truncated: inventory ? inventory.truncated : false
  readonly property bool applying: inventory ? inventory.applying : false
  readonly property string applyError: viewError || (inventory ? inventory.applyError || inventory.watchError : "")
  property string viewError: ""
  property var attachedProvider: null
  property var attachedContext: null

  function specialMetricValue(entry, key) {
    if (key === "status") return leafBadge(entry)
    if (key === "type") {
      var summary = entry.summary || ({})
      return String(summary.type || "")
    }
    return undefined
  }

  function statusText() {
    if (applying) return "Applying…"
    if (busy) return "Scanning…"
    if (applyError !== "") return applyError
    if (tree.item && tree.item.searching) return tree.item.visibleItems.length + " of " + items.length
    if (truncated) return items.length + " hooks (capped)"
    return items.length + " hooks"
  }

  function appliedAgents(entry) {
    if (!entry) return []
    if (Array.isArray(entry.appliedAgents) && entry.appliedAgents.length > 0) return entry.appliedAgents
    return [String(entry.agent || "")]
  }

  function takeFocus(part) {
    if (tree.item) tree.item.forceActiveFocus()
  }

  function refresh() {
    viewError = ""
    if (inventory) inventory.applyError = ""
    if (inventory && active) inventory.refresh(true)
  }

  function syncProvider() {
    var next = active ? provider : null
    if (next === attachedProvider && context === attachedContext) return
    if (attachedProvider) attachedProvider.detach(attachedContext)
    attachedProvider = next
    attachedContext = context
    if (next) next.attach(context)
  }

  function runApply(entry, agents, state) {
    if (!inventory || !active || applying || !entry || agents.length === 0) return
    var command = ["--project", anchorPath, "--id", String(entry.id), "--state", state, "--json"].concat(projectArguments)
    for (var i = 0; i < agents.length; i++) command.push("--agent", String(agents[i]))
    viewError = ""
    inventory.mutate("apply", command)
  }

  function binItem(entry) {
    if (!entry || !entry.source || String(entry.supportStatus || "") === "code-hosted" || String(entry.source.kind || "") !== "json") return null
    return { id: String(entry.id || ""), name: leafLabel(entry), kind: "hook", scope: String(entry.scope || ""),
             detail: String(entry.event || "") + " in " + String(entry.source.path || ""), path: String(entry.source.path || ""), paths: [],
             position: Math.max(0, allRows().indexOf(entry)), groups: [agentGroup(entry), eventGroup(entry)],
             metrics: entry.metrics && typeof entry.metrics === "object" ? entry.metrics : ({}) }
  }

  function allRows() {
    var binned = bin.item ? bin.item.rows : []
    return bin.item ? bin.item.mergeRows(items, binned, function(entry) { return [module.agentGroup(entry), module.eventGroup(entry)] }) : items
  }

  function toggleAgent(entry, agentId, on) {
    runApply(entry, [String(agentId)], on ? "on" : "off")
  }

  function applyAll(entry, on) {
    var applied = appliedAgents(entry).map(String)
    var own = String(entry.agent || "")
    var targets = []
    for (var i = 0; i < installedAgents.length; i++) {
      var agentId = String(installedAgents[i])
      if (on && applied.indexOf(agentId) < 0) targets.push(agentId)
      if (!on && agentId !== own) targets.push(agentId)
    }
    runApply(entry, targets, on ? "on" : "off")
  }

  function agentGroup(entry) {
    return String(entry.agent || "unknown")
  }

  function eventGroup(entry) {
    if (String(entry.supportStatus) === "code-hosted") return "Defined in code"
    var event = String(entry.event || "")
    return event === "" ? "Unknown event" : event
  }

  function leafLabel(entry) {
    if (!entry) return ""
    var source = entry.source || ({})
    if (String(entry.supportStatus) === "code-hosted") return String(source.name || source.path || "")
    var summary = entry.summary || ({})
    if (summary.label) return String(summary.label)
    return String(source.name || "") + "  " + String(summary.type || "")
  }

  function nameEntry(entry) {
    var digest = entry && entry.summary ? String(entry.summary.digest || "") : ""
    if (!digest || !inventory || !active) return false
    module.namingDigest = digest
    module.namingText = entry.summary.label ? String(entry.summary.label) : ""
    return true
  }

  function commitName(text) {
    var digest = module.namingDigest
    module.namingDigest = ""
    if (digest !== "" && inventory && active) inventory.mutate("label", ["--digest", digest, "--text", String(text), "--json"])
    if (tree.item) tree.item.forceActiveFocus()
  }

  function cancelName() {
    module.namingDigest = ""
    if (tree.item) tree.item.forceActiveFocus()
  }

  function leafDetail(entry) {
    if (!entry) return ""
    var parts = []
    parts.push(String(entry.scope || ""))
    if (String(entry.supportStatus) === "code-hosted") {
      parts.push(String(entry.entries) + " file" + (entry.entries === 1 ? "" : "s") + ", not inspected")
      return parts.join(", ")
    }
    var summary = entry.summary || ({})
    if (summary.label) parts.push(String((entry.source || ({})).name || ""))
    if (summary.matcher && summary.matcher !== "none") parts.push("matcher " + String(summary.matcher))
    if (summary.timeoutSeconds) parts.push(String(summary.timeoutSeconds) + "s")
    if (summary.envCount > 0) parts.push(String(summary.envCount) + " env")
    if (summary.hasCondition) parts.push("conditional")
    if (summary.digest) parts.push("#" + String(summary.digest))
    return parts.join(", ")
  }

  function leafBadge(entry) {
    if (entry.enabled === false) return "disabled"
    if (String(entry.supportStatus) === "undocumented-event") return "unknown event"
    if (String(entry.supportStatus) === "code-hosted") return "code"
    return entry.enabled === true ? "enabled" : ""
  }

  function searchText(entry) {
    var summary = entry.summary || ({})
    var source = entry.source || ({})
    return String(entry.agent || "") + " " + String(entry.event || "") + " " + String(entry.scope || "")
      + " " + String(entry.supportStatus || "") + " " + String(summary.type || "")
      + " " + String(source.name || "") + " " + String(source.path || "")
  }

  function entryPath(entry) {
    if (!entry) return ""
    if (entry.path) return String(entry.path)
    return entry.source && entry.source.path ? String(entry.source.path) : ""
  }

  function openEntry(entry) {
    var path = entryPath(entry)
    if (path && files) files.openDefault(path, context.screen, false)
  }

  function revealEntry(entry) {
    var path = entryPath(entry)
    if (files && context.paths.canonical(path)) files.navigateToLocation(context.paths.parent(path), context.screen, "browse")
  }

  function openSearch() {
    if (search.item) search.item.reveal()
  }

  function closeSearch() {
    query = ""
    takeFocus("")
  }

  function openFilter() {
    if (header.item) header.item.openFilter()
  }

  onProviderChanged: syncProvider()
  onContextChanged: syncProvider()
  onActiveChanged: syncProvider()
  Component.onCompleted: syncProvider()
  Component.onDestruction: if (attachedProvider) attachedProvider.detach(attachedContext)

  property string namingDigest: ""
  property string namingText: ""

  Keys.onPressed: function(event) {
    var action = KeyPlan.moduleAction(event.key, event.modifiers, Qt)
    if (action === "rescan") {
      module.refresh()
      event.accepted = true
    } else if (action === "name" && tree.item && module.namingDigest === "") {
      if (module.nameEntry(tree.item.currentLeaf())) event.accepted = true
    }
  }

  Rectangle {
    id: namer
    z: 70
    visible: module.namingDigest !== ""
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.top: header.bottom
    anchors.margins: Style.space(7)
    height: visible ? Style.space(34) : 0
    radius: Math.min(Style.cornerRadius, Style.space(4))
    color: Commons.Color.popups.background
    border.width: 1
    border.color: Commons.Color.accent
    onVisibleChanged: if (visible) { nameField.text = module.namingText; nameField.forceActiveFocus(); nameField.selectAll() }

    Text {
      id: namePrompt
      textFormat: Text.PlainText
      anchors.left: parent.left
      anchors.leftMargin: Style.space(10)
      anchors.verticalCenter: parent.verticalCenter
      text: "Name"
      color: Commons.Color.muted
      font.family: Style.font.family
      font.pixelSize: Typography.caption
      font.weight: Font.DemiBold
      font.letterSpacing: 0.4
    }

    TextInput {
      id: nameField
      anchors.left: namePrompt.right
      anchors.right: parent.right
      anchors.leftMargin: Style.space(10)
      anchors.rightMargin: Style.space(10)
      anchors.verticalCenter: parent.verticalCenter
      color: Commons.Color.popups.text
      selectionColor: Util.alpha(Commons.Color.accent, 0.35)
      selectedTextColor: Commons.Color.popups.text
      font.family: Style.font.family
      font.pixelSize: Typography.body
      maximumLength: 80
      clip: true
      Keys.onReturnPressed: module.commitName(text)
      Keys.onEnterPressed: module.commitName(text)
      Keys.onEscapePressed: module.cancelName()
      onActiveFocusChanged: if (!activeFocus && module.namingDigest !== "") module.cancelName()
    }
  }

  Rectangle {
    anchors.fill: parent
    color: module.paneBackground
  }

  Loader {
    id: viewLoader
    active: !!module.context
    source: module.context ? module.context.ui.url("PaneView") : ""
    onLoaded: {
      item.defaultMetric = "agents"
      item.options = module.metricOptions
      item.context = Qt.binding(function() { return module.context })
    }
  }

  Loader {
    id: header
    anchors.top: search.bottom
    anchors.topMargin: search.height > 0 ? Style.space(4) : 0
    anchors.left: parent.left
    anchors.right: parent.right
    source: module.context ? module.context.ui.url("PaneHeader") : ""
    onLoaded: {
      item.context = module.context
      item.title = "HOOKS"
      item.tabIndex = Qt.binding(function() { return module.context.tabIndex })
      item.reservedLeft = Qt.binding(function() { return module.context.cornerReserveLeft })
      item.reservedRight = Qt.binding(function() { return module.context.cornerReserveRight })
      item.highlighted = Qt.binding(function() { return module.activeFocus })
      item.view = Qt.binding(function() { return module.view })
      item.status = Qt.binding(function() { return module.status })
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
    active: module.active
    height: item && item.visible ? Style.space(32) : 0
    source: module.context ? module.context.ui.url("PaneSearchField") : ""
    onLoaded: {
      item.context = Qt.binding(function() { return module.context })
      item.prompt = "Filter hooks…"
      item.text = Qt.binding(function() { return module.query })
      item.showOptions = true
      item.caseSensitive = module.caseSensitive
      item.regex = module.regex
      item.optionsToggled.connect(function(nextCase, nextRegex) {
        module.caseSensitive = nextCase
        module.regex = nextRegex
      })
      item.textChanged.connect(function() { module.query = item.text })
      item.dismissed.connect(function() { module.closeSearch() })
      item.advanced.connect(function() { module.takeFocus("") })
    }
  }

  Loader {
    id: bin
    anchors.fill: parent
    z: 60
    readonly property bool wanted: module.active && !!module.files
    onWantedChanged: sync()
    Component.onCompleted: sync()
    function sync() {
      if (wanted) setSource(module.context.ui.url("ArtifactBin"), { service: module.files })
      else source = ""
    }
    onLoaded: {
      item.module = "hooks"
      item.context = Qt.binding(function() { return module.context })
      item.describe = function(entry) { return module.binItem(entry) }
      item.helperRoute = Qt.binding(function() {
        return module.inventory ? { provider: module.inventory.providerId, directory: "", helper: module.inventory.helperId } : null
      })
      item.removalArguments = function(entry) {
        return ["--project", module.anchorPath, "--id", String(entry.id), "--json"].concat(module.projectArguments)
      }
      item.changed.connect(function() {
        module.viewError = item.error
        if (module.active && tree.item) tree.item.forceActiveFocus()
        if (module.inventory) module.inventory.refresh()
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
    active: module.active
    visible: active
    source: module.context ? module.context.ui.url("ArtifactTree") : ""
    onLoaded: {
      item.context = Qt.binding(function() { return module.context })
      item.changed.connect(function() { module.refresh() })
      item.items = Qt.binding(function() { return module.allRows() })
      item.query = Qt.binding(function() { return module.query })
      item.caseSensitive = Qt.binding(function() { return module.caseSensitive })
      item.regex = Qt.binding(function() { return module.regex })
      item.editPathFor = function(entry) { return module.entryPath(entry) }
      item.fileActionsFor = function(entry) { return false }
      item.view = Qt.binding(function() { return module.view })
      item.installedAgents = Qt.binding(function() { return module.installedAgents })
      item.appliedAgents = function(entry) { return module.appliedAgents(entry) }
      item.specialMetricValue = function(entry, key) { return module.specialMetricValue(entry, key) }
      item.surfaceColor = Qt.binding(function() { return module.paneBackground })
      item.groupsFor = function(entry) {
        var binned = bin.item ? bin.item.groupFor(entry) : null
        return binned ? binned : [module.agentGroup(entry), module.eventGroup(entry)]
      }
      item.leafLabel = function(entry) {
        if (bin.item && bin.item.isBinned(entry)) return String(entry.name || "") + bin.item.stateSuffix(entry)
        return module.leafLabel(entry)
      }
      item.leafDetail = function(entry) { return bin.item && bin.item.isBinned(entry) ? String(entry.detail || "") : module.leafDetail(entry) }
      item.rowAction = function(entry) { return bin.item ? bin.item.rowAction(entry) : null }
      item.actionRequested.connect(function(entry) { if (bin.item) bin.item.ask(entry) })
      item.searchText = function(entry) { return module.searchText(entry) }
      item.filterKeys = ["agent", "event", "scope", "type"]
      item.searchFields = function(entry) { return ({ agent: entry.agent, event: entry.event, scope: entry.scope, type: entry.summary ? entry.summary.type : "" }) }
      item.activated.connect(function(entry) { module.openEntry(entry) })
      item.revealed.connect(function(entry) { module.revealEntry(entry) })
      item.searchRequested.connect(function() { module.openSearch() })
      item.filterRequested.connect(function() { module.openFilter() })
      item.agentToggled.connect(function(entry, agentId, on) { module.toggleAgent(entry, agentId, on) })
      item.agentsAllRequested.connect(function(entry, on) { module.applyAll(entry, on) })
      item.focusNextRequested.connect(function() { module.context.focusNext() })
      item.focusPreviousRequested.connect(function() { module.context.focusPrevious() })
      item.dismissRequested.connect(function() { module.context.closeBlade() })
    }
  }

  Text {
    textFormat: Text.PlainText
    anchors.centerIn: parent
    width: Math.max(0, parent.width - Style.space(40))
    visible: module.active && (module.items.length === 0 || module.loadError !== "")
    horizontalAlignment: Text.AlignHCenter
    wrapMode: Text.WordWrap
    text: module.loadError !== ""
      ? module.loadError
      : (module.busy ? "Scanning hooks…" : (module.query ? "No match" : "No configured hooks found"))
    color: module.loadError !== "" ? Commons.Color.urgent : Commons.Color.muted
    font.family: Style.font.family
    font.pixelSize: Typography.body
  }
}
