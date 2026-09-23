import "../../ui"

InventoryProvider {
  inventoryOptions: ({
    maximumItems: 256, scanArguments: ["--no-usage"], activityMethod: "usage", usageCountsMethod: "usage-counts",
    usageCountsArguments: function(inventory) { return ["--json", "--project", inventory.anchorPath].concat(inventory.projectArguments) },
    activityArguments: function(inventory) { return ["--json", "--project", inventory.anchorPath].concat(inventory.projectArguments) }
  })
}
