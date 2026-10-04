.pragma library

var BESIDE_FILES = "beside-files"
var MAXIMUM_RECORDED = 256
var MAXIMUM_TABS = 32

function normalize(value) {
  return String(value || "").trim().toLowerCase() === BESIDE_FILES ? BESIDE_FILES : ""
}

function recorded(value) {
  var list = Array.isArray(value) ? value : []
  var result = []
  for (var i = 0; i < list.length && result.length < MAXIMUM_RECORDED; i++) {
    var id = String(list[i] || "").trim()
    if (id !== "" && id.length <= 128 && !/[\u0000-\u001f\u007f]/.test(id) && result.indexOf(id) < 0) result.push(id)
  }
  return result
}

function locate(layout, moduleId) {
  var edges = ["left", "right"]
  for (var e = 0; e < edges.length; e++) {
    var blade = layout && layout[edges[e]]
    var slots = blade && Array.isArray(blade.slots) ? blade.slots : []
    for (var s = 0; s < slots.length; s++) {
      var tabs = Array.isArray(slots[s].modules) ? slots[s].modules : []
      for (var t = 0; t < tabs.length; t++)
        if (String(tabs[t].module || "") === moduleId) return { edge: edges[e], index: s, tab: t }
    }
  }
  return null
}

function targetSlot(layout, placed) {
  var files = locate(layout, "files")
  if (files) return files
  for (var i = placed.length - 1; i >= 0; i--) {
    var sibling = locate(layout, placed[i])
    if (sibling) return sibling
  }
  return null
}

function candidates(modules, order) {
  var ids = Array.isArray(order) ? order : Object.keys(modules || {})
  var result = []
  for (var i = 0; i < ids.length; i++) {
    var entry = modules ? modules[ids[i]] : null
    if (!entry || entry.compatible === false || entry.id === "files") continue
    if (normalize(entry.defaultPlacement) === BESIDE_FILES) result.push(String(entry.id))
  }
  return result
}

function plan(layout, modules, order, placedBefore) {
  var placed = recorded(placedBefore)
  var wanted = candidates(modules, order)
  var next = JSON.parse(JSON.stringify(layout || { left: { slots: [] }, right: { slots: [] } }))
  if (!next.left) next.left = { open: false, slots: [] }
  if (!Array.isArray(next.left.slots)) next.left.slots = []
  var added = []
  var noted = []
  for (var i = 0; i < wanted.length; i++) {
    var id = wanted[i]
    if (placed.indexOf(id) >= 0) continue
    if (placed.length >= MAXIMUM_RECORDED) break
    if (locate(next, id)) {
      placed.push(id)
      noted.push(id)
      continue
    }
    var target = targetSlot(next, placed)
    var tab = { module: id, state: {} }
    if (!target) {
      next.left.slots.push({ modules: [tab], active: 0, collapsed: false, fraction: -1 })
    } else {
      var slot = next[target.edge].slots[target.index]
      if (slot.modules.length < MAXIMUM_TABS) slot.modules.push(tab)
      else next[target.edge].slots.splice(target.index + 1, 0, { modules: [tab], active: 0, collapsed: false, fraction: -1 })
    }
    placed.push(id)
    added.push(id)
  }
  return { layout: next, placed: placed, added: added, noted: noted, changed: added.length > 0 || noted.length > 0 }
}
