use std::{fs, path::Path};

#[test]
fn empty_workspaces_route_focus_through_open_blades() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let focus = fs::read_to_string(root.join("blades/BladeFocusController.qml"))
        .expect("read BladeFocusController.qml");
    let backend =
        fs::read_to_string(root.join("src/hyprland/blades.rs")).expect("read hyprland blades");

    assert!(focus.contains("function focusEmptyWorkspace()"));
    assert!(focus.contains("name === \"closewindow\""));
    assert!(focus.contains("name === \"workspace\""));
    assert!(focus.contains("\"--empty-only\""));
    assert!(backend.contains("if options.empty_only"));
    assert!(backend.contains("return focus_empty_workspace(direction, options, &current)"));
    assert!(backend.contains("let edge = if target_state == \"open\""));
}

#[test]
fn focusing_a_module_keeps_the_tab_already_showing_it() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let focus = fs::read_to_string(root.join("blades/BladeFocusController.qml"))
        .expect("read BladeFocusController.qml");
    let start = focus
        .find("function focusModule(")
        .expect("focusModule exists");
    let body = &focus[start..start + focus[start..].find("\n  }\n").expect("focusModule closes")];

    let active = body
        .find("host.slotActiveTab(location.edge, location.index)")
        .expect("focusModule reads the slot's active tab");
    let select = body
        .find("host.setSlotTab(location.edge, location.index, location.tab)")
        .expect("focusModule selects a tab");
    assert!(
        active < select,
        "the active tab is consulted before a tab is selected"
    );
    assert!(body.contains("location.tab = activeTab"));
}
