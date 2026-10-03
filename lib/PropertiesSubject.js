.pragma library
.import "ActionGlyphs.js" as ActionGlyphs

var VERSION = 1

var KINDS = ["text", "multiline", "tags", "link", "code"]

var LIMITS = {
  title: 160,
  subtitle: 240,
  glyph: 8,
  glyphFamily: 64,
  fields: 48,
  label: 48,
  text: 1024,
  multiline: 8192,
  code: 8192,
  link: 2048,
  tags: 32,
  tag: 64,
  actions: 8,
  actionId: 64,
  actionText: 40,
  budget: 32768
}

var COLOR_TOKENS = ["accent", "muted", "urgent", "text"]

function clip(text, limit) {
  if (text.length <= limit) return text
  var cut = text.slice(0, Math.max(0, limit - 1))
  var last = cut.charCodeAt(cut.length - 1)
  if (last >= 0xd800 && last <= 0xdbff) cut = cut.slice(0, -1)
  return cut + "…"
}

function raw(value) {
  if (value === undefined || value === null) return ""
  if (typeof value === "string") return value
  if (typeof value === "number" || typeof value === "boolean") return String(value)
  return ""
}

function line(value, limit) {
  var text = raw(value).replace(/[\t\n\r\u2028\u2029]+/g, " ").replace(/[\u0000-\u001f\u007f-\u009f]/g, "").trim()
  return clip(text, limit)
}

function block(value, limit) {
  var text = raw(value).replace(/\r\n?/g, "\n").replace(/[\u0000-\u0008\u000b-\u001f\u007f-\u009f]/g, "")
  text = text.replace(/^\n+|\s+$/g, "")
  return clip(text, limit)
}

function webLink(value) {
  var text = raw(value).trim()
  if (!text || text.length > LIMITS.link || /[\s\u0000-\u001f\u007f-\u009f]/.test(text)) return ""
  return /^https?:\/\/[^\/?#@\s]+([\/?#].*)?$/i.test(text) ? text : ""
}

function color(value) {
  var text = value === undefined || value === null ? "" : String(value).trim()
  if (/^#([0-9a-f]{3}|[0-9a-f]{6}|[0-9a-f]{8})$/i.test(text)) return text.toLowerCase()
  return COLOR_TOKENS.indexOf(text) >= 0 ? text : ""
}

function family(value) {
  var text = raw(value).trim()
  if (!text || text.length > LIMITS.glyphFamily || !/^[A-Za-z0-9 ._-]+$/.test(text)) return ""
  return text
}

function tags(value) {
  var list = Array.isArray(value) ? value : (raw(value) ? [value] : [])
  var result = []
  var seen = ({})
  var dropped = false
  for (var i = 0; i < list.length; i++) {
    var tag = line(list[i], LIMITS.tag)
    if (!tag || seen[tag]) continue
    if (result.length >= LIMITS.tags) { dropped = true; break }
    seen[tag] = true
    result.push(tag)
  }
  return { values: result, dropped: dropped }
}

function field(value) {
  if (!value || typeof value !== "object" || Array.isArray(value)) return null
  var kind = KINDS.indexOf(String(value.kind || "text")) >= 0 ? String(value.kind || "text") : "text"
  var label = line(value.label, LIMITS.label)
  if (!label) return null
  if (kind === "tags") {
    var chips = tags(value.value)
    if (chips.values.length === 0) return null
    return { label: label, kind: kind, value: chips.values, text: chips.values.join(", "), cost: chips.values.join("").length, dropped: chips.dropped }
  }
  if (kind === "link") {
    var url = webLink(value.value)
    if (!url) kind = "text"
    else return { label: label, kind: kind, value: url, text: url, cost: url.length, dropped: false }
  }
  var text = kind === "multiline" || kind === "code" ? block(value.value, LIMITS[kind]) : line(value.value, LIMITS.text)
  if (!text) return null
  var limit = kind === "multiline" || kind === "code" ? LIMITS[kind] : LIMITS.text
  return { label: label, kind: kind, value: text, text: text, cost: text.length, dropped: raw(value.value).length > limit }
}

function actions(value) {
  var list = Array.isArray(value) ? value : []
  var result = []
  var seen = ({})
  var dropped = false
  for (var i = 0; i < list.length; i++) {
    var entry = list[i]
    if (!entry || typeof entry !== "object") continue
    var id = raw(entry.id)
    if (!/^[A-Za-z0-9][A-Za-z0-9._-]*$/.test(id) || id.length > LIMITS.actionId || seen[id]) continue
    if (result.length >= LIMITS.actions) { dropped = true; break }
    seen[id] = true
    var mark = line(entry.glyph, LIMITS.glyph + 1)
    var own = mark !== "" && mark.length <= LIMITS.glyph
    result.push({
      id: id,
      text: line(entry.text, LIMITS.actionText) || clip(id, LIMITS.actionText),
      glyph: own ? mark : ActionGlyphs.glyph(id),
      glyphFamily: own ? family(entry.glyphFamily) : "",
      urgent: ActionGlyphs.urgent(id)
    })
  }
  return { values: result, dropped: dropped }
}

function normalize(subject) {
  if (!subject || typeof subject !== "object" || Array.isArray(subject)) return null
  var title = line(subject.title, LIMITS.title)
  if (!title) return null
  var list = Array.isArray(subject.fields) ? subject.fields : []
  var fields = []
  var budget = LIMITS.budget
  var truncated = false
  for (var i = 0; i < list.length; i++) {
    var next = field(list[i])
    if (!next) continue
    if (fields.length >= LIMITS.fields || next.cost > budget) { truncated = true; break }
    budget -= next.cost
    if (next.dropped) truncated = true
    fields.push({ label: next.label, kind: next.kind, value: next.value, text: next.text })
  }
  var buttons = actions(subject.actions)
  var glyph = line(subject.glyph, LIMITS.glyph + 1)
  return {
    title: title,
    subtitle: line(subject.subtitle, LIMITS.subtitle),
    glyph: glyph.length <= LIMITS.glyph ? glyph : "",
    glyphFamily: family(subject.glyphFamily),
    color: color(subject.color),
    fields: fields,
    actions: buttons.values,
    truncated: truncated || buttons.dropped
  }
}

function hasAction(subject, id) {
  if (!subject || !Array.isArray(subject.actions)) return false
  for (var i = 0; i < subject.actions.length; i++) if (subject.actions[i].id === String(id)) return true
  return false
}
