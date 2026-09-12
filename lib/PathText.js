.pragma library

function pathText(value) {
  var text = String(value === undefined || value === null ? "" : value)
  return text.trim() === "" ? "" : text
}

function encodePart(value) {
  return encodeURIComponent(value).replace(/[!'()*]/g, function(character) {
    return "%" + character.charCodeAt(0).toString(16).toUpperCase()
  })
}

function normalizeParts(parts) {
  var result = []
  for (var i = 0; i < parts.length; i++) {
    if (!parts[i] || parts[i] === ".") continue
    if (parts[i] === "..") result.pop()
    else result.push(parts[i])
  }
  return "/" + result.join("/")
}

// Use a canonical byte URI for comparisons, including mixed plain/URI paths.
function fileUrl(value) {
  var text = String(value || "")
  if (text.indexOf("\0") >= 0) return ""
  if (/^file:/i.test(text)) {
    var match = text.match(/^file:\/\/(?:localhost)?(\/[^?#]*)$/i)
    if (!match || /[\x00-\x20\x7f\\]/.test(text) || /%(?![0-9a-f]{2})/i.test(text)) return ""
    text = match[1].replace(/%2f/gi, "/")
    var pieces = text.split(/(%[0-9a-f]{2})/i)
    for (var i = 0; i < pieces.length; i++) {
      if (/^%[0-9a-f]{2}$/i.test(pieces[i])) {
        var character = String.fromCharCode(parseInt(pieces[i].slice(1), 16))
        if (character === "\0") return ""
        pieces[i] = /^[A-Za-z0-9._~-]$/.test(character) ? character : pieces[i].toUpperCase()
      } else {
        try { pieces[i] = pieces[i].split("/").map(encodePart).join("/") }
        catch (_) { return "" }
      }
    }
    return "file://" + normalizeParts(pieces.join("").split("/"))
  }
  if (text.charAt(0) !== "/") return ""
  try { return "file://" + normalizeParts(text.split("/").map(encodePart)) }
  catch (_) { return "" }
}

function droppedPath(value) {
  var text = String(value === undefined || value === null ? "" : value)
  if (!/^file:\/\//i.test(text)) return ""
  var body = text.slice(7)
  if (/^localhost\//i.test(body)) body = body.slice(9)
  if (body.charAt(0) !== "/") return ""
  if (/%[0-9a-f]{2}/i.test(body)) {
    try { body = decodeURIComponent(body) }
    catch (_) { return "" }
  }
  if (body.indexOf("\0") >= 0) return ""
  return normalizeParts(body.split("/"))
}

function fromFileUrl(value) {
  var uri = fileUrl(value)
  if (!uri) return String(value || "")
  try { return decodeURIComponent(uri.slice(7)) }
  catch (_) { return uri }
}

function normalize(value, home) {
  var path = pathText(value)
  if (!path || path === "~") return home
  if (/^file:/i.test(path)) return fromFileUrl(path)
  if (path.indexOf("~/") === 0) return join(home, path.slice(2))
  if (path.charAt(0) !== "/") return join(home, path)
  return normalizeParts(path.split("/"))
}

function within(path, parent) {
  var target = fileUrl(path)
  var root = fileUrl(parent)
  return !!target && !!root && (target === root || target.indexOf(root.replace(/\/$/, "") + "/") === 0)
}

function parent(path) {
  var uri = fileUrl(path)
  if (!uri) return "/"
  var slash = uri.lastIndexOf("/")
  return fromFileUrl(slash <= 7 ? "file:///" : uri.slice(0, slash))
}

function join(directory, name) {
  var uri = fileUrl(directory)
  if (!uri) return String(directory || "")
  try { return fromFileUrl(uri.replace(/\/$/, "") + "/" + String(name).split("/").map(encodePart).join("/")) }
  catch (_) { return "" }
}

function remap(path, source, destination) {
  if (!within(path, source)) return path
  var target = fileUrl(path)
  var origin = fileUrl(source).replace(/\/$/, "")
  var replacement = fileUrl(destination).replace(/\/$/, "")
  return replacement ? fromFileUrl(replacement + (target.slice(origin.length) || "/")) : path
}

function displayText(text) {
  return text.replace(/[\\\x00-\x1f\x7f-\x9f]/g, function(character) {
    if (character === "\\") return "\\\\"
    if (character === "\n") return "\\n"
    if (character === "\r") return "\\r"
    if (character === "\t") return "\\t"
    return "\\u{" + character.charCodeAt(0).toString(16).toUpperCase() + "}"
  })
}

function displayEncoded(text) {
  var result = ""
  for (var i = 0; i < text.length;) {
    if (text.charAt(i) !== "%") { result += displayText(text.charAt(i++)); continue }
    var byte = parseInt(text.slice(i + 1, i + 3), 16)
    var count = byte < 128 ? 1 : byte >= 194 && byte < 224 ? 2 : byte < 240 && byte >= 224 ? 3 : byte >= 240 && byte <= 244 ? 4 : 0
    var decoded = ""
    if (count) {
      try { decoded = decodeURIComponent(text.slice(i, i + count * 3)) } catch (_) {}
    }
    if (decoded) { result += displayText(decoded); i += count * 3 }
    else { result += "\\x" + text.slice(i + 1, i + 3).toUpperCase(); i += 3 }
  }
  return result
}

function name(path) {
  var uri = fileUrl(path)
  return uri ? displayEncoded(uri.slice(uri.lastIndexOf("/") + 1)) || "/" : String(path || "")
}

function nameNeedsEscaping(path) {
  var uri = fileUrl(path)
  if (!uri) return false
  try {
    var text = decodeURIComponent(uri.slice(uri.lastIndexOf("/") + 1))
    return displayText(text) !== text
  } catch (_) { return true }
}

function relative(path, root) {
  var uri = fileUrl(path)
  if (!uri) return String(path || "")
  var base = fileUrl(root)
  return within(path, root) ? displayEncoded(uri.slice(base.replace(/\/$/, "").length).replace(/^\//, "")) || "." : displayEncoded(uri.slice(7))
}
