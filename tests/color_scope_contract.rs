use std::{fs, path::Path};

#[test]
fn all_color_scope_tints_property_columns_with_a_muted_entry_color() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let row = fs::read_to_string(root.join("panes/BrowserRow.qml")).unwrap();

    assert!(row.contains("readonly property bool colorsRow: entryColor !== \"\" && controller.folderColorScope === \"row\""));
    assert!(row.contains(
        "readonly property color mutedEntryColor: colorsRow ? Util.alpha(entryColor, 0.68) : Commons.Color.muted"
    ));
    assert!(row.contains("color: row.mutedEntryColor"));
    assert!(row.contains(
        "color: row.colorsRow ? row.mutedEntryColor : (row.selected ? Commons.Color.bar.text : Commons.Color.muted)"
    ));
}

#[test]
fn git_status_outranks_the_manual_entry_color_on_the_name_and_the_icon() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let row = fs::read_to_string(root.join("panes/BrowserRow.qml")).unwrap();

    assert!(row.contains(
        "readonly property bool colorsGit: controller.gitEnabled && !gitDeleted && !gitIgnored && gitStatus !== \"\""
    ));
    assert!(
        row.contains("readonly property color gitEntryColor: controller.gitStatusColor(gitStatus)")
    );
    for expression in [
        "? row.gitRowColor\n          : (row.hiddenEntry",
        "? row.gitRowColor\n            : (row.hiddenEntry",
    ] {
        assert!(
            row.contains(expression),
            "git status must be tested before the manual entry color: {expression}"
        );
    }
    assert!(row.contains("row.moreRow || row.showsGitIgnored\n          ? row.mutedEntryColor"));
    assert!(row.contains("row.showsGitIgnored\n        ? row.mutedEntryColor"));
}

#[test]
fn gitignored_entries_keep_their_file_icon_and_mark_the_git_column() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let row = fs::read_to_string(root.join("panes/BrowserRow.qml")).unwrap();

    assert!(row.contains(
        ": FileIcons.entryIcon(row.name, row.isDir, row.isSymlink, row.expanded && row.treeMode, row.isGitRepo, row.homeEntry))"
    ));
    assert!(!row.contains("row.gitIgnored\n          ? \"\""));
    assert!(
        row.contains("text: row.error ? \"\" : (row.showsGitIgnored ? \"\" : row.gitStatus)")
    );
    assert!(row.contains("visible: !row.showsGitIgnored && row.error === \"\""));
}

#[test]
fn hidden_entries_are_dimmed_while_keeping_their_git_color() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let row = fs::read_to_string(root.join("panes/BrowserRow.qml")).unwrap();

    assert!(row.contains(
        "readonly property color gitRowColor: hiddenEntry ? Util.alpha(gitEntryColor, 0.68) : gitEntryColor"
    ));
    assert!(!row.contains("row.showsGitIgnored || row.hiddenEntry"));
    assert!(row.contains("? row.gitRowColor"));
    assert!(!row.contains("row.moreRow || row.showsGitIgnored || row.hiddenEntry"));
    assert!(row.contains("row.showsGitIgnored ? \"\" : row.gitStatus"));
    assert!(!row.contains("row.hiddenEntry ? \"\""));
}
