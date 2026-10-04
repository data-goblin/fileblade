import QtQuick
import Quickshell.Io

Item {
  id: probe
  property var subject: null

  function descendants(item, result) {
    if (!item || result.length >= 10000 || result.indexOf(item) >= 0) return
    result.push(item)
    var children = item.children || []
    for (var i = 0; i < children.length; i++) descendants(children[i], result)
    if (item.contentItem) descendants(item.contentItem, result)
    var data = item.data || []
    for (var i = 0; i < data.length; i++)
      if (data[i] && data[i].contentItem) descendants(data[i], result)
  }

  function point(item) {
    var p = item.mapToGlobal(item.width / 2, item.height / 2)
    return { x: Math.round(p.x), y: Math.round(p.y + (subject && subject.context && subject.context.docked ? subject.context.surfaceOriginY : 0)), width: item.width, height: item.height }
  }

  function snapshot() {
    var m = subject
    if (m && m.moduleName) {
      var children = []
      descendants(m, children)
      var ready = false
      for (var n = 0; n < children.length; n++)
        if (children[n].sourceSize !== undefined && children[n].status === Image.Ready) ready = true
      return { ready: ready, icon: m.iconUrl, point: point(m), opened: m.opened }
    }
    if (!m || !m.grid) return ({ ready: false })
    var grid = m.grid
    var nodes = []
    descendants(m, nodes)
    var tiles = []
    var icons = []
    var rail = null
    var size = null
    var surface = []
    descendants(m.context.hostWindow ? m.context.hostWindow.contentItem : m, surface)
    var surfaceIcons = []
    var preview = ""
    for (var n = 0; n < surface.length; n++) {
      var node = surface[n]
      if (node.pictorial !== undefined && node.visible)
        surfaceIcons.push({ url: node.iconUrl, glyph: node.glyph, color: String(node.color), point: point(node) })
      if (node.previewSource !== undefined) preview = String(node.previewSource)
    }
    for (var i = 0; i < nodes.length; i++) {
      var node = nodes[i]
      if (node.path !== undefined && node.pending !== undefined) {
        var p = point(node)
        var gp = grid.mapToGlobal(0, 0)
        gp.y += m.context.docked ? m.context.surfaceOriginY : 0
        if (p.y > gp.y && p.y < gp.y + grid.height)
          tiles.push({ path: node.path, source: node.source, loading: node.loading, error: node.error, current: node.current, point: p })
      }
      if (node.pictorial !== undefined && node.iconUrl !== undefined) {
        var images = []
        descendants(node, images)
        var ready = false
        for (var j = 0; j < images.length; j++)
          if (images[j].sourceSize !== undefined && images[j].status === Image.Ready) ready = true
        icons.push({ url: node.iconUrl, color: String(node.color), ready: ready })
      }
      if (node.trackX !== undefined) {
        var start = node.mapToGlobal(node.trackX, 0)
        rail = { x: Math.round(start.x), y: Math.round(start.y + (m.context.docked ? m.context.surfaceOriginY : 0)), height: node.height,
          years: node.years, months: node.months, fraction: node.fraction,
          thumbY: node.thumbY, thumbLength: node.thumbLength, trackHeight: node.trackHeight, engaged: node.engaged,
          pill: node.pointerSection ? node.pointerSection.shortLabel : "" }
      }
      if (node.shortcutSmaller !== undefined) size = { step: node.clamped, steps: node.steps, point: point(node) }
    }
    return { ready: !!m.provider && m.provider.loaded && !m.provider.busy, count: grid.count,
      schema: m.context.settings.schema, preview: preview, surfaceIcons: surfaceIcons, state: m.context.slotState, library: m.provider.library, query: m.query, status: m.status, size: m.sizeStep, stepper: size, icons: icons, tiles: tiles, rail: rail,
      sections: grid.layout.sections, current: grid.current, columns: grid.layout.columns,
      scroll: grid.flickable.contentY, grid: point(grid), active: m.active,
      pending: m.provider && m.provider.thumbnails ? Object.keys(m.provider.thumbnails.inflight).length : -1 }
  }

  IpcHandler {
    target: probe.subject && probe.subject.context ? "fileblade.gallery-test." + (probe.subject.context.inPopout ? "popout" : "blade") : (probe.subject && probe.subject.moduleName ? "fileblade.gallery-test.bar" : "")
    enabled: !!probe.subject && (!!probe.subject.moduleName || (!!probe.subject.context && !probe.subject.context.retired))
    function status(): string { return JSON.stringify(probe.snapshot()) }
  }
}
