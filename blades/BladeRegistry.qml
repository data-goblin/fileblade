import QtQuick
import "../lib/Definitions.js" as Definitions
import "../lib/PathText.js" as PathText
import "../lib/DefaultPlacement.js" as DefaultPlacement

QtObject {
  id: registry

  property string pluginDir: ""
  property string userModulesDir: ""
  property string socketId: ""
  property int contractVersion: 1
  property var pluginRegistry: null
  property var service: null
  property var modules: ({})
  property var disabledModules: ({})
  property var order: []
  property bool scanning: false
  property bool rescanQueued: false
  property int scanGeneration: 0
  property int revision: 0
  property var fileModules: ({})
  property var catalogProviders: []
  readonly property var coreModules: ["skills", "memory", "hooks", "mcp"]
  readonly property var moduleAliases: ({
    "data-goblin.fileblade-skills/skills": "skills",
    "data-goblin.fileblade-memory/memory": "memory",
    "data-goblin.fileblade-hooks/hooks": "hooks",
    "data-goblin.fileblade-mcp/mcp": "mcp",
    "kurt.agent-skills/skills": "skills",
    "kurt.agent-memory/memory": "memory",
    "kurt.agent-hooks/hooks": "hooks",
    "kurt.agent-mcp/mcp": "mcp"
  })
  readonly property var builtinProviders: {
    var rows = []
    for (var i = 0; i < coreModules.length; i++) {
      var found = fileModules[coreModules[i]]
      if (found && found.source === "builtin" && found.providerState === "owned" && found.compatible)
        rows.push({ id: found.providerId, dir: found.sourceDir, entry: found.providerEntry })
    }
    return rows
  }
  readonly property int maximumModules: 128
  readonly property int maximumIdLength: 128
  readonly property int maximumTextLength: 512

  signal registryChanged()

  function canonicalModule(id) {
    var name = String(id || "").trim()
    return moduleAliases[name] || name
  }

  function isSafeRelativePath(value) {
    var text = String(value || "")
    return text.length > 0 && text.length <= maximumTextLength && text.charAt(0) !== "/"
      && text.indexOf("..") < 0 && !/[\u0000-\u001f\u007f]/.test(text)
  }

  function boundedText(value, fallback, limit) { return Definitions.boundedText(value, fallback, limit) }

  function moduleId(raw, idPrefix) {
    var bare = Definitions.safeId(raw.id, maximumIdLength)
    if (!bare) return ""
    var id = idPrefix ? idPrefix + "/" + bare : bare
    return id.length <= maximumIdLength ? id : ""
  }

  function normalizedModule(raw, sourceDir, source, idPrefix) {
    if (!raw || typeof raw !== "object" || Array.isArray(raw)) return null
    var id = moduleId(raw, idPrefix)
    var entry = boundedText(raw.entry, "Module.qml", maximumTextLength).trim()
    if (!id) return null
    if (!isSafeRelativePath(entry)) return null
    var directory = String(sourceDir || "").replace(/\/$/, "")
    if (!directory) return null
    var hostContract = Math.max(1, Math.floor(Number(raw.hostContract) || 1))
    var icon = typeof raw.icon === "string" && isSafeRelativePath(raw.icon) ? String(raw.icon) : ""
    return {
      id: id,
      name: boundedText(raw.name, raw.id, maximumTextLength),
      glyph: boundedText(raw.glyph, "", 16),
      iconUrl: icon !== "" ? PathText.fileUrl(PathText.join(directory, icon)) : "",
      description: boundedText(raw.description, "", maximumTextLength),
      entryUrl: PathText.fileUrl(PathText.join(directory, entry)),
      sourceDir: directory,
      source: boundedText(source, "builtin", maximumTextLength),
      providerId: source === "builtin" && !idPrefix && coreModules.indexOf(id) >= 0
        ? "fileblade.core." + id : boundedText(idPrefix, "", maximumIdLength),
      singleton: raw.singleton === undefined ? true : !!raw.singleton,
      minHeight: Math.max(0, Math.min(4096, Number(raw.minHeight) || 0)),
      hostContract: hostContract,
      providerEntry: isSafeRelativePath(raw.provider) ? String(raw.provider).trim() : "",
      providerState: raw.provider === null ? "stateless"
        : (isSafeRelativePath(raw.provider) ? "owned" : (raw.provider === undefined ? "legacy" : "invalid")),
      needsUpdate: false,
      compatible: hostContract <= contractVersion,
      category: Definitions.category(raw.category, source),
      settings: Definitions.settingsSpec(raw.settings),
      defaultPlacement: DefaultPlacement.normalize(raw.defaultPlacement)
    }
  }

  function packageModules(packageDefinition) {
    if (!packageDefinition || !Array.isArray(packageDefinition.modules)) return [packageDefinition]
    var definitions = packageDefinition.modules
    for (var i = 0; i < definitions.length; i++) {
      if (definitions[i] && definitions[i].hostContract === undefined && packageDefinition.hostContract !== undefined)
        definitions[i].hostContract = packageDefinition.hostContract
    }
    return definitions
  }

  function parseScanOutput(parsed) {
    var found = ({})
    var rows = parsed && Array.isArray(parsed.modules) ? parsed.modules.slice(0, maximumModules) : []
    var accepted = 0
    for (var i = 0; i < rows.length && accepted < maximumModules; i++) {
      var row = rows[i]
      var definitions = packageModules(row.definition)
      for (var j = 0; j < definitions.length && accepted < maximumModules; j++) {
        var module = normalizedModule(definitions[j], row.source_dir, row.source, "")
        if (module && !found[module.id]) {
          found[module.id] = module
          accepted++
        }
      }
    }
    fileModules = found
    scanning = false
    rebuild()
    if (rescanQueued) {
      rescanQueued = false
      Qt.callLater(rescan)
    }
  }

  function socketContributions(manifest) {
    if (!manifest) return null
    var extensions = manifest.extensions
    if (extensions && typeof extensions === "object" && !Array.isArray(extensions) && Array.isArray(extensions[socketId]))
      return extensions[socketId]
    return Array.isArray(manifest.bladeModules) ? manifest.bladeModules : null
  }

  function providerModules(manifest, pluginId, sourceDir) {
    var result = ({})
    var count = 0
    var contributed = socketContributions(manifest)
    if (!contributed) return result
    var directory = sourceDir === undefined || sourceDir === null || sourceDir === "" ? manifest.__sourceDir : sourceDir
    for (var i = 0; i < contributed.length && count < maximumModules; i++) {
      var module = normalizedModule(contributed[i], directory, "plugin:" + pluginId, pluginId)
      if (module && !result[module.id]) {
        result[module.id] = module
        count++
      }
    }
    return result
  }

  function mergeProviderModules(target, other, candidates) {
    var ids = Object.keys(candidates)
    var count = Object.keys(target).length
    for (var i = 0; i < ids.length && count < maximumModules; i++) {
      var id = ids[i]
      var canonical = canonicalModule(id)
      if (canonical !== id && fileModules[canonical] && fileModules[canonical].source === "builtin") continue
      if (target[id] || other[id]) continue
      target[id] = candidates[id]
      count++
    }
  }

  function providerSources() {
    var installed = pluginRegistry && pluginRegistry.installedPlugins ? pluginRegistry.installedPlugins : ({})
    var ids = Object.keys(installed)
    var sources = []
    for (var i = 0; i < ids.length && sources.length < maximumModules; i++) {
      var manifest = installed[ids[i]]
      if (!manifest || !socketContributions(manifest) || !manifest.__sourceDir) continue
      var providerEnabled = !pluginRegistry || typeof pluginRegistry.isEnabled !== "function" || pluginRegistry.isEnabled(ids[i])
      if (!providerEnabled) continue
      sources.push({ id: ids[i], dir: String(manifest.__sourceDir) })
    }
    var catalog = Array.isArray(catalogProviders) ? catalogProviders : []
    for (var j = 0; j < catalog.length && sources.length < maximumModules; j++) {
      var provider = catalog[j]
      if (!provider || !provider.id || !provider.enabled || installed[provider.id]) continue
      if (!provider.dir || !socketContributions(provider.manifest)) continue
      sources.push({ id: String(provider.id), dir: String(provider.dir) })
    }
    return sources
  }

  function manifestModules() {
    var result = ({})
    var disabled = ({})
    var installed = pluginRegistry && pluginRegistry.installedPlugins ? pluginRegistry.installedPlugins : ({})
    var ids = Object.keys(installed)
    for (var i = 0; i < ids.length && i < maximumModules * 2; i++) {
      var providerEnabled = !pluginRegistry || typeof pluginRegistry.isEnabled !== "function"
        || pluginRegistry.isEnabled(ids[i])
      var candidates = providerModules(installed[ids[i]], ids[i])
      if (providerEnabled) mergeProviderModules(result, disabled, candidates)
      else mergeProviderModules(disabled, result, candidates)
    }
    var catalog = Array.isArray(catalogProviders) ? catalogProviders : []
    for (var j = 0; j < catalog.length && j < maximumModules; j++) {
      var provider = catalog[j]
      if (!provider || !provider.id || installed[provider.id]) continue
      var fromDisk = providerModules(provider.manifest, String(provider.id), String(provider.dir || ""))
      var diskIds = Object.keys(fromDisk)
      for (var k = 0; k < diskIds.length; k++) {
        var candidate = fromDisk[diskIds[k]]
        if (candidate.providerState === "owned" || candidate.providerState === "stateless") continue
        candidate.needsUpdate = true
        candidate.compatible = false
      }
      if (provider.enabled) mergeProviderModules(result, disabled, fromDisk)
      else mergeProviderModules(disabled, result, fromDisk)
    }
    disabledModules = disabled
    return result
  }

  function rebuild() {
    var merged = ({})
    var fromFiles = fileModules
    var fileIds = Object.keys(fromFiles)
    for (var i = 0; i < fileIds.length; i++) merged[fileIds[i]] = fromFiles[fileIds[i]]
    var fromManifests = manifestModules()
    var manifestIds = Object.keys(fromManifests)
    var count = fileIds.length
    for (var j = 0; j < manifestIds.length && count < maximumModules; j++) {
      if (merged[manifestIds[j]]) continue
      merged[manifestIds[j]] = fromManifests[manifestIds[j]]
      count++
    }
    var ids = Object.keys(merged)
    ids.sort(function(left, right) {
      var byCategory = Definitions.categoryOrder(merged[left].category, merged[right].category)
      if (byCategory !== 0) return byCategory
      var leftBuiltin = merged[left].source === "builtin" ? 0 : 1
      var rightBuiltin = merged[right].source === "builtin" ? 0 : 1
      if (leftBuiltin !== rightBuiltin) return leftBuiltin - rightBuiltin
      return String(merged[left].name).localeCompare(String(merged[right].name))
    })
    modules = merged
    order = ids
    revision++
    registryChanged()
  }

  function module(id) {
    return modules[canonicalModule(id)] || null
  }

  function disabledModule(id) {
    return disabledModules[canonicalModule(id)] || null
  }

  function entryUrl(id) {
    var found = module(id)
    return found ? String(found.entryUrl) : ""
  }

  function settingKeys(found) {
    var keys = []
    var rows = found.settings && Array.isArray(found.settings.schema) ? found.settings.schema : []
    for (var i = 0; i < rows.length; i++) keys.push(String(rows[i].key))
    return keys
  }

  function ipcDocument(placed) {
    var rows = []
    for (var i = 0; i < order.length; i++) {
      var found = module(order[i])
      if (!found) continue
      rows.push({
        id: found.id,
        name: found.name,
        glyph: found.glyph,
        icon: found.iconUrl !== "" ? found.iconUrl : null,
        description: found.description,
        category: found.category,
        source: found.source,
        singleton: found.singleton,
        defaultPlacement: found.defaultPlacement !== "" ? found.defaultPlacement : null,
        entry: found.entryUrl,
        settings: { keys: settingKeys(found) },
        placed: placed(found.id) || null
      })
    }
    return JSON.stringify({ count: rows.length, modules: rows })
  }

  function rescan() {
    if (!pluginDir || !service) return
    if (scanning) {
      rescanQueued = true
      return
    }
    scanning = true
    scanGeneration++
    var requestGeneration = scanGeneration
    service.backendRequest("blade-modules", ["--plugin", pluginDir, "--user", userModulesDir], requestGeneration, function(response) {
      if (requestGeneration !== registry.scanGeneration) return
      registry.parseScanOutput(response)
    })
  }

  property Connections registryLink: Connections {
    target: registry.pluginRegistry
    ignoreUnknownSignals: true
    function onPluginsChanged() { registry.rebuild() }
  }

  onPluginDirChanged: rescan()
  onServiceChanged: rescan()
}
