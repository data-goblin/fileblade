import "../../ui"

InventoryProvider {
  inventoryOptions: ({
    maximumItems: 1024, itemsKey: "definitions", healthBasis: "configuration-only",
    exactProject: false, scanArguments: ["--watch", "--no-usage"],
    usageCountsMethod: "usage-counts", usageCountsItems: false,
    usageCountsArguments: function(inventory) { return ["--json", "--project", inventory.anchorPath] },
    activityMethod: "usage", activityArguments: function() { return ["--json"] }
  })
}
