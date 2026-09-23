.pragma library
.import "../../lib/Definitions.js" as Definitions

var HEADING = "Welcome to FileBlade"
var BODY = "Browse files, keep notes, and inspect your agent files. These blades are included and work without a registry."
var DISMISS = "Close Welcome"
var STATES = ["", "dismissed", "installed"]
var CORE = [
  { id: "files", name: "Files", description: "Browse and manage files" },
  { id: "notes", name: "Notes", description: "Plain-text notebook" },
  { id: "skills", name: "Skills", description: "Inspect agent skills" },
  { id: "memory", name: "Memory", description: "Agent instruction files" },
  { id: "hooks", name: "Hooks", description: "Configured event hooks" },
  { id: "mcp", name: "MCP", description: "Configured MCP servers" }
]
var HELP = [
  { name: "Find your way", text: "Open Files to browse a folder. Use the arrow keys or j/k to move, Enter to activate an entry, and Alt+Up to go to the parent folder. Press / to search or Ctrl+P to choose a location." },
  { name: "Arrange your blades", text: "Use + in a blade to add a module. Switch modules with their tabs. Welcome can be reopened from the same menu after you close it; closing Welcome does not remove Notes or other tabs." },
  { name: "Inspect agent files", text: "Skills, Memory, Hooks and MCP show declarations from your folders. Browsing them does not run hooks or start servers. Changing Skills or Memory requires Manage agent files in General settings. Removed items have their own recovery bin." },
  { name: "Keep a note", text: "Open Notes to write plain text, add a tab or rename one. The notebook has a 64 KiB total limit. Check the save status before closing; opening a note does not send it to an agent." }
]
var LOCAL_CATALOG = { version: 1, entries: [] }

function firstRun(state) { return String(state || "") === "" }
function updated(state, seenVersion, currentVersion) {
  if (firstRun(state)) return false
  var seen = String(seenVersion || ""), current = String(currentVersion || "")
  return seen !== "" && current !== "" && seen !== current
}
function needsVersionAdoption(state, seenVersion, currentVersion) {
  return !firstRun(state) && String(seenVersion || "") === "" && String(currentVersion || "") !== ""
}
function pending(state, seenVersion, currentVersion) {
  return firstRun(state) || updated(state, seenVersion, currentVersion)
}
function repositoryUrl(manifest) {
  var url = manifest && typeof manifest.repository === "string" ? manifest.repository.trim() : ""
  var match = /^https:\/\/github\.com\/([A-Za-z0-9_.-]+)\/([A-Za-z0-9_.-]+)$/.exec(url)
  if (!match) return ""
  if (match[1] === "." || match[1] === ".." || match[2] === "." || match[2] === "..") return ""
  return url
}
function issuesUrl(manifest) {
  var base = repositoryUrl(manifest)
  return base === "" ? "" : base + "/issues/new"
}
function releaseUrl(manifest, version) {
  var base = repositoryUrl(manifest), tag = String(version || "").trim()
  return base === "" || tag === "" ? "" : base + "/releases/tag/v" + tag
}
function releaseNotesUrl(manifest) {
  var base = repositoryUrl(manifest)
  return base === "" ? "" : base + "/blob/main/features/release/release-notes.md"
}
function normalizeState(state) { return STATES.indexOf(String(state || "")) >= 0 ? String(state || "") : "" }
function welcomeSlot() { return { id: "welcome", modules: [{ module: "welcome" }, { module: "notes" }], active: 0 } }

function catalog(text, contractVersion) {
  if (typeof text !== "string" || text.length > 65536) return null
  var document
  try { document = JSON.parse(text) } catch (_) { return null }
  if (!document || document.version !== 1 || !Array.isArray(document.entries) || document.entries.length > 128) return null
  var entries = [], ids = Object.create(null)
  for (var entry of document.entries) {
    if (!entry || typeof entry !== "object" || Array.isArray(entry)
        || typeof entry.id !== "string" || !Definitions.MODULE_ID_PATTERN.test(entry.id) || entry.id.length > 128 || ids[entry.id]
        || typeof entry.name !== "string" || !entry.name.trim() || entry.name.length > 64
        || typeof entry.source !== "string" || entry.source.length > 2048 || !/^https:\/\/[A-Za-z0-9.-]+(?:\/[\x21-\x7e]*)?$/.test(entry.source)
        || !Number.isInteger(entry.hostContract) || entry.hostContract < 1 || entry.hostContract > 65535
        || ["on-demand", "persistent"].indexOf(entry.lifecycle) < 0) return null
    var name = Definitions.boundedText(entry.name, "", 64)
    if (!name.trim()) return null
    ids[entry.id] = true
    entries.push({ id: entry.id, name: name, source: entry.source,
      hostContract: entry.hostContract, compatible: entry.hostContract <= contractVersion, lifecycle: entry.lifecycle })
  }
  return { version: 1, entries: entries }
}
