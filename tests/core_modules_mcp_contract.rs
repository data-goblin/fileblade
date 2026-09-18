use serde_json::Value;
use std::fs;
use std::path::Path;

#[test]
fn the_mcp_module_keeps_its_qml_and_helper_contract() {
    let core = Path::new(env!("CARGO_MANIFEST_DIR"));
    let root = core.join("modules/mcp");
    let module = fs::read_to_string(root.join("blades/Module.qml")).unwrap();
    let keys = fs::read_to_string(root.join("components/RescanKeyContract.qml")).unwrap();
    let provider = fs::read_to_string(root.join("Provider.qml")).unwrap();
    let shared_provider = fs::read_to_string(core.join("ui/InventoryProvider.qml")).unwrap();
    let cli = fs::read_to_string(core.join("python/agent_mcp/cli.py")).unwrap();

    for target in ["searchLoader", "treeLoader"] {
        for option in ["caseSensitive", "regex"] {
            let wanted =
                format!("target: {target}.item; property: \"{option}\"; value: root.{option}");
            assert!(module.contains(&wanted), "In: {wanted}");
        }
    }
    for shared in [
        "busy",
        "truncated",
        "applying",
        "loadError",
        "applyError",
        "watchError",
    ] {
        assert!(
            module.contains(&format!("inventory.{shared}")),
            "In: {shared}"
        );
    }
    for duplicate in [
        "boundedRows",
        "boundedMetrics",
        "scanProcess",
        "applyProcess",
        "refreshDebounce",
    ] {
        assert!(!module.contains(duplicate), "NotIn: {duplicate}");
    }
    for stale in [
        "metricKey",
        "setMetric(",
        "restoreMetric",
        "onMetricChosen",
        "property: \"metricOptions\"",
        "context.state",
    ] {
        assert!(!module.contains(stale), "NotIn: {stale}");
    }
    for callback in ["groupsFor", "leafLabel", "leafDetail", "searchText"] {
        assert!(
            module.contains(&format!("item.{callback} = function")),
            "In: {callback}"
        );
        assert!(
            !module.contains(&format!("property: \"{callback}\"; value: function")),
            "NotIn: {callback}"
        );
    }
    assert!(!root.join("Service.qml").exists());
    assert!(!root.join("HostGuard.qml").exists());

    let manifest: Value =
        serde_json::from_slice(&fs::read(root.join("source.json")).unwrap()).unwrap();
    let helper = &manifest["helpers"][0];
    assert_eq!(helper["entry"], "bin/agent-mcpctl");
    assert_eq!(
        helper["read"],
        serde_json::json!(["list", "recovery-list", "usage"])
    );
    assert_eq!(
        helper["write"],
        serde_json::json!([
            "apply",
            "prepare-remove",
            "remove-prepared",
            "restore",
            "discard",
            "usage-forget"
        ])
    );

    assert!(
        module.contains("property var context: null"),
        "In: {}",
        "property var context: null"
    );
    assert!(
        module.contains("function onOptionsToggled(nextCase, nextRegex)"),
        "In: {}",
        "function onOptionsToggled(nextCase, nextRegex)"
    );
    assert!(
        module.contains("item.editPathFor = function(entry) { return root.absoluteSource(entry) }"),
        "In: {}",
        "item.editPathFor = function(entry) { return root.absoluteSource(entry) }"
    );
    assert!(
        module.contains("path: absoluteSource(entry), paths: []"),
        "In: {}",
        "path: absoluteSource(entry), paths: []"
    );
    assert!(
        module.contains("files.navigateToLocation(context.paths.parent(path)"),
        "In: {}",
        "files.navigateToLocation(context.paths.parent(path)"
    );
    assert!(
        module.contains("item.fileActionsFor = function(entry) { return false }"),
        "In: {}",
        "item.fileActionsFor = function(entry) { return false }"
    );
    assert!(
        module.contains("inventory ? inventory.anchorPath"),
        "In: {}",
        "inventory ? inventory.anchorPath"
    );
    assert!(
        module.contains("readonly property var definitions: inventory ? inventory.items"),
        "In: {}",
        "readonly property var definitions: inventory ? inventory.items"
    );
    assert!(
        provider.contains("itemsKey: \"definitions\", healthBasis: \"configuration-only\""),
        "In: {}",
        "itemsKey: \"definitions\", healthBasis: \"configuration-only\""
    );
    assert!(
        provider.contains("exactProject: false, scanArguments: [\"--watch\"]"),
        "In: {}",
        "exactProject: false, scanArguments: [\"--watch\"]"
    );
    assert!(
        provider.contains("InventoryProvider {"),
        "In: {}",
        "InventoryProvider {"
    );
    assert!(
        shared_provider.contains("observers: Qt.binding(function() { return provider.observers })"),
        "In: {}",
        "observers: Qt.binding(function() { return provider.observers })"
    );
    assert!(module.contains("Component.onDestruction: if (attachedProvider) attachedProvider.detach(attachedContext)"), "In: {}", "Component.onDestruction: if (attachedProvider) attachedProvider.detach(attachedContext)");
    assert!(
        module.contains("onInventoryActiveChanged: syncProvider()"),
        "In: {}",
        "onInventoryActiveChanged: syncProvider()"
    );
    assert!(
        module.contains("if (!item.source) return item.path ? String(item.path) : \"\""),
        "In: {}",
        "if (!item.source) return item.path ? String(item.path) : \"\""
    );
    assert!(
        module.contains("root.absoluteSource(entry)].join(\" \")"),
        "In: {}",
        "root.absoluteSource(entry)].join(\" \")"
    );
    assert!(
        !module.contains("entry.source.path].join"),
        "NotIn: {}",
        "entry.source.path].join"
    );
    assert!(!module.contains("syntaxKeys"), "NotIn: {}", "syntaxKeys");
    assert!(
        module.contains("context.contractVersion >= 1"),
        "In: {}",
        "context.contractVersion >= 1"
    );
    assert!(
        !module.contains("context.contractVersion === 1"),
        "NotIn: {}",
        "context.contractVersion === 1"
    );
    assert!(
        module.contains("context.bladeOpen && !context.collapsed"),
        "In: {}",
        "context.bladeOpen && !context.collapsed"
    );
    assert_eq!(module.matches("active: root.inventoryActive").count(), 3);
    assert!(
        module.contains("wanted: root.inventoryActive && !!root.files"),
        "In: {}",
        "wanted: root.inventoryActive && !!root.files"
    );
    assert!(
        module.contains("setSource(root.context.ui.url(\"ArtifactBin\"), { service: root.files })"),
        "In: {}",
        "setSource(root.context.ui.url(\"ArtifactBin\"), { service: root.files })"
    );
    assert!(module.contains("item.module = \"mcp\"\n      item.context = Qt.binding(function() { return root.context })"), "In: {}", "item.module = \"mcp\"\n      item.context = Qt.binding(function() { return root.context })");
    assert!(
        module.contains("item.helperRoute = Qt.binding("),
        "In: {}",
        "item.helperRoute = Qt.binding("
    );
    assert!(
        module.contains("item.removalArguments = function(entry)"),
        "In: {}",
        "item.removalArguments = function(entry)"
    );
    assert!(!module.contains("binProcess"), "NotIn: {}", "binProcess");
    assert!(!module.contains("binCallback"), "NotIn: {}", "binCallback");
    assert!(module.contains("binLoader.item.mergeRows(definitions, binned, function(entry) { return [entry.agent, entry.scope === \"plugin\" ? \"user\" : entry.scope] })"), "In: {}", "binLoader.item.mergeRows(definitions, binned, function(entry) { return [entry.agent, entry.scope === \"plugin\" ? \"user\" : entry.scope] })");
    assert!(
        module.contains("position: Math.max(0, allRows().indexOf(entry))"),
        "In: {}",
        "position: Math.max(0, allRows().indexOf(entry))"
    );
    assert!(
        module
            .contains("groups: [entry.agent, entry.scope === \"plugin\" ? \"user\" : entry.scope]"),
        "In: {}",
        "groups: [entry.agent, entry.scope === \"plugin\" ? \"user\" : entry.scope]"
    );
    assert!(
        module.contains("metrics: entry.metrics"),
        "In: {}",
        "metrics: entry.metrics"
    );
    assert!(
        cli.contains("add_argument(\"--record-id\", required=True)"),
        "In: {}",
        "add_argument(\"--record-id\", required=True)"
    );
    assert!(
        cli.contains("add_argument(\"--payload-stdin\", action=\"store_true\", required=True)"),
        "In: {}",
        "add_argument(\"--payload-stdin\", action=\"store_true\", required=True)"
    );
    assert!(
        !cli.contains("add_argument(\"--payload\")"),
        "NotIn: {}",
        "add_argument(\"--payload\")"
    );
    assert!(
        module.contains("property: \"tabIndex\""),
        "In: {}",
        "property: \"tabIndex\""
    );
    assert!(
        module.contains("property: \"reservedLeft\""),
        "In: {}",
        "property: \"reservedLeft\""
    );
    assert!(
        module.contains("property: \"reservedRight\""),
        "In: {}",
        "property: \"reservedRight\""
    );
    assert!(
        module.contains("readonly property var view: viewLoader.item"),
        "In: {}",
        "readonly property var view: viewLoader.item"
    );
    assert!(
        module.contains("root.context.ui.url(\"PaneView\")"),
        "In: {}",
        "root.context.ui.url(\"PaneView\")"
    );
    assert!(
        module.contains("readonly property var metricOptions"),
        "In: {}",
        "readonly property var metricOptions"
    );
    assert!(
        module.contains("{ key: \"uses\", label: \"Uses\", kind: \"number\" }"),
        "In: {}",
        "{ key: \"uses\", label: \"Uses\", kind: \"number\" }"
    );
    assert!(module.contains("{ key: \"usesAgent\", label: \"Uses (agent)\", shortLabel: \"USES (AGENT)\", kind: \"number\" }"), "In: {}", "{ key: \"usesAgent\", label: \"Uses (agent)\", shortLabel: \"USES (AGENT)\", kind: \"number\" }");
    assert!(module.contains("{ key: \"usesUser\", label: \"Uses (user)\", shortLabel: \"USES (USER)\", kind: \"number\" }"), "In: {}", "{ key: \"usesUser\", label: \"Uses (user)\", shortLabel: \"USES (USER)\", kind: \"number\" }");
    assert!(
        !module.contains("label: \"Tokens"),
        "NotIn: {}",
        "label: \"Tokens"
    );
    assert!(
        module.contains(
            "function onAgentsAllRequested(entry, on) { root.toggleAllAgents(entry, on) }"
        ),
        "In: {}",
        "function onAgentsAllRequested(entry, on) { root.toggleAllAgents(entry, on) }"
    );
    assert!(
        module.contains("property: \"defaultMetric\"; value: \"uses\""),
        "In: {}",
        "property: \"defaultMetric\"; value: \"uses\""
    );
    assert_eq!(
        module
            .matches("property: \"view\"; value: root.view")
            .count(),
        2
    );
    assert!(
        module.contains("property: \"installedAgents\"; value: root.installedAgents"),
        "In: {}",
        "property: \"installedAgents\"; value: root.installedAgents"
    );
    assert!(
        module
            .contains("files && Array.isArray(files.installedAgents) ? files.installedAgents : []"),
        "In: {}",
        "files && Array.isArray(files.installedAgents) ? files.installedAgents : []"
    );
    assert!(
        module.contains("function specialMetricValue(entry, key)"),
        "In: {}",
        "function specialMetricValue(entry, key)"
    );
    assert!(
        module.contains("function appliedAgents(entry)"),
        "In: {}",
        "function appliedAgents(entry)"
    );
    assert!(
        module.contains("item.appliedAgents = function"),
        "In: {}",
        "item.appliedAgents = function"
    );
    assert!(
        module.contains("function onFilterRequested()"),
        "In: {}",
        "function onFilterRequested()"
    );
    assert!(
        module.contains("headerLoader.item.openFilter()"),
        "In: {}",
        "headerLoader.item.openFilter()"
    );
    assert!(
        module.contains("function onAgentToggled(entry, agentId, on)"),
        "In: {}",
        "function onAgentToggled(entry, agentId, on)"
    );
    assert!(
        module.contains("function onAgentsAllRequested(entry, on)"),
        "In: {}",
        "function onAgentsAllRequested(entry, on)"
    );
    assert!(
        module.contains("[\"--project\", projectPath, \"--id\", String(entry.id)]"),
        "In: {}",
        "[\"--project\", projectPath, \"--id\", String(entry.id)]"
    );
    assert!(
        module.contains("inventory.mutate(\"apply\", command)"),
        "In: {}",
        "inventory.mutate(\"apply\", command)"
    );
    assert!(
        module.contains("command.push(\"--state\", state, \"--json\")"),
        "In: {}",
        "command.push(\"--state\", state, \"--json\")"
    );
    assert!(module.contains("readonly property string status: applying ? \"Applying\u{2026}\" : (applyMessage !== \"\" ? applyMessage : statusText)"), "In: {}", "readonly property string status: applying ? \"Applying\u{2026}\" : (applyMessage !== \"\" ? applyMessage : statusText)");
    assert!(
        module.contains("property: \"status\"; value: root.status"),
        "In: {}",
        "property: \"status\"; value: root.status"
    );
    assert!(
        !module.contains("headerStatus"),
        "NotIn: {}",
        "headerStatus"
    );
    assert!(
        module.contains("\"Applying\u{2026}\""),
        "In: {}",
        "\"Applying\u{2026}\""
    );
    assert!(
        module.contains("applyMessage !== \"\" ? applyMessage : statusText"),
        "In: {}",
        "applyMessage !== \"\" ? applyMessage : statusText"
    );
    assert!(
        module.contains("return id !== own"),
        "In: {}",
        "return id !== own"
    );
    assert!(
        module.contains("item.specialMetricValue = function"),
        "In: {}",
        "item.specialMetricValue = function"
    );
    assert!(
        !module.contains("showDescriptions"),
        "NotIn: {}",
        "showDescriptions"
    );
    assert!(
        !module.contains("detailToggle"),
        "NotIn: {}",
        "detailToggle"
    );
    assert!(!module.contains("\"DESC\""), "NotIn: {}", "\"DESC\"");
    assert!(
        module.contains("visible: !!headerLoader.item && headerLoader.item.visible"),
        "In: {}",
        "visible: !!headerLoader.item && headerLoader.item.visible"
    );
    assert!(
        !module.contains("item.leafBadge"),
        "NotIn: {}",
        "item.leafBadge"
    );
    assert!(
        module.contains("function onFocusNextRequested()"),
        "In: {}",
        "function onFocusNextRequested()"
    );
    assert!(
        module.contains("function onFocusPreviousRequested()"),
        "In: {}",
        "function onFocusPreviousRequested()"
    );
    assert!(
        module.contains("Components.RescanKeyContract"),
        "In: {}",
        "Components.RescanKeyContract"
    );
    assert!(
        module.contains("item.source.redacted !== false"),
        "In: {}",
        "item.source.redacted !== false"
    );
    assert!(
        module.contains("Quickshell.env(\"CODEX_HOME\")"),
        "In: {}",
        "Quickshell.env(\"CODEX_HOME\")"
    );
    assert!(
        module.contains("if (loadError) return loadError"),
        "In: {}",
        "if (loadError) return loadError"
    );
    assert!(!module.contains("runAction("), "NotIn: {}", "runAction(");
    assert!(!module.contains("Shortcut {"), "NotIn: {}", "Shortcut {");
    assert!(
        !module.contains("Keys.priority"),
        "NotIn: {}",
        "Keys.priority"
    );
    assert_eq!(module.matches("event.accepted = true").count(), 1);
    assert!(
        keys.contains("key === Qt.Key_R && modifiers === Qt.ShiftModifier"),
        "In: {}",
        "key === Qt.Key_R && modifiers === Qt.ShiftModifier"
    );
    assert!(!module.contains("\u{00b7}"), "NotIn: the middle dot");
    assert!(
        module.contains(
            "context.metrics.options([\"off\", \"agents\", \"status\", { key: \"transport\", shortLabel: \"TYPE\" }, "
        ),
        "In: the metric options list"
    );
    assert!(
        module.contains(
            "\"updated\", \"created\", \"tokens\", \"characters\", \"words\", \"bytes\", \"summary\"])"
        ),
        "In: the metric options tail"
    );
}
