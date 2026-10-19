use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use std::process::Command;
use tempfile::tempdir;

fn backend(root: &Path, arguments: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_fileblade"))
        .arg("_backend")
        .args(arguments)
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .env("HOME", root.join("home"))
        .env("XDG_CONFIG_HOME", root.join("config"))
        .env("XDG_STATE_HOME", root.join("state"))
        .env("LC_ALL", "C")
        .output()
        .unwrap();
    let text = String::from_utf8(output.stdout).unwrap();
    let last = text
        .lines()
        .rev()
        .find(|line| !line.trim().is_empty())
        .unwrap_or_default();
    serde_json::from_str(last).unwrap_or_else(|error| panic!("{error}: {text}"))
}

fn inflight_dir(root: &Path) -> std::path::PathBuf {
    let mut pending = vec![root.join("state")];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().is_some_and(|name| name == "inflight") {
                    return path;
                }
                pending.push(path);
            }
        }
    }
    panic!("no inflight directory under {}", root.display());
}

fn write_private(path: &Path, text: String) {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
        .unwrap();
    file.write_all(text.as_bytes()).unwrap();
}

fn intent_count(root: &Path) -> usize {
    fs::read_dir(inflight_dir(root))
        .map(|entries| entries.count())
        .unwrap_or(0)
}

#[test]
fn a_completed_move_leaves_no_intent_behind() {
    let temporary = tempdir().unwrap();
    let root = temporary.path();
    let source = root.join("from");
    let destination = root.join("to");
    fs::create_dir_all(&source).unwrap();
    fs::create_dir_all(&destination).unwrap();
    fs::write(source.join("doc.txt"), "body").unwrap();

    let moved = backend(
        root,
        &[
            "move",
            "--source",
            source.join("doc.txt").to_str().unwrap(),
            "--destination",
            destination.to_str().unwrap(),
        ],
    );
    assert_eq!(moved["ok"], true, "{moved}");
    assert!(destination.join("doc.txt").exists());
    assert_eq!(
        intent_count(root),
        0,
        "a finished move must remove its intent"
    );
}

#[test]
fn a_staged_item_from_a_killed_backend_is_restored_on_the_next_start() {
    let temporary = tempdir().unwrap();
    let root = temporary.path();
    let folder = root.join("docs");
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join("warm-up.txt"), "x").unwrap();
    let first = backend(
        root,
        &[
            "move",
            "--source",
            folder.join("warm-up.txt").to_str().unwrap(),
            "--destination",
            root.to_str().unwrap(),
        ],
    );
    assert_eq!(first["ok"], true, "{first}");

    let stage = folder.join(".fileblade-stage-deadbeef");
    fs::create_dir(&stage).unwrap();
    fs::write(stage.join("item-cafe"), "thesis").unwrap();
    write_private(
        &inflight_dir(root).join("deadbeef.json"),
        json!({
            "kind": "stage",
            "stage": stage,
            "item": "item-cafe",
            "original": folder.join("thesis.txt"),
            "pid": 1
        })
        .to_string(),
    );
    let partial = root.join(".fileblade-partial-0badf00d");
    fs::create_dir(&partial).unwrap();
    fs::write(partial.join("item"), "half").unwrap();
    write_private(
        &inflight_dir(root).join("0badf00d.json"),
        json!({"kind": "partial", "partial": partial, "pid": 1}).to_string(),
    );

    let recovered = backend(root, &["recover"]);
    assert_eq!(recovered["ok"], true, "{recovered}");
    assert_eq!(
        recovered["restored"],
        json!([folder.join("thesis.txt").to_string_lossy()]),
        "{recovered}"
    );
    assert_eq!(
        fs::read_to_string(folder.join("thesis.txt")).unwrap(),
        "thesis"
    );
    assert!(!stage.exists(), "the empty stage directory is removed");
    assert!(!partial.exists(), "the partial copy is removed");
    assert_eq!(intent_count(root), 0);

    let again = backend(root, &["recover"]);
    assert_eq!(again["restored"], json!([]));
}

#[test]
fn a_staged_item_whose_original_name_is_taken_is_reported_not_overwritten() {
    let temporary = tempdir().unwrap();
    let root = temporary.path();
    let folder = root.join("docs");
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join("seed.txt"), "x").unwrap();
    let first = backend(
        root,
        &[
            "move",
            "--source",
            folder.join("seed.txt").to_str().unwrap(),
            "--destination",
            root.to_str().unwrap(),
        ],
    );
    assert_eq!(first["ok"], true, "{first}");

    let stage = folder.join(".fileblade-stage-1234");
    fs::create_dir(&stage).unwrap();
    fs::write(stage.join("item-1"), "old").unwrap();
    fs::write(folder.join("report.txt"), "new").unwrap();
    write_private(
        &inflight_dir(root).join("1234.json"),
        json!({"kind": "stage", "stage": stage, "item": "item-1", "original": folder.join("report.txt"), "pid": 1}).to_string(),
    );

    let recovered = backend(root, &["recover"]);
    assert_eq!(recovered["restored"], json!([]), "{recovered}");
    assert_eq!(
        recovered["conflicts"].as_array().map(Vec::len),
        Some(1),
        "{recovered}"
    );
    assert_eq!(
        fs::read_to_string(folder.join("report.txt")).unwrap(),
        "new"
    );
    assert!(stage.join("item-1").exists(), "the staged item stays put");
    assert_eq!(
        intent_count(root),
        1,
        "a conflict keeps its intent for the next sweep"
    );
}

#[test]
fn an_intent_on_an_absent_volume_waits_and_is_restored_when_the_volume_returns() {
    let temporary = tempdir().unwrap();
    let root = temporary.path();
    fs::create_dir_all(root.join("docs")).unwrap();
    fs::write(root.join("docs/warm-up.txt"), "x").unwrap();
    let first = backend(
        root,
        &[
            "move",
            "--source",
            root.join("docs/warm-up.txt").to_str().unwrap(),
            "--destination",
            root.to_str().unwrap(),
        ],
    );
    assert_eq!(first["ok"], true, "{first}");

    let mounted = root.join("media/vol");
    let trash = mounted.join(".Trash-1000/files");
    let stage = trash.join(".fileblade-stage-cafe");
    write_private(
        &inflight_dir(root).join("cafe.json"),
        json!({
            "kind": "stage",
            "stage": stage,
            "item": "item-cafe",
            "original": trash.join("thesis.txt"),
            "pid": 1
        })
        .to_string(),
    );

    for _ in 0..2 {
        let waiting = backend(root, &["recover"]);
        assert_eq!(waiting["ok"], true, "{waiting}");
        assert_eq!(
            waiting["deferred"],
            json!([{
                "path": inflight_dir(root).join("cafe.json").to_string_lossy(),
                "folder": trash.to_string_lossy()
            }]),
            "{waiting}"
        );
        assert_eq!(
            intent_count(root),
            1,
            "the intent survives while the volume is away"
        );
    }

    fs::create_dir_all(&stage).unwrap();
    fs::write(stage.join("item-cafe"), "thesis").unwrap();
    let returned = backend(root, &["recover"]);
    assert_eq!(returned["ok"], true, "{returned}");
    assert_eq!(returned["deferred"], json!([]), "{returned}");
    assert_eq!(
        returned["restored"],
        json!([trash.join("thesis.txt").to_string_lossy()]),
        "{returned}"
    );
    assert_eq!(
        fs::read_to_string(trash.join("thesis.txt")).unwrap(),
        "thesis"
    );
    assert_eq!(intent_count(root), 0);
}

#[test]
fn an_intent_whose_stage_folder_is_gone_is_cleared() {
    let temporary = tempdir().unwrap();
    let root = temporary.path();
    let folder = root.join("docs");
    fs::create_dir_all(&folder).unwrap();
    fs::write(folder.join("warm-up.txt"), "x").unwrap();
    let first = backend(
        root,
        &[
            "move",
            "--source",
            folder.join("warm-up.txt").to_str().unwrap(),
            "--destination",
            root.to_str().unwrap(),
        ],
    );
    assert_eq!(first["ok"], true, "{first}");

    write_private(
        &inflight_dir(root).join("beef.json"),
        json!({
            "kind": "stage",
            "stage": folder.join(".fileblade-stage-beef"),
            "item": "item-beef",
            "original": folder.join("gone.txt"),
            "pid": 1
        })
        .to_string(),
    );
    let recovered = backend(root, &["recover"]);
    assert_eq!(recovered["ok"], true, "{recovered}");
    assert_eq!(recovered["deferred"], json!([]), "{recovered}");
    assert_eq!(intent_count(root), 0);
}
