use std::fs;
use std::path::Path;

fn source(path: &str) -> String {
    fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
        .unwrap_or_else(|error| panic!("unable to read {path}: {error}"))
}

#[test]
fn ctrl_f_toggles_deep_search_and_focuses_its_field_from_every_focus_state() {
    let service = source("Service.qml");
    let pane = source("panes/TreePane.qml");
    let quick_nav = source("modules/files/QuickNavOverlay.qml");
    let card = source("ui/QuickNavCard.qml");

    assert!(service.contains("function toggleSearchDeep() { return setSearchDeep(!searchDeep) }"));
    assert!(pane.contains(
        "function toggleDeepSearch() {\n    if (controller.quickNavActive) controller.stopQuickNav()\n    if (mediaActive) mediaRecursive = !mediaRecursive\n    else controller.toggleSearchDeep()\n    focusSearch()\n  }"
    ));
    assert!(pane.contains(
        "if (KeyRouter.listModeAction(event, false) === \"deep\") {\n      toggleDeepSearch()\n      return true"
    ));
    assert!(pane.contains("deep: function() { toggleDeepSearch() }"));
    assert!(pane.contains(
        "onDeepToggled: { if (root.mediaActive) root.mediaRecursive = !root.mediaRecursive; else controller.toggleSearchDeep() }"
    ));
    assert!(pane.contains("Keys.priority: Keys.BeforeItem"));
    assert!(card.contains("import \"../lib/KeyRouter.js\" as KeyRouter"));
    assert!(card.contains("if (KeyRouter.listModeAction(event, false) === \"deep\")"));
    assert!(quick_nav.contains("PluginUi.QuickNavCard {"));
    assert!(quick_nav.contains("onDeepRequested: overlay.toggleDeepSearch()"));
    assert!(!quick_nav.contains("channelChip"));
    assert!(!quick_nav.contains("Enter opens the folder"));
}
