import QtQuick
import qs.Commons
import "KeyPlan.js" as KeyPlan
import "../../../theme"
import qs.Commons as Commons

FocusScope {
  id: module

  property var context: null

  readonly property string title: "Memory"
  readonly property var shortcuts: [
    {
      title: "Memory",
      items: [
        { shortcut: "r", text: "Reveal in files" },
        { shortcut: "d  Delete", text: "Soft delete, delete, or restore" },
        { shortcut: "s", text: "Cycle sort" },
        { shortcut: "f", text: "Filter" },
        { shortcut: "Shift+R", text: "Rescan" }
      ]
    }
  ].concat(files && files.keybindings ? [files.keybindings.treeShortcuts] : [])
  readonly property var metricOptions: context ? context.metrics.options(["off", "agents", "updated", "created", "tokens", "characters", "words", "bytes", "summary"]) : []
  readonly property var view: viewLoader.item
  readonly property var files: context ? context.service("files") : null
  readonly property var provider: context ? context.providerService : null
  readonly property var inventory: provider ? provider.inventory : null
  readonly property string anchorPath: inventory ? inventory.anchorPath : ""
  readonly property var projectArguments: inventory ? inventory.projectArguments : []
  readonly property bool active: !!context && context.bladeOpen !== false && context.collapsed !== true
  readonly property color paneBackground: Qt.lighter(Commons.Color.background, 1.035)

  readonly property var items: inventory ? inventory.items : []
  readonly property string loadError: inventory ? inventory.loadError : (provider ? provider.error : "")
  readonly property bool busy: inventory ? inventory.busy : false
  readonly property bool truncated: inventory ? inventory.truncated : false
  readonly property bool applying: inventory ? inventory.applying : false
  property string binError: ""
  readonly property string applyError: binError || (inventory ? inventory.applyError || inventory.watchError : "")
  property var attachedProvider: null
  property var attachedContext: null
  readonly property string status: {
    if (applying) return "Applying…"
    if (applyError !== "") return applyError
    if (busy) return "Scanning…"
    if (tree.item && tree.item.searching) return tree.item.visibleItems.length + " of " + items.length
    if (truncated) return items.length + " files (capped)"
    return items.length + " files"
  }

  function takeFocus(part) {
    if (tree.item) tree.item.forceActiveFocus()
  }

  function refresh() {
    binError = ""
    if (inventory) inventory.applyError = ""
    if (inventory && active) inventory.refresh(true)
  }

  function rescan() {
    if (inventory && active) inventory.refresh()
  }

  function syncProvider() {
    var next = active ? provider : null
    if (next === attachedProvider && context === attachedContext) return
    if (attachedProvider) attachedProvider.detach(attachedContext)
    attachedProvider = next
    attachedContext = context
    if (next) next.attach(context)
  }

  function installedAgents() {
    return files && Array.isArray(files.installedAgents) ? files.installedAgents.map(String) : []
  }

  function appliedAgents(entry) {
    var readers = entry && Array.isArray(entry.readers) ? entry.readers : []
    var ids = []
    for (var i = 0; i < readers.length; i++) {
      var id = String(readers[i] && readers[i].agent ? readers[i].agent : "")
      if (id !== "" && ids.indexOf(id) < 0) ids.push(id)
    }
    return ids
  }

  function applyAgents(entry, agents, on) {
    if (!inventory || !entry || applying || !Array.isArray(agents) || agents.length === 0) return
    var command = ["--project", anchorPath, "--id", String(entry.id || ""),
                   "--state", on ? "on" : "off", "--json"].concat(projectArguments)
    for (var i = 0; i < agents.length; i++) command.push("--agent", String(agents[i]))
    inventory.mutate("apply", command)
  }

  function applyAll(entry, on) {
    applyAgents(entry, installedAgents(), on)
  }

  function binItem(entry) {
    if (!entry || entry.inlineBytes !== null && entry.inlineBytes !== undefined) return null
    var paths = [String(entry.path || "")]
    var aliases = Array.isArray(entry.aliases) ? entry.aliases : []
    for (var i = 0; i < aliases.length; i++) if (paths.indexOf(String(aliases[i])) < 0) paths.push(String(aliases[i]))
    return { id: String(entry.id || ""), name: String(entry.name || ""), kind: String(entry.kind || ""), scope: String(entry.scope || ""),
             detail: String(entry.detail || ""), path: String(entry.path || ""), realpath: String(entry.realpath || ""), paths: paths,
             position: Math.max(0, allRows().indexOf(entry)), groups: groupPath(entry),
             metrics: entry.metrics && typeof entry.metrics === "object" ? entry.metrics : ({}) }
  }

  function allRows() {
    var binned = bin.item ? bin.item.rows : []
    return bin.item ? bin.item.mergeRows(items, binned, function(entry) { return module.groupPath(entry) }) : items
  }

  function scopeGroup(entry) {
    var scope = String(entry.scope || "")
    if (scope === "managed") return "Managed"
    if (scope === "project" || scope === "local") return "Project"
    if (scope === "extra") return "Extra roots"
    if (scope === "bin") return "Bin"
    return "User"
  }

  function kindGroup(entry) {
    var kind = String(entry.kind || "")
    if (kind === "auto-memory") return "Auto memory"
    if (kind === "rules") return "Rules"
    if (kind === "system-prompt") return "System prompt"
    if (kind === "extra") return "Configured"
    return "Instructions"
  }

  function pluginName(entry) {
    return String(entry.scope || "") === "plugin" ? String(entry.note || entry.source || "Plugin") : ""
  }

  function groupPath(entry) {
    var scope = scopeGroup(entry)
    var plugin = pluginName(entry)
    if (plugin) return [scope, plugin]
    var kind = kindGroup(entry)
    return kind === "Instructions" ? [scope] : [scope, kind]
  }

  readonly property var pluginNames: items.reduce(function(found, entry) {
    var plugin = pluginName(entry)
    if (plugin && found.indexOf(plugin) < 0) found.push(plugin)
    return found
  }, [])

  function groupGlyph(path) {
    return path.length === 2 && pluginNames.indexOf(path[1]) >= 0 ? "󰚥" : ""
  }

  function stateSuffix(entry) {
    if (String(entry.kind || "") === "bin") return bin.item ? bin.item.stateSuffix(entry) : ""
    if (entry.excluded === true) return " (excluded)"
    if (entry.inlineBytes !== null && entry.inlineBytes !== undefined) return " (inline)"
    if (Array.isArray(entry.badges) && entry.badges.indexOf("partly-excluded") >= 0) return " (partly excluded)"
    return ""
  }

  function leafLabel(entry) {
    return String(entry.name || "") + stateSuffix(entry)
  }

  function searchText(entry) {
    var readers = Array.isArray(entry.readers) ? entry.readers : []
    var agents = ""
    for (var i = 0; i < readers.length; i++) agents += " " + String(readers[i].agent)
    return String(entry.name || "") + " " + String(entry.detail || "") + " " + String(entry.path || "")
      + " " + String(entry.kind || "") + " " + String(entry.scope || "") + agents + stateSuffix(entry)
  }

  function openEntry(entry) {
    if (entry && files) files.openDefault(String(entry.path), context.screen, false)
  }

  function revealEntry(entry) {
    if (!entry || !files) return
    var path = String(entry.path)
    if (context.paths.canonical(path)) files.navigateToLocation(context.paths.parent(path), context.screen, "browse")
  }

  onProviderChanged: syncProvider()
  onContextChanged: syncProvider()
  onActiveChanged: syncProvider()
  Component.onCompleted: syncProvider()
  Component.onDestruction: if (attachedProvider) attachedProvider.detach(attachedContext)

  Loader {
    id: viewLoader
    source: module.context ? module.context.ui.url("PaneView") : ""
    onLoaded: {
      item.defaultMetric = "tokens"
      item.defaultSorts = [{ key: "tokens", desc: true }]
      item.options = module.metricOptions
      item.context = Qt.binding(function() { return module.context })
    }
  }

  Rectangle {
    anchors.fill: parent
    color: module.paneBackground
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
      item.title = "MEMORY"
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
      item.prompt = "Filter memory…"
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

  property string query: ""
  property bool caseSensitive: false
  property bool regex: false

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
      item.fileActionsFor = function(entry) { return entry.inlineBytes === null || entry.inlineBytes === undefined }
      item.view = Qt.binding(function() { return module.view })
      item.installedAgents = Qt.binding(function() { return module.files ? module.files.installedAgents : [] })
      item.appliedAgents = function(entry) { return module.appliedAgents(entry) }
      item.surfaceColor = Qt.binding(function() { return module.paneBackground })
      item.groupsFor = function(entry) {
        var binned = bin.item ? bin.item.groupFor(entry) : null
        return binned ? binned : module.groupPath(entry)
      }
      item.rowAction = function(entry) { return bin.item ? bin.item.rowAction(entry) : null }
      item.actionRequested.connect(function(entry) { if (bin.item) bin.item.ask(entry) })
      item.groupBadge = function(path) { return "" }
      item.groupGlyph = function(path) { return module.groupGlyph(path) }
      item.leafLabel = function(entry) { return module.leafLabel(entry) }
      item.searchText = function(entry) { return module.searchText(entry) }
      item.filterKeys = ["agent", "scope", "kind"]
      item.searchFields = function(entry) { return ({ agent: module.appliedAgents(entry), scope: entry.scope, kind: entry.kind }) }
      item.activated.connect(function(entry) { module.openEntry(entry) })
      item.revealed.connect(function(entry) { module.revealEntry(entry) })
      item.searchRequested.connect(function() { module.openSearch() })
      item.filterRequested.connect(function() { module.openFilter() })
      item.agentToggled.connect(function(entry, agentId, on) { module.applyAgents(entry, [agentId], on) })
      item.agentsAllRequested.connect(function(entry, on) { module.applyAll(entry, on) })
      item.focusNextRequested.connect(function() { module.context.focusNext() })
      item.focusPreviousRequested.connect(function() { module.context.focusPrevious() })
      item.dismissRequested.connect(function() { module.context.closeBlade() })
    }
  }

  Keys.onPressed: function(event) {
    if (KeyPlan.action(event.key, event.modifiers) !== "rescan") return
    module.refresh()
    event.accepted = true
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
      item.module = "memory"
      item.context = Qt.binding(function() { return module.context })
      item.describe = function(entry) { return module.binItem(entry) }
      item.changed.connect(function() {
        module.binError = item.error
        module.takeFocus("")
        module.rescan()
      })
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
      : (module.busy ? "Scanning memory…" : (module.query ? "No match" : "No memory files found"))
    color: module.loadError !== "" ? Commons.Color.urgent : Commons.Color.muted
    font.family: Style.font.family
    font.pixelSize: Typography.body
  }
}
