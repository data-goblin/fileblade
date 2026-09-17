import QtQuick
import Quickshell.Hyprland
import "../lib/MonitorMode.js" as MonitorMode
import Quickshell

Item {
  required property var host
  readonly property var config: host.config
  readonly property var legacyDefaults: host.legacyDefaults
  readonly property int minimumWidth: host.minimumWidth
  readonly property int slotHandleSize: host.slotHandleSize
  readonly property var layout: host.layout
  readonly property var edges: host.edges
  readonly property var registry: host.registry
  readonly property var activeSlots: host.activeSlots
  readonly property var windowAddresses: host.windowAddresses
  readonly property string monitorMode: host.monitorMode
  readonly property bool animateBlades: host.animateBlades
  readonly property real fontScale: host.fontScale
  readonly property int maximumSlots: 64
  readonly property int maximumTabsPerSlot: 32
  readonly property int maximumIdentifierLength: 128
  readonly property int maximumSlotStateBytes: 96 * 1024
  readonly property int maximumLayoutBytes: 240 * 1024
  readonly property int maximumStateKeyLength: 128

  function normalizeEdge(value) {
    return String(value || "").toLowerCase() === "right" ? "right" : "left"
  }

  function normalizeMonitorMode(value) {
    return MonitorMode.normalize(value)
  }

  readonly property string focusedMonitorName: Hyprland.focusedMonitor ? String(Hyprland.focusedMonitor.name || "") : ""
  readonly property string primaryScreenName: Quickshell.screens.length > 0 ? String(Quickshell.screens[0].name || "") : ""
  readonly property string monitorLock: host.monitorLock
  property var openedOn: ({ left: "", right: "" })
  readonly property var screenNames: {
    var names = []
    for (var i = 0; i < Quickshell.screens.length; i++) names.push(String(Quickshell.screens[i].name || ""))
    return names
  }

  function openedOnFor(edge) {
    return String(openedOn[normalizeEdge(edge)] || "")
  }

  function noteOpened(edge) {
    var target = normalizeEdge(edge)
    var next = { left: openedOn.left, right: openedOn.right }
    next[target] = MonitorMode.invocationName(monitorMode, { lock: monitorLock, focused: focusedMonitorName })
    openedOn = next
  }

  function noteClosed(edge) {
    var target = normalizeEdge(edge)
    var next = { left: openedOn.left, right: openedOn.right }
    next[target] = ""
    openedOn = next
  }

  property string lastFocusedMonitorName: ""

  function adoptInvocationScreens() {
    var next = { left: openedOn.left, right: openedOn.right }
    var changed = false
    for (var i = 0; i < edges.length; i++) {
      var edge = edges[i]
      var wanted = isOpen(edge) ? MonitorMode.invocationName(monitorMode, { lock: monitorLock, focused: focusedMonitorName }) : ""
      if (monitorMode === "active" && isOpen(edge) && next[edge] !== "") continue
      if (next[edge] !== wanted) { next[edge] = wanted; changed = true }
    }
    if (changed) openedOn = next
  }

  onFocusedMonitorNameChanged: {
    var previous = lastFocusedMonitorName
    lastFocusedMonitorName = focusedMonitorName
    if (previous === "" && focusedMonitorName !== "") adoptInvocationScreens()
  }
  onMonitorModeChanged: adoptInvocationScreens()
  onMonitorLockChanged: adoptInvocationScreens()
  onLayoutChanged: adoptInvocationScreens()

  function normalizeMode(value) {
    return String(value || "").toLowerCase() === "window" ? "window" : "docked"
  }

  function clampNumber(value, fallback, minimum, maximum) {
    var parsed = Number(value)
    if (!isFinite(parsed)) return fallback
    return Math.max(minimum, Math.min(maximum, parsed))
  }

  function emptyBlade(edge) {
    return { open: false, width: edge === "right" ? 360 : 380, mode: "docked", slots: [] }
  }

  function slotIdFor(module, index, taken) {
    var base = String(module || "slot").replace(/[^A-Za-z0-9_.-]+/g, "-").slice(0, maximumIdentifierLength - 8) || "slot"
    var candidate = base
    var serial = 1
    while (taken[candidate]) {
      serial++
      candidate = base + "-" + serial
    }
    return candidate
  }

  readonly property var moduleAliases: ({
    "kurt.notes/notes": "notes",
    "kurt.git/git": "data-goblin.fileblade-git/git",
    "kurt.agent-memory/memory": "data-goblin.fileblade-memory/memory",
    "kurt.agent-skills/skills": "data-goblin.fileblade-skills/skills",
    "kurt.agent-hooks/hooks": "data-goblin.fileblade-hooks/hooks",
    "kurt.agent-mcp/mcp": "data-goblin.fileblade-mcp/mcp"
  })

  function aliasedModule(value) {
    var name = String(value || "").trim()
    if (!name || name.length > maximumIdentifierLength || /[\u0000-\u001f\u007f]/.test(name)) return ""
    return moduleAliases[name] || name
  }

  function normalizeTab(raw) {
    var source = raw && typeof raw === "object" ? raw : { module: raw }
    var module = aliasedModule(source.module)
    if (!module) return null
    var state = source.state && typeof source.state === "object" && !Array.isArray(source.state) ? source.state : ({})
    var stateText = serialized(state, 0)
    if (!stateText || utf8Length(stateText) > maximumSlotStateBytes) state = ({})
    return { module: module, state: state }
  }

  function normalizeTabs(source) {
    var list = []
    var raw = Array.isArray(source.modules) ? source.modules : [source]
    for (var i = 0; i < raw.length && i < maximumTabsPerSlot; i++) {
      var tab = normalizeTab(raw[i])
      if (tab) list.push(tab)
    }
    return list
  }

  function normalizeSlot(raw, index, taken) {
    var source = raw && typeof raw === "object" ? raw : { module: raw }
    var tabs = normalizeTabs(source)
    if (tabs.length === 0) return null
    var active = Math.max(0, Math.min(tabs.length - 1, Math.floor(Number(source.active) || 0)))
    var id = String(source.id || "").trim()
    if (!/^[A-Za-z0-9][A-Za-z0-9._-]{0,127}$/.test(id)) id = slotIdFor(tabs[0].module, index, taken)
    if (taken[id]) id = slotIdFor(tabs[0].module, index, taken)
    taken[id] = true
    var fraction = Number(source.fraction)
    return {
      id: id,
      modules: tabs,
      active: active,
      collapsed: source.collapsed === true,
      fraction: isFinite(fraction) && fraction > 0 && fraction < 1 ? fraction : -1
    }
  }

  function normalizeBlade(raw, edge) {
    var base = emptyBlade(edge)
    if (!raw || typeof raw !== "object") return base
    var taken = ({})
    var slots = []
    var rawSlots = Array.isArray(raw.slots) ? raw.slots : []
    for (var i = 0; i < rawSlots.length && i < maximumSlots; i++) {
      var slot = normalizeSlot(rawSlots[i], i, taken)
      if (slot) slots.push(slot)
    }
    return {
      open: typeof raw.open === "boolean" ? raw.open : base.open,
      width: Math.round(clampNumber(raw.width, base.width, minimumWidth, 1600)),
      mode: normalizeMode(raw.mode),
      slots: slots
    }
  }

  function normalizeLayout(raw) {
    var source = raw && typeof raw === "object" ? raw : ({})
    var blades = source.blades && typeof source.blades === "object" ? source.blades : source
    return {
      left: normalizeBlade(blades.left, "left"),
      right: normalizeBlade(blades.right, "right")
    }
  }

  function cloneLayout(value) {
    return JSON.parse(JSON.stringify(value))
  }

  function utf8Length(value) {
    var text = String(value || "")
    var bytes = 0
    for (var i = 0; i < text.length; i++) {
      var code = text.charCodeAt(i)
      if (code < 0x80) bytes++
      else if (code < 0x800) bytes += 2
      else if (code >= 0xD800 && code <= 0xDBFF
          && i + 1 < text.length
          && text.charCodeAt(i + 1) >= 0xDC00 && text.charCodeAt(i + 1) <= 0xDFFF) {
        bytes += 4
        i++
      } else bytes += 3
    }
    return bytes
  }

  function serialized(value, indentation) {
    try {
      var text = JSON.stringify(value, null, indentation || 0)
      return typeof text === "string" ? text : ""
    } catch (error) {
      return ""
    }
  }

  function validStateKey(key) {
    return key.length > 0 && key.length <= maximumStateKeyLength
      && !/[\u0000-\u001f\u007f]/.test(key)
  }

  function stateWithValue(current, key, value) {
    var source = current && typeof current === "object" && !Array.isArray(current) ? current : ({})
    var stateText = serialized(source, 0)
    var state = stateText ? JSON.parse(stateText) : ({})
    if (value === undefined || value === null) delete state[key]
    else {
      var valueText = serialized(value, 0)
      if (!valueText) return null
      state[key] = JSON.parse(valueText)
    }
    stateText = serialized(state, 0)
    return stateText && utf8Length(stateText) <= maximumSlotStateBytes ? state : null
  }

  function layoutWithinLimit(next) {
    var text = serialized({ version: 1, monitorMode: monitorMode, monitorLock: monitorLock, animations: animateBlades, fontScale: fontScale, blades: next }, 2)
    return !!text && utf8Length(text + "\n") <= maximumLayoutBytes
  }

  function setSlotStateValue(edge, slotId, key, value) {
    var target = normalizeEdge(edge)
    var index = findSlotId(target, slotId)
    var stateKey = String(key || "")
    if (index < 0 || !validStateKey(stateKey)) return false
    var next = cloneLayout(layout)
    var slot = next[target].slots[index]
    var active = Math.max(0, Math.min(slot.modules.length - 1, Number(slot.active) || 0))
    var state = stateWithValue(slot.modules[active].state, stateKey, value)
    if (!state) return false
    slot.modules[active].state = state
    if (!layoutWithinLimit(next)) return false
    host.replaceLayout(next, true)
    return true
  }

  function setTabStateValue(edge, slotIndex, tabIndex, key, value) {
    var target = normalizeEdge(edge)
    var index = Number(slotIndex)
    var tab = Number(tabIndex)
    var stateKey = String(key || "")
    if (!validIndex(index, slots(target).length) || !validStateKey(stateKey)) return false
    var next = cloneLayout(layout)
    var slot = next[target].slots[index]
    if (!validIndex(tab, slot.modules.length)) return false
    var state = stateWithValue(slot.modules[tab].state, stateKey, value)
    if (!state) return false
    slot.modules[tab].state = state
    if (!layoutWithinLimit(next)) return false
    host.replaceLayout(next, true)
    return true
  }

  function normalizePlacement(value) {
    var placement = String(value || "").toLowerCase()
    if (placement === "top") placement = "above"
    if (placement === "bottom") placement = "below"
    return ["above", "right", "below", "none"].indexOf(placement) >= 0 ? placement : "below"
  }

  function legacyLayout(legacy) {
    var source = legacy && typeof legacy === "object" ? legacy : ({})
    var placement = normalizePlacement(source.propertiesPlacement)
    var fraction = clampNumber(source.propertiesVerticalFraction, 0.34, 0.18, 0.72)
    var open = typeof source.open === "boolean" ? source.open : false
    var files = { id: "files", module: "files", fraction: -1, state: {} }
    var properties = { id: "properties", module: "properties", fraction: fraction, state: {} }
    var leftBlade = { open: open, width: Math.round(clampNumber(source.sidebarWidth, 380, minimumWidth, 1600)), slots: [] }
    var rightBlade = { open: false, width: Math.round(clampNumber(source.propertiesBladeWidth, 360, minimumWidth, 1600)), slots: [] }
    if (placement === "right") {
      leftBlade.slots = [files]
      rightBlade.slots = [properties]
      rightBlade.open = open
    } else if (placement === "above") {
      leftBlade.slots = [properties, files]
    } else if (placement === "none") {
      leftBlade.slots = [files]
    } else {
      leftBlade.slots = [files, properties]
    }
    var notesSlot = { id: "notes", modules: host.welcomePending ? [{ module: "welcome" }, { module: "notes" }] : [{ module: "notes" }], active: 0 }
    rightBlade.slots.unshift(notesSlot)
    if (host.welcomePending) rightBlade.open = true
    return normalizeLayout({ left: leftBlade, right: rightBlade })
  }

  function defaultLayout() {
    if (config && config.blades && typeof config.blades === "object") return normalizeLayout(config.blades)
    return legacyLayout(legacyDefaults)
  }

  function bladeFor(edge) {
    var target = normalizeEdge(edge)
    var current = layout && layout[target] ? layout[target] : null
    return current ? current : emptyBlade(target)
  }

  function isOpen(edge) {
    return !!bladeFor(edge).open
  }

  function bladeMode(edge) {
    return normalizeMode(bladeFor(edge).mode)
  }

  function isWindowMode(edge) {
    return bladeMode(edge) === "window"
  }

  function isDockedOpen(edge) {
    return isOpen(edge) && !isWindowMode(edge)
  }

  function bladeState(edge) {
    if (!isOpen(edge)) return "closed"
    return isWindowMode(edge) ? "window" : "open"
  }

  function windowTitle(edge) {
    return "Omarchy FileBlade " + normalizeEdge(edge) + " blade"
  }

  function windowAddress(edge) {
    return String(windowAddresses[normalizeEdge(edge)] || "")
  }

  function bladeWidth(edge) {
    return Number(bladeFor(edge).width) || emptyBlade(normalizeEdge(edge)).width
  }

  function slots(edge) {
    var list = bladeFor(edge).slots
    return Array.isArray(list) ? list : []
  }

  function slotAt(edge, index) {
    var list = slots(edge)
    return index >= 0 && index < list.length ? list[index] : null
  }

  function slotCollapsed(edge, index) {
    var slot = slotAt(edge, Number(index))
    return !!(slot && slot.collapsed === true)
  }

  function setSlotCollapsed(edge, index, value) {
    var target = normalizeEdge(edge)
    var slotIndex = Number(index)
    if (!validIndex(slotIndex, slots(target).length)) return false
    var desired = !!value
    if (slotCollapsed(target, slotIndex) === desired) return true
    host.updateBlade(target, function(blade) { blade.slots[slotIndex].collapsed = desired }, true)
    return true
  }

  function toggleSlotCollapsed(edge, index) {
    return setSlotCollapsed(edge, index, !slotCollapsed(edge, index))
  }

  function findModule(moduleId) {
    var target = String(moduleId || "")
    for (var edgeIndex = 0; edgeIndex < edges.length; edgeIndex++) {
      var list = slots(edges[edgeIndex])
      for (var slotIndex = 0; slotIndex < list.length; slotIndex++) {
        var tabs = Array.isArray(list[slotIndex].modules) ? list[slotIndex].modules : []
        for (var tabIndex = 0; tabIndex < tabs.length; tabIndex++)
          if (String(tabs[tabIndex].module) === target)
            return { edge: edges[edgeIndex], index: slotIndex, tab: tabIndex }
      }
    }
    return null
  }

  function findSlotId(edge, slotId) {
    var list = slots(edge)
    for (var i = 0; i < list.length; i++) if (String(list[i].id) === String(slotId)) return i
    return -1
  }

  function normalizedFractions(edge) {
    var list = slots(edge)
    var result = []
    var known = 0
    var autos = 0
    for (var i = 0; i < list.length; i++) {
      var fraction = Number(list[i].fraction)
      if (isFinite(fraction) && fraction > 0 && fraction < 1) known += fraction
      else autos++
    }
    var share = autos > 0 ? Math.max(Math.max(0, 1 - known), autos / list.length) : 0
    var scale = known > 0 ? (1 - share) / known : 1
    for (var j = 0; j < list.length; j++) {
      var value = Number(list[j].fraction)
      if (isFinite(value) && value > 0 && value < 1) result.push(value * scale)
      else result.push(autos > 0 ? share / autos : 0)
    }
    return result
  }

  function slotGeometry(edge, stackHeight) {
    var list = slots(edge)
    var fractions = normalizedFractions(edge)
    var handleSpace = Math.max(0, list.length - 1) * slotHandleSize
    var slotSpace = Math.max(0, Number(stackHeight) - handleSpace)
    var desiredCollapsed = []
    var desiredCollapsedTotal = 0
    var expandedWeight = 0
    var expandedCount = 0
    for (var i = 0; i < list.length; i++) {
      var collapsed = list[i].collapsed === true
      var desired = collapsed ? host.collapsedSlotHeight : 0
      desiredCollapsed.push(desired)
      desiredCollapsedTotal += desired
      if (!collapsed) {
        expandedWeight += fractions[i]
        expandedCount++
      }
    }
    var collapsedScale = desiredCollapsedTotal > slotSpace && desiredCollapsedTotal > 0
      ? slotSpace / desiredCollapsedTotal
      : 1
    var heights = []
    var fixedUsed = 0
    for (var j = 0; j < list.length; j++) {
      var fixed = Math.round(desiredCollapsed[j] * collapsedScale)
      heights.push(fixed)
      fixedUsed += fixed
    }
    var expandedSpace = Math.max(0, slotSpace - fixedUsed)
    var usedExpanded = 0
    var cumulativeWeight = 0
    for (var k = 0; k < list.length; k++) {
      if (list[k].collapsed === true) continue
      var weight = expandedWeight > 0 ? fractions[k] : (expandedCount > 0 ? 1 / expandedCount : 0)
      cumulativeWeight += weight
      var target = expandedWeight > 0
        ? Math.round(expandedSpace * cumulativeWeight / expandedWeight)
        : Math.round(expandedSpace * cumulativeWeight)
      heights[k] = Math.max(0, target - usedExpanded)
      usedExpanded = target
    }
    var tops = []
    var y = 0
    for (var m = 0; m < heights.length; m++) {
      tops.push(y)
      y += heights[m] + (m < heights.length - 1 ? slotHandleSize : 0)
    }
    return { tops: tops, heights: heights, expandedSpace: expandedSpace, expandedWeight: expandedWeight }
  }

  function slotTops(edge, stackHeight) {
    return slotGeometry(edge, stackHeight).tops
  }

  function slotTitle(edge, index) {
    return moduleTitle(slotModuleAt(edge, index, -1))
  }

  function moduleTitle(moduleId) {
    var module = registry.module(String(moduleId || ""))
    return module ? String(module.name) : String(moduleId || "")
  }

  function slotTabs(edge, index) {
    var slot = slotAt(edge, index)
    return slot && Array.isArray(slot.modules) ? slot.modules : []
  }

  function slotActiveTab(edge, index) {
    var slot = slotAt(edge, index)
    var count = slot && Array.isArray(slot.modules) ? slot.modules.length : 0
    return count > 0 ? Math.max(0, Math.min(count - 1, Math.floor(Number(slot.active) || 0))) : 0
  }

  function slotModuleAt(edge, index, tabIndex) {
    var tabs = slotTabs(edge, index)
    if (tabs.length === 0) return ""
    var wanted = Number(tabIndex)
    if (!isFinite(wanted) || wanted < 0) wanted = slotActiveTab(edge, index)
    return String((tabs[Math.min(wanted, tabs.length - 1)] || tabs[0]).module)
  }

  function propertiesPlacement() {
    var location = findModule("properties")
    if (!location) return "none"
    if (location.edge === "right") return "right"
    var filesLocation = findModule("files")
    if (filesLocation && filesLocation.edge === "left" && filesLocation.index > location.index) return "above"
    return "below"
  }

  function layoutDocument() {
    return { version: 1, monitorMode: monitorMode, monitorLock: monitorLock, animations: animateBlades, fontScale: fontScale, blades: cloneLayout(layout) }
  }

  function activeSlot(edge) {
    var target = normalizeEdge(edge)
    var index = Number(activeSlots[target]) || 0
    var count = slots(target).length
    return count === 0 ? 0 : Math.max(0, Math.min(count - 1, index))
  }

  function panelActiveFor(panelScreen, edge) {
    if (!panelScreen) return false
    var name = String(panelScreen.name || "")
    if (edge !== undefined && edge !== null && edge !== "")
      return MonitorMode.eligible(monitorMode, name, { lock: monitorLock, openedOn: openedOnFor(edge) })
    for (var i = 0; i < edges.length; i++)
      if (MonitorMode.eligible(monitorMode, name, { lock: monitorLock, openedOn: openedOnFor(edges[i]) })) return true
    return monitorMode !== "active" && MonitorMode.eligible(monitorMode, name, { lock: monitorLock, openedOn: "" })
  }

  function screenNamed(name) {
    var wanted = String(name || "")
    for (var i = 0; i < Quickshell.screens.length; i++)
      if (wanted !== "" && String(Quickshell.screens[i].name) === wanted) return Quickshell.screens[i]
    return null
  }

  function preferredScreen(edge) {
    var openedName = ""
    if (edge !== undefined && edge !== null && edge !== "") openedName = isOpen(edge) ? openedOnFor(edge) : ""
    else for (var i = 0; i < edges.length; i++) if (isOpen(edges[i]) && openedOnFor(edges[i]) !== "") { openedName = openedOnFor(edges[i]); break }
    var named = screenNamed(MonitorMode.preferredName(monitorMode, { openedOn: openedName, lock: monitorLock, focused: focusedMonitorName, primary: primaryScreenName }))
    if (named) return named
    return monitorMode === "all" && Quickshell.screens.length > 0 ? Quickshell.screens[0] : null
  }

  function referenceScreen(candidate, edge) {
    if (candidate && panelActiveFor(candidate, edge)) return candidate
    var preferred = preferredScreen(edge)
    if (preferred) return preferred
    return Quickshell.screens.length > 0 ? Quickshell.screens[0] : null
  }

  function configuredEdges() {
    var result = []
    for (var i = 0; i < edges.length; i++) if (slots(edges[i]).length > 0) result.push(edges[i])
    return result
  }

  function validIndex(index, length) {
    return isFinite(index) && Math.floor(index) === index && index >= 0 && index < length
  }

  function validMoveTab(tabIndex, length) {
    var wanted = Number(tabIndex)
    return !isFinite(wanted) || wanted < 0 || validIndex(wanted, length)
  }

  function wholeSlotMove(tabIndex, length) {
    var wanted = Number(tabIndex)
    return length === 1 || !isFinite(wanted) || wanted < 0
  }

  function targetSlotIndex(value, length) {
    var position = Number(value)
    return !isFinite(position) || Math.floor(position) !== position || position < 0 ? length : Math.min(position, length)
  }

}
