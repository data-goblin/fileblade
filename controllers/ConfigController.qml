import QtQuick
import "../lib/MonitorMode.js" as MonitorMode
import qs.Commons

Item {
  required property var service

  readonly property var typeLabels: ({
    "application/json": "JSON",
    "application/pdf": "PDF",
    "application/zip": "ZIP",
    "application/x-7z-compressed": "7Z",
    "application/x-shellscript": "Shell",
    "image/gif": "GIF",
    "image/jpeg": "JPEG",
    "image/png": "PNG",
    "image/svg+xml": "SVG",
    "image/webp": "WebP",
    "text/css": "CSS",
    "text/csv": "CSV",
    "text/html": "HTML",
    "text/javascript": "JavaScript",
    "text/markdown": "Markdown",
    "text/plain": "Text",
    "text/x-python": "Python"
  })

  function pluginConfig() {
    var config = service.shell && service.shell.shellConfig ? service.shell.shellConfig : null
    var plugins = config && Array.isArray(config.plugins) ? config.plugins : []
    for (var i = 0; i < plugins.length; i++) {
      var entry = plugins[i]
      if (!entry || String(entry.id || "") !== "data-goblin.fileblade") continue
      return entry.settings && typeof entry.settings === "object" ? entry.settings : entry
    }
    return ({})
  }

  function boolValue(value, fallback) {
    return typeof value === "boolean" ? value : fallback
  }

  function numberValue(value, fallback, minimum, maximum) {
    var parsed = Number(value)
    if (!isFinite(parsed)) return fallback
    return Math.max(minimum, Math.min(maximum, parsed))
  }

  function normalizePlacement(value) {
    var placement = String(value || "").toLowerCase()
    if (placement === "top") placement = "above"
    if (placement === "bottom") placement = "below"
    return ["above", "right", "below"].indexOf(placement) >= 0 ? placement : "below"
  }

  function normalizeModeBadge(value) {
    var badge = String(value || "").toLowerCase()
    if (badge === "top" || badge === "on" || badge === "true") badge = "header"
    if (badge === "bottom") badge = "footer"
    if (badge === "off" || badge === "false" || badge === "none") badge = "hidden"
    return ["header", "footer", "hidden"].indexOf(badge) >= 0 ? badge : "header"
  }

  function normalizeDragOut(value) {
    var mode = String(value || "").toLowerCase()
    if (mode === "wayland" || mode === "handoff") mode = "system"
    if (mode === "path" || mode === "type") mode = "paste"
    return ["paste", "system"].indexOf(mode) >= 0 ? mode : "paste"
  }

  function normalizeMonitorMode(value) {
    return MonitorMode.normalize(value)
  }

  readonly property var priorityPropertyChoices: [
    { key: "none", label: "Off", shortLabel: "OFF", kind: "text", glyph: "󰅖" },
    { key: "size", label: "Size", shortLabel: "SIZE", kind: "number", glyph: "󰋊" },
    { key: "type", label: "Type", shortLabel: "TYPE", kind: "text", glyph: "󰈔" },
    { key: "modified", label: "Modified", shortLabel: "MODIFIED", kind: "date", glyph: "󰥔" },
    { key: "created", label: "Created", shortLabel: "CREATED", kind: "date", glyph: "󰃭" },
    { key: "git", label: "Git status", shortLabel: "GIT", kind: "number", glyph: "±", group: "git", filter: false },
    { key: "repo", label: "Repo", shortLabel: "REPO", kind: "text", glyph: "󰊢", group: "git" },
    { key: "branch", label: "Branch", shortLabel: "BRANCH", kind: "text", glyph: "󰘬", group: "git" },
    { key: "worktree", label: "Worktree", shortLabel: "WORKTREE", kind: "text", glyph: "󰉖", group: "git" }
  ]

  readonly property var gitStatusDetailChoices: [
    { key: "modified", label: "Modified", glyph: "M" },
    { key: "deleted", label: "Deleted", glyph: "D" },
    { key: "new", label: "Untracked", glyph: "?" }
  ]

  function normalizeGitStatusDetails(value) {
    var source = Array.isArray(value) ? value : String(value === undefined || value === null ? "" : value).split(",")
    var aliases = ({
      "git-modified": "modified", "git-modified-count": "modified", "modified-count": "modified",
      "git-deleted": "deleted", "git-deleted-count": "deleted", "deleted-count": "deleted",
      "git-new": "new", "git-new-count": "new", "new-count": "new", "untracked": "new"
    })
    var selected = ({})
    for (var i = 0; i < source.length; i++) {
      var raw = String(source[i] || "").trim().toLowerCase()
      var key = aliases[raw] || raw
      if (["modified", "deleted", "new"].indexOf(key) >= 0) selected[key] = true
    }
    return ["modified", "deleted", "new"].filter(function(key) { return selected[key] === true })
  }

  function gitStatusDetailsFromColumns(value) {
    var source = Array.isArray(value) ? value : []
    return normalizeGitStatusDetails(source.filter(function(key) {
      return String(key || "").toLowerCase().indexOf("git-") === 0
    }))
  }

  function normalizePriorityColumns(value) {
    var source = Array.isArray(value) ? value : String(value === undefined || value === null ? "" : value).split(",")
    var result = []
    for (var i = 0; i < source.length; i++) {
      var raw = String(source[i] || "").trim().toLowerCase()
      var legacyGitDetail = ["git-modified", "git-modified-count", "modified-count",
        "git-deleted", "git-deleted-count", "deleted-count",
        "git-new", "git-new-count", "new-count", "untracked"].indexOf(raw) >= 0
      if (raw === "" || raw === "git" || legacyGitDetail) continue
      var key = normalizePriorityProperty(source[i])
      if (key !== "none" && result.indexOf(key) < 0) result.push(key)
    }
    return result
  }

  function normalizePriorityProperty(value) {
    var property = String(value || "").trim().toLowerCase()
    if (["off", "hidden", "false"].indexOf(property) >= 0) property = "none"
    if (["filetype", "file-type", "kind", "mime"].indexOf(property) >= 0) property = "type"
    if (["date", "datetime", "mtime", "modified-date"].indexOf(property) >= 0) property = "modified"
    if (["birth", "birthtime", "creation", "creation-date"].indexOf(property) >= 0) property = "created"
    if (["repository", "git-repo"].indexOf(property) >= 0) property = "repo"
    if (["status", "git-status", "vcs"].indexOf(property) >= 0) property = "git"
    if (normalizeGitStatusDetails([property]).length > 0 && property !== "modified") property = "none"
    return ["none", "size", "type", "modified", "created", "git", "repo", "branch", "worktree"].indexOf(property) >= 0 ? property : "modified"
  }

  function priorityPropertyLabel(value, shortLabel) {
    var property = normalizePriorityProperty(value)
    for (var i = 0; i < service.priorityPropertyChoices.length; i++) {
      var choice = service.priorityPropertyChoices[i]
      if (choice.key === property) return shortLabel ? choice.shortLabel : choice.label
    }
    return shortLabel ? "MODIFIED" : "Modified"
  }

  function fileTypeLabel(name, isDir, isSymlink, kind, mime) {
    if (isDir) return isSymlink ? "Folder link" : "Folder"
    if (isSymlink) return "Link"
    var contentType = String(mime || "").toLowerCase()
    if (typeLabels[contentType]) return typeLabels[contentType]
    var label = contentTypeLabel(contentType)
    if (label) return label
    return extensionLabel(name, kind)
  }

  function contentTypeLabel(contentType) {
    if (!contentType || contentType === "application/octet-stream") return ""
    var subtype = contentType.indexOf("/") >= 0 ? contentType.split("/").pop() : contentType
    subtype = subtype.replace(/^x-/, "").replace(/\+xml$/, "").replace(/[-_]+/g, " ")
    return subtype ? subtype.charAt(0).toUpperCase() + subtype.slice(1) : ""
  }

  function extensionLabel(name, kind) {
    var filename = String(name || "")
    var dot = filename.lastIndexOf(".")
    var extension = dot > 0 && dot < filename.length - 1 ? filename.slice(dot + 1) : ""
    if (extension && extension.length <= 10) return extension.toUpperCase()
    var fallback = String(kind || "")
    return fallback && fallback !== "File" ? fallback : "File"
  }

  function priorityValue(name, isDir, isSymlink, sizeText, kind, mime, modified, created) {
    return priorityValueFor(service.priorityProperty, name, isDir, isSymlink, sizeText, kind, mime, modified, created)
  }

  function priorityValueFor(key, name, isDir, isSymlink, sizeText, kind, mime, modified, created) {
    if (key === "none" || key === "off") return ""
    if (key === "size") return String(sizeText || "—")
    if (key === "type") return fileTypeLabel(name, isDir, isSymlink, kind, mime)
    var timestamp = String(key === "created" ? created : modified)
    return timestamp ? timestamp.slice(0, 16) : "—"
  }

  function gitStatusColor(status) {
    var marker = String(status || "")
    if (marker === "D" || marker === "U") return Color.urgent
    if (marker === "A" || marker === "?") return "#98c379"
    if (marker === "R" || marker === "C") return "#61afef"
    if (marker === "M" || marker === "T") return "#e5c07b"
    return Color.muted
  }
}
