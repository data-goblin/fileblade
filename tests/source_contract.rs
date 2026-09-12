use std::collections::HashSet;
use std::ffi::OsStr;
use std::fs;
use std::io::Write;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn ignored(root: &Path, paths: &[PathBuf]) -> HashSet<PathBuf> {
    let mut found = HashSet::new();
    if paths.is_empty() {
        return found;
    }
    let Ok(mut child) = Command::new("git")
        .current_dir(root)
        .args(["check-ignore", "-z", "--stdin"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
    else {
        return found;
    };
    let mut payload = Vec::new();
    for path in paths {
        payload.extend_from_slice(path.as_os_str().as_bytes());
        payload.push(0);
    }
    let mut stdin = child.stdin.take().expect("git stdin");
    let writer = std::thread::spawn(move || stdin.write_all(&payload));
    let output = child.wait_with_output();
    let _ = writer.join();
    let Ok(output) = output else {
        return found;
    };
    if !matches!(output.status.code(), Some(0) | Some(1)) {
        return found;
    }
    for chunk in output.stdout.split(|byte| *byte == 0) {
        if !chunk.is_empty() {
            found.insert(PathBuf::from(OsStr::from_bytes(chunk)));
        }
    }
    found
}

fn files(root: &Path, extensions: &[&str]) -> Vec<PathBuf> {
    let mut pending = vec![root.to_path_buf()];
    let mut found = Vec::new();
    while let Some(directory) = pending.pop() {
        for entry in fs::read_dir(directory).expect("read source directory") {
            let entry = entry.expect("source entry");
            let path = entry.path();
            let name = path
                .file_name()
                .and_then(|value| value.to_str())
                .unwrap_or("");
            if path.is_dir() {
                if !matches!(name, "target")
                    && !name.starts_with('.')
                    && !path.ends_with("src/extension_template/files")
                {
                    pending.push(path);
                }
            } else if path
                .extension()
                .and_then(|value| value.to_str())
                .is_some_and(|extension| extensions.contains(&extension))
            {
                found.push(path);
            }
        }
    }
    found.sort();
    let skipped = ignored(root, &found);
    found.retain(|path| !skipped.contains(path));
    found
}

fn text(path: &Path) -> String {
    fs::read_to_string(path).unwrap_or_else(|error| panic!("read {}: {error}", path.display()))
}

fn module_text(root: &Path, module: &str) -> String {
    let single = root.join(format!("src/{module}.rs"));
    if single.is_file() {
        return text(&single);
    }
    files(&root.join("src").join(module), &["rs"])
        .iter()
        .map(|path| text(path))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn production_runtime_is_rust_with_one_resident_qml_process() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let qml = files(root, &["qml", "js"]);
    let maintainer_home = ["/home", "kurt"].join("/");
    let process_owners: Vec<_> = qml
        .iter()
        .filter(|path| text(path).contains("Process {"))
        .collect();
    assert_eq!(process_owners.len(), 1, "QML must own one resident process");
    assert_eq!(
        process_owners[0]
            .file_name()
            .and_then(|value| value.to_str()),
        Some("BackendClient.qml")
    );
    let backend_client = text(&root.join("controllers/BackendClient.qml"));
    assert!(backend_client.contains("stdinEnabled: true"));
    assert!(backend_client.contains("\"serve\""));
    assert!(backend_client.contains("Component.onDestruction: stop()"));
    assert_eq!(
        backend_client.matches("Quickshell.execDetached(").count(),
        2
    );
    assert!(backend_client.contains("[root.cliPath, \"_backend\", \"plugin-install\"]"));
    assert!(
        backend_client.contains("\"dim-windows\", \"--state\", \"off\", \"--after-exit\", owner")
    );

    for path in &qml {
        let source = text(path);
        assert!(
            path.ends_with("controllers/BackendClient.qml") || !source.contains("execDetached"),
            "execDetached in {}",
            path.display()
        );
        assert!(
            !source.contains("inotifywait"),
            "inotifywait in {}",
            path.display()
        );
        assert!(
            !source.contains(&maintainer_home),
            "host path in {}",
            path.display()
        );
    }
    for path in files(root, &["rs", "md", "json", "toml", "sh"]) {
        assert!(
            !text(&path).contains(&maintainer_home),
            "maintainer home in public source {}",
            path.display()
        );
    }
    assert!(!text(&root.join("fileblade")).contains(&maintainer_home));
    assert!(!text(&root.join("fileblade")).contains("python"));
    let hyprland = module_text(root, "hyprland");
    assert!(!hyprland.contains("hypr_eval"));
    assert!(!hyprland.contains("layer-rules"));
    let blade_host = text(&root.join("blades/BladeHost.qml"));
    assert!(!blade_host.contains("applyLayerRules"));
    assert!(!blade_host.contains("configreloaded"));
    let python_support = files(root, &["py"]);
    for path in &python_support {
        assert!(
            path.ends_with("python/fileblade_paths.py")
                || path.ends_with("python/fileblade_process.py")
                || path.ends_with("python/fileblade_inventory.py")
                || path.ends_with("python/fileblade_mutations.py")
                || path.ends_with("tests/test_python_paths.py")
                || path.ends_with("tests/test_python_process.py")
                || path.ends_with("tests/test_python_inventory.py")
                || path.ends_with("tests/test_python_mutations.py")
                || path.ends_with("tests/test_version_hook.py")
                || path.ends_with("tests/companion_readonly.py")
                || path.ends_with("scripts/fileblade-extension-image.py")
                || path.ends_with("tests/test_extension_image.py"),
            "Python is only shared companion support or a developer helper, not a core backend: {}",
            path.display()
        );
    }
    assert_eq!(python_support.len(), 12);
}

#[test]
fn rust_and_qml_contracts_keep_output_and_state_boundaries_explicit() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    for path in files(root, &["qml"]) {
        let source = text(&path);
        if source.contains("PluginUi.") {
            assert!(
                source.lines().any(|line| {
                    let line = line.trim();
                    line.starts_with("import \"") && line.ends_with(" as PluginUi")
                }),
                "PluginUi used without a local import in {}",
                path.display()
            );
        }
    }
    for path in files(&root.join("src"), &["rs"])
        .into_iter()
        .chain(files(&root.join("crates"), &["rs"]))
    {
        let source = text(&path);
        assert!(
            !source.contains("#[allow"),
            "allow attribute in {}",
            path.display()
        );
        assert!(
            !source.contains("println!("),
            "raw stdout in {}",
            path.display()
        );
        assert!(
            !source.contains("eprintln!("),
            "raw stderr in {}",
            path.display()
        );
    }

    let state = text(&root.join("controllers/StateController.qml"));
    assert!(state.contains("backendRequest(\"state-read\""));
    assert!(state.contains("backendRequest(\"state-write\""));
    assert!(state.contains("property var treeSort: []"));
    let layout = text(&root.join("blades/BladeHost.qml"));
    assert!(layout.contains("backendRequest(\"layout-read\""));
    assert!(layout.contains("backendRequest(\"layout-write\""));
    let favorites = text(&root.join("panes/FavoritesPanel.qml"));
    assert!(!favorites.contains("controller.trashResource"));
    let tree_pane = text(&root.join("panes/TreePane.qml"));
    assert!(tree_pane.contains("key: \"desktop-trash\""));
    assert!(!tree_pane.contains("TrashPlace"));
    assert!(tree_pane.contains("preferredHeight: header.integrated ? 0"));
    let tab_bar = text(&root.join("blades/BladeTabBar.qml"));
    assert!(tab_bar.contains("id: fileActions"));
    assert!(tab_bar.contains("text: \"×\""));
    assert!(tab_bar.contains(
        "{ key: \"close\", glyph: \"×\", label: \"Close tab\", enabled: bar.slot.tabs.length > 1 }"
    ));
    assert!(tab_bar.contains("bar.slot.requestCloseTab"));
    let slot = text(&root.join("blades/BladeSlot.qml"));
    assert!(slot.contains("pendingCloseTarget = TabIdentity.capture(tabs, index, slotId)"));
    assert!(slot.contains("if (tabs.length > 1 && TabIdentity.matches(tabs, slotId, target)) host.removeTab(edge, slotIndex, index)"));
    assert!(slot.contains("onTabsChanged: dropStaleCloseTab()"));
    assert!(slot.contains("onSlotIdChanged: { dropStaleCloseTab();"));
    assert_eq!(
        slot.matches("onSlotIdChanged").count(),
        1,
        "one handler per signal"
    );
    let location = text(&root.join("controllers/LocationController.qml"));
    assert!(location.contains(
        "var late = String(request.monitor || \"\") !== service.bladeHost.focusedMonitorName"
    ));
    let navigation = text(&root.join("controllers/NavigationController.qml"));
    assert!(navigation.contains("focusTarget = targetScreen || service.preferredScreen() || null"));
    assert!(navigation.contains(
        "var screen = controller.focusTarget
"
    ));
    assert!(tab_bar.contains("closePointer.containsMouse ? Color.urgent"));
    assert!(tab_bar.contains("last.width + Math.round(tabRow.spacing / 2)"));
    assert!(!tab_bar.contains(
        "anchors.fill: parent\n    visible: bar.dropIndex >= 0\n    color: Util.alpha(Color.accent, 0.16)"
    ));
    let blade_slot = text(&root.join("blades/BladeSlot.qml"));
    assert!(blade_slot.contains("tabs.length <= 1"));
    assert!(blade_slot.contains("id: closeTabDialog"));
    assert!(blade_slot.contains("id: moduleMenu"));
    assert!(blade_slot.contains("prompt: \"Find module…\""));
    let trash_view = text(&root.join("panes/TrashView.qml"));
    assert!(trash_view.contains("else if (dialogMode === \"empty\") controller.emptyTrash()"));
    assert!(trash_view.contains("controller.trashBusy && controller.trashCount === 0"));
    assert!(trash_view.contains("delegate: BrowserRow"));
    assert!(trash_view.contains("replace(\"T\", \" \")"));
    assert!(trash_view.contains("Keys.onPressed: function(event) { root.handleDialogKey(event) }"));
    assert!(trash_view.contains("dialogMode === \"restore-to\" ? [-1, 2, 0, 1] : [0, 1]"));
    assert!(trash_view.contains("if (!actionKeys.isRepeat(event)) acceptDialog()"));
    assert!(trash_view.contains("required property var actionKeys"));
    assert!(trash_view.contains("if (dialogMode !== \"\") return focusDialog(dialogFocusIndex)"));
    assert!(text(&root.join("panes/TreePane.qml")).contains("trashView.focusList()"));
    let hint = text(&root.join("ui/HintTip.qml"));
    assert!(!hint.contains("Qt.callLater(attach)"));
    assert!(hint.contains("onTriggered: tip.attach()"));
    assert!(!trash_view.contains("id: trashRootRow"));
    let trash = text(&root.join("controllers/TrashController.qml"));
    assert!(trash.contains("property int noticeDuration: 3000"));
    assert!(trash.contains("showNotice(\"Restored\")"));
    for command in [
        "trash-list",
        "trash-restore",
        "trash-delete",
        "trash-empty",
        "trash-prune",
    ] {
        assert!(
            trash.contains(command),
            "missing resident {command} integration"
        );
    }
    let preferences = text(&root.join("controllers/PreferencesController.qml"));
    assert!(preferences.contains("trashAnswered ? settings.trashRetentionDays : 0"));
    assert!(trash.contains("service.trashCleanupConsent"));
    assert!(state.contains("property double trashLastClearedAt: 0"));
    assert!(!state.contains("trashRetentionDays: trashRetentionDays"));
    let artifact_bin = text(&root.join("ui/ArtifactBin.qml"));
    assert!(artifact_bin.contains("property var context: null"));
    assert!(artifact_bin.contains("bin.context.hostWindow.actionKeys"));
    assert!(artifact_bin.contains("actionKeys: binKeys"));
    let action_dialog = text(&root.join("ui/ActionDialog.qml"));
    assert!(action_dialog.contains("property Item returnFocusItem: null"));
    assert!(
        action_dialog
            .contains("activeFocus && dialog.Window.window && dialog.Window.window.active")
    );
    assert!(action_dialog.contains("target.Window.window === dialog.Window.window"));
    assert!(
        text(&root.join("panes/ScriptActionRows.qml"))
            .contains("actionKeys: strip.menu.actionKeys")
    );
    for surface in ["BladeSurface", "BladeWindow"] {
        let surface = text(&root.join(format!("blades/{surface}.qml")));
        assert!(surface.contains("releaseRoot:"));
    }
    assert!(artifact_bin.contains("actions.registerRestore(module, helperRoute)"));
    assert!(!artifact_bin.contains("removeAction"));
    assert!(!artifact_bin.contains("restoreAction"));
    assert!(artifact_bin.contains("helperRoute ? \"bin-remove\" : \"bin-put\""));
    assert!(artifact_bin.contains("--follow-symlinks"));
    assert!(artifact_bin.contains("item.realpath = realpath"));
    assert!(artifact_bin.contains("function mergeRows(live, cached, groupsFor)"));
    assert!(artifact_bin.contains("entry.groups"));
    assert!(!artifact_bin.contains("Disable keeps it listed"));
    assert!(!artifact_bin.contains("Restore this, or delete it forever"));
    assert!(artifact_bin.contains("ArtifactBinListing {"));
    assert!(artifact_bin.contains("property alias rows: listing.rows"));
    assert!(artifact_bin.contains("function refresh() {\n    listing.refresh()\n  }"));
    let bin_listing = text(&root.join("ui/ArtifactBinListing.qml"));
    assert!(bin_listing.contains("cancelBackendRequest(previous.id, previous.generation, true)"));
    assert!(bin_listing.contains("Component.onDestruction: { stopping = true; suspend() }"));
    let artifact_tree = text(&root.join("ui/ArtifactTree.qml"));
    assert!(artifact_tree.contains("title: \"Symlink\""));
    assert!(artifact_tree.contains("visible: row.linkHovered"));
    assert!(artifact_tree.contains("anchorItem: row.linkAnchor"));
    assert!(artifact_tree.contains("linkOnRight: !linked"));
    assert!(!artifact_tree.contains("visible: rowHover.hovered && !row.isGroup && tree.isLinked"));
    let pane_row = text(&root.join("ui/PaneRow.qml"));
    assert!(pane_row.contains("linkHover.hovered ? Color.accent"));
    assert!(pane_row.contains("property bool linkOnRight: true"));
    assert!(pane_row.contains("row.linkOnRight ? linkGlyph : rowGlyph"));
    let file_icons = text(&root.join("lib/FileIcons.js"));
    assert!(file_icons.contains("function entryIcon(name, isDir, isSymlink, expanded, isGitRepo)"));
    assert!(file_icons.contains("if (isSymlink) return fileIcon(name, true)"));
    assert!(artifact_tree.contains("target.slice(0, -9)"));
}

#[test]
fn previews_are_bounded_stable_scrollable_and_pointer_scoped() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let properties = text(&root.join("panes/PropertiesPane.qml"));
    assert!(properties.contains("onImagePreviewKeyChanged: requestThumbnail()"));
    assert!(properties.contains("controller.backendRequest(\"thumbnail\", args, generation"));
    assert!(properties.contains("localFileUrl(String(response.path))"));
    assert!(!properties.contains("localFileUrl(String(entry.path || \"\")) + \"?v=\""));
    assert!(properties.contains("!entry.is_symlink"));
    assert!(properties.contains("[\"image/jpeg\", \"image/png\", \"image/webp\"]"));
    assert!(properties.contains("imagePreviewByteLimit: 16 * 1024 * 1024"));
    assert!(properties.contains("+ \"?v=\" + encodeURIComponent(key)"));
    assert!(properties.contains("readonly property string textPreviewKey:"));
    assert!(properties.contains("onTextPreviewKeyChanged: requestPreview()"));
    assert!(properties.contains("previewLoadingTimer.restart()"));
    assert!(
        properties.contains("controller.cancelBackendRequest(previewRequestId, previewGeneration)")
    );
    assert!(properties.contains("key !== root.textPreviewKey"));
    assert!(properties.contains("interactive: !filePreview.hovered"));
    assert!(properties.contains("visible: !filePreview.hovered && (hasAbove || hasBelow)"));
    assert!(!properties.contains("Load preview"));
    assert!(!properties.contains("approvedImagePreviewKey"));
    assert!(!properties.contains("previewSource: isImage ?"));

    let preview = text(&root.join("ui/FilePreview.qml"));
    assert!(preview.contains("retainWhileLoading: true"));
    assert!(preview.contains("cache: true"));
    assert!(preview.contains("Flickable {"));
    assert!(preview.contains("interactive: false"));
    assert!(preview.contains("HoverHandler { id: previewHover }"));
    assert!(preview.contains("WheelHandler {"));
    assert!(preview.contains("blocking: true"));
    assert!(preview.contains("event.accepted = true"));
    assert!(preview.contains("ScrollEdgeFade {"));
    let application_icon = text(&root.join("ui/SafeApplicationIcon.qml"));
    assert!(application_icon.contains("FileIcons.safeThemeIconName(iconName)"));
    assert!(application_icon.contains("property string trustedIconSource:"));
    assert!(application_icon.contains("colorizationColor: root.iconColor"));
    assert!(application_icon.contains("shaders/LuminanceGlyph.frag.qsb"));
    assert!(application_icon.contains("shaders/DarkGlyph.frag.qsb"));
    assert!(application_icon.contains("icon.status !== Image.Ready"));
    assert!(
        application_icon.contains("root.resolvedSource !== \"\" && icon.status === Image.Ready")
    );
    for path in ["panes/MenuButton.qml", "ui/DropWheel.qml"] {
        let source = text(&root.join(path));
        assert!(source.contains("SafeApplicationIcon {"));
        assert!(!source.contains("icon.indexOf(\"file://\")"));
        assert!(!source.contains("icon.charAt(0) === \"/\""));
    }
    let menu = text(&root.join("panes/FileActionsMenu.qml"));
    assert!(!menu.contains("SafeApplicationIcon {"));
    assert!(menu.contains("appIcon: icon"));
}

#[test]
fn scroll_indicators_are_thin_shared_and_the_trees_carry_a_marked_ruler() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let indicator = text(&root.join("ui/AccentScrollBar.qml"));
    assert!(indicator.contains("policy: ScrollBar.AsNeeded"));
    assert!(indicator.contains("implicitWidth: 1"));
    assert!(indicator.contains("color: Color.accent"));

    for path in files(root, &["qml"]) {
        assert!(
            !text(&path).contains("ScrollBar.vertical: ScrollBar"),
            "stock scrollbar in {}",
            path.display()
        );
    }

    for path in [
        "panes/TreePane.qml",
        "panes/TrashView.qml",
        "panes/FavoritesPanel.qml",
        "panes/PropertiesPane.qml",
    ] {
        assert!(
            !text(&root.join(path)).contains("ScrollBar.vertical:"),
            "attached scrollbar returned to {path}"
        );
    }

    let ruler = text(&root.join("ui/MarkedScrollBar.qml"));
    assert!(ruler.contains("visible: scrollable"));
    assert!(ruler.contains("width: Style.space(4)"));
    assert!(ruler.contains("color: Color.accent"));
    assert!(ruler.contains("color: modelData.color"));
    assert!(ruler.contains("function scrollToPointer(y)"));
    assert!(ruler.contains("readonly property real awayOpacity: 0.5"));
    assert!(ruler.contains(
        "opacity: ScrollMarks.inView(modelData.fraction, bar.viewRange) ? 1 : bar.awayOpacity"
    ));

    let marks = text(&root.join("lib/ScrollMarks.js"));
    assert!(marks.contains("var RANKS = { D: 4, U: 4, M: 3, T: 3, A: 2, \"?\": 2, R: 1, C: 1 }"));
    assert!(marks.contains("function collect(source, slots, statusOf)"));

    let pane = text(&root.join("panes/TreePane.qml"));
    assert!(pane.contains("PluginUi.MarkedScrollBar {"));
    assert!(pane.contains("marks: root.activeList === treeList ? root.gitMarks : []"));
    assert!(pane.contains("ScrollMarks.collect(controller.treeModel, scrollRuler.slots)"));
    assert!(pane.contains("controller.gitStatusColor(collected[i].status)"));
    assert!(pane.contains("if (!controller.gitEnabled || !controller.scrollMarks) {"));

    let state = text(&root.join("controllers/StateController.qml"));
    assert!(state.contains("property bool scrollMarks: true"));
    assert!(state.contains("scrollMarks: service.boolValue(config.scrollMarks, true),"));
    let service = text(&root.join("Service.qml"));
    assert!(service.contains("function setScrollMarks(value)"));
    let ipc = text(&root.join("controllers/FileTreeIpc.qml"));
    assert!(ipc.contains("scrollMarks: service.scrollMarks,"));
    assert!(ipc.contains("function setScrollMarks(enabled: string): string {"));
    let settings = text(&root.join("modules/files/FilesSettings.qml"));
    assert!(settings.contains("label: \"Git marks on the scroll ruler\""));
    assert!(settings.contains("root.controller.setScrollMarks(!root.controller.scrollMarks)"));

    let tree = text(&root.join("ui/ArtifactTree.qml"));
    assert!(tree.contains("MarkedScrollBar {"));
    assert!(
        tree.contains("property var rowMark: function(item) { return tree.defaultMark(item) }")
    );
    assert!(
        tree.contains("marksEnabled ? ScrollMarks.collect(rows, ruler.slots, markStatus) : []")
    );
    assert!(
        tree.contains("readonly property bool marksEnabled: !files || files.scrollMarks !== false")
    );
}

#[test]
fn settings_rows_sit_under_muted_group_headings() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let group = text(&root.join("ui/SettingsGroup.qml"));
    assert!(group.contains("readonly property bool settingsGroup: true"));
    assert!(group.contains("text: title.toUpperCase()"));

    let files = text(&root.join("modules/files/FilesSettings.qml"));
    for title in ["Tree", "Git", "Trash and drives"] {
        assert!(
            files.contains(&format!("PluginUi.SettingsGroup {{ title: \"{title}\" }}")),
            "Files settings lack the {title} group"
        );
    }
    let tree = files.find("title: \"Tree\"").unwrap();
    let git = files.find("title: \"Git\"").unwrap();
    let trash = files.find("title: \"Trash and drives\"").unwrap();
    assert!(tree < git && git < trash);
    let position = |label: &str| files.find(&format!("label: \"{label}\"")).unwrap();
    for label in [
        "Column",
        "Hidden files",
        "Property icons",
        "Color applies to",
        "Folder context",
        "Mode badge",
    ] {
        assert!(
            position(label) > tree && position(label) < git,
            "{label} belongs under Tree"
        );
    }
    for label in ["Git status", "Git marks on the scroll ruler"] {
        assert!(
            position(label) > git && position(label) < trash,
            "{label} belongs under Git"
        );
    }
    for label in ["Confirm trash", "Trash retention", "Show system volumes"] {
        assert!(
            position(label) > trash,
            "{label} belongs under Trash and drives"
        );
    }

    let section = text(&root.join("blades/BladeModuleSection.qml"));
    assert!(section.contains("if (child.settingsGroup === true) {"));
    assert!(section.contains("function settleGroup(heading, hits)"));
    assert!(section.contains("child.groupLead = group !== \"\" && group !== lastGroup"));
    let sheet = text(&root.join("blades/BladeSettings.qml"));
    assert!(sheet.contains("if (item.group !== undefined) parts.push(String(item.group))"));

    let form = text(&root.join("ui/SettingsForm.qml"));
    assert!(form.contains("SettingsGroup {"));
    assert!(form.contains("visible: entry.groupLead"));
    let definitions = text(&root.join("lib/Definitions.js"));
    assert!(definitions.contains("group: textField(raw.group, \"\", MAXIMUM_LABEL_LENGTH).trim()"));
    let extensions = text(&root.join("EXTENSIONS.md"));
    assert!(extensions.contains("group:         max 64;"));
}

#[test]
fn tree_refreshes_keep_surviving_rows_and_the_scroll_anchor() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let rows = text(&root.join("lib/KeyedRows.js"));
    assert!(rows.contains("function syncSegment(model, start, count, rows, keyField)"));
    assert!(rows.contains("model.setProperty(index, field, row[field])"));

    let controller = text(&root.join("controllers/TreeController.qml"));
    assert!(controller.contains("service.treeRowsReplacing()\n    treeModel.clear()"));
    assert!(controller.contains(
        "KeyedRows.syncSegment(treeModel, parentIndex + 1, endIndex - parentIndex - 1, replacement, \"path\")"
    ));
    assert!(controller.contains("if (stats.structural) markTreeStructureChanged()"));
    assert!(controller.contains("if (stats.updated > 0) treeRowsRevision++"));
    assert!(
        !controller.contains("if (removeCount > 0) treeModel.remove(parentIndex + 1, removeCount)")
    );
    assert!(controller.contains("readonly property bool treeLoading:"));

    let service = text(&root.join("Service.qml"));
    assert!(service.contains("signal treeRowsReplacing()"));
    assert!(service.contains("readonly property alias treeLoading: treeController.treeLoading"));
    assert!(service.contains("property alias treeRowsRevision: treeController.treeRowsRevision"));

    let anchor = text(&root.join("ui/ListAnchor.qml"));
    assert!(anchor.contains("view.positionViewAtIndex(index, ListView.Beginning)"));
    assert!(anchor.contains("if (loading) return false"));

    let pane = text(&root.join("panes/TreePane.qml"));
    assert!(pane.contains("PluginUi.ListAnchor {"));
    assert!(
        pane.contains(
            "function onTreeRowsReplacing() { if (treeList.visible) treeAnchor.capture() }"
        )
    );
    assert!(pane.contains("onMovementStarted: treeAnchor.clear()"));
    assert!(pane.contains("if (!root.revealPending) return"));
    assert!(pane.contains(
        "function onSelectedPathChanged() { root.revealPending = true; root.restoreTreeCursor() }"
    ));
    assert!(!pane.contains("function onTreeStructureRevisionChanged() { root.restoreTreeCursor() }\n    function onRootPathChanged"));

    let tree = text(&root.join("ui/ArtifactTree.qml"));
    assert!(tree.contains("if (!syncingRows) Qt.callLater(showCurrent)"));
    assert!(tree.contains("if (!kept) Qt.callLater(showCurrent)"));
    assert!(tree.contains("ListAnchor {"));
    assert!(tree.contains("onMovementStarted: anchor.clear()"));
}

#[test]
fn detached_blades_expose_edge_redocking_and_window_toggle_routing() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let window = text(&root.join("blades/BladeWindow.qml"));
    assert!(window.contains("window.host.redock(window.edge, window.screen)"));
    assert!(window.contains("anchors.left: window.isRight ? undefined : parent.left"));
    assert!(window.contains("anchors.right: window.isRight ? parent.right : undefined"));

    let host = text(&root.join("blades/BladeHost.qml"));
    assert!(host.contains("function windowToggle()"));
    assert!(host.contains("dispatchWindow([\"--action\", \"float\"])"));

    let ipc = text(&root.join("controllers/FileTreeIpc.qml"));
    for name in [
        "controllers/FileTreeIpc.qml",
        "controllers/PickerController.qml",
        "controllers/NavigationController.qml",
        "controllers/SearchController.qml",
        "controllers/ActionMenuController.qml",
        "controllers/DropWheelController.qml",
        "Service.qml",
        "blades/BladeHost.qml",
    ] {
        assert!(
            !text(&root.join(name)).contains("Quickshell.screens[0]"),
            "{name} must resolve screens through BladeLayout.preferredScreen/referenceScreen"
        );
    }
    let layout = text(&root.join("blades/BladeLayout.qml"));
    assert!(layout.contains("import \"../lib/MonitorMode.js\" as MonitorMode"));
    assert!(
        layout.contains(
            "Hyprland.focusedMonitor ? String(Hyprland.focusedMonitor.name || \"\") : \"\""
        )
    );
    assert!(layout.contains("function panelActiveFor(panelScreen, edge)"));
    assert!(layout.contains("function noteOpened(edge)"));
    assert!(layout.contains("monitorLock: monitorLock, animations: animateBlades"));
    assert!(ipc.contains("function setMonitorMode(mode: string, monitor: string): string"));
    assert!(ipc.contains("focusedMonitor: bladeHost.focusedMonitorName,"));
    assert!(ipc.contains("bladeScreens: { left: bladeHost.bladeScreenName(\"left\"), right: bladeHost.bladeScreenName(\"right\") },"));
    assert!(
        ipc.contains("if (String(monitor || \"\") !== \"\" && !screen) return \"unknown-monitor\"")
    );
    assert!(ipc.contains("if (!target) return \"off-screen\""));
    assert!(ipc.contains("pendingTrashCount: Array.isArray(service.pendingTrashPaths) ? service.pendingTrashPaths.length : 0,"));
    let settings_sheet = text(&root.join("blades/BladeSettings.qml"));
    assert!(settings_sheet.contains("label: \"Monitors\""));
    assert!(settings_sheet.contains("PluginUi.DropdownRow {"));
    let dropdown = text(&root.join("ui/DropdownRow.qml"));
    assert!(dropdown.contains("OptionPopup {"));
    assert!(dropdown.contains("checked: String(option.key) === row.value"));
    let focus = text(&root.join("blades/BladeFocusController.qml"));
    assert!(focus.contains("function reconcileOwnership()"));
    let surface_text = text(&root.join("blades/BladeSurface.qml"));
    assert!(
        surface_text.contains("openedAt = Date.now()\n      pointerRefocusRequired = true"),
        "a surface that maps under a stationary pointer must wait for movement before taking focus"
    );
    assert!(
        !focus.contains("focusedmon\" && host.monitorMode"),
        "blades must not follow monitor focus; they stay where they were invoked"
    );
    assert!(focus.contains("if (!isWindowMode(target) && screen && isOpen(target) && !host.panelActiveFor(screen, target)) return false"));
    let host_text = text(&root.join("blades/BladeHost.qml"));
    assert!(host_text.contains("if (desired) bladeLayout.noteOpened(target)"));
    assert!(
        host_text.contains(
            "if (mode === \"locked\" && !screenNamed(wanted)) return \"unknown-monitor\""
        )
    );
    assert!(
        surface_text.contains("readonly property int bladeWidth: Math.max(host.minimumWidth, Math.min(liveWidth > 0 ? liveWidth : storedWidth, surfaceWidth))"),
        "each surface clamps its rendered width to its own screen without rewriting the stored width"
    );
    assert!(
        ipc.contains("return bladeHost.toggleBladeFocus(edge, bladeHost.preferredScreen(edge))")
    );
    assert!(ipc.contains(
        "return bladeHost.toggleBladeFocus(\"left\", bladeHost.preferredScreen(\"left\"))"
    ));
    assert!(ipc.contains("? \"focused\" : \"no-screen\""));
    assert!(focus.contains("? \"opened\" : \"no-screen\""));
    assert!(layout.contains("onLayoutChanged: adoptInvocationScreens()"));
    assert!(layout.contains(
        "if (previous === \"\" && focusedMonitorName !== \"\") adoptInvocationScreens()"
    ));
    assert!(host_text.contains("function validateScreenOwners()"));
    assert!(host_text.contains("function onScreensChanged() { host.validateScreenOwners() }"));
    assert_eq!(
        focus
            .matches("\"--left-monitor\", host.bladeScreenName(\"left\"),")
            .count(),
        2
    );
    let native_window = text(&root.join("blades/BladeWindow.qml"));
    assert!(native_window.contains("screen: creationScreen"));
    assert!(native_window.contains("if (windowMode) chooseCreationScreen()"));
    assert!(
        native_window.contains("window.host.reportFocus(window.edge, activeFocus, window.screen)")
    );
    assert!(focus.contains("if (screen) focusedScreen = screen"));
    let wheel = text(&root.join("controllers/DropWheelController.qml"));
    assert!(wheel.contains("wheelScreen = targetScreen || service.referenceScreen(null)"));
    assert!(focus.contains("function bladePointerExited(edge, screen)"));
    assert!(focus.contains("service.backendRequest(\"hover-target\""));
    assert!(!focus.contains("hover-watch"));
    assert!(focus.contains("BladePointerFocusWatch"));
    let surface = text(&root.join("blades/BladeSurface.qml"));
    assert!(surface.contains("HyprlandFocusGrab"));
    assert!(surface.contains("active: surface.bladeOpen && !surface.keyboardFocusReleased"));
    assert!(surface.contains("onCleared: surface.handleFocusGrabCleared()"));
    assert!(surface.contains("HoverHandler"));
    assert!(surface.contains(
        "if (!surface.pointerRefocusRequired && !surface.bladeFocused && !surface.host.dragActive)"
    ));
    assert!(surface.contains("WlrLayershell.layer: WlrLayer.Top"));
    assert!(surface.contains("keyboardFocusReleased ? WlrKeyboardFocus.None"));
    assert!(surface.contains(": WlrKeyboardFocus.OnDemand"));
    assert!(!surface.contains("WlrKeyboardFocus.Exclusive"));
    assert!(!surface.contains("keyboardFocusForced"));
    assert!(surface.contains("visible: panelEnabled && !windowMode && (bladeOpen || !parked)"));
    let slot = text(&root.join("blades/BladeSlot.qml"));
    assert!(slot.contains("slot.activeFocus && host.focusedEdge === edge"));
    assert!(slot.contains("if (context.collapsed || ownsFocus"));
    assert!(
        !slot.contains("slot.context"),
        "QML ids are lexical, not slot properties"
    );

    let ipc = text(&root.join("controllers/FileTreeIpc.qml"));
    assert!(ipc.contains("function windowToggle(): string"));
    assert!(ipc.contains("target: \"data-goblin.fileblade\""));
    assert!(ipc.contains("target: \"data-goblin.fileblade.control\""));
    assert!(!ipc.contains("requestCapability"));
    assert!(!ipc.contains("capabilityResult"));
    assert!(!ipc.contains("consumeCapability"));
    assert!(ipc.contains("function renameSelection(name: string): string"));
    assert!(ipc.contains("function trashSelection(): string"));
    assert!(!ipc.contains("blades: bladeHost.layoutDocument().blades"));
    let read_handler = ipc
        .split("property IpcHandler readHandler")
        .nth(1)
        .expect("read-only IPC handler");
    for method in [
        "paste",
        "moveSelectionTo",
        "renameSelection",
        "createEntry",
        "trashSelection",
        "undo",
        "redo",
        "selectEntries",
        "setRoot",
    ] {
        assert!(
            !read_handler.contains(&format!("function {method}(")),
            "{method} leaked onto the read-only IPC target"
        );
    }

    let backend = module_text(root, "hyprland");
    assert!(backend.contains("hl.dsp.window.float({ action = \\\"toggle\\\" })"));
}

#[test]
fn trash_asks_first_with_cancel_selected_unless_the_setting_is_off() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let state = text(&root.join("controllers/StateController.qml"));
    let operations = text(&root.join("controllers/OperationController.qml"));
    let tree = text(&root.join("panes/TreePane.qml"));
    let properties = text(&root.join("panes/PropertiesPane.qml"));
    let menu = text(&root.join("panes/FileActionsMenu.qml"));
    let settings = text(&root.join("modules/files/FilesSettings.qml"));
    assert!(state.contains("property bool confirmTrash: true"));
    let focus = text(&root.join("blades/BladeFocusController.qml"));
    assert!(focus.contains("function toggleBladeFocus(edge, targetScreen)"));
    assert!(
        !focus.contains("return \"focused\""),
        "toggleBladeFocus must close an open blade in one press instead of focusing it"
    );
    assert!(operations.contains("function requestTrash(paths)"));
    assert!(operations.contains("if (!service.confirmTrash) return trashSelection(targets)"));
    assert!(tree.contains("trash: function() { controller.requestTrash() }"));
    let service = text(&root.join("Service.qml"));
    assert!(service.contains("property var pendingTrashPaths: []"));
    assert!(service.contains("function resolveTrashConfirmation(confirm)"));
    assert!(operations.contains("service.pendingTrashPaths = targets.slice()"));
    assert!(service.contains("property int trashConfirmationSerial: 0"));
    assert!(operations.contains("service.trashConfirmationSerial++"));
    let binding = text(&root.join("ui/TrashConfirmationBinding.qml"));
    assert!(binding.contains("if (binding.dialog.opened) binding.dialog.close()"));
    assert!(
        binding
            .contains("binding.requestSerial = Number(binding.controller.trashConfirmationSerial)")
    );
    assert!(binding.contains("if (!current) {"));
    assert!(binding.contains("onPaneVisibleChanged: if (!paneVisible) retire()"));
    assert!(binding.contains("Component.onDestruction: retire()"));
    assert!(tree.contains("PluginUi.TrashConfirmationBinding {"));
    assert!(tree.contains("trashConfirmation.resolve(key)"));
    assert!(tree.contains("trashConfirmation.resolve(\"cancel\")"));
    assert!(!tree.contains("function onTrashConfirmationRequested(paths)"));
    assert!(
        !tree.contains("property var pendingPaths: []"),
        "the trash confirmation must not keep per-screen pending paths"
    );
    assert!(!tree.contains("openMenuForCurrent(view, \"trash\")"));
    assert!(tree.contains("[{ key: \"cancel\", label: \"Cancel\" }, { key: \"trash\", label: \"Move to Trash\", danger: true }]"));
    assert!(properties.contains("trash: function() { controller.requestTrash() }"));
    assert!(menu.contains("controller.requestTrash(root.paths)"));
    assert!(settings.contains("label: \"Confirm trash\""));
}

#[test]
fn git_metadata_refreshes_are_scoped_paced_and_free_of_git_directory_noise() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let service = text(&root.join("Service.qml"));
    let tree = text(&root.join("controllers/TreeController.qml"));
    let watch = text(&root.join("controllers/WatchController.qml"));
    let cache = text(&root.join("src/git/cache.rs"));
    let filesystem = text(&root.join("src/filesystem/mod.rs"));

    assert!(
        tree.contains("function scheduleGitMetadataRefresh(reason, repoRoot)"),
        "a git refresh must name the repository that changed"
    );
    assert!(service.contains(
        "function scheduleGitMetadataRefresh(reason, repoRoot) { treeController.scheduleGitMetadataRefresh(reason, repoRoot) }"
    ));
    assert!(
        tree.contains("function scopedGitMetadataPaths(scopeRoots)")
            && tree.contains("gitMetadataFullSweepPending"),
        "event driven refreshes must be scoped to the dirty repositories"
    );
    assert!(
        !tree.contains("Qt.callLater(function() { service.requestVisibleGitMetadataRefresh"),
        "a queued git refresh must go through the debounce timer, never an unpaced Qt.callLater"
    );
    assert!(
        tree.contains("gitStatusPollBackoff") && tree.contains("gitStatusPollBackoffLimit"),
        "the git status poll must back off while it keeps finding nothing"
    );
    assert!(tree.contains("(service.watcherRunning ? 6 : 1)"));
    assert!(cache.contains("const CACHE_CAPACITY: usize = 64"));
    assert!(cache.contains("&& !refresh"));
    assert!(filesystem.contains("cached_git_worktree_status("));
    assert!(tree.contains("arguments.push(\"--fresh-git\")"));
    assert!(!watch.contains("--fresh-git"));
    assert!(!tree.contains("hasUndecoratedGitRepositoryRows"));
    assert!(
        tree.contains("function mergedRepositoryDirectories(repositories)"),
        "a scoped batch must merge repositories instead of replacing the known set"
    );
    assert!(
        !tree.contains("selectedCount"),
        "TreeController does not declare selectedCount; use selectedPaths.length"
    );
    assert!(
        watch.contains("function gitDirectoryEntryMatters(changedPath, gitDirectory)")
            && watch.contains("gitDecorationEntries"),
        "git directory writes that cannot change decoration must not trigger git"
    );
    for entry in ["index", "HEAD", "packed-refs"] {
        assert!(
            watch.contains(&format!("\"{entry}\"")),
            "{entry} must still count as a decoration change"
        );
    }
}

#[test]
fn creation_time_is_requested_only_when_the_tree_needs_it() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let tree = text(&root.join("controllers/TreeController.qml"));
    let navigation = text(&root.join("controllers/NavigationController.qml"));
    assert!(tree.contains("service.priorityColumns.indexOf(\"created\") >= 0"));
    assert!(tree.contains("key === \"created\""));
    assert!(tree.contains("filter.created !== undefined"));
    assert!(tree.contains("arguments.push(\"--include-created\")"));
    assert!(navigation.contains("if (createdChanged) service.refreshTree()"));
}

#[test]
fn resident_backend_reserves_and_prioritizes_an_interactive_read_lane() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let client = text(&root.join("controllers/BackendClient.qml"));
    let queue = text(&root.join("lib/RequestQueue.js"));
    assert!(client.contains("RequestQueue.interactive(command, options)"));
    assert!(client.contains("RequestQueue.nextWaitingIndex(priorities, inFlight, limits)"));
    assert!(queue.contains("maximumConcurrency(limits) - 1"));
    assert!(queue.contains("\"children-window\""));
    assert!(queue.contains("\"project-root\""));
    assert!(!queue.contains("\"search\","));
    assert!(!queue.contains("\"git-metadata-batch\","));
}

#[test]
fn edge_whitespace_in_a_typed_name_reaches_the_backend() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let menu = text(&root.join("panes/FileActionsMenu.qml"));

    // The backend keeps a name byte for byte, so the dialog must not trim the
    // value it submits. A trailing space is a legal file name.
    assert!(
        menu.contains("var value = PathText.pathText(controller.actionInput)"),
        "the action dialog must submit the typed name unmodified"
    );
    assert!(
        !menu.contains("var value = controller.actionInput.trim()"),
        "trimming the submitted name drops a legal trailing space"
    );
    // Emptiness is still whitespace-only, both for the submit guard and for
    // whether the confirm button is live.
    assert!(menu.contains("controller.actionInput.trim() !== \"\""));
    let path_text = text(&root.join("lib/PathText.js"));
    assert!(path_text.contains("text.trim() === \"\" ? \"\" : text"));
}

#[test]
fn directory_emptiness_is_not_tracked_anywhere() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(!root.join("controllers/EmptyProbeController.qml").exists());
    assert!(!root.join("src/filesystem/empty.rs").exists());
    for relative in [
        "Service.qml",
        "panes/BrowserRow.qml",
        "controllers/TreeController.qml",
        "controllers/SelectionController.qml",
    ] {
        let source = text(&root.join(relative));
        assert!(
            !source.contains("isEmpty") && !source.contains("emptyKnown"),
            "{relative} still tracks directory emptiness"
        );
    }
}

#[test]
fn notes_module_is_bundled_and_bounded() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let definition: serde_json::Value =
        serde_json::from_str(&text(&root.join("modules/notes/blade.json"))).unwrap();
    assert_eq!(definition["id"], "notes");
    assert_eq!(definition["hostContract"], 1);
    assert_eq!(definition["singleton"], true);
    assert_eq!(definition["entry"], "Module.qml");
    let layout = text(&root.join("blades/BladeLayout.qml"));
    assert!(
        layout.contains("\"kurt.notes/notes\": \"notes\""),
        "layouts saved while Notes was a satellite must resolve to the bundled module"
    );
    let state = text(&root.join("modules/notes/NotesState.js"));
    assert!(state.contains("var CAP_BYTES = 65536"));
    assert!(state.lines().next().unwrap().contains(".pragma library"));
    for name in [
        "utf8Length",
        "clampToBytes",
        "normalize",
        "hydration",
        "persistPlan",
        "statusText",
    ] {
        assert!(state.contains(&format!("function {name}(")), "{name}");
    }
    let module = text(&root.join("modules/notes/Module.qml"));
    assert!(module.contains("Component.onDestruction: flush()"));
    assert!(module.contains("onActiveFocusChanged: if (!activeFocus) module.flush()"));
    assert!(module.contains("if (!active) flush()"));
    assert!(module.contains("context.collapsed === true"));
    assert!(module.contains("context.bladeOpen !== false"));
    assert!(module.contains("KeyPlan.isTabCycle(event.key, event.modifiers)"));
    assert!(module.contains("context.state.set(\"text\", next)"));
    assert!(module.contains("textFormat: TextEdit.PlainText"));
    assert!(module.contains("NoteTabs {"));
    assert!(!module.contains("Markdown"));
    assert!(!module.contains("RichText"), "notes stay plain text");
    for (offset, _) in module.match_indices("context.state.set(\"") {
        let key = module[offset + "context.state.set(\"".len()..]
            .split('"')
            .next()
            .unwrap_or_default();
        assert!(
            key == "text",
            "Notes writes slot key {key}; only its bounded notebook document is allowed"
        );
    }
    for forbidden in [
        "FileView",
        "Process",
        "Quickshell.execDetached",
        "XMLHttpRequest",
        "Qt.openUrlExternally",
    ] {
        assert!(!module.contains(forbidden), "{forbidden}");
    }
    let plan = text(&root.join("modules/notes/KeyPlan.js"));
    assert!(plan.contains("function isTabCycle(") && plan.contains("function editorAction("));
    assert!(plan.contains("Qt.Key_Backtab"));
}

#[test]
fn update_check_is_opt_out_bounded_and_manual_in_the_footer() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let controller = text(&root.join("controllers/UpdateController.qml"));
    assert!(
        !controller.contains("import Quickshell"),
        "the controller stays testable under qmltestrunner"
    );
    assert!(controller.contains("readonly property int checkIntervalMs: 6 * 60 * 60 * 1000"));
    assert!(controller.contains("host.config.checkUpdates !== false"));
    assert!(controller.contains("service.backendRequest(\"update-check\""));
    assert!(!controller.contains("update-apply"));
    assert!(!controller.contains("function apply("));
    assert!(
        controller
            .contains("FileBlade only checks for updates; it does not install them while running.")
    );
    assert!(controller.contains("function onStateReadyChanged() { if (controller.service.stateReady) controller.checkIfStale() }"));
    let service = text(&root.join("Service.qml"));
    assert!(service.contains("onAnyOpenChanged: if (anyOpen) updateController.checkIfStale()"));
    let registry = text(&root.join("blades/BladeRegistry.qml"));
    assert!(registry.contains("function providerSources()"));
    let state = text(&root.join("controllers/StateController.qml"));
    assert!(state.contains("property double updateCheckedAt: 0"));
    assert!(state.contains("updateCheckedAt: updateCheckedAt"));
    assert!(state.contains("function markUpdateChecked(timestamp)"));
    assert!(controller.contains("service.markUpdateChecked(Date.now())"));
    let surface = text(&root.join("blades/BladeSurface.qml"));
    assert!(surface.contains("id: footerControls"));
    assert!(surface.contains("parent.width - footerControls.width - Style.space(26)"));
    assert!(surface.contains("updates.dialogLines()"));
    assert!(!surface.contains("updates.apply("));
    let updates = module_text(root, "updates");
    assert!(updates.contains("const MAX_REPOSITORIES: usize = 16;"));
    assert!(updates.contains("\"GIT_TERMINAL_PROMPT\", \"0\""));
    for forbidden in [
        "\"merge\"",
        "\"reset\"",
        "cargo build",
        "plugin\", \"validate",
        "rescanPlugins",
        ".spawn_detached()",
    ] {
        assert!(
            !updates.contains(forbidden),
            "update checker retained mutation path: {forbidden}"
        );
    }
    let backend = text(&root.join("src/backend/mod.rs"));
    assert!(!backend.contains("UpdateApply"));
}

#[test]
fn contributed_blade_modules_receive_their_singleton_provider_service() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let registry = text(&root.join("blades/BladeRegistry.qml"));
    assert!(registry.contains("providerId: boundedText(idPrefix, \"\", maximumIdLength)"));
    assert!(
        registry.contains(
            "normalizedModule(contributed[i], directory, \"plugin:\" + pluginId, pluginId)"
        )
    );
    assert!(registry.contains(
        "var directory = sourceDir === undefined || sourceDir === null || sourceDir === \"\" ? manifest.__sourceDir : sourceDir"
    ));
    assert!(
        registry.contains("property var catalogProviders: []"),
        "the registry accepts providers the shell no longer discloses"
    );
    let host = text(&root.join("blades/BladeHost.qml"));
    assert!(
        host.contains("signal bladeOpened(string edge)")
            && host.contains("if (desired) bladeOpened(target)"),
        "opening one blade is observable even while another blade is already open"
    );
    let catalog = text(&root.join("controllers/ExtensionCatalog.qml"));
    assert!(catalog.contains("function requestRefresh()"));
    assert!(
        catalog.contains("path.indexOf(\"/plugins\") >= 0 || path.indexOf(\"shell.json\") >= 0"),
        "an enable or disable that only rewrites the shell configuration wakes the catalog"
    );
    let service_wiring = text(&root.join("Service.qml"));
    assert!(
        service_wiring.contains("function onBladeOpened(edge) { if (service.backendReady) extensionCatalog.refreshIfStale() }"),
        "a blade opening re-reads a stale catalog"
    );
    assert!(
        service_wiring.contains("service.home + \"/.config/omarchy\"")
            && !service_wiring.contains("/.config/omarchy/shell.json\""),
        "the catalog watches directories, which is all the subscription accepts"
    );

    let slot = text(&root.join("blades/BladeSlot.qml"));
    assert!(slot.contains(
        "providerId: slot.moduleInfo ? String(slot.moduleInfo.providerId || \"\") : \"\""
    ));

    let context = text(&root.join("blades/BladeContext.qml"));
    assert!(context.contains("readonly property var providerService: service(providerId)"));
    assert!(context.contains("typeof shell.serviceFor === \"function\""));
    assert!(context.contains("shell.serviceFor(key)"));

    let plugins = text(&root.join("EXTENSIONS.md"));
    assert!(plugins.contains("context.providerId"));
    assert!(plugins.contains("context.providerService"));
    assert!(plugins.contains("context.service(pluginId)"));
    assert!(plugins.contains("potentially once per screen"));
    assert!(plugins.contains("shared scanners, subprocesses, watchers, caches"));
}

#[test]
fn qml_test_runner_failures_cannot_be_hidden_by_output_matching() {
    let runner = text(&Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/run"));
    assert!(!runner.contains(") || true"));
    assert!(runner.contains("status=$?"));
    assert!(runner.contains("FILEBLADE_QMLTESTRUNNER"));
    assert!(runner.contains(
        "for candidate in /usr/lib/qt6/bin/qmltestrunner /usr/lib64/qt6/bin/qmltestrunner qmltestrunner-qt6"
    ));
    assert!(runner.contains("if ((status != 0))"));
    assert!(runner.contains("Config: Using QtTest library 6\\."));
    assert!(runner.contains("Totals: [0-9]+ passed, 0 failed,"));
    assert!(runner.contains("Finished testing of qmltestrunner"));
}

#[test]
fn dynamic_metric_inputs_are_runtime_safe() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let picker = text(&root.join("ui/MetricPicker.qml"));
    for property in ["view.options", "view.columns", "view.sorts"] {
        assert!(
            picker.contains(&format!("Array.isArray({property})")),
            "{property} must be guarded while dynamic views initialize"
        );
    }

    let service = text(&root.join("Service.qml"));
    assert!(service.contains("id: fileTreeIpc"));
}

#[test]
fn pinned_git_picker_keeps_status_details_separate_from_column_options() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let service = text(&root.join("Service.qml"));
    assert!(service.contains(
        "readonly property var gitStatusDetailChoices: configController.gitStatusDetailChoices"
    ));

    let picker = text(&root.join("ui/MetricPicker.qml"));
    assert!(picker.contains("if (pinned && !detailPicker) return []"));
    assert!(picker.contains("if (adder || detailPicker) return list"));
    assert!(picker.contains("if (!picker.pinned || picker.detailPicker) picker.open()"));
    assert!(picker.contains("text: \"Git status indicators\""));
    assert!(!picker.contains("pinned ? \"Toggle columns\""));
}

#[test]
fn hidden_entries_are_visible_by_default_and_reuse_the_leading_marker_slot() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let state = text(&root.join("controllers/StateController.qml"));
    assert!(state.contains("property bool showHidden: true"));
    assert!(state.contains("showHidden: service.boolValue(config.showHidden, true)"));

    let row = text(&root.join("panes/BrowserRow.qml"));
    assert!(row.contains("readonly property bool hiddenEntry:"));
    assert!(row.contains("readonly property bool favoriteAvailable: favoriteMode || depth > 0"));
    assert!(row.contains(
        "row.hiddenEntry || (!row.customInteraction && row.favoriteAvailable && (row.favorite || row.hovered))"
    ));
    assert!(row.contains("HoverHandler { id: hoverTracker }"));
    assert!(row.contains("text: row.hiddenEntry ? \"󰈉\""));
    assert!(row.contains("!row.gitDeleted && row.favoriteAvailable && mouse.x >= favoriteGlyph.x"));
    assert!(row.contains("x: Style.space(3) + row.depth * Style.space(13)"));
    assert!(row.contains("x: favoriteGlyph.x + favoriteGlyph.width"));
    assert!(row.contains("x: disclosure.x + disclosure.width + Style.space(1)"));
}

#[test]
fn git_status_can_be_disabled_without_leaving_git_work_or_presentation() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let state = text(&root.join("controllers/StateController.qml"));
    assert!(state.contains("property bool gitEnabled: true"));
    assert!(state.contains("gitEnabled: service.boolValue(config.gitEnabled, true)"));
    assert!(state.contains("gitEnabled: gitEnabled"));

    let settings = text(&root.join("modules/files/FilesSettings.qml"));
    assert!(settings.contains("label: \"Git status\""));
    assert!(settings.contains("checked: root.controller.gitEnabled"));
    assert!(settings.contains("root.controller.setGitEnabled(!root.controller.gitEnabled)"));

    let tree = text(&root.join("controllers/TreeController.qml"));
    assert!(tree.contains("if (!gitEnabled) arguments.push(\"--no-git\")"));
    assert!(tree.contains("if (!gitEnabled || !open) return"));
    assert!(tree.contains("running: service.gitEnabled && service.open"));
    assert!(tree.contains("function resetGitIntegration()"));

    let search = text(&root.join("controllers/SearchController.qml"));
    assert!(search.contains("if (!service.gitEnabled) arguments.push(\"--no-git\")"));
    let pane = text(&root.join("panes/TreePane.qml"));
    assert!(pane.contains("visible: controller.gitEnabled"));
    assert!(pane.contains("readonly property real gitColumnWidth: !controller.gitEnabled"));
    let properties = text(&root.join("panes/PropertiesPane.qml"));
    assert!(properties.contains("controller.gitEnabled && entry.git_status"));
    assert!(properties.contains("controller.gitEnabled && entry.git_repo_root"));
}

#[test]
fn shared_folder_context_defaults_to_selection_and_can_follow_the_git_project() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let state = text(&root.join("controllers/StateController.qml"));
    let project = text(&root.join("controllers/ProjectController.qml"));
    let settings = text(&root.join("modules/files/FilesSettings.qml"));
    let surface = text(&root.join("blades/BladeSurface.qml"));

    assert!(state.contains("property bool projectContext: false"));
    assert!(state.contains("projectContext: service.boolValue(config.projectContext, false)"));
    assert!(state.contains("projectContext: projectContext"));
    assert!(project.contains("readonly property string folderContextPath:"));
    assert!(project.contains("readonly property string contextPath: projectContextActive ? projectRoot : folderContextPath"));
    assert!(project.contains("projectMarker === \".git\""));
    assert!(settings.contains("label: \"Folder context\""));
    assert!(settings.contains("{ key: \"project\", label: \"Git project\" }"));
    assert!(surface.contains("surface.host.services.files.contextPath"));
    let ipc = text(&root.join("controllers/FileTreeIpc.qml"));
    assert!(ipc.contains("contextPath: service.contextPath"));
    assert!(ipc.contains("projectContext: service.projectContext"));
}

#[test]
fn a_drag_leaving_a_blade_only_reaches_the_system_when_the_setting_asks_for_it() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let state = text(&root.join("controllers/StateController.qml"));
    let config = text(&root.join("controllers/ConfigController.qml"));
    let settings = text(&root.join("modules/files/FilesSettings.qml"));
    let row = text(&root.join("panes/BrowserRow.qml"));
    let wheel = text(&root.join("controllers/DropWheelController.qml"));
    let drop_target = text(&root.join("panes/BrowserDropTarget.qml"));
    let paths = text(&root.join("lib/PathText.js"));
    let ipc = text(&root.join("controllers/FileTreeIpc.qml"));

    assert!(state.contains("property string dragOut: \"paste\""));
    assert!(state.contains("dragOut: service.normalizeDragOut(config.dragOut)"));
    assert!(state.contains("dragOut: dragOut,"));
    assert!(
        config.contains("return [\"paste\", \"system\"].indexOf(mode) >= 0 ? mode : \"paste\"")
    );
    let service = text(&root.join("Service.qml"));
    assert!(service.contains(
        "function normalizeDragOut(value) { return configController.normalizeDragOut(value) }"
    ));
    assert!(service.contains("function setDragOut(value)"));
    assert!(settings.contains("label: \"Drag out\""));
    assert!(settings.contains("{ key: \"system\", label: \"System drag\" }"));
    assert!(ipc.contains("dragOut: service.dragOut,"));

    assert!(row.contains("if (held & Qt.RightButton) return false"));
    assert!(row.contains("if (keys & (Qt.ShiftModifier | Qt.ControlModifier)) return false"));
    assert!(row.contains("acceptedButtons: Qt.LeftButton | Qt.RightButton"));
    assert!(row.contains("if (openWheelNow) controller.dropWheel.openAfterDrag()"));
    assert!(wheel.contains("function openAfterDrag()"));
    assert!(row.contains(
        "row.wheelGesture = (Number(dragHandler.centroid.pressedButtons) & Qt.RightButton) !== 0"
    ));
    assert!(
        row.contains("row.Drag.dragType = row.systemDragActive ? Drag.Automatic : Drag.Internal")
    );
    assert!(row.contains(
        "row.systemDragActive = row.systemDragWanted(dragHandler.centroid.pressedButtons, dragHandler.centroid.modifiers)"
    ));
    assert!(wheel.contains("readonly property string pathForm:"));
    assert!(drop_target.contains("keys: [\"fileblade-entry\", \"text/uri-list\"]"));
    assert!(drop_target.contains("PathText.droppedPath(drop.urls[i])"));
    assert!(paths.contains("function droppedPath(value)"));
}

#[test]
fn drop_drag_leaves_the_blade_at_the_sheet_edge_not_the_layer_edge() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let surface = text(&root.join("blades/BladeSurface.qml"));
    assert!(surface.contains("implicitWidth: surfaceWidth"));
    assert!(surface.contains("function containsScenePoint(x, y)"));
    assert!(surface.contains("x >= sheet.x && x <= sheet.x + sheet.width"));
    assert!(surface.contains(
        "readonly property int surfaceOriginY: barPosition === \"top\" ? liveBarSize : 0"
    ));
    assert!(surface.contains("host.shell.bar.barHidden ? 0"));
    let window = text(&root.join("blades/BladeWindow.qml"));
    assert!(window.contains("function containsScenePoint(x, y)"));
    let row = text(&root.join("panes/BrowserRow.qml"));
    let drop_target = text(&root.join("panes/BrowserDropTarget.qml"));
    assert!(drop_target.contains("drop.proposedAction === Qt.CopyAction"));
    assert!(!drop_target.contains("drop.modifiers"));
    assert!(row.contains("DragPlan.disjointPaths(controller.selectedPaths)"));
    assert!(row.contains("controller.selectionUris(row.draggedPaths)"));
    assert!(row.contains("String(left.path || \"\") === row.path"));
    assert!(row.contains("Drag.hotSpot.x: dragHandler.centroid.position.x"));
    assert!(row.contains("Drag.hotSpot.y: dragHandler.centroid.position.y"));
    assert!(row.contains("row.Drag.active = true"));
    assert!(row.contains("else row.Drag.drop()"));
    assert!(row.contains("if (row.dragCanceled) row.Drag.cancel()"));
    assert!(row.contains("Drag.keys: [\"fileblade-entry\"]"));
    assert!(drop_target.contains("keys: [\"fileblade-entry\", \"text/uri-list\"]"));
    assert!(drop_target.contains("interval: 500"));
    assert!(drop_target.contains("controller.setDirectoryExpanded(rowItem.path, true)"));
    assert!(drop_target.contains(
        "controller.moveSelectionTo(rowItem.path, drop.proposedAction === Qt.CopyAction, paths)"
    ));
    assert!(row.contains("ownerView.contentY = Math.max"));
    assert!(
        row.contains(
            "cursorShape: dragHandler.active ? Qt.ClosedHandCursor : Qt.PointingHandCursor"
        )
    );
    assert!(row.contains("cursorShape: active ? Qt.ClosedHandCursor : Qt.PointingHandCursor"));
    assert!(!row.contains("Qt.OpenHandCursor"));
    let operations = text(&root.join("controllers/OperationController.qml"));
    assert!(
        operations
            .contains("var sources = Array.isArray(paths) ? paths.slice() : service.selectedPaths")
    );
    assert!(operations.contains("command(copying ? \"copy\" : \"move\", sources, destination)"));
    for file in ["panes/BrowserRow.qml", "ui/ArtifactTree.qml"] {
        let source = text(&root.join(file));
        assert!(
            source.contains("!window.containsScenePoint(scene.x, scene.y)"),
            "{file}"
        );
        assert!(
            !source.contains("scene.x > window.width"),
            "{file} tests the layer edge, not the sheet"
        );
    }
    let wheel = text(&root.join("ui/DropWheel.qml"));
    // Wheel wedges are dark and everything on them is one accent tint. The outer
    // band sits on the darkest part of the wheel and an icon's own antialiasing
    // dilutes the tint, so application icons are lifted above the ring glyphs.
    assert!(wheel.contains("context.fillStyle = overlay.alpha(Color.popups.background, 0.94)"));
    // A highlighted icon shows its own artwork; the rest stay tinted.
    assert!(wheel.contains("monochrome: !wedge.active"));
    assert!(wheel.contains("monochrome: !child.active"));
    // Separators are cut at one constant width rather than left as an angular
    // gap, which would be a hair at the hub and a chasm at the rim.
    assert!(wheel.contains("globalCompositeOperation = \"destination-out\""));
    // The wheel is dark and lands on whatever is behind it, so each wedge
    // carries its own edge rather than relying on the fill to separate it.
    assert!(wheel.contains("strokeStyle = overlay.alpha(Color.accent, active ? 0.85 : 0.42)"));
    assert!(!wheel.contains("wedgeGap"));
    assert!(
        wheel.contains("iconColor: wedge.active ? Qt.lighter(Color.accent, 1.5) : Color.accent")
    );
    assert!(wheel.contains("iconColor: Qt.lighter(Color.accent, child.active ? 1.75 : 1.45)"));
    assert!(!wheel.contains("iconColor: Color.popups.background"));
    assert!(!wheel.contains("fallbackColor: Color.popups.background"));
    // The alpha mask keeps an icon's own silhouette; the luminance mask eats a
    // flat glyph such as Neovim's mark down to a sliver.
    assert!(wheel.matches("icon_mask || \"alpha\"").count() >= 2);
    assert!(wheel.matches("trustedIconSource:").count() >= 2);
    assert!(
        wheel.contains("mask: wheelHere && !controller.wheelFromDrag ? fullInput : passThrough")
    );
    assert!(wheel.contains(
        "ghostHere = controller.dragActive && controller.dragDocked && !controller.wheelOpen"
    ));
    assert!(
        wheel.contains("readonly property var entry: overlay.controller.dragEntries.length > 0")
    );
    assert!(wheel.contains("text: String(ghost.entry.name || \"\")"));
}

#[test]
fn hover_exit_waits_while_the_action_menu_is_open() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let focus = text(&root.join("blades/BladeFocusController.qml"));
    assert!(
        focus.contains("readonly property bool menuOpen: !!(service && service.actionMenuOpen)")
    );
    assert!(focus.contains("host.pointerHeld || menuOpen"));
    assert!(focus.contains("if (!pointerBusy && hoverExitPending) hoverExitTimer.restart()"));
    assert!(focus.contains("if (!hoverExitPending || pointerBusy"));
    let pointer_watch = text(&root.join("blades/BladePointerFocusWatch.qml"));
    assert!(pointer_watch.contains("service.backendRequest(\"hover-target\""));
    assert!(pointer_watch.contains("if (!watch.baselineReady)"));
    assert!(pointer_watch.contains("if (!moved)"));
    let script = text(&root.join("tests/vm/menu-focus.sh"));
    assert!(script.contains("parked-focus focusedBlade left"));
    assert!(script.contains("outside-click-closes actionMenuOpen false"));
    let window_focus = text(&root.join("tests/vm/window-click-focus.sh"));
    assert!(window_focus.contains("key meta_l-b"));
    assert!(window_focus.contains("key shift-meta_l-b"));
    assert!(window_focus.contains("left-rest-retained"));
    assert!(window_focus.contains("motion-released \"\""));
    assert!(window_focus.contains("right-click-released \"\""));
    assert!(window_focus.contains("switch-right right"));
}

#[test]
fn artifact_tree_taps_expand_without_losing_right_clicks_or_drags() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let tree = text(&root.join("ui/ArtifactTree.qml"));
    assert!(tree.contains(
        "MouseArea {\n        anchors.fill: parent\n        acceptedButtons: Qt.LeftButton | Qt.RightButton"
    ));
    assert!(!tree.contains("acceptedButtons: Qt.LeftButton | Qt.RightButton\n        z: -1"));
    assert!(tree.contains("mouse.x <= Style.space(38) + row.indent"));
    assert!(tree.contains("tree.openMenu(row.entry, row, mouse.x, mouse.y, \"actions\")"));
    assert!(tree.contains("DragHandler {\n        id: rowDrag"));
    let pane_row = text(&root.join("ui/PaneRow.qml"));
    assert!(pane_row.contains("id: metricRow\n    z: 1"));
    assert!(pane_row.contains("id: actionsLoader\n    z: 1"));
}

#[test]
fn artifact_tree_inherits_navigation_without_swallowing_domain_shortcuts() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let header = text(&root.join("ui/PaneHeader.qml"));
    assert!(
        header.contains("property bool highlighted: !!context && context.slotFocused === true")
    );
    let tree = text(&root.join("ui/ArtifactTree.qml"));
    assert!(tree.contains("KeyRouter.artifactAction(event)"));
    assert!(tree.contains("typeof keyHandler === \"function\" && keyHandler(event, repeated)"));
    assert!(tree.contains("property var fileActionsFor:"));
    assert!(tree.contains("isBinned(item) || !fileActionsFor(item)"));
    assert!(tree.contains("return handler() !== false"));
    assert!(tree.contains("return openMenu(item, rowItem"));
    assert!(tree.contains("KeyRouter.ignoresAutoRepeat(action, event.key)"));
    assert!(tree.contains("rowKey(rows[i]) === cursorKey"));
    assert!(tree.contains("return JSON.stringify(path)"));
    assert!(tree.contains("Qt.callLater(showCurrent)"));
    assert!(tree.contains("ArtifactTreeFolders.expand(tree)"));
    let edit = tree
        .split("function editCurrent()")
        .nth(1)
        .unwrap()
        .split("function touches")
        .next()
        .unwrap();
    assert!(edit.contains("editPathFor(item)"));
    assert!(!edit.contains("entryFor("));
    assert!(edit.contains("!isBinned(item)"));
    assert!(tree.contains(
        "if (!item || !item.path || isBinned(item) || !fileActionsFor(item)) return null"
    ));
    assert!(tree.contains("enabled: !row.isGroup && !!tree.entryFor(row.entry)"));
    let search = text(&root.join("ui/PaneSearchField.qml"));
    assert!(search.contains("property bool showDeepOption: false"));
    assert!(search.contains("model: field.showDeepOption ? 3 : 2"));
    assert!(text(&root.join("panes/TreePane.qml")).contains("showDeepOption: true"));
}

#[test]
fn ipc_handlers_export_only_typed_functions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ipc = text(&root.join("controllers/FileTreeIpc.qml"));
    let mut depth = 0usize;
    let mut handler_depth: Option<usize> = None;
    let mut handler_target = String::new();
    for (number, line) in ipc.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("property IpcHandler") && trimmed.ends_with("IpcHandler {") {
            handler_depth = Some(depth);
        }
        if handler_depth.is_some() {
            if let Some(target) = trimmed.strip_prefix("target: ") {
                handler_target = target.trim_matches('"').to_string();
            }
            if let Some(signature) = trimmed.strip_prefix("function ") {
                let open = signature.find('(').expect("function signature");
                let close = signature.find(')').expect("function signature");
                let parameters = &signature[open + 1..close];
                for parameter in parameters
                    .split(',')
                    .map(str::trim)
                    .filter(|p| !p.is_empty())
                {
                    assert!(
                        parameter.contains(':'),
                        "line {}: untyped parameter `{parameter}` inside IpcHandler {handler_target}; Quickshell drops the whole target",
                        number + 1
                    );
                }
                assert!(
                    signature[close + 1..].trim_start().starts_with(':'),
                    "line {}: IPC function without a return type inside IpcHandler {handler_target}",
                    number + 1
                );
            }
        }
        depth += line.matches('{').count();
        depth = depth.saturating_sub(line.matches('}').count());
        if handler_depth.is_some_and(|start| depth <= start) && trimmed.ends_with('}') {
            handler_depth = None;
            handler_target.clear();
        }
    }
}

#[test]
fn ipc_targets_match_between_qml_and_cli() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ipc = text(&root.join("controllers/FileTreeIpc.qml"));
    let cli = module_text(root, "public_cli");
    let mut qml_targets: Vec<String> = ipc
        .lines()
        .filter_map(|line| line.trim().strip_prefix("target: "))
        .map(|value| value.trim_matches('"').to_string())
        .collect();
    let mut cli_targets: Vec<String> = cli
        .lines()
        .filter_map(|line| {
            let line = line.trim();
            let rest = line.strip_prefix("const ")?;
            let (name, value) = rest.split_once(": &str = ")?;
            name.ends_with("TARGET")
                .then(|| value.trim_end_matches(';').trim_matches('"').to_string())
        })
        .collect();
    qml_targets.sort();
    qml_targets.dedup();
    cli_targets.sort();
    cli_targets.dedup();
    assert!(!qml_targets.is_empty(), "FileTreeIpc exports no IPC target");
    assert_eq!(
        qml_targets, cli_targets,
        "IPC targets exported by FileTreeIpc.qml and dialled by public_cli.rs differ"
    );
}

#[test]
fn service_forwarders_resolve_to_controller_functions() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let service = text(&root.join("Service.qml"));
    let mut ids = std::collections::HashMap::new();
    let lines: Vec<&str> = service.lines().collect();
    for (index, line) in lines.iter().enumerate() {
        let trimmed = line.trim();
        if let Some(kind) = trimmed.strip_suffix(" {")
            && kind.ends_with("Controller")
            && let Some(next) = lines.get(index + 1)
            && let Some(id) = next.trim().strip_prefix("id: ")
        {
            ids.insert(id.to_string(), kind.to_string());
        }
    }
    assert!(!ids.is_empty(), "Service.qml declares no controllers");
    for (index, line) in lines.iter().enumerate() {
        let Some(rest) = line.strip_prefix("  function ") else {
            continue;
        };
        let Some(body) = rest.split_once('{').map(|(_, body)| body.trim()) else {
            continue;
        };
        let call = body.strip_prefix("return ").unwrap_or(body);
        let Some((receiver, tail)) = call.split_once('.') else {
            continue;
        };
        let Some(kind) = ids.get(receiver) else {
            continue;
        };
        let Some(target) = tail.split('(').next() else {
            continue;
        };
        if target.is_empty() || !target.chars().all(|c| c.is_alphanumeric() || c == '_') {
            continue;
        }
        let controller = text(&root.join(format!("controllers/{kind}.qml")));
        assert!(
            controller.contains(&format!("function {target}(")),
            "Service.qml:{}: forwards to {receiver}.{target}, which {kind}.qml does not define",
            index + 1
        );
    }
}

#[test]
fn exported_ipc_verbs_have_a_cli_caller_or_a_documented_reason() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let ipc = text(&root.join("controllers/FileTreeIpc.qml"));
    let cli = module_text(root, "public_cli");
    let architecture = text(&root.join("ARCHITECTURE.md"));
    let mut exported = Vec::new();
    let mut depth = 0usize;
    let mut handler_depth: Option<usize> = None;
    for line in ipc.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("property IpcHandler") && trimmed.ends_with("IpcHandler {") {
            handler_depth = Some(depth);
        }
        if handler_depth.is_some()
            && let Some(rest) = trimmed.strip_prefix("function ")
            && let Some(name) = rest.split('(').next()
        {
            exported.push(name.to_string());
        }
        depth += line.matches('{').count();
        depth = depth.saturating_sub(line.matches('}').count());
        if handler_depth.is_some_and(|start| depth <= start) && trimmed.ends_with('}') {
            handler_depth = None;
        }
    }
    let documented: Vec<String> = architecture
        .split("### IPC verbs without a CLI subcommand")
        .nth(1)
        .and_then(|section| section.split("```yaml").nth(1))
        .and_then(|block| block.split("```").next())
        .map(|block| {
            block
                .lines()
                .filter_map(|line| {
                    line.split_once(':')
                        .map(|(name, _)| name.trim().to_string())
                })
                .collect()
        })
        .unwrap_or_default();
    assert!(!exported.is_empty() && !documented.is_empty());
    for verb in exported {
        let quoted = format!("\"{verb}\"");
        assert!(
            cli.contains(&quoted) || documented.contains(&verb),
            "IPC verb {verb} has no fileblade subcommand and no entry in ARCHITECTURE.md under IPC verbs without a CLI subcommand"
        );
    }
}

#[test]
fn default_folder_opening_is_a_shared_fileblade_behavior() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let service = text(&root.join("Service.qml"));
    let launcher = text(&root.join("controllers/LaunchController.qml"));
    let menu = text(&root.join("panes/FileActionsMenu.qml"));
    let focus = text(&root.join("blades/BladeFocusController.qml"));
    let rust_launch = text(&root.join("src/hyprland/launch.rs"));
    let plugins = text(&root.join("EXTENSIONS.md"));

    assert!(service.contains("function openDefault(path, targetScreen, directoryHint)"));
    assert!(service.contains(
        "enqueueLaunch(path, \"default\", \"\", defaultOpenScreen(targetScreen), directoryHint)"
    ));
    assert!(launcher.contains("service.backendRequest(\"stat-batch\""));
    assert!(launcher.contains("activeLaunch.mode === \"default\""));
    assert!(launcher.contains("service.navigateToLocation(target, targetScreen, \"browse\")"));
    assert!(launcher.contains("status = \"Opened in FileBlade\""));
    assert!(menu.contains("controller.actionMenuScreen"));
    assert!(menu.contains("root.entry ? !!root.entry.is_dir : undefined"));
    assert!(focus.contains("host.setSlotTab(location.edge, location.index, location.tab)"));
    assert!(rust_launch.contains("\"navigate\".to_string()"));
    assert!(plugins.contains("`openDefault` is the shared open primitive for every module"));
}

#[test]
fn module_definitions_are_normalized_once_and_grouped_by_category() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let definitions = text(&root.join("lib/Definitions.js"));
    assert!(definitions.starts_with(".pragma library\n"));
    for member in [
        "var MODULE_ID_PATTERN = /^[A-Za-z0-9][A-Za-z0-9._-]*(\\/[A-Za-z0-9][A-Za-z0-9._-]*)?$/",
        "var CATEGORY_PATTERN = /^[A-Za-z][A-Za-z0-9 ]{0,31}$/",
        "var MAXIMUM_SCHEMA_ROWS = 32",
        "function boundedText(value, fallback, limit)",
        "function safeId(value, limit)",
        "function category(raw, source)",
        "function categoryOrder(left, right)",
        "function dirName(moduleId)",
        "function settingsSpec(raw)",
    ] {
        assert!(
            definitions.contains(member),
            "Definitions.js lacks {member}"
        );
    }
    assert!(
        !definitions.contains('\u{0}'),
        "control characters are written as escapes"
    );
    let registry = text(&root.join("blades/BladeRegistry.qml"));
    assert!(registry.contains("import \"../lib/Definitions.js\" as Definitions"));
    assert!(registry.contains(
        "function boundedText(value, fallback, limit) { return Definitions.boundedText(value, fallback, limit) }"
    ));
    assert!(registry.contains("var bare = Definitions.safeId(raw.id, maximumIdLength)"));
    assert!(registry.contains("category: Definitions.category(raw.category, source)"));
    assert!(registry.contains("settings: Definitions.settingsSpec(raw.settings)"));
    assert!(
        registry
            .contains("Definitions.categoryOrder(merged[left].category, merged[right].category)")
    );
    assert!(registry.contains("function ipcDocument(placed)"));
    assert!(registry.contains("settings: { keys: settingKeys(found) }"));
    assert!(registry.contains("placed: placed(found.id) || null"));
    let ipc = text(&root.join("controllers/FileTreeIpc.qml"));
    assert!(ipc.contains(
        "return bladeHost.registry.ipcDocument(function(id) { return bladeHost.findModule(id) })"
    ));
    assert!(!ipc.contains("entry: module.entryUrl"));
    let settings = text(&root.join("blades/BladeSettings.qml"));
    assert!(settings.contains("delegate: BladeModuleSection {"));
    // The revert link sits in the footer, asks first, and defaults its
    // selection to Cancel so a stray Enter cannot wipe the layout.
    assert!(settings.contains("text: \"Revert to default settings\""));
    assert!(settings.contains("onClicked: root.confirmRevert()"));
    assert!(settings.contains(
        "\"Are you sure?\\nThis will revert to default settings and layouts. It doesn't affect key bindings.\""
    ));
    assert!(settings.contains("[{ key: \"revert\", label: \"Yes\", danger: true }, { key: \"cancel\", label: \"Cancel\" }]"));
    assert!(settings.contains("revertDialog.selectedIndex = 1"));
    assert!(settings.contains("if (key === \"revert\") root.host.revertDefaults()"));
    let host = text(&root.join("blades/BladeHost.qml"));
    assert!(host.contains("function revertDefaults() {"));
    assert!(host.contains("service.resetSettings()"));
    let state = text(&root.join("controllers/StateController.qml"));
    assert!(state.contains("function resetSettings() {"));
    assert!(
        !state[state.find("function resetSettings() {").unwrap()..]
            .split("function normalizedTrashRetentionDays")
            .next()
            .unwrap()
            .contains("favorites ="),
        "reverting settings keeps the favourites"
    );
    assert!(settings.contains("current = { category: String(module.category), modules: [] }"));
    assert!(settings.contains("text: String(group.modelData.category).toUpperCase()"));
    assert!(
        settings
            .contains("\"add module \" + String(module.category) + \" \" + String(module.name)")
    );
    let section = text(&root.join("blades/BladeModuleSection.qml"));
    assert!(section.contains("required property var sheet"));
    assert!(section.contains(
        "sheet.rowHaystack(child, modelData.title + (heading ? \" \" + heading.title : \"\"))"
    ));
    assert!(section.contains("textFormat: Text.PlainText"));
    assert!(!section.contains("RichText"));
    let slot = text(&root.join("blades/BladeSlot.qml"));
    assert!(slot.contains("rows.push({ kind: \"separator\", label: category })"));
    assert!(slot.contains("section: category })"));
    let popup = text(&root.join("ui/OptionPopup.qml"));
    assert!(popup.contains(
        "readonly property bool labeled: separator && String(modelData.label || \"\") !== \"\""
    ));
    assert!(popup.contains("visible: optionRow.separator && !optionRow.labeled"));
    assert!(popup.contains("row.kind !== \"separator\" && String(row.label)"));
    let cli = module_text(root, "public_cli");
    assert!(cli.contains("RootCommand::Modules => modules(),"));
    assert!(cli.contains("object_response(\"bladeModules\", &[])"));
    assert!(cli.contains("lines.push(category.to_uppercase());"));
    assert!(cli.contains(".take(MAX_CATEGORY_CHARS)"));
    assert!(cli.contains("if module[\"placed\"].is_object()"));
    for module in ["files", "notes", "properties"] {
        let definition: serde_json::Value =
            serde_json::from_str(&text(&root.join(format!("modules/{module}/blade.json"))))
                .unwrap();
        assert!(
            definition.get("settings").is_none(),
            "{module} declares no settings"
        );
        assert!(
            definition.get("category").is_none(),
            "{module} keeps the default category"
        );
    }
    let plugins = text(&root.join("EXTENSIONS.md"));
    assert!(
        plugins.contains("category:     one word or a short phrase (max 32) the picker groups by")
    );
    assert!(plugins.contains(
        "defaults to `Module` for built-in and user modules, `Plugin` for manifest ones"
    ));
    assert!(plugins.contains("grouped by `category`"));
}

#[test]
fn declarative_settings_render_through_one_form_and_one_coercion_path() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let library = text(&root.join("lib/SettingsForm.js"));
    assert!(library.starts_with(".pragma library\n.import \"Definitions.js\" as Definitions\n"));
    for member in [
        "function row(schema, key)",
        "function coerce(row, raw)",
        "function effective(row, raw)",
        "function rowModel(schema, values)",
        "function stepValue(row, current, direction)",
        "function inlineChoice(row)",
        "return Definitions.coerceDefault(row, raw)",
        "Object.prototype.hasOwnProperty.call(values, key)",
        "result.length < Definitions.MAXIMUM_SCHEMA_ROWS",
    ] {
        assert!(library.contains(member), "SettingsForm.js lacks {member}");
    }
    let context = text(&root.join("blades/BladeContext.qml"));
    assert!(context.contains("import \"../lib/SettingsForm.js\" as SettingsForm"));
    assert!(context.contains("property var definition: null"));
    assert!(
        context.contains("readonly property string category: definition && definition.category")
    );
    assert!(context.contains("property QtObject settings: QtObject {"));
    for member in [
        "function has(key)",
        "function get(key)",
        "function set(key, value)",
        "SettingsForm.effective(found, context.state.get(found.key, undefined))",
        "coerced !== undefined && context.state.set(found.key, coerced)",
    ] {
        assert!(
            context.contains(member),
            "BladeContext.settings lacks {member}"
        );
    }
    let slot = text(&root.join("blades/BladeSlot.qml"));
    assert!(
        slot.contains("readonly property var settingsContext: loader.moduleContext || context")
    );
    assert!(slot.contains("BladeModuleLoader {"));
    assert!(slot.contains("loader.loadModule(entryUrl)"));
    assert!(slot.contains("onModuleIdChanged: scheduleReload()"));
    assert!(slot.contains("onSlotIdChanged: { dropStaleCloseTab(); scheduleReload() }"));
    assert!(slot.contains("definition: slot.moduleInfo"));
    let sheet = text(&root.join("blades/BladeSettings.qml"));
    assert!(sheet.contains("var context = item.settingsContext || null"));
    assert!(sheet.contains("if (item.settingsComponent || declared)"));
    let section = text(&root.join("blades/BladeModuleSection.qml"));
    assert!(section.contains("import \"../ui\" as PluginUi"));
    assert!(section.contains("PluginUi.SettingsForm {"));
    assert!(section.contains("formShown = countShown(settingsForm.children)"));
    assert!(section.contains("section.moduleContext.settings.set(key, value)"));
    assert!(section.contains(
        "sheet.rowHaystack(child, modelData.title + (heading ? \" \" + heading.title : \"\"))"
    ));
    for name in [
        "ui/SettingsForm.qml",
        "ui/NumberRow.qml",
        "ui/TextRow.qml",
        "ui/SelectRow.qml",
    ] {
        let source = text(&root.join(name));
        let texts = source
            .lines()
            .filter(|line| line.trim() == "Text {")
            .count();
        let plain = source.matches("textFormat: Text.PlainText").count();
        assert!(texts > 0, "{name} renders text");
        assert_eq!(texts, plain, "{name}: every Text is PlainText");
        assert!(
            !source.contains("RichText"),
            "{name} never renders rich text"
        );
        assert!(!source.contains("Process"), "{name} spawns nothing");
        assert!(
            source.contains("import \"../lib/SettingsForm.js\" as Form"),
            "{name} reads values through SettingsForm.js"
        );
    }
    let form = text(&root.join("ui/SettingsForm.qml"));
    assert!(form.contains("required property var schema"));
    assert!(form.contains("required property var values"));
    assert!(form.contains("signal changed(string key, var value)"));
    assert!(form.contains("readonly property var rows: Form.rowModel(schema, values)"));
    assert!(form.contains("readonly property string label: String(row.label)"));
    assert!(form.contains("readonly property string detail: String(row.description)"));
    assert!(form.contains("readonly property var options: row.options"));
    assert!(form.contains("return Form.inlineChoice(row) ? choiceRow : selectRow"));
    let number = text(&root.join("ui/NumberRow.qml"));
    assert!(number.contains("maximumLength: Form.MAXIMUM_FIELD_LENGTH"));
    assert!(number.contains(
        "var base = field.activeFocus ? Form.coerce(control.row, field.text.trim()) : undefined"
    ));
    assert!(number.contains(
        "Form.stepValue(control.row, base === undefined ? control.row.value : base, direction)"
    ));
    assert!(number.contains("field.text = Form.formatValue(control.row, next)"));
    assert!(form.contains("model: form.rows.length"));
    let text_row = text(&root.join("ui/TextRow.qml"));
    assert!(text_row.contains("maximumLength: control.limit"));
    assert!(text_row.contains("readonly property bool pathRow: row.type === \"path\""));
    assert!(text_row.contains("Keys.onEscapePressed"));
    let select = text(&root.join("ui/SelectRow.qml"));
    assert!(select.contains("menu.rows = Form.popupRows(control.row, control.row.value)"));
    let manifest: serde_json::Value = serde_json::from_str(&text(
        &root.join("examples/data-goblin.blade-example/manifest.example.json"),
    ))
    .unwrap();
    let clock = &manifest["extensions"]["data-goblin.fileblade/blade"][0];
    let schema = clock["settings"]["schema"].as_array().unwrap();
    let keys: Vec<&str> = schema
        .iter()
        .map(|row| row["key"].as_str().unwrap())
        .collect();
    assert_eq!(keys, ["format", "caption", "scale"]);
    assert_eq!(clock["settings"]["defaults"]["format"], "HH:mm:ss");
    let clock_qml = text(&root.join("examples/data-goblin.blade-example/blades/Clock.qml"));
    assert!(clock_qml.contains("context.settings && context.settings.has(key) ? context.settings.get(key) : context.state.get(key, fallback)"));
    assert!(clock_qml.contains("context.settings.set(\"format\", next)"));
    for module in ["files", "notes", "properties"] {
        let definition: serde_json::Value =
            serde_json::from_str(&text(&root.join(format!("modules/{module}/blade.json"))))
                .unwrap();
        assert!(definition.get("settings").is_none());
    }
    let plugins = text(&root.join("EXTENSIONS.md"));
    assert!(plugins.contains("## Settings without QML"));
    assert!(plugins.contains("context.settings.get(key)"));
    assert!(plugins.contains("context.settings.set(key, value)"));
    assert!(plugins.contains("context.settings.has(key)"));
    assert!(plugins.contains("context.category"));
    assert!(plugins.contains("settings:     optional `{ defaults, schema }`"));
    assert!(plugins.contains("the same rows Omarchy's `barWidget.schema`"));
    assert!(plugins.contains("A QML `settings` Component still works and renders below the"));
}

#[test]
fn module_directories_are_created_by_the_backend_and_exposed_on_the_context() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let context = text(&root.join("blades/BladeContext.qml"));
    for member in [
        "property QtObject paths: QtObject",
        "function canonical(path) { return PathText.fileUrl(path) }",
        "function parent(path) { return PathText.parent(path) }",
        "function join(directory, name) { return PathText.join(directory, name) }",
        "function within(path, directory) { return PathText.within(path, directory) }",
        "function name(path) { return PathText.name(path) }",
        "readonly property string stateDir: host ? host.dirs.stateDir(moduleId) : \"\"",
        "readonly property string configDir: host ? host.dirs.configDir(moduleId) : \"\"",
        "readonly property bool dirsReady: host ? host.dirs.ready(moduleId) : false",
        "function ensureDirs(callback)",
        "return host ? host.dirs.ensure(moduleId, callback) : false",
    ] {
        assert!(context.contains(member), "BladeContext lacks {member}");
    }
    let slot = text(&root.join("blades/BladeSlot.qml"));
    assert!(slot.contains("host.dirs.ensure(moduleId)"));
    let host = text(&root.join("blades/BladeHost.qml"));
    assert!(host.contains("BladeModuleDirs {"));
    assert!(host.contains("property alias dirs: moduleDirs"));
    assert!(host.contains(
        "function tabSeedState(moduleId) { return tabController.tabSeedState(moduleId) }"
    ));
    let tabs = text(&root.join("blades/BladeTabs.qml"));
    assert!(tabs.contains("function tabTitle(edge, slotIndex, tabIndex)"));
    let service = text(&root.join("Service.qml"));
    assert!(service.contains(
        "function moduleDirs(id, callback) { return bladeHost.dirs.ensure(id, callback) }"
    ));
    let dirs = text(&root.join("blades/BladeModuleDirs.qml"));
    assert!(dirs.contains("import \"../lib/Definitions.js\" as Definitions"));
    assert!(dirs.contains(
        "readonly property string configRoot: (host ? host.configDir : \"\") + \"/config/\""
    ));
    let config_root = dirs
        .lines()
        .find(|line| line.contains("property string configRoot"))
        .expect("BladeModuleDirs declares configRoot");
    assert!(
        !config_root.contains("modules"),
        "config never lands in the user-module scan root: {config_root}"
    );
    assert!(dirs.contains("stateHome + \"/omarchy/fileblade/modules/\""));
    assert!(dirs.contains("property var known: Object.create(null)"));
    assert!(dirs.contains("readonly property int maximumModules: 128"));
    assert!(dirs.contains("return entry ? entry.result.stateDir : stateRoot + name"));
    assert!(dirs.contains("return entry ? entry.result.configDir : configRoot + name"));
    assert!(dirs.contains("service.backendRequest(\"module-dirs\", [\"--module\", String(id)]"));
    assert!(dirs.contains("if (entry.pending.length >= maximumWaiters)"));
    assert!(dirs.contains("delete known[name]"));
    assert!(!dirs.contains("Process"));
    let module_dirs = module_text(root, "module_dirs");
    assert!(module_dirs.contains("pub const MAX_MODULE_ID_BYTES: usize = 128;"));
    assert!(module_dirs.contains("crate::paths::state_dir().join(STATE_SEGMENT)"));
    assert!(module_dirs.contains("crate::paths::config_dir().join(CONFIG_SEGMENT)"));
    assert!(module_dirs.contains("const CONFIG_SEGMENT: &str = \"config\";"));
    assert!(module_dirs.contains("ensure_private_directory(&state)?;"));
    assert!(module_dirs.contains("if let Err(error) = ensure_private_directory(&config)"));
    assert!(module_dirs.contains("let _ = std::fs::remove_dir(&state);"));
    let backend = module_text(root, "backend");
    assert!(backend.contains("ModuleDirs(ModuleDirsArgs),"));
    assert!(!backend.contains("| BackendCommand::ModuleDirs"));
    let cli = module_text(root, "public_cli");
    assert!(cli.contains("\"module-dirs\".to_string()"));
    let plugins = text(&root.join("EXTENSIONS.md"));
    assert!(plugins.contains("context.stateDir"));
    assert!(plugins.contains("moduleDirs("));
    assert!(plugins.contains("fileblade module-dirs <id>"));
    let architecture = text(&root.join("ARCHITECTURE.md"));
    assert!(architecture.contains("~/.local/state/omarchy/fileblade/modules/<id>/:"));
    assert!(architecture.contains("~/.config/omarchy/fileblade/config/<id>/:"));
}

#[test]
fn script_actions_are_normalized_once_in_rust_and_never_echo_a_command_vector() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let spec = text(&root.join("src/actions/spec.rs"));
    for bound in [
        "pub const MAX_ACTIONS_PER_SOURCE: usize = 16;",
        "pub const MAX_ARGV: usize = 32;",
        "pub const MAX_TIMEOUT_SECONDS: u64 = 900;",
        "pub const MAX_SELECTION_ENV_BYTES: usize = 65536;",
        "pub const MAX_SELECTION_TARGETS: usize = 256;",
        "pub const MAX_CAPTURED_RUNS: usize = 4;",
        "pub const MAX_ERRORS_PER_SOURCE: usize = 16;",
        "pub const MAX_ERRORS_TOTAL: usize = 64;",
        "pub const SOCKET_KEY: &str = \"data-goblin.fileblade/action\";",
    ] {
        assert!(spec.contains(bound), "src/actions/spec.rs lacks {bound}");
    }
    let scan = text(&root.join("src/actions/scan.rs"));
    let row = scan
        .split("pub fn row(")
        .nth(1)
        .and_then(|body| body.split("\npub fn ").next())
        .expect("scan.rs builds the action-list row");
    assert!(
        !row.contains("argv"),
        "action-list rows must not carry argv"
    );
    assert!(!row.contains("cwd"), "action-list rows must not carry cwd");
    assert!(
        spec.contains("if !confined(&resolved, root)"),
        "plugin argv[0] must stay under the plugin root"
    );
    assert!(scan.contains("if plugin == USER_PLUGIN_ID {"));
    let run = text(&root.join("src/actions/run.rs"));
    assert!(run.contains("if request.paths.iter().any(String::is_empty)"));
    assert!(run.contains(".retain_tail(true)"));
    assert!(run.contains("values.push((\"FILEBLADE_SELECTION_JSON\", document));"));
    assert!(run.contains("values.push((\"FILEBLADE_SELECTION_FILE\", file));"));
    let selection = text(&root.join("src/actions/run/selection.rs"));
    assert!(selection.contains("\"dir\": is_directory,"));
    assert!(run.contains("resolve_program(&action, &root, source)"));
    assert!(run.contains("run_cancellable(cancelled)"));
    assert!(run.contains("crate::module_dirs::ensure(module)?"));
    let backend = module_text(root, "backend");
    assert!(backend.contains("ActionList(ActionListArgs),"));
    assert!(backend.contains("ActionRun(ActionRunArgs),"));
    assert!(backend.contains("| BackendCommand::ActionRun(_)"));
    assert!(!backend.contains("| BackendCommand::ActionList"));
    let audit = module_text(root, "audit");
    assert!(audit.contains("\"action-run\","));
    assert!(audit.contains("if command == \"action-run\""));
    let cli = module_text(root, "public_cli");
    for verb in ["\"actions\"", "\"actionResult\"", "\"runAction\""] {
        assert!(cli.contains(verb), "public_cli never dials {verb}");
    }
    let architecture = text(&root.join("ARCHITECTURE.md"));
    assert!(architecture.contains("actions, actionResult"));
    let security = text(&root.join("SECURITY.md"));
    assert!(security.contains("Script actions"));
    let manifest: serde_json::Value = serde_json::from_str(&text(
        &root.join("examples/data-goblin.blade-example/manifest.example.json"),
    ))
    .unwrap();
    let dump = &manifest["extensions"]["data-goblin.fileblade/action"][0];
    assert_eq!(dump["id"], "dump");
    assert_eq!(dump["argv"][0], "scripts/dump-env");
    let script = root.join("examples/data-goblin.blade-example/scripts/dump-env");
    let mode = fs::metadata(&script)
        .expect("dump-env")
        .permissions()
        .mode();
    assert!(mode & 0o100 != 0, "the example action is executable");
    let plugins = text(&root.join("EXTENSIONS.md"));
    assert!(plugins.contains("## Script actions"));
    assert!(plugins.contains("data-goblin.fileblade/action"));
    assert!(plugins.contains("fileblade action <key>"));
}

#[test]
fn script_action_rows_render_plain_text_and_reach_the_controller_through_the_service() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let rows = text(&root.join("panes/ScriptActionRows.qml"));
    let texts = rows.lines().filter(|line| line.trim() == "Text {").count();
    assert!(texts > 0, "ScriptActionRows renders text");
    assert_eq!(
        texts,
        rows.matches("textFormat: Text.PlainText").count(),
        "every Text in ScriptActionRows is PlainText"
    );
    assert!(!rows.contains("RichText"));
    assert!(!rows.contains("Process"));
    assert!(rows.contains("controller.services.actions"));
    assert!(rows.contains("visible: rows.length > 0"));
    assert!(rows.contains("import \"../lib/ActionRows.js\" as ActionRows"));
    let menu = text(&root.join("panes/FileActionsMenu.qml"));
    assert_eq!(
        menu.matches("ScriptActionRows {").count(),
        1,
        "the menu holds one script action strip"
    );
    assert!(menu.contains("controller.focusTree(null, true)"));
    assert!(
        !menu.contains("component MenuButton"),
        "MenuButton lives in panes/MenuButton.qml"
    );
    let button = text(&root.join("panes/MenuButton.qml"));
    assert!(button.contains("required property var menu"));
    assert!(button.contains("control.menu.focusRelative(control, 1)"));
    let controller = text(&root.join("controllers/ActionController.qml"));
    assert!(controller.contains("backendRequest(\"action-list\""));
    assert!(controller.contains("backendRequest(\"action-run\""));
    assert!(controller.contains("service.decodedJsonDocument(encoded)"));
    assert!(controller.contains("Component.onCompleted: refresh()"));
    assert!(controller.contains("onRegistryChanged: refresh()"));
    assert!(
        !controller.contains("argv"),
        "the host never holds a command vector"
    );
    assert!(controller.contains("import \"../lib/ActionRows.js\" as ActionRows"));
    assert!(!controller.contains("Process"));
    let service = text(&root.join("Service.qml"));
    assert!(service.contains("var map = ({ files: service, actions: actionController })"));
    assert!(
        service.contains(
            "for (var i = 0; i < ids.length; i++) if (!map[ids[i]]) map[ids[i]] = supplied[ids[i]]"
        ),
        "a contributed provider reaches its module through the services map"
    );
    let ipc = text(&root.join("controllers/FileTreeIpc.qml"));
    assert!(ipc.contains("function actions(): string"));
    assert!(ipc.contains("function actionResult(requestId: string): string"));
    assert!(
        ipc.contains("function runAction(key: string, pathsJson: string, yes: string): string")
    );
}

#[test]
fn no_automatic_agent_instruction_path_is_installed_with_the_plugin() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let candidates = [
        "AGENTS.md",
        "CLAUDE.md",
        "GEMINI.md",
        ".cursorrules",
        ".clinerules",
        ".github/copilot-instructions.md",
        ".claude",
        ".codex",
        ".agents",
    ];
    let listed = Command::new("git")
        .current_dir(root)
        .arg("ls-files")
        .arg("--")
        .args(candidates)
        .output()
        .expect("git ls-files runs");
    assert!(listed.status.success(), "git ls-files failed");
    let tracked = String::from_utf8_lossy(&listed.stdout);
    assert!(
        tracked.trim().is_empty(),
        "cloned into the plugin directory, where coding agents read it on their own: {}",
        tracked.trim()
    );
    let guidelines = text(&root.join("docs/agent-guidelines.md"));
    assert!(guidelines.contains("# Instructions for agents"));
    let contributing = text(&root.join("CONTRIBUTING.md"));
    assert!(contributing.contains("[docs/agent-guidelines.md](docs/agent-guidelines.md)"));
}

#[test]
fn qml_objects_do_not_bind_the_same_signal_twice() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut offenders = Vec::new();
    for path in files(root, &["qml"]) {
        let source = text(&path);
        let mut scopes: Vec<Vec<String>> = vec![Vec::new()];
        let mut in_string: Option<char> = None;
        let mut in_block_comment = false;
        for (line_number, line) in source.lines().enumerate() {
            let trimmed = line.trim_start();
            if !in_block_comment
                && in_string.is_none()
                && let Some(rest) = trimmed.strip_prefix("on")
            {
                let name: String = rest
                    .chars()
                    .take_while(|c| c.is_ascii_alphanumeric() || *c == '_')
                    .collect();
                if name.chars().next().is_some_and(|c| c.is_ascii_uppercase())
                    && rest[name.len()..].starts_with(':')
                {
                    let handler = format!("on{name}");
                    let scope = scopes.last_mut().unwrap();
                    if scope.contains(&handler) {
                        offenders.push(format!(
                            "{}:{}: {handler} bound twice in one object",
                            path.display(),
                            line_number + 1
                        ));
                    } else {
                        scope.push(handler);
                    }
                }
            }
            let mut chars = line.chars().peekable();
            while let Some(c) = chars.next() {
                if in_block_comment {
                    if c == '*' && chars.peek() == Some(&'/') {
                        chars.next();
                        in_block_comment = false;
                    }
                    continue;
                }
                if let Some(quote) = in_string {
                    if c == '\\' {
                        chars.next();
                    } else if c == quote {
                        in_string = None;
                    }
                    continue;
                }
                match c {
                    '/' if chars.peek() == Some(&'/') => break,
                    '/' if chars.peek() == Some(&'*') => {
                        chars.next();
                        in_block_comment = true;
                    }
                    '"' | '\'' | '`' => in_string = Some(c),
                    '{' => scopes.push(Vec::new()),
                    '}' if scopes.len() > 1 => {
                        scopes.pop();
                    }
                    _ => {}
                }
            }
        }
    }
    assert!(offenders.is_empty(), "{}", offenders.join("\n"));
}
