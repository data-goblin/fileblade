import QtQuick
import Quickshell
import "../../blades" as Blades

ShellRoot {
  id: probe

  Blades.BladeRegistry { id: moduleRegistry; contractVersion: 3 }

  QtObject {
    id: fixture
    property var config: ({})
    property var legacyDefaults: ({})
    property int minimumWidth: 200
    property int slotHandleSize: 10
    property var layout: ({ left: { slots: [] }, right: { slots: [] } })
    property var edges: ["left", "right"]
    property var registry: moduleRegistry
    property var activeSlots: ({ left: 0, right: 0 })
    property var windowAddresses: ({})
    property string monitorMode: "all"
    property string monitorLock: ""
    property bool animateBlades: false
  }

  Blades.BladeLayout { id: layout; host: fixture }

  function require(value, message) {
    if (!value) throw new Error(message)
  }

  function tab(module, label) {
    return { module: module, state: { query: label, future: { keep: label } }, savedExtra: label }
  }

  function slot(id, modules, active) {
    return { id: id, modules: modules, active: active, collapsed: false, fraction: -1 }
  }

  function restored(left, right) {
    return layout.normalizeLayout({ left: { slots: left }, right: { slots: right || [] } })
  }

  function roundTrip(value, label) {
    fixture.layout = value
    var document = JSON.parse(JSON.stringify(layout.layoutDocument()))
    require(JSON.stringify(layout.normalizeLayout(document)) === JSON.stringify(value), label + " persisted recovery round trip")
    require(JSON.stringify(layout.normalizeLayout(value)) === JSON.stringify(value), label + " idempotent")
  }

  function sameSlotCollisions() {
    var names = ["skills", "memory", "hooks", "mcp"]
    var goblins = "kurt.goblin-images/goblin-images"
    for (var n = 0; n < names.length; n++) {
      var name = names[n]
      var modules = [tab("data-goblin.fileblade-" + name + "/" + name, "legacy"), tab(name, "current"), tab("kurt.agent-" + name + "/" + name, "older"), tab(goblins, "goblins")]
      for (var active = 0; active < modules.length; active++) {
        var raw = { left: { slots: [slot("mixed", modules, active)] }, right: { slots: [] } }
        var original = JSON.stringify(raw)
        var next = layout.normalizeLayout(raw)
        var winner = active === 3 ? 0 : active
        var saved = next.left.slots[0]
        require(saved.modules.length === 2 && saved.modules[0].module === name, name + " aliases collapse")
        require(saved.modules[0].state.query === modules[winner].state.query, name + " active state wins")
        require(saved.active === (active === 3 ? 1 : 0), name + " active tab remapped")
        require(saved.modules[1].module === goblins && saved.modules[1].state.query === "goblins", name + " Goblins survives")
        require(next.singletonRecovery.length === 2, name + " both conflicts recoverable")
        for (var i = 0; i < next.singletonRecovery.length; i++) {
          var record = next.singletonRecovery[i]
          require(record.version === 1 && record.module === name && record.edge === "left" && record.slotId === "mixed" && record.slotIndex === 0, name + " recovery location")
          require(record.tabIndex !== winner && JSON.stringify(record.tab) === JSON.stringify(modules[record.tabIndex]), name + " original alias and complete state recovered")
        }
        require(JSON.stringify(raw) === original, name + " original document unchanged")
        roundTrip(next, name + " active " + active)
      }
    }
  }

  function crossLayoutCollisions() {
    var goblins = "kurt.goblin-images/goblin-images"
    var next = restored([
      slot("inactive", [tab("data-goblin.fileblade-mcp/mcp", "inactive"), tab(goblins, "goblins")], 1),
      slot("selected", [tab("kurt.agent-mcp/mcp", "left active")], 0)
    ], [slot("right", [tab("mcp", "right active")], 0)])
    require(next.left.slots.length === 2 && next.right.slots.length === 0, "cross-edge empty slot removed")
    require(next.left.slots[0].modules[0].module === goblins && next.left.slots[0].active === 0, "active Goblins keeps selection")
    require(next.left.slots[1].modules[0].state.query === "left active", "active occurrence then left edge wins")
    require(next.singletonRecovery.length === 2 && next.singletonRecovery[1].edge === "right" && next.singletonRecovery[1].active, "cross-edge conflicting active state preserved")
    roundTrip(next, "cross-edge")

    next = restored([slot("winner", [tab("mcp", "first")], 0)], [
      slot("next", [tab("data-goblin.fileblade-mcp/mcp", "removed active"), tab(goblins, "next")], 0),
      slot("previous", [tab("unrelated.module/repeat", "previous"), tab("kurt.agent-mcp/mcp", "removed last")], 1)
    ])
    require(next.left.slots[0].modules[0].state.query === "first", "left active wins ties")
    require(next.right.slots[0].active === 0 && next.right.slots[0].modules[0].module === goblins, "lost active selects next Goblins")
    require(next.right.slots[1].active === 0 && next.right.slots[1].modules[0].state.query === "previous", "lost last active selects previous")
    require(next.singletonRecovery[0].tab.state.query === "removed active" && next.singletonRecovery[1].tab.state.query === "removed last", "both displaced selections recoverable")
    roundTrip(next, "active fallback")
  }

  function declaredSingletonsAndRecovery() {
    moduleRegistry.modules = ({ repeat: { singleton: false }, once: { singleton: true } })
    moduleRegistry.disabledModules = ({ disabled: { singleton: true } })
    var next = restored([
      slot("first", [tab("repeat", "one"), tab("once", "inactive"), tab("disabled", "one")], 0),
      slot("second", [tab("repeat", "two"), tab("once", "active"), tab("disabled", "two")], 1)
    ])
    require(next.left.slots[0].modules[0].state.query === "one" && next.left.slots[1].modules[0].state.query === "two", "declared non-singletons remain separate")
    require(next.left.slots[1].modules[1].module === "once" && next.left.slots[1].modules[1].state.query === "active", "declared singleton uses active preference")
    require(next.singletonRecovery.length === 2, "enabled and disabled declarations deduplicate")
    next.singletonRecovery.push({ version: 9, future: { original: "preserve" } })
    roundTrip(next, "future recovery evidence")
    moduleRegistry.modules = ({})
    moduleRegistry.disabledModules = ({})
    next = restored([slot("unknown", [tab("unrelated.module/unknown", "one"), tab("unrelated.module/unknown", "two")], 1)])
    require(next.left.slots[0].modules.length === 2 && next.singletonRecovery === undefined, "unknown declarations are not assumed singletons")
  }

  function recoveryCapacity() {
    var modules = [tab("mcp", "a"), tab("data-goblin.fileblade-mcp/mcp", "b"), tab("kurt.agent-mcp/mcp", "c")]
    for (var i = 0; i < modules.length; i++) modules[i].state.payload = "x".repeat(75 * 1024)
    var raw = { version: 1, monitorMode: "all", monitorLock: "", animations: false,
      blades: { left: { open: false, width: 380, mode: "docked", slots: [slot("large", modules, 0)] }, right: { open: false, width: 360, mode: "docked", slots: [] } } }
    var padding = layout.maximumLayoutBytes - layout.utf8Length(layout.serialized(raw, 2) + "\n") - 1
    require(padding > 0, "capacity fixture padding")
    modules[0].state.payload += "x".repeat(padding)
    require(layout.utf8Length(layout.serialized(raw, 2) + "\n") < layout.maximumLayoutBytes, "original capacity fixture fits")
    var next = layout.normalizeLayout(raw)
    require(next.singletonRecovery.length === 2 && next.singletonRecovery[1].tab.state.payload.length === 75 * 1024, "capacity never truncates recovery state")
    require(!layout.layoutWithinLimit(next), "over-capacity recovery refuses save instead of dropping evidence")
    roundTrip(next, "capacity")
  }

  function run() {
    var initial = layout.defaultLayout()
    var properties = initial.left.slots[1]
    require(properties.modules.map(function(tab) { return tab.module }).join(",") === "properties,branches" && properties.active === 0, "first open places Branches behind Properties")
    var saved = restored([probe.slot("custom", [tab("properties", "kept")], 0)])
    require(saved.left.slots[0].modules.length === 1, "saved layouts do not regain a dismissed Branches tab")
    var names = ["skills", "memory", "hooks", "mcp"]
    for (var i = 0; i < names.length; i++) {
      var name = names[i]
      var legacy = "data-goblin.fileblade-" + name + "/" + name
      var slot = layout.normalizeSlot({ id: "saved", modules: [{ module: legacy, state: { query: "custom", future: 7 } }], active: 0 }, 0, {})
      require(slot.modules[0].module === name, "slot alias " + name)
      require(slot.modules[0].state.future === 7 && slot.modules[0].state.query === "custom", "slot state " + name)
      fixture.layout = ({ left: { slots: [slot] }, right: { slots: [] } })
      require(layout.findModule(name).index === 0, "canonical singleton " + name)
      require(layout.findModule(legacy).index === 0, "legacy singleton " + name)
      require(layout.findModule("kurt.agent-" + name + "/" + name).index === 0, "older singleton " + name)
    }
    var goblins = "kurt.goblin-images/goblin-images"
    var mixed = layout.normalizeSlot({ id: "mixed", modules: [{ module: "data-goblin.fileblade-mcp/mcp", state: { query: "saved" } }, { module: goblins, state: { stackLayers: false } }], active: 1 }, 0, {})
    require(mixed.active === 1 && mixed.modules[1].module === goblins, "Goblins active tab")
    require(mixed.modules[1].state.stackLayers === false && mixed.modules[0].state.query === "saved", "mixed tab state")
    sameSlotCollisions()
    crossLayoutCollisions()
    declaredSingletonsAndRecovery()
    recoveryCapacity()
    console.log("CORE_SLOTS_PASS")
    Qt.quit()
  }

  Component.onCompleted: Qt.callLater(function() {
    try { probe.run() }
    catch (error) { console.error("CORE_SLOTS_FAIL " + error); Qt.quit() }
  })
}
