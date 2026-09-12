use fileblade::secure;
use serde_json::{Value, json};
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;
use tempfile::tempdir;

#[test]
fn copy_preserves_metadata_and_uses_private_journal_state() {
    let temporary = tempdir().unwrap();
    let source = temporary.path().join("source");
    let destination = temporary.path().join("destination");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(source.join("body.txt"), "body").unwrap();
    fs::set_permissions(source.join("body.txt"), fs::Permissions::from_mode(0o640)).unwrap();
    symlink("body.txt", source.join("link")).unwrap();

    let payload = backend(
        temporary.path(),
        &[
            "copy",
            "--destination",
            destination.to_str().unwrap(),
            "--source",
            source.to_str().unwrap(),
        ],
    );

    assert_eq!(payload["ok"], true, "{payload}");
    let copied = destination.join("source");
    assert_eq!(fs::read_to_string(copied.join("body.txt")).unwrap(), "body");
    assert_eq!(
        fs::read_link(copied.join("link")).unwrap(),
        PathBuf::from("body.txt")
    );
    assert_eq!(
        fs::metadata(copied.join("body.txt"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o640
    );
    for path in [
        temporary.path().join("state"),
        temporary.path().join("state/journal.json"),
        temporary.path().join("state/journal.json.lock"),
    ] {
        let expected = if path.is_dir() { 0o700 } else { 0o600 };
        assert_eq!(
            fs::metadata(path).unwrap().permissions().mode() & 0o777,
            expected
        );
    }
}

#[test]
fn artifact_directory_round_trip_is_private_and_resumable() {
    let temporary = tempdir().unwrap();
    let tree = temporary.path().join("tree");
    fs::create_dir(&tree).unwrap();
    fs::create_dir(tree.join("nested")).unwrap();
    fs::write(tree.join("nested/body.txt"), "nested body").unwrap();
    fs::set_permissions(
        tree.join("nested/body.txt"),
        fs::Permissions::from_mode(0o640),
    )
    .unwrap();
    symlink("nested/body.txt", tree.join("link")).unwrap();
    let item = json!({
        "id": "tree-1",
        "name": "Tree",
        "kind": "directory",
        "scope": "test",
        "position": 3,
        "groups": ["Project", "Instructions"],
        "metrics": {"tokens": 42, "updated": "2026-09-03"},
        "paths": [tree.to_string_lossy()],
        "payload": {"answer": 42}
    });

    let put = backend(
        temporary.path(),
        &["bin-put", "--module", "trees", "--item", &item.to_string()],
    );
    assert_eq!(put["ok"], true, "{put}");
    let entry_id = put["entry"].as_str().unwrap();
    let opaque_id = entry_id.strip_prefix("bin:").unwrap();
    assert_eq!(opaque_id.len(), 32);
    assert!(opaque_id.bytes().all(|byte| byte.is_ascii_hexdigit()));
    assert!(!entry_id.contains("tree-1"));
    assert!(!tree.exists());
    let listed = backend(temporary.path(), &["bin-list", "--module", "trees"]);
    assert_eq!(listed["items"][0]["position"], 3);
    assert_eq!(
        listed["items"][0]["groups"],
        json!(["Project", "Instructions"])
    );
    assert_eq!(listed["items"][0]["metrics"]["tokens"], 42);
    assert_eq!(listed["items"][0]["metrics"]["updated"], "2026-09-03");
    assert_eq!(listed["items"][0]["scope"], "test");
    let stored = PathBuf::from(listed["items"][0]["realpath"].as_str().unwrap());
    assert_eq!(
        fs::metadata(&stored).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(
        fs::metadata(stored.ancestors().nth(2).unwrap())
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o700
    );

    let restored = backend(
        temporary.path(),
        &[
            "bin-restore",
            "--module",
            "trees",
            "--id",
            put["entry"].as_str().unwrap(),
        ],
    );
    assert_eq!(restored["ok"], true, "{restored}");
    assert_eq!(
        fs::read_to_string(tree.join("nested/body.txt")).unwrap(),
        "nested body"
    );
    assert_eq!(
        fs::read_link(tree.join("link")).unwrap(),
        PathBuf::from("nested/body.txt")
    );
    assert_eq!(
        fs::metadata(tree.join("nested/body.txt"))
            .unwrap()
            .permissions()
            .mode()
            & 0o777,
        0o640
    );
    assert!(
        backend(temporary.path(), &["bin-list", "--module", "trees"])["items"]
            .as_array()
            .unwrap()
            .is_empty()
    );

    let exact = temporary.path().join("exact.txt");
    fs::write(&exact, "same").unwrap();
    let exact_item = json!({"id": "exact", "paths": [exact.to_string_lossy()]});
    let put = backend(
        temporary.path(),
        &[
            "bin-put",
            "--module",
            "trees",
            "--item",
            &exact_item.to_string(),
        ],
    );
    fs::write(&exact, "same").unwrap();
    let resumed = backend(
        temporary.path(),
        &[
            "bin-restore",
            "--module",
            "trees",
            "--id",
            put["entry"].as_str().unwrap(),
        ],
    );
    assert_eq!(resumed["ok"], true, "{resumed}");
    assert_eq!(resumed["results"][0]["changed"], false);

    let logical = json!({"id": "logical", "name": "Logical", "paths": [], "payload": {"value": 7}});
    let logical_put = backend(
        temporary.path(),
        &[
            "bin-put",
            "--module",
            "trees",
            "--item",
            &logical.to_string(),
        ],
    );
    let prepared = backend(
        temporary.path(),
        &[
            "bin-restore",
            "--module",
            "trees",
            "--id",
            logical_put["entry"].as_str().unwrap(),
        ],
    );
    assert_eq!(prepared["ok"], true, "{prepared}");
    assert_eq!(prepared["pending_commit"], true);
    assert_eq!(prepared["payload"]["value"], 7);
    assert_eq!(
        backend(temporary.path(), &["bin-list", "--module", "trees"])["items"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    let committed = backend(
        temporary.path(),
        &[
            "bin-purge",
            "--module",
            "trees",
            "--id",
            logical_put["entry"].as_str().unwrap(),
        ],
    );
    assert_eq!(committed["ok"], true, "{committed}");
}

#[test]
fn artifact_bin_mutates_symlink_targets_and_keeps_the_links() {
    let temporary = tempdir().unwrap();
    let target_file = temporary.path().join("target-memory.md");
    let target_directory = temporary.path().join("target-skill");
    let file_link = temporary.path().join("memory.md");
    let directory_link = temporary.path().join("skill");
    fs::write(&target_file, "memory").unwrap();
    fs::create_dir(&target_directory).unwrap();
    fs::write(target_directory.join("SKILL.md"), "skill").unwrap();
    symlink(&target_file, &file_link).unwrap();
    symlink(&target_directory, &directory_link).unwrap();
    let item = json!({
        "id": "linked-artifacts",
        "name": "Linked artifacts",
        "path": file_link.to_string_lossy(),
        "paths": [file_link.to_string_lossy(), directory_link.to_string_lossy()]
    });

    let put = backend(
        temporary.path(),
        &["bin-put", "--module", "links", "--item", &item.to_string()],
    );
    assert_eq!(put["ok"], true, "{put}");
    assert!(
        fs::symlink_metadata(&file_link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(
        fs::symlink_metadata(&directory_link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(!target_file.exists());
    assert!(!target_directory.exists());
    let listed = backend(temporary.path(), &["bin-list", "--module", "links"]);
    assert_eq!(
        listed["items"][0]["linkTarget"],
        target_file.to_string_lossy().as_ref()
    );

    let entry_id = put["entry"].as_str().unwrap().strip_prefix("bin:").unwrap();
    let manifest_path = temporary
        .path()
        .join("data/fileblade/bin/links")
        .join(entry_id)
        .join("manifest.json");
    let original_manifest = fs::read(&manifest_path).unwrap();
    let mut legacy_manifest: Value = serde_json::from_slice(&original_manifest).unwrap();
    legacy_manifest["realpath"] = Value::String(String::new());
    legacy_manifest["items"][0]["type"] = Value::String("symlink".to_string());
    legacy_manifest["items"][0]["size"] = Value::from(0);
    legacy_manifest["items"][0]["target"] =
        Value::String(target_file.to_string_lossy().into_owned());
    fs::write(
        &manifest_path,
        serde_json::to_vec(&legacy_manifest).unwrap(),
    )
    .unwrap();
    let legacy_listed = backend(temporary.path(), &["bin-list", "--module", "links"]);
    assert_eq!(
        legacy_listed["items"][0]["linkTarget"],
        target_file.to_string_lossy().as_ref()
    );
    fs::write(&manifest_path, original_manifest).unwrap();

    let restored = backend(
        temporary.path(),
        &[
            "bin-restore",
            "--module",
            "links",
            "--id",
            put["entry"].as_str().unwrap(),
        ],
    );
    assert_eq!(restored["ok"], true, "{restored}");
    assert_eq!(fs::read_to_string(&target_file).unwrap(), "memory");
    assert_eq!(
        fs::read_to_string(target_directory.join("SKILL.md")).unwrap(),
        "skill"
    );
    assert_eq!(fs::read_to_string(&file_link).unwrap(), "memory");
    assert_eq!(
        fs::read_to_string(directory_link.join("SKILL.md")).unwrap(),
        "skill"
    );
}

#[test]
fn satellite_trash_is_listed_centrally_and_pruned_by_age() {
    let temporary = tempdir().unwrap();
    let old = temporary.path().join("old.txt");
    let current = temporary.path().join("current.txt");
    let unknown = temporary.path().join("unknown.txt");
    fs::write(&old, "old").unwrap();
    fs::write(&current, "current").unwrap();
    fs::write(&unknown, "unknown").unwrap();

    let old_put = backend(
        temporary.path(),
        &[
            "bin-put",
            "--module",
            "test",
            "--item",
            &json!({"id": "old", "name": "Old", "paths": [old]}).to_string(),
        ],
    );
    let current_put = backend(
        temporary.path(),
        &[
            "bin-put",
            "--module",
            "test",
            "--item",
            &json!({"id": "current", "name": "Current", "paths": [current]}).to_string(),
        ],
    );
    let unknown_put = backend(
        temporary.path(),
        &[
            "bin-put",
            "--module",
            "test",
            "--item",
            &json!({"id": "unknown", "name": "Unknown", "paths": [unknown]}).to_string(),
        ],
    );
    set_manifest_age(
        temporary.path(),
        "test",
        old_put["entry"].as_str().unwrap(),
        Some(0),
        "2000-01-01 00:00",
    );
    set_manifest_age(
        temporary.path(),
        "test",
        unknown_put["entry"].as_str().unwrap(),
        None,
        "not-a-date",
    );

    let listed = backend(temporary.path(), &["trash-list", "--limit", "100"]);
    assert_eq!(listed["ok"], true, "{listed}");
    assert_eq!(listed["count"], 3);
    assert!(
        listed["entries"]
            .as_array()
            .unwrap()
            .iter()
            .all(|entry| entry["source"] == "satellite")
    );

    let pruned = backend(temporary.path(), &["trash-prune", "--days", "7"]);
    assert_eq!(pruned["ok"], true, "{pruned}");
    assert_eq!(pruned["completed"], 1);
    assert!(pruned["unknown_age"].as_u64().unwrap_or(0) >= 1);
    let remaining = backend(temporary.path(), &["bin-list", "--module", "test"]);
    assert_eq!(remaining["items"].as_array().unwrap().len(), 2);
    assert!(
        remaining["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["id"] == current_put["entry"])
    );
    assert!(
        remaining["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["id"] == unknown_put["entry"])
    );

    let emptied = backend(temporary.path(), &["trash-empty"]);
    assert_eq!(emptied["ok"], true, "{emptied}");
    assert_eq!(emptied["completed"], 2);
    assert_eq!(
        backend(temporary.path(), &["trash-list", "--limit", "100"])["count"],
        0
    );
}

fn set_manifest_age(root: &Path, module: &str, entry: &str, epoch: Option<i64>, text: &str) {
    let path = root
        .join("data/fileblade/bin")
        .join(module)
        .join(entry.strip_prefix("bin:").unwrap())
        .join("manifest.json");
    let mut manifest: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
    manifest["deletedAt"] = Value::String(text.to_string());
    if let Some(value) = epoch {
        manifest["deletedAtEpoch"] = Value::from(value);
    } else {
        manifest.as_object_mut().unwrap().remove("deletedAtEpoch");
    }
    fs::write(path, serde_json::to_vec(&manifest).unwrap()).unwrap();
}

#[test]
fn secure_mutations_cancel_cleanly_and_never_replace() {
    let temporary = tempdir().unwrap();
    let source = temporary.path().join("source.txt");
    let cancelled_target = temporary.path().join("cancelled.txt");
    fs::write(&source, "source").unwrap();
    let cancelled = AtomicBool::new(true);
    let mut ignored: fn(&Path) = |_| {};
    let error = secure::copy_path_noreplace(&source, &cancelled_target, &cancelled, &mut ignored)
        .unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::Interrupted);
    assert!(!cancelled_target.exists());
    assert!(fs::read_dir(temporary.path()).unwrap().all(|entry| {
        !entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .starts_with(".fileblade-partial-")
    }));

    let copied = temporary.path().join("copied.txt");
    let active = AtomicBool::new(false);
    let mut observed_private_stage = false;
    let mut observe = |_: &Path| {
        for entry in fs::read_dir(temporary.path()).unwrap().flatten() {
            if entry
                .file_name()
                .to_string_lossy()
                .starts_with(".fileblade-partial-")
            {
                observed_private_stage = true;
                assert_eq!(
                    entry.metadata().unwrap().permissions().mode() & 0o777,
                    0o700
                );
            }
        }
    };
    secure::copy_path_noreplace(&source, &copied, &active, &mut observe).unwrap();
    assert!(observed_private_stage);
    assert_eq!(fs::read_to_string(copied).unwrap(), "source");

    let occupied = temporary.path().join("occupied.txt");
    fs::write(&occupied, "occupied").unwrap();
    let error = secure::rename_noreplace(&source, &occupied).unwrap_err();
    assert_eq!(error.kind(), std::io::ErrorKind::AlreadyExists);
    assert_eq!(fs::read_to_string(&source).unwrap(), "source");
    assert_eq!(fs::read_to_string(&occupied).unwrap(), "occupied");
}

#[test]
fn secure_mutations_reject_intermediate_symlink_traversal() {
    let temporary = tempdir().unwrap();
    let real = temporary.path().join("real");
    let alias = temporary.path().join("alias");
    fs::create_dir(&real).unwrap();
    fs::write(real.join("victim.txt"), "victim").unwrap();
    symlink(&real, &alias).unwrap();

    let source = temporary.path().join("source.txt");
    fs::write(&source, "source").unwrap();
    assert!(secure::create_file_noreplace(&alias.join("created.txt"), 0o600).is_err());
    assert!(secure::rename_noreplace(&source, &alias.join("moved.txt")).is_err());

    let active = AtomicBool::new(false);
    let mut ignored: fn(&Path) = |_| {};
    assert!(
        secure::copy_path_noreplace(&source, &alias.join("copied.txt"), &active, &mut ignored)
            .is_err()
    );
    assert!(
        secure::copy_path_noreplace(
            &alias.join("victim.txt"),
            &temporary.path().join("escaped.txt"),
            &active,
            &mut ignored
        )
        .is_err()
    );
    assert!(secure::remove_path(&alias.join("victim.txt")).is_err());

    assert_eq!(fs::read_to_string(&source).unwrap(), "source");
    assert_eq!(
        fs::read_to_string(real.join("victim.txt")).unwrap(),
        "victim"
    );
    assert!(!real.join("created.txt").exists());
    assert!(!real.join("moved.txt").exists());
    assert!(!real.join("copied.txt").exists());
    assert!(!temporary.path().join("escaped.txt").exists());

    let guarded = temporary.path().join("guarded.txt");
    let original = temporary.path().join("original.txt");
    fs::write(&guarded, "guarded").unwrap();
    let expected = secure::entry_stat(&guarded).unwrap().identity();
    fs::rename(&guarded, &original).unwrap();
    fs::write(&guarded, "replacement").unwrap();
    assert!(secure::remove_nondirectory_matching(&guarded, expected).is_err());
    assert_eq!(fs::read_to_string(&guarded).unwrap(), "replacement");
    assert_eq!(fs::read_to_string(&original).unwrap(), "guarded");

    let relocation = temporary.path().join("relocation.txt");
    let preserved = temporary.path().join("preserved.txt");
    let destination = temporary.path().join("relocated.txt");
    fs::write(&relocation, "relocation").unwrap();
    let expected = secure::entry_stat(&relocation).unwrap().identity();
    fs::rename(&relocation, &preserved).unwrap();
    fs::write(&relocation, "replacement").unwrap();
    assert!(
        secure::relocate_noreplace_matching(
            &relocation,
            &destination,
            expected,
            &active,
            &mut ignored
        )
        .is_err()
    );
    assert_eq!(fs::read_to_string(&relocation).unwrap(), "replacement");
    assert_eq!(fs::read_to_string(&preserved).unwrap(), "relocation");
    assert!(!destination.exists());

    let final_link = temporary.path().join("final-link");
    symlink(real.join("victim.txt"), &final_link).unwrap();
    secure::remove_nondirectory(&final_link).unwrap();
    assert!(!final_link.exists());
    assert_eq!(
        fs::read_to_string(real.join("victim.txt")).unwrap(),
        "victim"
    );
}

#[test]
fn resolved_destinations_stay_bound_when_the_visible_directory_is_replaced() {
    let temporary = tempdir().unwrap();
    let destination = temporary.path().join("destination");
    let retained = temporary.path().join("destination-retained");
    fs::create_dir(&destination).unwrap();
    let directory = secure::open_directory_nofollow(&destination).unwrap();
    let created = secure::resolved_child(&directory, &destination, "created.txt".as_ref()).unwrap();
    let copied = secure::resolved_child(&directory, &destination, "copied.txt".as_ref()).unwrap();
    let moved = secure::resolved_child(&directory, &destination, "moved.txt".as_ref()).unwrap();

    fs::rename(&destination, &retained).unwrap();
    fs::create_dir(&destination).unwrap();
    secure::create_file_noreplace_resolved(&created, 0o600).unwrap();

    let copy_source_path = temporary.path().join("copy-source.txt");
    fs::write(&copy_source_path, "copy").unwrap();
    let copy_source = secure::resolved_parent(&copy_source_path).unwrap();
    let active = AtomicBool::new(false);
    let mut ignored: fn(&Path) = |_| {};
    secure::copy_path_noreplace_resolved(&copy_source, &copied, &active, &mut ignored).unwrap();

    let move_source_path = temporary.path().join("move-source.txt");
    fs::write(&move_source_path, "move").unwrap();
    let move_source = secure::resolved_parent(&move_source_path).unwrap();
    let expected = secure::entry_stat_resolved(&move_source)
        .unwrap()
        .identity();
    secure::relocate_noreplace_matching_resolved(
        move_source,
        &moved,
        expected,
        &active,
        &mut ignored,
    )
    .unwrap();

    assert!(retained.join("created.txt").exists());
    assert_eq!(
        fs::read_to_string(retained.join("copied.txt")).unwrap(),
        "copy"
    );
    assert_eq!(
        fs::read_to_string(retained.join("moved.txt")).unwrap(),
        "move"
    );
    assert_eq!(fs::read_dir(&destination).unwrap().count(), 0);
}

#[test]
fn journal_refuses_modified_outputs_and_force_round_trips_through_trash() {
    let temporary = tempdir().unwrap();
    let tools = temporary.path().join("tools");
    fs::create_dir(&tools).unwrap();
    let gio = tools.join("gio");
    fs::write(
        &gio,
        r#"#!/bin/sh
shift
if [ "$1" = "--" ]; then shift; fi
mkdir -p "$XDG_DATA_HOME/Trash/files" "$XDG_DATA_HOME/Trash/info"
for path in "$@"; do
    name="${path##*/}"
    mv -- "$path" "$XDG_DATA_HOME/Trash/files/$name"
    printf '[Trash Info]\nPath=%s\nDeletionDate=2026-09-02T12:00:00\n' "$path" > "$XDG_DATA_HOME/Trash/info/$name.trashinfo"
done
"#,
    )
    .unwrap();
    fs::set_permissions(&gio, fs::Permissions::from_mode(0o700)).unwrap();
    let source = temporary.path().join("source.txt");
    let destination = temporary.path().join("destination");
    fs::write(&source, "before").unwrap();
    fs::create_dir(&destination).unwrap();
    let copied = backend_with_tools(
        temporary.path(),
        &[
            "copy",
            "--destination",
            destination.to_str().unwrap(),
            "--source",
            source.to_str().unwrap(),
        ],
        Some(&tools),
    );
    assert_eq!(copied["ok"], true, "{copied}");
    let output = destination.join("source.txt");
    fs::write(&output, "after").unwrap();
    let refused = backend_with_tools(temporary.path(), &["undo"], Some(&tools));
    assert_eq!(refused["ok"], false, "{refused}");
    assert_eq!(refused["refused"], true);
    assert!(output.exists());
    let forced = backend_with_tools(temporary.path(), &["undo", "--force"], Some(&tools));
    assert_eq!(forced["ok"], true, "{forced}");
    assert!(!output.exists());
    let redone = backend_with_tools(temporary.path(), &["redo"], Some(&tools));
    assert_eq!(redone["ok"], true, "{redone}");
    assert_eq!(fs::read_to_string(output).unwrap(), "after");
}

#[test]
fn directory_undo_detects_same_size_edits_despite_newer_siblings() {
    let temporary = tempdir().unwrap();
    let root = temporary.path();
    let source = root.join("source");
    let destination = root.join("destination");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&destination).unwrap();
    fs::write(source.join("note.txt"), "old").unwrap();
    fs::write(source.join("future.txt"), "future").unwrap();
    filetime::set_file_mtime(
        source.join("future.txt"),
        filetime::FileTime::from_unix_time(2_000_000_000, 0),
    )
    .unwrap();
    let copied = backend(
        root,
        &[
            "copy",
            "--source",
            source.to_str().unwrap(),
            "--destination",
            destination.to_str().unwrap(),
        ],
    );
    assert_eq!(copied["ok"], true, "{copied}");
    let note = destination.join("source/note.txt");
    let modified = filetime::FileTime::from_last_modification_time(&fs::metadata(&note).unwrap());
    fs::write(&note, "new").unwrap();
    filetime::set_file_mtime(&note, modified).unwrap();
    let refused = backend(root, &["undo"]);
    assert_eq!(refused["refused"], true, "{refused}");
    assert_eq!(fs::read_to_string(note).unwrap(), "new");
}

#[test]
fn trash_uses_a_pinned_parent_when_the_visible_ancestor_is_replaced() {
    let temporary = tempdir().unwrap();
    let tools = temporary.path().join("tools");
    let selected_parent = temporary.path().join("selected");
    let moved_parent = temporary.path().join("selected-moved");
    let outside = temporary.path().join("outside");
    fs::create_dir(&tools).unwrap();
    fs::create_dir(&selected_parent).unwrap();
    fs::create_dir(&outside).unwrap();
    let selected = selected_parent.join("victim.txt");
    let outside_file = outside.join("victim.txt");
    fs::write(&selected, "selected").unwrap();
    fs::write(&outside_file, "outside").unwrap();
    let gio = tools.join("gio");
    fs::write(
        &gio,
        format!(
            r#"#!/bin/sh
shift
if [ "$1" = "--" ]; then shift; fi
mv -- '{}' '{}'
ln -s -- '{}' '{}'
mkdir -p "$XDG_DATA_HOME/Trash/files" "$XDG_DATA_HOME/Trash/info"
for path in "$@"; do
    name="${{path##*/}}"
    mv -- "$path" "$XDG_DATA_HOME/Trash/files/$name"
    printf '[Trash Info]\nPath=%s\nDeletionDate=2026-09-02T12:00:00\n' "$path" > "$XDG_DATA_HOME/Trash/info/$name.trashinfo"
done
"#,
            selected_parent.display(),
            moved_parent.display(),
            outside.display(),
            selected_parent.display()
        ),
    )
    .unwrap();
    fs::set_permissions(&gio, fs::Permissions::from_mode(0o700)).unwrap();

    let payload = backend_with_tools(
        temporary.path(),
        &["trash", "--path", selected.to_str().unwrap()],
        Some(&tools),
    );
    assert_eq!(payload["ok"], true, "{payload}");
    assert_eq!(payload["paths"][0], selected.to_string_lossy().as_ref());
    assert_eq!(fs::read_to_string(&outside_file).unwrap(), "outside");
    assert!(selected_parent.is_symlink());
    assert!(!moved_parent.join("victim.txt").exists());
    let info = fs::read_dir(temporary.path().join("data/Trash/info"))
        .unwrap()
        .next()
        .unwrap()
        .unwrap()
        .path();
    assert!(
        info.file_name()
            .unwrap()
            .to_string_lossy()
            .starts_with("victim.txt-")
    );
    assert_eq!(
        fs::read_dir(temporary.path().join("data/Trash/info"))
            .unwrap()
            .count(),
        1
    );
    assert_eq!(
        fs::read_dir(temporary.path().join("data/Trash/files"))
            .unwrap()
            .count(),
        1
    );
    assert!(
        fs::read_to_string(info)
            .unwrap()
            .contains(&format!("Path={}", selected.display()))
    );
}

#[test]
fn artifact_trash_follows_a_symlink_without_removing_the_link() {
    let temporary = tempdir().unwrap();
    let tools = temporary.path().join("tools");
    fs::create_dir(&tools).unwrap();
    let target = temporary.path().join("memory-target.md");
    let link = temporary.path().join("memory.md");
    fs::write(&target, "memory").unwrap();
    symlink(&target, &link).unwrap();
    let gio = tools.join("gio");
    fs::write(
        &gio,
        r#"#!/bin/sh
shift
if [ "$1" = "--" ]; then shift; fi
mkdir -p "$XDG_DATA_HOME/Trash/files" "$XDG_DATA_HOME/Trash/info"
for path in "$@"; do
    name="${path##*/}"
    mv -- "$path" "$XDG_DATA_HOME/Trash/files/$name"
    printf '[Trash Info]\nPath=%s\nDeletionDate=2026-09-02T12:00:00\n' "$path" > "$XDG_DATA_HOME/Trash/info/$name.trashinfo"
done
"#,
    )
    .unwrap();
    fs::set_permissions(&gio, fs::Permissions::from_mode(0o700)).unwrap();

    let payload = backend_with_tools(
        temporary.path(),
        &[
            "trash",
            "--follow-symlinks",
            "--path",
            link.to_str().unwrap(),
        ],
        Some(&tools),
    );
    assert_eq!(payload["ok"], true, "{payload}");
    assert_eq!(payload["paths"][0], target.to_string_lossy().as_ref());
    assert!(
        fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
    assert!(!target.exists());
}

#[test]
fn journal_read_never_follows_a_replaced_state_file() {
    let temporary = tempdir().unwrap();
    let state = temporary.path().join("state");
    fs::create_dir(&state).unwrap();
    fs::set_permissions(&state, fs::Permissions::from_mode(0o700)).unwrap();
    let victim = temporary.path().join("victim.json");
    fs::write(&victim, "private data").unwrap();
    symlink(&victim, state.join("journal.json")).unwrap();
    let payload = backend(temporary.path(), &["journal"]);
    assert_eq!(payload["ok"], false, "{payload}");
    assert_eq!(fs::read_to_string(victim).unwrap(), "private data");
}

fn backend(root: &Path, arguments: &[&str]) -> Value {
    backend_with_tools(root, arguments, None)
}

#[test]
fn journal_failures_do_not_hide_successful_create_and_rename() {
    let temporary = tempdir().unwrap();
    let root = temporary.path();
    fs::write(root.join("state"), "blocked journal parent").unwrap();
    let created = backend(
        root,
        &[
            "create",
            "--parent",
            root.to_str().unwrap(),
            "--name",
            "created.txt",
        ],
    );
    assert_eq!(created["ok"], true, "{created}");
    assert!(!created["journal_warning"].as_str().unwrap().is_empty());
    let source = root.join("created.txt");
    assert_eq!(created["path"], source.to_string_lossy().as_ref());
    let renamed = backend(
        root,
        &[
            "rename",
            "--path",
            source.to_str().unwrap(),
            "--name",
            "renamed.txt",
        ],
    );
    assert_eq!(renamed["ok"], true, "{renamed}");
    assert!(!renamed["journal_warning"].as_str().unwrap().is_empty());
    assert_eq!(
        renamed["mappings"][0]["source"],
        source.to_string_lossy().as_ref()
    );
    assert!(root.join("renamed.txt").exists());
}

fn backend_with_tools(root: &Path, arguments: &[&str], tools: Option<&Path>) -> Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_fileblade"));
    command
        .args(["--output", "json", "_backend"])
        .args(arguments)
        .env("XDG_DATA_HOME", root.join("data"))
        .env("XDG_STATE_HOME", root.join("state-home"))
        .env("FILEBLADE_JOURNAL", root.join("state/journal.json"));
    if let Some(tools) = tools {
        let mut paths = vec![tools.to_path_buf()];
        paths.extend(std::env::split_paths(
            &std::env::var_os("PATH").unwrap_or_default(),
        ));
        command.env("PATH", std::env::join_paths(paths).unwrap());
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout)
        .lines()
        .rfind(|line| !line.trim().is_empty())
        .map(|line| serde_json::from_str(line).unwrap())
        .unwrap()
}

#[test]
fn clipboard_text_writes_one_absolute_path_per_line() {
    let temporary = tempdir().unwrap();
    let tools = temporary.path().join("tools");
    fs::create_dir(&tools).unwrap();
    let capture = temporary.path().join("clipboard.txt");
    fs::write(
        tools.join("wl-copy"),
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" > '{}.args'\ncat > '{}'\n",
            capture.display(),
            capture.display()
        ),
    )
    .unwrap();
    fs::set_permissions(tools.join("wl-copy"), fs::Permissions::from_mode(0o755)).unwrap();
    let odd = temporary.path().join("it's |odd| name.txt");
    fs::write(&odd, "x").unwrap();
    let payload = backend_with_tools(
        temporary.path(),
        &[
            "clipboard-text",
            "--path",
            odd.to_str().unwrap(),
            "--path",
            temporary.path().to_str().unwrap(),
        ],
        Some(&tools),
    );
    assert_eq!(payload["ok"], true, "{payload}");
    assert_eq!(payload["paths"], 2);
    let expected = format!("{}\n{}", odd.display(), temporary.path().display());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        if fs::read_to_string(&capture).unwrap_or_default() == expected {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(fs::read_to_string(&capture).unwrap(), expected);
    assert_eq!(
        fs::read_to_string(format!("{}.args", capture.display()))
            .unwrap()
            .trim(),
        "--type text/plain"
    );
}

#[test]
fn copying_files_reports_a_clipboard_program_that_refuses_to_run() {
    let temporary = tempdir().unwrap();
    let tools = temporary.path().join("tools");
    fs::create_dir(&tools).unwrap();
    fs::write(tools.join("wl-copy"), "#!/bin/sh\nexit 1\n").unwrap();
    fs::set_permissions(tools.join("wl-copy"), fs::Permissions::from_mode(0o755)).unwrap();
    let file = temporary.path().join("one.txt");
    fs::write(&file, "x").unwrap();
    let payload = backend_with_tools(
        temporary.path(),
        &["clipboard-write", "--path", file.to_str().unwrap()],
        Some(&tools),
    );
    assert_eq!(payload["ok"], false, "{payload}");
    assert!(
        payload["error"]
            .as_str()
            .unwrap_or_default()
            .contains("exited with"),
        "{payload}"
    );
}

#[test]
fn copying_files_hands_the_uri_list_to_the_clipboard_program() {
    let temporary = tempdir().unwrap();
    let tools = temporary.path().join("tools");
    fs::create_dir(&tools).unwrap();
    let capture = temporary.path().join("uris.txt");
    fs::write(
        tools.join("wl-copy"),
        format!(
            "#!/bin/sh\nprintf '%s\\n' \"$*\" > '{}.args'\ncat > '{}'\nsleep 2\n",
            capture.display(),
            capture.display()
        ),
    )
    .unwrap();
    fs::set_permissions(tools.join("wl-copy"), fs::Permissions::from_mode(0o755)).unwrap();
    let file = temporary.path().join("one.txt");
    fs::write(&file, "x").unwrap();
    let payload = backend_with_tools(
        temporary.path(),
        &["clipboard-write", "--path", file.to_str().unwrap()],
        Some(&tools),
    );
    assert_eq!(payload["ok"], true, "{payload}");
    let expected = format!("file://{}\r\n", file.display());
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        if fs::read_to_string(&capture).unwrap_or_default() == expected {
            break;
        }
        std::thread::sleep(std::time::Duration::from_millis(10));
    }
    assert_eq!(fs::read_to_string(&capture).unwrap(), expected);
    assert_eq!(
        fs::read_to_string(format!("{}.args", capture.display()))
            .unwrap()
            .trim(),
        "--type text/uri-list"
    );
}

#[test]
fn entry_names_refuse_only_whitespace_and_keep_every_legal_name() {
    use fileblade::operations::{checked_name, create_path, rename_path};

    for refused in [
        "", " ", "   ", "\t", "\n", "\u{00a0}", "\u{3000}", ".", "..", "a/b", "a\0b",
    ] {
        assert!(
            checked_name(refused).is_err(),
            "{refused:?} must be refused as an entry name"
        );
    }
    for allowed in [
        "  two  ",
        " leading",
        "trailing ",
        "a b",
        ".hidden",
        "...",
        " .",
        ". ",
        "a\nb",
        "naïve — file",
    ] {
        assert_eq!(
            checked_name(allowed).unwrap(),
            allowed,
            "{allowed:?} must be allowed and preserved byte for byte"
        );
    }

    let temporary = tempdir().unwrap();
    let root = temporary.path();
    assert_eq!(
        create_path(root.to_str().unwrap(), " ", false, "")["ok"],
        false
    );
    assert_eq!(
        create_path(root.to_str().unwrap(), " ", true, "")["ok"],
        false
    );
    assert_eq!(
        create_path(root.to_str().unwrap(), "  spaced  ", false, "")["ok"],
        true
    );
    assert!(root.join("  spaced  ").is_file());

    let existing = root.join(" ");
    fs::write(&existing, "made outside fileblade").unwrap();
    assert_eq!(
        rename_path(existing.to_str().unwrap(), "recovered.txt", "")["ok"],
        true,
        "an entry that is already named \" \" must still be renameable to something valid"
    );
    assert!(root.join("recovered.txt").is_file());
    assert!(!existing.exists());
}
