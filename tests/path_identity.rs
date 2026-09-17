use fileblade::common::{display_path, parse_display_name, parse_path, path_text};
use serde_json::Value;
use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::process::Command;

#[test]
fn path_protocol_round_trips_every_filename_byte() {
    for byte in 1_u8..=255 {
        if byte == b'/' {
            continue;
        }
        let name = [b'f', byte, b'.', b'x'];
        let path = Path::new("/identity").join(OsStr::from_bytes(&name));
        assert_eq!(parse_path(&path_text(&path)).unwrap(), path, "byte {byte}");
        assert_eq!(
            parse_display_name(&display_path(Path::new(OsStr::from_bytes(&name))))
                .unwrap()
                .as_bytes(),
            name,
            "display byte {byte}"
        );
    }
    for uri in [
        "file://remote/tmp/x",
        "file:///tmp/%",
        "file:///tmp/%ZZ",
        "file:///tmp/%00",
        "file:///tmp/x#y",
        "file:///tmp/x?y",
        "file:relative",
        "file:///tmp/\nx",
    ] {
        assert!(parse_path(uri).is_err(), "{uri:?}");
    }
}

#[test]
fn escaped_display_does_not_impersonate_another_name() {
    let raw = Path::new(OsStr::from_bytes(b"\xff.txt"));
    assert_eq!(display_path(raw), "\\xFF.txt");
    assert_ne!(display_path(raw), display_path(Path::new("�.txt")));
    assert_ne!(display_path(raw), display_path(Path::new("\\xFF.txt")));
}

#[test]
fn native_arguments_and_replaced_environments_keep_exact_bytes() {
    let raw = OsStr::from_bytes(b"/fixture/\xff.txt");
    let output = fileblade::command::CommandSpec::new("/bin/sh")
        .args([
            "-c",
            "printf '%s\\0%s\\0' \"$1\" \"$FILEBLADE_PATH\"",
            "path-test",
        ])
        .args([raw])
        .env_clear()
        .env("FILEBLADE_PATH", raw)
        .run()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(
        output.stdout,
        [raw.as_bytes(), b"\0", raw.as_bytes(), b"\0"].concat()
    );
}

#[test]
fn git_status_and_tree_search_keep_native_ancestors() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join(OsStr::from_bytes(b"repo-\xfe"));
    let nested = root.join(OsStr::from_bytes(b"dir-\xff"));
    fs::create_dir_all(&nested).unwrap();
    let raw = nested.join(OsStr::from_bytes(b"\xff.txt"));
    let unicode = nested.join("�.txt");
    fs::write(&raw, "raw bytes").unwrap();
    fs::write(&unicode, "unicode spelling").unwrap();
    let initialized = Command::new("git")
        .args(["init", "-q"])
        .arg(&root)
        .status()
        .unwrap();
    assert!(initialized.success());
    let root_text = path_text(&root);
    let result = backend(
        temp.path(),
        &["search", "--root", &root_text, "--query", ".txt", "--tree"],
    );
    assert_eq!(result["ok"], true, "{result}");
    let rows = result["entries"].as_array().unwrap();
    for path in [&nested, &raw, &unicode] {
        let row = rows
            .iter()
            .find(|entry| parse_path(entry["path"].as_str().unwrap()).unwrap() == *path)
            .unwrap_or_else(|| panic!("missing {}: {result}", path_text(path)));
        if path != &nested {
            assert_eq!(row["git_status"], "?", "{row}");
            assert_eq!(row["git_repo_root"], root_text, "{row}");
        } else {
            assert_eq!(row["ancestor"], true, "{row}");
        }
    }
    let repository = fileblade::git::git_repository(&root_text).unwrap();
    assert_eq!(repository.root, root);
    assert_eq!(repository.git_dir, root.join(".git"));
}

#[test]
fn escaped_rename_rejects_separators_and_can_change_a_byte_name() {
    let temp = tempfile::tempdir().unwrap();
    let raw = temp.path().join(OsStr::from_bytes(b"\xff.txt"));
    fs::write(&raw, "raw bytes").unwrap();
    for name in [
        "\\x00",
        "\\x2fescape",
        "..",
        "\\x2e\\x2e",
        "\\q",
        "\\u{110000}",
    ] {
        let refused = backend(
            temp.path(),
            &[
                "rename",
                "--path",
                &path_text(&raw),
                "--name",
                name,
                "--name-escaped",
            ],
        );
        assert_eq!(refused["ok"], false, "{refused}");
        assert_eq!(fs::read(&raw).unwrap(), b"raw bytes");
    }
    let renamed = backend(
        temp.path(),
        &[
            "rename",
            "--path",
            &path_text(&raw),
            "--name",
            "\\xFE.txt",
            "--name-escaped",
        ],
    );
    assert_eq!(renamed["ok"], true, "{renamed}");
    assert_eq!(
        fs::read(temp.path().join(OsStr::from_bytes(b"\xfe.txt"))).unwrap(),
        b"raw bytes"
    );
    assert!(!raw.exists());
}

#[test]
fn listing_and_rename_keep_the_two_spellings_distinct() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("files");
    fs::create_dir(&root).unwrap();
    let raw = root.join(OsStr::from_bytes(b"\xff.txt"));
    let unicode = root.join("�.txt");
    fs::write(&raw, "raw bytes").unwrap();
    fs::write(&unicode, "unicode spelling").unwrap();
    let rows = backend(
        temp.path(),
        &[
            "children-batch",
            "--path",
            root.to_str().unwrap(),
            "--no-git",
        ],
    );
    assert_eq!(rows["ok"], true, "{rows}");
    let entries = rows["results"][0]["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 2);
    assert_ne!(entries[0]["path"], entries[1]["path"], "{rows}");
    let selected = entries
        .iter()
        .find(|entry| parse_path(entry["path"].as_str().unwrap()).unwrap() == raw)
        .unwrap();
    let unchanged = backend(
        temp.path(),
        &[
            "rename",
            "--path",
            selected["path"].as_str().unwrap(),
            "--name",
            "\\xFF.txt",
            "--name-escaped",
        ],
    );
    assert_eq!(unchanged["ok"], true, "{unchanged}");
    assert!(raw.exists());
    let renamed = backend(
        temp.path(),
        &[
            "rename",
            "--path",
            selected["path"].as_str().unwrap(),
            "--name",
            "renamed.txt",
        ],
    );
    assert_eq!(renamed["ok"], true, "{renamed}");
    assert_eq!(
        fs::read_to_string(root.join("renamed.txt")).unwrap(),
        "raw bytes"
    );
    assert_eq!(fs::read_to_string(&unicode).unwrap(), "unicode spelling");
    assert!(!raw.exists());
    let undone = backend(temp.path(), &["undo"]);
    assert_eq!(undone["ok"], true, "{undone}");
    assert_eq!(fs::read_to_string(&raw).unwrap(), "raw bytes");
    assert_eq!(fs::read_to_string(&unicode).unwrap(), "unicode spelling");
}

#[test]
fn copy_and_move_preserve_byte_names_and_undo_destinations() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let raw = root.join(OsStr::from_bytes(b"\xff.txt"));
    let unicode = root.join("�.txt");
    let destination = root.join(OsStr::from_bytes(b"folder-\xfe"));
    fs::create_dir(&destination).unwrap();
    fs::write(&raw, "raw bytes").unwrap();
    fs::write(&unicode, "unicode spelling").unwrap();
    for name in [b"\xff.txt".as_slice(), b"\xff copy.txt".as_slice()] {
        let copied = backend(
            root,
            &[
                "copy",
                "--source",
                &path_text(&raw),
                "--destination",
                &path_text(&destination),
            ],
        );
        assert_eq!(copied["ok"], true, "{copied}");
        assert_eq!(
            fs::read(destination.join(OsStr::from_bytes(name))).unwrap(),
            b"raw bytes"
        );
    }
    let moved_name = root.join("movable.txt");
    fs::write(&moved_name, "move me").unwrap();
    let moved = backend(
        root,
        &[
            "move",
            "--source",
            &path_text(&moved_name),
            "--destination",
            &path_text(&destination),
        ],
    );
    assert_eq!(moved["ok"], true, "{moved}");
    assert!(!moved_name.exists());
    let undone = backend(root, &["undo"]);
    assert_eq!(undone["ok"], true, "{undone}");
    assert_eq!(fs::read(moved_name).unwrap(), b"move me");
    assert_eq!(fs::read(raw).unwrap(), b"raw bytes");
    assert_eq!(fs::read(unicode).unwrap(), b"unicode spelling");
}

#[test]
fn search_paging_preview_and_recent_do_not_reconstruct_paths_from_labels() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join(OsStr::from_bytes(b"folder-\xfe"));
    fs::create_dir(&root).unwrap();
    let raw = root.join(OsStr::from_bytes(b"\xff.txt"));
    let unicode = root.join("�.txt");
    fs::write(&raw, "raw needle").unwrap();
    fs::write(&unicode, "unicode needle").unwrap();
    let root_text = path_text(&root);
    for arguments in [
        vec![
            "children-window",
            "--path",
            &root_text,
            "--count",
            "10",
            "--no-git",
        ],
        vec![
            "search", "--root", &root_text, "--query", ".txt", "--no-git",
        ],
        vec![
            "search",
            "--root",
            &root_text,
            "--query",
            "content:needle",
            "--no-git",
        ],
    ] {
        let result = backend(temp.path(), &arguments);
        assert_eq!(result["ok"], true, "{result}");
        let paths = result["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|entry| parse_path(entry["path"].as_str().unwrap()).unwrap())
            .collect::<std::collections::HashSet<_>>();
        assert!(paths.contains(&raw), "{result}");
        assert!(paths.contains(&unicode), "{result}");
        assert_eq!(paths.len(), 2, "{result}");
    }
    let read = backend(temp.path(), &["read-text", "--path", &path_text(&raw)]);
    assert_eq!(read["text"], "raw needle", "{read}");
    let visit = backend(
        temp.path(),
        &[
            "frecency-visit",
            "--path",
            &path_text(&raw),
            "--path",
            &path_text(&unicode),
        ],
    );
    assert_eq!(visit["ok"], true, "{visit}");
    let recent = backend(temp.path(), &["frecency-list", "--show-hidden"]);
    let entries = recent["entries"].as_array().unwrap();
    assert!(
        entries.iter().any(|entry| entry["path"] == path_text(&raw)),
        "{recent}"
    );
    assert!(
        entries
            .iter()
            .any(|entry| entry["path"] == path_text(&unicode)),
        "{recent}"
    );
    let uri = url::Url::from_file_path(&raw).unwrap().to_string();
    assert_eq!(
        fileblade::desktop::local_path_from_file_uri(&uri).unwrap(),
        raw
    );
    let launch = fileblade::hyprland::launch_command(&raw, "default", "", 0).unwrap();
    assert_eq!(parse_path(launch.last().unwrap()).unwrap(), raw);
}

#[test]
fn trash_undo_returns_only_the_selected_spelling() {
    let temp = tempfile::Builder::new()
        .prefix("path-identity-")
        .tempdir_in(Path::new(env!("CARGO_MANIFEST_DIR")).join("target"))
        .unwrap();
    let raw = temp.path().join(OsStr::from_bytes(b"\xff.txt"));
    let unicode = temp.path().join("�.txt");
    fs::write(&raw, "raw bytes").unwrap();
    fs::write(&unicode, "unicode spelling").unwrap();
    let trashed = backend(temp.path(), &["trash", "--path", &path_text(&raw)]);
    assert_eq!(trashed["ok"], true, "{trashed}");
    assert!(!raw.exists());
    assert_eq!(fs::read(&unicode).unwrap(), b"unicode spelling");
    let undone = backend(temp.path(), &["undo"]);
    assert_eq!(undone["ok"], true, "{undone}");
    assert_eq!(fs::read(raw).unwrap(), b"raw bytes");
    assert_eq!(fs::read(unicode).unwrap(), b"unicode spelling");
}

#[test]
fn artifact_bin_round_trips_a_tree_with_both_spellings_and_a_byte_symlink() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join(OsStr::from_bytes(b"tree-\xfe"));
    fs::create_dir(&root).unwrap();
    let raw = root.join(OsStr::from_bytes(b"\xff.txt"));
    fs::write(&raw, "raw bytes").unwrap();
    let long_name = [b'\xff'; 255];
    fs::write(root.join(OsStr::from_bytes(&long_name)), "long name").unwrap();
    fs::write(root.join("�.txt"), "unicode spelling").unwrap();
    symlink(OsStr::from_bytes(b"\xff.txt"), root.join("link")).unwrap();
    let item = serde_json::json!({"id": "byte-tree", "paths": [path_text(&root)]});
    let binned = backend(
        temp.path(),
        &[
            "bin-put",
            "--module",
            "identity",
            "--item",
            &item.to_string(),
        ],
    );
    assert_eq!(binned["ok"], true, "{binned}");
    assert!(!root.exists());
    let restored = backend(
        temp.path(),
        &[
            "bin-restore",
            "--module",
            "identity",
            "--id",
            binned["entry"].as_str().unwrap(),
        ],
    );
    assert_eq!(restored["ok"], true, "{restored}");
    assert_eq!(fs::read(raw).unwrap(), b"raw bytes");
    assert_eq!(
        fs::read(root.join(OsStr::from_bytes(&long_name))).unwrap(),
        b"long name"
    );
    assert_eq!(fs::read(root.join("�.txt")).unwrap(), b"unicode spelling");
    assert_eq!(
        fs::read_link(root.join("link"))
            .unwrap()
            .as_os_str()
            .as_bytes(),
        b"\xff.txt"
    );
}

fn backend(root: &Path, arguments: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_fileblade"))
        .arg("_backend")
        .args(arguments)
        .env("XDG_STATE_HOME", root.join("state"))
        .env("XDG_DATA_HOME", root.join("data"))
        .env("FILEBLADE_JOURNAL", root.join("state/journal.json"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(
        output
            .stdout
            .split(|byte| *byte == b'\n')
            .rfind(|line| !line.is_empty())
            .unwrap(),
    )
    .unwrap()
}
