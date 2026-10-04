import QtQuick
import QtTest
import "../../blades" as Blades
import "../../lib/DefaultPlacement.js" as DefaultPlacement

TestCase {
  id: test
  name: "DefaultPlacement"

  readonly property string fabric: "data-goblin.fileblade-fabric/fabric"
  readonly property string databricks: "data-goblin.fileblade-databricks/databricks"

  QtObject {
    id: shellPlugins
    property var installedPlugins: ({})
    property var disabled: []
    signal pluginsChanged()
    function isEnabled(id) { return disabled.indexOf(String(id)) < 0 }
  }

  Blades.BladeRegistry {
    id: registry
    socketId: "data-goblin.fileblade/blade"
    contractVersion: 3
  }

  function manifest(id, module, name, placement) {
    var contribution = { id: module, name: name, entry: "blades/Module.qml", provider: "Provider.qml", hostContract: 3, category: "Data" }
    if (placement !== undefined) contribution.defaultPlacement = placement
    return { schemaVersion: 1, id: id, kinds: ["service"], entryPoints: { service: "Service.qml" }, extensions: { "data-goblin.fileblade/blade": [contribution] } }
  }

  function installNatively(rows) {
    var providers = []
    for (var i = 0; i < rows.length; i++)
      providers.push({ id: rows[i].id, dir: "/extensions/" + rows[i].id, manifest: rows[i], enabled: true })
    registry.catalogProviders = providers
    registry.rebuild()
  }

  function installAsPlugins(rows) {
    var installed = ({})
    for (var i = 0; i < rows.length; i++) {
      var copy = JSON.parse(JSON.stringify(rows[i]))
      copy.__sourceDir = "/plugins/" + rows[i].id
      installed[rows[i].id] = copy
    }
    shellPlugins.installedPlugins = installed
    registry.rebuild()
  }

  function both() {
    return [
      manifest("data-goblin.fileblade-fabric", "fabric", "Fabric", "beside-files"),
      manifest("data-goblin.fileblade-databricks", "databricks", "Databricks", "beside-files")
    ]
  }

  function freshLayout() {
    return {
      left: { open: true, width: 380, mode: "docked", slots: [
        { id: "files", modules: [{ module: "files", state: {} }], active: 0, collapsed: false, fraction: -1 },
        { id: "properties", modules: [{ module: "properties", state: {} }, { module: "branches", state: {} }], active: 0, collapsed: false, fraction: 0.34 }
      ] },
      right: { open: false, width: 360, mode: "docked", slots: [
        { id: "notes", modules: [{ module: "notes", state: {} }], active: 0, collapsed: false, fraction: -1 }
      ] }
    }
  }

  function modulesOf(slot) {
    return slot.modules.map(function(tab) { return tab.module }).join(",")
  }

  function discover(layout, placed) {
    return DefaultPlacement.plan(layout, registry.modules, registry.order, placed)
  }

  function init() {
    registry.fileModules = ({})
    registry.pluginRegistry = null
    registry.catalogProviders = []
    shellPlugins.installedPlugins = ({})
    shellPlugins.disabled = []
    registry.rebuild()
  }

  function test_the_manifest_field_reaches_the_module_definition() {
    installNatively([manifest("a.b", "beside", "Beside", "beside-files"), manifest("c.d", "plain", "Plain"), manifest("e.f", "odd", "Odd", "top-right")])
    compare(registry.module("a.b/beside").defaultPlacement, "beside-files")
    compare(registry.module("c.d/plain").defaultPlacement, "")
    compare(registry.module("e.f/odd").defaultPlacement, "")
    var listed = JSON.parse(registry.ipcDocument(function() { return null }))
    var byId = ({})
    for (var i = 0; i < listed.modules.length; i++) byId[listed.modules[i].id] = listed.modules[i]
    compare(byId["a.b/beside"].defaultPlacement, "beside-files")
    compare(byId["c.d/plain"].defaultPlacement, null)
  }

  function test_native_install_lands_as_tabs_beside_files_without_taking_the_selection() {
    installNatively(both())
    var layout = freshLayout()
    layout.left.slots[0].active = 0
    var result = discover(layout, [])
    compare(result.added.length, 2)
    compare(modulesOf(result.layout.left.slots[0]), "files," + databricks + "," + fabric)
    compare(result.layout.left.slots[0].active, 0)
    compare(result.layout.left.slots.length, 2)
    compare(modulesOf(result.layout.left.slots[1]), "properties,branches")
    compare(result.layout.right.slots.length, 1)
    compare(result.layout.left.open, true)
    compare(result.layout.right.open, false)
    compare(result.placed.sort().join(","), [databricks, fabric].sort().join(","))
  }

  function test_plugin_install_lands_in_discovery_order_after_existing_tabs() {
    registry.pluginRegistry = shellPlugins
    installAsPlugins([both()[0]])
    var layout = freshLayout()
    layout.left.slots[0].modules.push({ module: "skills", state: {} })
    layout.left.slots[0].active = 1
    var first = discover(layout, [])
    compare(modulesOf(first.layout.left.slots[0]), "files,skills," + fabric)
    compare(first.layout.left.slots[0].active, 1)
    installAsPlugins(both())
    var second = discover(first.layout, first.placed)
    compare(second.added.join(","), databricks)
    compare(modulesOf(second.layout.left.slots[0]), "files,skills," + fabric + "," + databricks)
  }

  function test_a_closed_or_moved_tab_is_never_placed_again() {
    installNatively(both())
    var first = discover(freshLayout(), [])
    var edited = JSON.parse(JSON.stringify(first.layout))
    edited.left.slots[0].modules = edited.left.slots[0].modules.filter(function(tab) { return tab.module !== fabric })
    edited.left.slots[0].modules = edited.left.slots[0].modules.filter(function(tab) { return tab.module !== databricks })
    edited.right.slots[0].modules.push({ module: databricks, state: {} })
    var again = discover(edited, first.placed)
    compare(again.changed, false)
    compare(modulesOf(again.layout.left.slots[0]), "files")
    compare(modulesOf(again.layout.right.slots[0]), "notes," + databricks)
  }

  function test_disable_and_enable_keeps_the_users_choice() {
    registry.pluginRegistry = shellPlugins
    installAsPlugins(both())
    var first = discover(freshLayout(), [])
    var closed = JSON.parse(JSON.stringify(first.layout))
    closed.left.slots[0].modules.pop()
    shellPlugins.disabled = ["data-goblin.fileblade-fabric"]
    registry.rebuild()
    compare(registry.module(fabric), null)
    shellPlugins.disabled = []
    registry.rebuild()
    var result = discover(closed, first.placed)
    compare(result.changed, false)
  }

  function test_a_module_already_in_the_layout_is_only_recorded() {
    installNatively(both())
    var layout = freshLayout()
    layout.left.slots.push({ id: "fabric", modules: [{ module: fabric, state: {} }], active: 0, collapsed: false, fraction: -1 })
    var result = discover(layout, [])
    compare(result.added.join(","), databricks)
    compare(result.noted.join(","), fabric)
    compare(result.layout.left.slots.length, 3)
    compare(modulesOf(result.layout.left.slots[2]), fabric)
    compare(modulesOf(result.layout.left.slots[0]), "files," + databricks)
  }

  function test_modules_without_the_field_or_incompatible_stay_unplaced() {
    var newer = manifest("g.h", "future", "Future", "beside-files")
    newer.extensions["data-goblin.fileblade/blade"][0].hostContract = 9
    installNatively([manifest("c.d", "plain", "Plain"), newer])
    var result = discover(freshLayout(), [])
    compare(result.changed, false)
    compare(modulesOf(result.layout.left.slots[0]), "files")
  }

  function test_without_files_a_module_joins_an_earlier_sibling_or_gets_its_own_section() {
    installNatively([both()[0]])
    var layout = freshLayout()
    layout.left.slots.shift()
    var first = discover(layout, [])
    compare(first.layout.left.slots.length, 2)
    compare(modulesOf(first.layout.left.slots[1]), fabric)
    installNatively(both())
    var second = discover(first.layout, first.placed)
    compare(modulesOf(second.layout.left.slots[1]), fabric + "," + databricks)
    var empty = { left: { open: false, slots: [] }, right: { open: false, slots: [] } }
    var lone = discover(empty, [])
    compare(lone.layout.left.slots.length, 1)
    compare(modulesOf(lone.layout.left.slots[0]), databricks + "," + fabric)
  }

  function test_files_on_the_right_blade_still_receives_the_tab() {
    installNatively([both()[1]])
    var layout = freshLayout()
    var files = layout.left.slots.shift()
    layout.right.slots.push(files)
    var result = discover(layout, [])
    compare(modulesOf(result.layout.right.slots[1]), "files," + databricks)
  }

  function test_a_full_section_spills_into_a_new_section_below_it() {
    installNatively([both()[0]])
    var layout = freshLayout()
    for (var i = 0; i < 31; i++) layout.left.slots[0].modules.push({ module: "notes-" + i, state: {} })
    var result = discover(layout, [])
    compare(result.layout.left.slots[0].modules.length, 32)
    compare(modulesOf(result.layout.left.slots[1]), fabric)
    compare(modulesOf(result.layout.left.slots[2]), "properties,branches")
  }

  function test_the_record_is_bounded_and_clean() {
    compare(DefaultPlacement.recorded(["a/b", "a/b", "", 7, "c\u0001d"]).join(","), "a/b,7")
    compare(DefaultPlacement.recorded("a/b").length, 0)
    compare(DefaultPlacement.normalize(" Beside-Files "), "beside-files")
  }
}
