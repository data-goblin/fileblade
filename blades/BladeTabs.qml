import QtQuick

Item {
  id: tabs

  required property var host

  function tabSeedState(moduleId) {
    if (String(moduleId) === "files" && host.service) return { root: String(host.service.rootPath || "") }
    return {}
  }

  function tabTitle(edge, slotIndex, tabIndex) {
    var list = host.slotTabs(edge, slotIndex)
    var index = Number(tabIndex)
    var tab = index >= 0 && index < list.length ? list[index] : null
    var moduleId = tab ? String(tab.module || "") : ""
    var state = tab && tab.state && typeof tab.state === "object" ? tab.state : ({})
    var label = String(state.label || "").trim()
    if (moduleId === "files" && label !== "") return label
    var root = String(state.root || "")
    if (moduleId === "files" && root !== "" && list.filter(function(item) { return String(item.module || "") === "files" }).length > 1) {
      var name = root.replace(/\/+$/, "").split("/").pop()
      return name || "/"
    }
    return host.moduleTitle(moduleId)
  }

  function detachTab(next, edge, slotIndex, tabIndex) {
    var slot = next[edge].slots[slotIndex]
    var list = slot.modules
    var wanted = Number(tabIndex)
    if (!isFinite(wanted) || wanted < 0 || wanted >= list.length || list.length === 1)
      return next[edge].slots.splice(slotIndex, 1)[0]
    var taken = list.splice(wanted, 1)[0]
    var active = Math.max(0, Math.min(list.length, Number(slot.active) || 0))
    if (wanted < active || active >= list.length) active--
    slot.active = Math.max(0, active)
    return { modules: [taken], active: 0, fraction: -1 }
  }

  function findSlotIn(list, slotId) {
    for (var i = 0; i < list.length; i++) if (list[i].id === slotId) return i
    return -1
  }

  function insertionPosition(value, length) {
    var position = Number(value)
    return isFinite(position) && position >= 0 ? Math.min(Math.floor(position), length) : length
  }

  function insertTabs(slot, moved, insertAt) {
    var position = insertionPosition(insertAt, slot.modules.length)
    for (var j = 0; j < moved.modules.length; j++) slot.modules.splice(position + j, 0, moved.modules[j])
    slot.active = position
  }

  function reorderTab(edge, slotIndex, tabIndex, insertAt) {
    var list = host.slotTabs(edge, slotIndex)
    var from = Number(tabIndex)
    if (!host.validIndex(from, list.length)) return false
    var position = insertionPosition(insertAt, list.length)
    if (position === from || position === from + 1) return false
    if (position > from) position--
    host.updateBlade(edge, function(blade) {
      var slot = blade.slots[slotIndex]
      var moved = slot.modules.splice(from, 1)[0]
      slot.modules.splice(position, 0, moved)
      slot.active = position
    }, true)
    return true
  }

  function moveTabInto(sourceEdge, sourceIndex, tabIndex, targetEdge, targetSlotIndex, insertAt) {
    var source = host.normalizeEdge(sourceEdge)
    var target = host.normalizeEdge(targetEdge)
    var from = Number(sourceIndex)
    var into = Number(targetSlotIndex)
    if (!host.validIndex(from, host.slots(source).length) || !host.validIndex(into, host.slots(target).length)) return false
    if (!host.validMoveTab(tabIndex, host.slots(source)[from].modules.length)) return false
    if (source === target && from === into) return reorderTab(source, from, tabIndex, insertAt)
    var next = host.cloneLayout(host.layout)
    var targetId = next[target].slots[into].id
    var moved = detachTab(next, source, from, tabIndex)
    var destination = findSlotIn(next[target].slots, targetId)
    if (destination < 0) return false
    insertTabs(next[target].slots[destination], moved, insertAt)
    if (next[source].slots.length === 0) next[source].open = false
    host.replaceLayout(next, true)
    return true
  }

  function sendTabAcross(edge, slotIndex, tabIndex, targetScreen) {
    var source = host.normalizeEdge(edge)
    var target = source === "left" ? "right" : "left"
    if (!host.moveSlotTo(source, slotIndex, target, 0, tabIndex)) return false
    Qt.callLater(function() { host.focusBlade(target, targetScreen, 0, "", true) })
    return true
  }

  function moveTabToSlot(edge, slotIndex, tabIndex, targetSlotIndex) {
    return moveTabInto(edge, slotIndex, tabIndex, edge, targetSlotIndex, undefined)
  }

  function removeTabsAfter(edge, slotIndex, tabIndex) {
    var target = host.normalizeEdge(edge)
    var index = Number(slotIndex)
    var list = host.slotTabs(target, index)
    var wanted = Number(tabIndex)
    if (!host.validIndex(wanted, list.length) || wanted === list.length - 1) return false
    host.updateBlade(target, function(blade) {
      var slot = blade.slots[index]
      slot.modules.splice(wanted + 1)
      slot.active = Math.max(0, Math.min(Number(slot.active) || 0, wanted))
    }, true)
    return true
  }

  function setSlotTab(edge, slotIndex, tabIndex) {
    var target = host.normalizeEdge(edge)
    var index = Number(slotIndex)
    var list = host.slotTabs(target, index)
    var wanted = Number(tabIndex)
    if (!host.validIndex(wanted, list.length)) return false
    if (host.slotActiveTab(target, index) === wanted) return true
    host.updateBlade(target, function(blade) { blade.slots[index].active = wanted }, true)
    return true
  }

  function cycleSlotTab(edge, slotIndex, delta) {
    var target = host.normalizeEdge(edge)
    var index = Number(slotIndex)
    var count = host.slotTabs(target, index).length
    if (count < 2) return false
    var step = Number(delta) < 0 ? -1 : 1
    return setSlotTab(target, index, (host.slotActiveTab(target, index) + step + count) % count)
  }

  function addTab(edge, slotIndex, moduleId, state) {
    var target = host.normalizeEdge(edge)
    var index = Number(slotIndex)
    if (!host.validIndex(index, host.slots(target).length)) return false
    var module = String(moduleId || "")
    if (!module) return false
    var entry = { module: module, state: state && typeof state === "object" ? state : ({}) }
    host.updateBlade(target, function(blade) {
      var slot = blade.slots[index]
      slot.modules.push(entry)
      slot.active = slot.modules.length - 1
    }, true)
    return true
  }

  function removeTab(edge, slotIndex, tabIndex) {
    var target = host.normalizeEdge(edge)
    var index = Number(slotIndex)
    var list = host.slotTabs(target, index)
    var wanted = Number(tabIndex)
    if (!host.validIndex(wanted, list.length)) return false
    if (list.length === 1) return host.removeSlot(target, index)
    host.updateBlade(target, function(blade) {
      var slot = blade.slots[index]
      var active = Math.max(0, Math.min(slot.modules.length - 1, Number(slot.active) || 0))
      slot.modules.splice(wanted, 1)
      if (wanted < active || active >= slot.modules.length) active--
      slot.active = Math.max(0, active)
    }, true)
    return true
  }
}
