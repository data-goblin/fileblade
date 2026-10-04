import QtQuick
import "../lib/SearchQuery.js" as SearchQuery
import "../lib/Highlight.js" as Highlight

Item {
  id: grammar
  visible: false
  width: 0
  height: 0

  readonly property int version: 1
  readonly property real halfLifeSeconds: 7 * 86400
  readonly property var syntax: [
    { shortcut: "rdme", text: "Fuzzy (fzf)" },
    { shortcut: "'word", text: "Substring" },
    { shortcut: "^start  end$", text: "Anchors" },
    { shortcut: "\"phrase\"", text: "Exact match" },
    { shortcut: "-word  !word", text: "Exclude" },
    { shortcut: "name:word", text: "Only the name" }
  ]

  function parse(query, filterKeys, options) { return SearchQuery.parse(query, filterKeys, options) }
  function matches(spec, record) { return SearchQuery.matches(spec, record) }
  function rank(spec, record) { return SearchQuery.rank(spec, record) }
  function compareRanks(left, right) { return SearchQuery.compareRanks(left, right) }
  function rankedTree(rows, spec, recordOf) { return SearchQuery.rankedTree(rows, spec, recordOf) }
  function rankedGroups(groups) { return SearchQuery.rankedGroups(groups) }

  function words(spec) {
    if (!spec || !Array.isArray(spec.terms)) return ""
    return spec.terms.filter(function(term) { return !term.negate && !term.pattern }).map(function(term) { return term.text }).join(" ")
  }

  function spans(text, spec) {
    var wanted = words(spec)
    return wanted === "" ? "" : Highlight.serializeSpans(Highlight.subsequenceSpans(text, wanted))
  }

  function markup(text, serialized, color) {
    return Highlight.markup(text, Highlight.parseSpans(serialized), color)
  }

  function frecency(entry, now) {
    if (!entry) return 0
    var score = Number(entry.score) || 0
    var last = Number(entry.last) || 0
    var moment = Number(now) || Math.floor(Date.now() / 1000)
    return score * Math.pow(2, -Math.max(0, moment - last) / halfLifeSeconds)
  }

  function ranked(query, records, options) {
    var settings = options && typeof options === "object" ? options : ({})
    var spec = SearchQuery.parse(query, settings.filterKeys || [], settings)
    var limit = Math.max(1, Number(settings.limit) || 200)
    var list = Array.isArray(records) ? records : []
    var hits = []
    if (spec.invalid) return { rows: [], spec: spec, matched: 0, error: spec.invalid }
    for (var i = 0; i < list.length; i++) {
      var record = list[i]
      if (!record) continue
      var score = SearchQuery.rank(spec, record)
      if (score === null) continue
      hits.push({ record: record, rank: score, heat: Number(record.frecency) || 0, index: i })
    }
    hits.sort(function(left, right) {
      return SearchQuery.compareRanks(left.rank, right.rank) || right.heat - left.heat
        || (String(left.record.text || left.record.name) < String(right.record.text || right.record.name) ? -1
          : String(left.record.text || left.record.name) > String(right.record.text || right.record.name) ? 1 : left.index - right.index)
    })
    var rows = []
    for (var h = 0; h < hits.length && h < limit; h++) {
      var hit = hits[h]
      rows.push({ record: hit.record, rank: hit.rank, frecency: hit.heat,
                  nameSpans: spans(String(hit.record.name || ""), spec), relativeSpans: spans(String(hit.record.text || ""), spec) })
    }
    return { rows: rows, spec: spec, matched: hits.length, error: "" }
  }
}
