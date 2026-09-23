import QtQuick
import Quickshell
import Quickshell.Io
import Quickshell.Wayland
import qs.Commons

Item {
  id: probe
  required property var service
  property bool barVisible: false
  property string goblinsDirectory: Quickshell.env("FILEBLADE_GOBLINS_FIXTURE")
  property var goblinsManifest: null
  property string fixtureError: ""

  function objects() {
    var result = [], queue = [service, barWindow], seen = []
    while (queue.length && seen.length < 20000) {
      var item = queue.shift()
      if (!item || seen.indexOf(item) >= 0) continue
      seen.push(item)
      result.push(item)
      for (var key of ["children", "data", "instances"]) {
        var entries = item[key]
        if (entries && entries.length !== undefined)
          for (var i = 0; i < entries.length; i++) queue.push(entries[i])
      }
      for (var child of ["item", "contentItem"]) if (item[child]) queue.push(item[child])
    }
    return result
  }

  function geometry(item, hostWindow) {
    var point = item.mapToItem(null, 0, 0)
    var window = hostWindow || item.hostWindow
    if (window) {
      point.x += Number(window.surfaceOriginX) || 0
      point.y += Number(window.surfaceOriginY) || 0
    }
    return { x: point.x, y: point.y, width: item.width, height: item.height }
  }

  function snapshot() {
    var slots = [], popouts = [], stacks = [], tabs = [], dragCards = [], surfaces = []
    for (var item of objects()) {
      if (item.liveBarSize !== undefined && item.bladeOpen && item.surfaceOriginY !== undefined)
        surfaces.push({ edge: item.edge, barHidden: item.host.shell.bar.barHidden, liveBarSize: item.liveBarSize, surfaceOriginX: item.surfaceOriginX, surfaceOriginY: item.surfaceOriginY, width: item.width, height: item.height })
      if (item.slotIndex !== undefined && item.moduleItem !== undefined && item.loadFailed !== undefined && item.bladeOpen && item.hostActive) {
        slots.push({ edge: item.edge, index: item.slotIndex, module: item.moduleId,
          loaded: !!item.moduleItem, failed: item.loadFailed, title: item.title,
          providerError: item.providerError, geometry: geometry(item), collapsed: service.bladeHost.slotCollapsed(item.edge, item.slotIndex),
          inventory: inventorySnapshot(item.moduleItem) })
      }
      if (item.refreshActive && item.screenWidth !== undefined && item.active && item.contentItem) {
        for (var child of item.contentItem.children)
          if (child.visible && child.opacity > 0 && child.width > 0 && child.height > 0 && child.color !== undefined) dragCards.push(geometry(child))
      }
      if (item.tabItem && item.dropLineX && item.slot && item.slot.bladeOpen && item.visible) {
        var entries = []
        for (var index = 0; index < item.slot.tabs.length; index++) {
          var tab = item.tabItem(index)
          if (tab) entries.push({ module: tab.moduleId, geometry: geometry(tab, item.slot.hostWindow) })
        }
        tabs.push({ edge: item.slot.edge, slot: item.slot.slotIndex, entries: entries,
          dropIndex: item.dropIndex, lineX: item.dropLineX(), geometry: geometry(item, item.slot.hostWindow) })
      }
      if (item.moduleContext !== undefined && item.popoutState !== undefined)
        popouts.push({ module: item.moduleId, opened: item.opened, loaded: !!item.moduleItem, notice: item.notice, title: item.title })
      if (item.dropLineY && item.slotItem && item.bladeOpen)
        stacks.push({ edge: item.edge, geometry: geometry(item), dropVisible: item.dropVisible, dropLineY: item.dropLineY(), dropTabSlot: item.dropTabSlot, dropTabBand: item.dropTabBand })
    }
    return { slots: slots, popouts: popouts, stacks: stacks, surfaces: surfaces, tabs: tabs, dragCards: dragCards, barLoaded: !!widget.item,
      barOpened: widget.item ? widget.item.opened : false, fixtureError: fixtureError,
      providerErrors: service.bladeHost.providerErrors,
      drag: { active: service.bladeHost.dragActive, edge: service.bladeHost.dropEdge,
        index: service.bladeHost.dropIndex, tabSlot: service.bladeHost.dropTabSlot,
        tabBand: service.bladeHost.dropTabBand, tabIndex: service.bladeHost.dropTabIndex,
        noop: service.bladeHost.dropNoop } }
  }

  function serviceFor(id) { return id === "data-goblin.fileblade" ? service : service.services[id] || null }

  function inventorySnapshot(module) {
    var inventory = module ? module.inventory : null
    if (!inventory) return null
    return { ready: inventory.ready, busy: inventory.busy, count: inventory.items.length,
      pending: inventory.usagePending, activity: inventory.activity, error: inventory.loadError || inventory.watchError,
      rows: inventory.items.slice(0, 16).map(function(row) { return { id: row.id, name: row.name, uses: row.uses } }) }
  }

  Repeater {
    id: inventoryTimings
    model: ["skills", "mcp"]
    delegate: Item {
      id: timing
      required property string modelData
      readonly property var provider: probe.serviceFor("fileblade.core." + modelData)
      readonly property var inventory: provider ? provider.inventory : null
      property double openedAt: 0
      property double firstRowsAt: 0
      property double itemsAt: 0
      property double countsAt: 0
      property double activityAt: 0
      function opened() {
        if (!inventory || !inventory.ready) return
        openedAt = Date.now()
        firstRowsAt = inventory.items.length ? openedAt : 0
      }
      onInventoryChanged: opened()
      Connections {
        target: timing.inventory
        function onReadyChanged() { timing.opened() }
        function onItemsChanged() {
          timing.itemsAt = Date.now()
          if (timing.inventory.ready && !timing.firstRowsAt && timing.inventory.items.length)
            timing.firstRowsAt = timing.itemsAt
        }
        function onUsageCountsChanged() { timing.countsAt = Date.now() }
        function onActivityChanged() { timing.activityAt = Date.now() }
      }
    }
  }

  function inventories() {
    var result = ({})
    for (var index = 0; index < inventoryTimings.count; index++) {
      var timing = inventoryTimings.itemAt(index)
      if (!timing) continue
      var snapshot = inventorySnapshot(timing)
      if (!snapshot) continue
      result[timing.modelData] = Object.assign(snapshot, { openedAt: timing.openedAt,
        firstRowsAt: timing.firstRowsAt, itemsAt: timing.itemsAt, countsAt: timing.countsAt,
        activityAt: timing.activityAt, now: Date.now() })
    }
    return result
  }

  FileView {
    path: probe.goblinsDirectory ? probe.goblinsDirectory + "/manifest.json" : ""
    printErrors: false
    onLoaded: {
      try {
        probe.goblinsManifest = JSON.parse(text())
      } catch (error) { probe.fixtureError = String(error) }
    }
  }

  PanelWindow {
    id: barWindow
    visible: probe.barVisible
    anchors { top: true; left: true; right: true }
    implicitHeight: Style.bar.sizeHorizontal
    color: "transparent"
    mask: Region { x: 900; y: 0; width: 40; height: Style.bar.sizeHorizontal }
    exclusionMode: ExclusionMode.Ignore
    WlrLayershell.namespace: "fileblade-qualification-bar"
    WlrLayershell.layer: WlrLayer.Overlay
    WlrLayershell.keyboardFocus: WlrKeyboardFocus.None

    QtObject {
      id: facade
      property var shell: probe
      property bool vertical: false
      property string position: "top"
      property int barSize: Style.bar.sizeHorizontal
      property string fontFamily: Style.font.family
      property color barForeground: Color.foreground
      property color urgent: Color.urgent
      property bool foregroundAnimationEnabled: true
      property var activePopout: null
      property var clickTargets: []
      function registerClickTarget(item) { clickTargets = clickTargets.concat([item]) }
      function unregisterClickTarget(item) { clickTargets = clickTargets.filter(function(entry) { return entry !== item }) }
      function hideTooltip(item) {}
      function showTooltip(item, text) {}
      function requestPopout(item) { activePopout = item }
      function releasePopout(item) { if (activePopout === item) activePopout = null }
      function moduleWidgets(name) { return widget.item ? [widget.item] : [] }
    }

    Loader {
      id: widget
      x: 900
      width: 40
      height: Style.bar.sizeHorizontal
      onStatusChanged: if (status === Loader.Error) probe.fixtureError = "Goblins BarWidget import failed"
    }
  }

  IpcHandler {
    target: "fileblade.qualification"
    function status(): string { return JSON.stringify(probe.snapshot()) }
    function inventories(): string { return JSON.stringify(probe.inventories()) }
    function goblins(enabled: string): string {
      if (enabled === "true") {
        probe.service.extensionCatalog.providers = [{ id: probe.goblinsManifest.id, dir: probe.goblinsDirectory, enabled: true, manifest: probe.goblinsManifest }]
        probe.service.bladeHost.registry.rebuild()
        probe.barVisible = true
        widget.setSource(Util.fileUrl(probe.goblinsDirectory + "/GoblinBarWidget.qml"), {
          bar: facade, settings: { showNavbarIcon: true, width: 460, height: 600 }
        })
      } else {
        if (widget.item) widget.item.close()
        widget.source = ""
        probe.barVisible = false
      }
      return "ok"
    }
  }
}
