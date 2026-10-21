import QtQuick
import Quickshell
import qs.Commons
import "../components" as Components
import qs.Commons as Commons

FocusScope {
  id: root
  property var context: null

  readonly property string title: "MCP"
  readonly property var metricOptions: context ? context.metrics.options(["off", "agents", "status", { key: "transport", shortLabel: "TYPE" }, { key: "uses", label: "Uses", kind: "number" }, { key: "usesAgent", label: "Uses (agent)", shortLabel: "USES (AGENT)", kind: "number" }, { key: "usesUser", label: "Uses (user)", shortLabel: "USES (USER)", kind: "number" }, "updated", "created", "tokens", "characters", "words", "bytes", "summary"]) : []
  readonly property var files: context ? context.service("files") : null
  readonly property var shortcuts: files && files.keybindings ? [files.keybindings.treeShortcuts] : []
  readonly property var installedAgents: files && Array.isArray(files.installedAgents) ? files.installedAgents : []
  readonly property var provider: context ? context.providerService : null
  readonly property var inventory: provider ? provider.inventory : null
  readonly property string projectPath: inventory ? inventory.anchorPath : ""
  readonly property bool inventoryActive: !!context && context.contractVersion >= 1 && context.bladeOpen && !context.collapsed
  readonly property var view: viewLoader.item
  readonly property string status: applying ? "Applying…" : (applyMessage !== "" ? applyMessage : statusText)

  readonly property var definitions: inventory ? inventory.items : []
  property string query: ""
  property bool caseSensitive: false
  property bool regex: false
  readonly property string statusText: inventoryStatus()
  readonly property string applyMessage: viewError || (inventory ? inventory.applyError || inventory.watchError : "")
  readonly property string loadError: inventory ? inventory.loadError : (provider ? provider.error : "")
  readonly property bool busy: inventory ? inventory.busy : false
  readonly property bool applying: inventory ? inventory.applying : false
  property string viewError: ""
  property var attachedProvider: null
  property var attachedContext: null
  readonly property var observedGlyphs: ({ tool: "󰖷", resource: "󰈔", "resource-list": "󰉹", prompt: "󰍩" })

  function takeFocus(part) {
    if (part === "search" && searchLoader.item) searchLoader.item.reveal()
    else if (treeLoader.item) treeLoader.item.forceActiveFocus()
    else forceActiveFocus()
  }

  function specialMetricValue(entry, key) {
    if (entry.observedKind) return undefined
    if (key === "status") return String(entry.state || "unknown")
    if (key === "transport") return String(entry.transport || "unknown")
    return undefined
  }

  function appliedAgents(entry) {
    return entry && Array.isArray(entry.appliedAgents) ? entry.appliedAgents : []
  }

  function binItem(entry) {
    if (!entry || !entry.source || (String(entry.scope || "") !== "user" && String(entry.scope || "") !== "project")) return null
    return { id: String(entry.id || ""), name: String(entry.name || ""), kind: "mcp", scope: String(entry.scope || ""),
             detail: String(entry.transport || "") + " in " + String(entry.source.path || ""), path: absoluteSource(entry), paths: [],
             position: Math.max(0, allRows().indexOf(entry)), groups: [entry.agent, entry.scope === "plugin" ? "user" : entry.scope],
             metrics: entry.metrics && typeof entry.metrics === "object" ? entry.metrics : ({}) }
  }

  function allRows() {
    var binned = binLoader.item ? binLoader.item.rows : []
    return binLoader.item ? binLoader.item.mergeRows(definitions, binned, function(entry) { return [entry.agent, entry.scope === "plugin" ? "user" : entry.scope] }) : definitions
  }

  function isBinned(entry) {
    return !!binLoader.item && binLoader.item.isBinned(entry)
  }

  function observedRows(entry) {
    var observed = Array.isArray(entry.observed) ? entry.observed.slice(0, 64) : []
    return observed.filter(function(raw) {
      return raw && Object.prototype.hasOwnProperty.call(root.observedGlyphs, String(raw.kind))
    }).map(function(raw) {
      var kind = String(raw.kind), name = String(raw.name || "")
      return { id: [entry.id, kind, name].join(":"), observedKind: kind, name: kind === "resource-list" ? "resource list" : name,
               metrics: { uses: Math.max(0, Number(raw.uses) || 0), failed: Math.max(0, Number(raw.failed) || 0) } }
    })
  }

  function inventoryStatus() {
    if (context && !(context.contractVersion >= 1)) return "Host contract mismatch"
    if (!inventoryActive) return "Paused"
    if (loadError) return loadError
    if (heatmapLoader.active && inventory && inventory.activityError) return "Activity: " + inventory.activityError
    if (heatmapLoader.active && inventory && inventory.activity && inventory.activity.ingestPending) return "Reading activity…"
    if (busy) return "Reading configuration…"
    return definitions.length + " definition" + (definitions.length === 1 ? "" : "s")
      + ", not probed" + (inventory && inventory.truncated ? ", truncated" : "")
  }

  function rescan() {
    viewError = ""
    if (inventory) inventory.applyError = ""
    if (inventory && inventoryActive) inventory.refresh(true)
  }

  function refresh() {
    if (inventory && inventoryActive) inventory.refresh()
  }

  function syncProvider() {
    var next = inventoryActive ? provider : null
    if (next === attachedProvider && context === attachedContext) return
    if (attachedProvider) attachedProvider.detach(attachedContext)
    attachedProvider = next
    attachedContext = context
    if (next) next.attach(context)
  }

  function runApply(entry, agents, state) {
    if (!inventory || !inventoryActive || applying || !entry || !entry.id || agents.length === 0) return
    var command = ["--project", projectPath, "--id", String(entry.id)]
    for (var index = 0; index < agents.length; index++) command.push("--agent", String(agents[index]))
    command.push("--state", state, "--json")
    viewError = ""
    inventory.mutate("apply", command)
  }

  function toggleAgent(entry, agentId, on) {
    runApply(entry, [agentId], on ? "on" : "off")
  }

  function toggleAllAgents(entry, on) {
    var installed = installedAgents.map(String)
    var applied = appliedAgents(entry).map(String)
    if (on) {
      runApply(entry, installed.filter(function(id) { return applied.indexOf(id) < 0 }), "on")
      return
    }
    var own = String(entry.agentId || "")
    runApply(entry, installed.filter(function(id) { return id !== own }), "off")
  }

  function absoluteSource(item) {
    if (!item) return ""
    if (!item.source) return item.path ? String(item.path) : ""
    if (item.source.redacted !== false) return ""
    var path = String(item.source.path || "")
    if (path === "<project>") return projectPath
    if (path.indexOf("<project>/") === 0) return context.paths.join(projectPath, path.slice(10))
    var home = Quickshell.env("HOME") || ""
    var codexHome = Quickshell.env("CODEX_HOME") || home + "/.codex"
    if (path === "<codex-home>") return codexHome
    if (path.indexOf("<codex-home>/") === 0) return context.paths.join(codexHome, path.slice(13))
    if (path === "~") return home
    if (path.indexOf("~/") === 0) return context.paths.join(home, path.slice(2))
    return context.paths.canonical(path) ? path : ""
  }

  function activate(item) {
    var path = absoluteSource(item)
    if (path !== "" && files && files.openDefault) files.openDefault(path, context.screen, false)
  }

  function reveal(item) {
    var path = absoluteSource(item)
    if (files && context.paths.canonical(path)) files.navigateToLocation(context.paths.parent(path), context.screen, "browse")
  }

  onProviderChanged: syncProvider()
  onContextChanged: syncProvider()
  onInventoryActiveChanged: syncProvider()
  Component.onCompleted: syncProvider()
  Component.onDestruction: if (attachedProvider) attachedProvider.detach(attachedContext)

  Keys.onPressed: function(event) {
    if (rescanKey.accepts(event.key, event.modifiers)) {
      root.rescan()
      event.accepted = true
    }
  }

  Components.RescanKeyContract { id: rescanKey }

  Loader {
    id: viewLoader
    source: root.context && root.context.ui ? root.context.ui.url("PaneView") : ""
  }

  Binding { target: viewLoader.item; property: "context"; value: root.context; when: !!viewLoader.item }
  Binding { target: viewLoader.item; property: "options"; value: root.metricOptions; when: !!viewLoader.item }
  Binding { target: viewLoader.item; property: "defaultMetric"; value: "uses"; when: !!viewLoader.item }
  Binding { target: viewLoader.item; property: "activityOption"; value: true; when: !!viewLoader.item }

  Loader {
    id: headerLoader
    anchors.top: searchLoader.bottom
    anchors.topMargin: searchLoader.height > 0 ? Style.space(4) : 0
    anchors.left: parent.left
    anchors.right: parent.right
    source: root.context && root.context.ui ? root.context.ui.url("PaneHeader") : ""
  }

  Binding { target: headerLoader.item; property: "context"; value: root.context; when: !!headerLoader.item }
  Binding { target: headerLoader.item; property: "view"; value: root.view; when: !!headerLoader.item }
  Binding { target: headerLoader.item; property: "title"; value: "MCP"; when: !!headerLoader.item }
  Binding { target: headerLoader.item; property: "status"; value: root.status; when: !!headerLoader.item }
  Binding { target: headerLoader.item; property: "tabIndex"; value: root.context ? root.context.tabIndex : -1; when: !!headerLoader.item }
  Binding { target: headerLoader.item; property: "reservedLeft"; value: root.context ? root.context.cornerReserveLeft : 0; when: !!headerLoader.item }
  Binding { target: headerLoader.item; property: "reservedRight"; value: (root.context ? root.context.cornerReserveRight : 0) + Style.space(28); when: !!headerLoader.item }

  Rectangle {
    anchors.right: parent.right
    anchors.rightMargin: Style.space(7) + (root.context ? root.context.cornerReserveRight : 0)
    anchors.verticalCenter: headerLoader.verticalCenter
    width: Style.space(24)
    height: Style.space(24)
    visible: !!headerLoader.item && headerLoader.item.visible
    radius: Math.min(Style.cornerRadius, Style.space(4))
    color: refreshPointer.containsMouse ? Util.alpha(Commons.Color.accent, 0.22) : "transparent"

    Components.PlainText {
      anchors.centerIn: parent
      text: root.busy || root.applying ? "…" : "↻"
      color: refreshPointer.containsMouse ? Commons.Color.accent : Commons.Color.muted
    }

    MouseArea {
      id: refreshPointer
      anchors.fill: parent
      hoverEnabled: true
      cursorShape: Qt.PointingHandCursor
      onClicked: root.rescan()
    }
  }

  Loader {
    id: searchLoader
    height: item && item.visible ? Style.space(32) : 0
    anchors.top: parent.top
    anchors.topMargin: height > 0 ? Style.space(6) : 0
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.leftMargin: Style.space(7)
    anchors.rightMargin: Style.space(7)
    active: root.inventoryActive
    source: active && root.context && root.context.ui ? root.context.ui.url("PaneSearchField") : ""
  }

  Binding { target: searchLoader.item; property: "text"; value: root.query; when: !!searchLoader.item }
  Binding { target: searchLoader.item; property: "context"; value: root.context; when: !!searchLoader.item }
  Binding { target: searchLoader.item; property: "showOptions"; value: true; when: !!searchLoader.item }
  Binding { target: searchLoader.item; property: "caseSensitive"; value: root.caseSensitive; when: !!searchLoader.item }
  Binding { target: searchLoader.item; property: "regex"; value: root.regex; when: !!searchLoader.item }
  Binding { target: searchLoader.item; property: "tabTarget"; value: heatmapLoader.item && heatmapLoader.item.visible ? heatmapLoader.item : treeLoader.item; when: !!searchLoader.item }
  Binding { target: searchLoader.item; property: "prompt"; value: "Filter MCP…"; when: !!searchLoader.item }
  Connections {
    target: searchLoader.item
    ignoreUnknownSignals: true
    function onTextChanged() { if (root.query !== String(target.text)) root.query = String(target.text) }
    function onOptionsToggled(nextCase, nextRegex) {
      root.caseSensitive = nextCase
      root.regex = nextRegex
    }
    function onDismissed() {
      if (root.query !== "") root.query = ""
      if (treeLoader.item) treeLoader.item.forceActiveFocus()
    }
    function onAdvanced() { if (treeLoader.item) treeLoader.item.forceActiveFocus() }
    function onAccepted() { if (treeLoader.item) treeLoader.item.activateCurrent() }
  }

  Loader {
    id: heatmapLoader
    onActiveChanged: if (!active && root.inventoryActive && treeLoader.item) treeLoader.item.forceActiveFocus()
    height: item && item.visible ? item.implicitHeight : 0
    anchors.top: headerLoader.bottom
    anchors.topMargin: height > 0 ? Style.space(4) : 0
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.leftMargin: Style.space(7)
    anchors.rightMargin: Style.space(7)
    active: root.inventoryActive && !!root.view && root.view.activity && root.height >= Style.space(300)
    source: active && root.context && root.context.ui ? root.context.ui.url("UsageHeatmap") : ""
  }

  Binding { target: heatmapLoader.item; property: "inventory"; value: root.inventory; when: !!heatmapLoader.item }
  Binding { target: heatmapLoader.item; property: "unitLabel"; value: "MCP calls"; when: !!heatmapLoader.item }
  Connections {
    target: heatmapLoader.item
    ignoreUnknownSignals: true
    function onDismissed() { if (treeLoader.item) treeLoader.item.forceActiveFocus() }
    function onPreviousRequested() { if (searchLoader.item) searchLoader.item.reveal() }
  }

  Loader {
    id: binLoader
    anchors.fill: parent
    z: 60
    readonly property bool wanted: root.inventoryActive && !!root.files
    onWantedChanged: sync()
    Component.onCompleted: sync()
    function sync() {
      if (wanted) setSource(root.context.ui.url("ArtifactBin"), { service: root.files })
      else source = ""
    }
    onLoaded: {
      item.module = "mcp"
      item.context = Qt.binding(function() { return root.context })
      item.describe = function(entry) { return root.binItem(entry) }
      item.helperRoute = Qt.binding(function() {
        return root.inventory ? { provider: root.inventory.providerId, directory: "", helper: root.inventory.helperId } : null
      })
      item.removalArguments = function(entry) {
        return ["--project", root.projectPath, "--id", String(entry.id), "--json"]
      }
      item.changed.connect(function() {
        root.viewError = item.error
        if (root.inventoryActive && treeLoader.item) treeLoader.item.forceActiveFocus()
        if (root.inventory) root.inventory.refresh()
      })
    }
  }

  Loader {
    id: treeLoader
    anchors.top: heatmapLoader.bottom
    anchors.topMargin: Style.space(6)
    anchors.left: parent.left
    anchors.right: parent.right
    anchors.bottom: parent.bottom
    active: root.inventoryActive
    source: active && root.context && root.context.ui ? root.context.ui.url("ArtifactTree") : ""
    onLoaded: {
      item.context = Qt.binding(function() { return root.context })
      item.editPathFor = function(entry) { return root.absoluteSource(entry) }
      item.fileActionsFor = function(entry) { return false }
      item.expandableItems = true
      item.childMetrics = true
      item.expansionKey = function(entry) { return Array.isArray(entry.observed) && entry.observed.length > 0 ? String(entry.id || "") : "" }
      item.childrenFor = function(entry) { return root.observedRows(entry) }
      item.leafGlyph = function(entry) { return entry.observedKind ? root.observedGlyphs[entry.observedKind] : item.defaultGlyph(entry) }
      item.changed.connect(function() { root.refresh() })
      item.groupsFor = function(entry) {
        return root.isBinned(entry) ? binLoader.item.groupFor(entry) : [entry.agent, entry.scope === "plugin" ? "user" : entry.scope]
      }
      item.leafLabel = function(entry) { return String(entry.name || "") + (root.isBinned(entry) ? binLoader.item.stateSuffix(entry) : "") }
      item.leafDetail = function(entry) {
        if (entry.observedKind) return entry.metrics.failed > 0 ? entry.metrics.failed + " failed" : ""
        if (root.isBinned(entry)) return String(entry.detail || "")
        return String(entry.source.path || "") + ", " + String(entry.transport || "unknown")
      }
      item.rowAction = function(entry) { return binLoader.item ? binLoader.item.rowAction(entry) : null }
      item.specialMetricValue = function(entry, key) { return root.specialMetricValue(entry, key) }
      item.appliedAgents = function(entry) { return root.appliedAgents(entry) }
      item.searchText = function(entry) { return [entry.name, entry.agent, entry.scope, entry.transport, entry.state, root.absoluteSource(entry)].join(" ") }
      item.filterKeys = ["agent", "scope", "transport", "status"]
      item.searchFields = function(entry) { return ({ agent: entry.agentId || entry.agent, scope: entry.scope, transport: entry.transport, status: entry.state }) }
    }
  }

  Binding { target: treeLoader.item; property: "items"; value: root.allRows(); when: !!treeLoader.item }
  Binding { target: treeLoader.item; property: "query"; value: root.query; when: !!treeLoader.item }
  Binding { target: treeLoader.item; property: "caseSensitive"; value: root.caseSensitive; when: !!treeLoader.item }
  Binding { target: treeLoader.item; property: "regex"; value: root.regex; when: !!treeLoader.item }
  Binding { target: treeLoader.item; property: "view"; value: root.view; when: !!treeLoader.item }
  Binding { target: treeLoader.item; property: "installedAgents"; value: root.installedAgents; when: !!treeLoader.item }

  Connections {
    target: treeLoader.item
    ignoreUnknownSignals: true
    function onActivated(item) { root.activate(item) }
    function onRevealed(item) { root.reveal(item) }
    function onSearchRequested() { if (searchLoader.item) searchLoader.item.reveal() }
    function onFilterRequested() { if (headerLoader.item && headerLoader.item.openFilter) headerLoader.item.openFilter() }
    function onAgentToggled(entry, agentId, on) { root.toggleAgent(entry, agentId, on) }
    function onAgentsAllRequested(entry, on) { root.toggleAllAgents(entry, on) }
    function onActionRequested(entry) { if (binLoader.item) binLoader.item.ask(entry) }
    function onFocusNextRequested() { root.context.focusNext() }
    function onFocusPreviousRequested() { root.context.focusPrevious() }
    function onDismissRequested() { root.context.closeBlade() }
  }

  Components.PlainText {
    anchors.centerIn: treeLoader
    width: Math.max(0, treeLoader.width - Style.space(32))
    visible: root.inventoryActive && !root.busy && root.definitions.length === 0
    text: root.loadError || "No supported MCP configuration found"
    color: root.loadError ? Commons.Color.urgent : Commons.Color.muted
    horizontalAlignment: Text.AlignHCenter
    wrapMode: Text.WordWrap
  }
}
