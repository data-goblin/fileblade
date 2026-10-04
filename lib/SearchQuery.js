.pragma library

function fold(value) {
  return String(value === undefined || value === null ? "" : value).toLowerCase()
}

function skipWhitespace(text, index) {
  while (index < text.length && /\s/.test(text[index])) index++
  return index
}

function readNegation(text, index) {
  var negate = index + 1 < text.length && (text[index] === "-" || text[index] === "!") && !/\s/.test(text[index + 1])
  return { negate: negate, index: index + (negate ? 1 : 0) }
}

function readField(text, index, keys) {
  var boundaries = [text.indexOf(" ", index), text.indexOf("\"", index)].filter(function(position) { return position >= 0 })
  var colon = text.indexOf(":", index)
  if (colon <= index || (boundaries.length && colon > Math.min.apply(null, boundaries))) return { key: "", index: index }
  var candidate = fold(text.slice(index, colon))
  return keys.indexOf(candidate) >= 0 ? { key: candidate, index: colon + 1 } : { key: "", index: index }
}

function readValue(text, index) {
  if (index < text.length && text[index] === "\"") {
    var closing = text.indexOf("\"", index + 1)
    var end = closing < 0 ? text.length : closing
    return { value: text.slice(index + 1, end), exact: true, index: closing < 0 ? text.length : closing + 1 }
  }
  var stop = index
  while (stop < text.length && !/\s/.test(text[stop])) stop++
  return { value: text.slice(index, stop), exact: false, index: stop }
}

function normalizeFilter(key, value) {
  var folded = fold(value)
  if (key === "type") {
    var kinds = {
      folder: "dir", folders: "dir", dir: "dir", dirs: "dir", directory: "dir", directories: "dir",
      file: "file", files: "file",
      link: "link", links: "link", symlink: "link", symlinks: "link",
      repo: "repo", repos: "repo", git: "repo", repository: "repo",
      image: "image", images: "image", img: "image", picture: "image",
      text: "text", txt: "text",
      video: "video", videos: "video",
      audio: "audio", sound: "audio", music: "audio"
    }
    return kinds[folded] || ""
  }
  if (key === "format") return folded.replace(/^\.+/, "")
  if (key === "in") return folded.replace(/^\/+|\/+$/g, "")
  return folded
}

function termSyntax(value, exact, regex) {
  if (exact || regex) return { text: value, kind: exact ? "substring" : "fuzzy" }
  var text = value
  var kind = "fuzzy"
  if (text[0] === "'") { text = text.slice(1); kind = "substring" }
  else if (text[0] === "^") { text = text.slice(1); kind = "prefix" }
  if (text.length > 1 && text[text.length - 1] === "$" && text[text.length - 2] !== "\\") {
    text = text.slice(0, -1)
    kind = kind === "fuzzy" ? "suffix" : "exact"
  }
  return { text: text.replace(/\\\$/g, "$"), kind: kind }
}

function parse(query, filterKeys, options) {
  var keys = ["name"].concat(Array.isArray(filterKeys) ? filterKeys.map(fold) : [])
  var settings = options && typeof options === "object" ? options : {}
  var spec = { terms: [], filters: {}, empty: true, caseSensitive: !!settings.caseSensitive, regex: !!settings.regex, invalid: "" }
  var text = String(query || "").trim()
  var index = 0
  while (index < text.length) {
    index = skipWhitespace(text, index)
    if (index >= text.length) break
    var negation = readNegation(text, index)
    var field = readField(text, negation.index, keys)
    var value = readValue(text, field.index)
    index = value.index
    if (!value.value) continue
    if (field.key === "" || field.key === "name") {
      var syntax = termSyntax(value.value, value.exact, spec.regex)
      if (!syntax.text) continue
      spec.empty = false
      var term = { text: syntax.text, kind: syntax.kind, exact: value.exact, negate: negation.negate, field: field.key, caseSensitive: spec.caseSensitive || value.exact, pattern: null }
      if (spec.regex) {
        try { term.pattern = new RegExp(syntax.text, term.caseSensitive ? "" : "i") }
        catch (error) { spec.invalid = spec.invalid || String(error.message || error) }
      }
      spec.terms.push(term)
      continue
    }
    spec.empty = false
    var bucket = spec.filters[field.key] || (spec.filters[field.key] = { wanted: [], excluded: [] })
    var parts = value.value.split(",")
    for (var i = 0; i < parts.length; i++) {
      var part = typeof settings.normalize === "function" ? fold(settings.normalize(field.key, parts[i].trim())) : normalizeFilter(field.key, parts[i].trim())
      if (part) (negation.negate ? bucket.excluded : bucket.wanted).push(part)
    }
  }
  return spec
}

function fieldValues(record, key) {
  var value = record && record.fields ? record.fields[key] : undefined
  if (value === undefined || value === null) return []
  if (typeof value === "object" && value.length !== undefined) {
    var list = []
    for (var i = 0; i < value.length; i++) list.push(fold(value[i]))
    return list
  }
  return [fold(value)]
}

function subsequence(haystack, needle) {
  var position = 0
  for (var i = 0; i < needle.length; i++) {
    position = haystack.indexOf(needle[i], position)
    if (position < 0) return false
    position++
  }
  return true
}

function termMatches(term, record) {
  var haystack = term.field === "name" ? String(record.name || "") : String(record.text || "")
  var found
  if (term.pattern) found = term.pattern.test(haystack)
  else {
    var source = term.caseSensitive ? haystack : fold(haystack)
    var needle = term.caseSensitive ? term.text : fold(term.text)
    if (term.kind === "fuzzy") found = subsequence(source, needle)
    else if (term.kind === "prefix") found = source.indexOf(needle) === 0
    else if (term.kind === "suffix") found = source.length >= needle.length && source.lastIndexOf(needle) === source.length - needle.length
    else if (term.kind === "exact") found = source === needle
    else found = source.indexOf(needle) >= 0
  }
  return term.negate ? !found : found
}

function filterMatches(bucket, values) {
  var wantedHit = bucket.wanted.length === 0 || bucket.wanted.some(function(item) { return values.indexOf(item) >= 0 })
  var excludedHit = bucket.excluded.some(function(item) { return values.indexOf(item) >= 0 })
  return wantedHit && !excludedHit
}

function matches(spec, record) {
  if (spec && spec.invalid) return false
  if (!spec || spec.empty) return true
  for (var i = 0; i < spec.terms.length; i++)
    if (!termMatches(spec.terms[i], record)) return false
  for (var key in spec.filters)
    if (!filterMatches(spec.filters[key], fieldValues(record, key))) return false
  return true
}

// Lower ranks win: filename matches, then exact/prefix/substring/subsequence,
// then tighter and earlier letters. Filters alone leave the user's sort intact.
function rank(spec, record) {
  if (!matches(spec, record)) return null
  var result = [0, 0, 0, 0, 0, 0]
  var positive = false
  for (var i = 0; i < spec.terms.length; i++) {
    var term = spec.terms[i]
    if (term.negate) continue
    positive = true
    var source = String(record.name || "")
    if (!termMatches(term, { name: source, text: source })) {
      source = String(record.text || "")
      result[0]++
    }
    var needle = term.caseSensitive ? term.text : fold(term.text)
    source = term.caseSensitive ? source : fold(source)
    var start = term.pattern ? source.search(term.pattern) : source.indexOf(needle)
    var kind = start < 0 ? 3 : start > 0 ? 2 : source === needle ? 0 : 1
    var gaps = 0
    if (start < 0) {
      var position = 0
      for (var c = 0; c < needle.length; c++) {
        var found = source.indexOf(needle[c], position)
        if (c === 0) start = found
        else gaps += found - position
        position = found + 1
      }
    }
    result[1] = Math.max(result[1], kind)
    result[2] += kind
    result[3] += gaps
    result[4] += start
  }
  if (positive) result[5] = String(record.name || "").length
  return result
}

function compareRanks(left, right) {
  if (!left || !right) return left === right ? 0 : left ? -1 : 1
  for (var i = 0; i < left.length; i++) {
    var difference = left[i] - right[i]
    if (difference !== 0) return difference
  }
  return 0
}

// Compare each branch at its first differing ancestor so groups stay together.
function rankedGroups(groups) {
  var branches = ({})
  for (var i = 0; i < groups.length; i++) {
    var group = groups[i]
    for (var depth = 1; depth <= group.path.length; depth++) {
      var key = JSON.stringify(group.path.slice(0, depth))
      if (!branches[key]) branches[key] = { rank: group.rank, index: i }
      else if (compareRanks(group.rank, branches[key].rank) < 0) branches[key].rank = group.rank
    }
  }
  return groups.slice().sort(function(left, right) {
    for (var depth = 0; depth < Math.min(left.path.length, right.path.length); depth++) {
      if (left.path[depth] === right.path[depth]) continue
      var a = branches[JSON.stringify(left.path.slice(0, depth + 1))]
      var b = branches[JSON.stringify(right.path.slice(0, depth + 1))]
      return compareRanks(a.rank, b.rank) || a.index - b.index
    }
    return left.path.length - right.path.length
  })
}

// Rank siblings by their best matching descendant while retaining ancestors.
function rankedTree(rows, spec, recordOf) {
  var roots = [], stack = [], nodes = [], matched = 0
  for (var i = 0; i < rows.length; i++) {
    var row = rows[i]
    var depth = Number(row.depth) || 0
    while (stack.length && stack[stack.length - 1].depth >= depth) stack.pop()
    var parent = stack.length ? stack[stack.length - 1] : null
    var score = rank(spec, recordOf(row))
    if (score) matched++
    var node = { row: row, index: i, depth: depth, rank: score, parent: parent, children: [] }
    ;(parent ? parent.children : roots).push(node)
    nodes.push(node)
    stack.push(node)
  }
  for (var n = nodes.length - 1; n >= 0; n--) {
    var child = nodes[n]
    if (child.parent && compareRanks(child.rank, child.parent.rank) < 0) child.parent.rank = child.rank
  }
  function ordered(children) {
    return children.filter(function(node) { return node.rank !== null }).sort(function(a, b) {
      return compareRanks(a.rank, b.rank) || a.index - b.index
    })
  }
  var pending = ordered(roots).reverse(), result = []
  while (pending.length) {
    var next = pending.pop()
    result.push(next.row)
    var children = ordered(next.children)
    for (var j = children.length - 1; j >= 0; j--) pending.push(children[j])
  }
  return { rows: result, matched: matched }
}
