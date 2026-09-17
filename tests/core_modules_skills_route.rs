#[path = "common/skill_fixtures.rs"]
mod skill_fixtures;

use serde_json::{Value, json};
use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::process::Command;

fn invoke(home: &Path, method: &str, write: bool, arguments: Value) -> std::process::Output {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    Command::new(env!("CARGO_BIN_EXE_fileblade"))
        .args([
            "_backend",
            if write { "helper-write" } else { "helper-read" },
            "--provider",
            "fileblade.core.skills",
            "--plugin-dir",
            "",
            "--helper",
            "inventory",
            "--method",
            method,
            "--arguments",
            &arguments.to_string(),
        ])
        .env("HOME", home)
        .env("FILEBLADE_APP_ROOT", repository)
        .env("XDG_CONFIG_HOME", home.join(".config"))
        .env("XDG_STATE_HOME", home.join(".local/state"))
        .env("XDG_CACHE_HOME", home.join(".cache"))
        .env("XDG_DATA_HOME", home.join(".local/share"))
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .current_dir(repository)
        .output()
        .unwrap()
}

fn backend(home: &Path, method: &str, write: bool, arguments: Value) -> Value {
    let output = invoke(home, method, write, arguments);
    assert!(
        !output.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

#[test]
fn the_in_process_route_lists_and_applies_native_byte_skills() {
    let base = tempfile::Builder::new()
        .prefix("fileblade-skills-route-")
        .tempdir()
        .unwrap();
    let home = base.path().join("home");
    let project = home.join(OsStr::from_bytes(b"repo-\xff"));
    let skills = project.join(".claude/skills");
    fs::create_dir_all(&skills).unwrap();
    let names: [&[u8]; 3] = [b"\xff", b"plain", b"\\xFF"];
    for name in names {
        let directory = skills.join(OsStr::from_bytes(name));
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("SKILL.md"), "# original\n").unwrap();
    }
    let listed = backend(
        &home,
        "list",
        false,
        json!([
            "--json",
            "--exact",
            "--project",
            fileblade::common::path_text(&project),
            "--home",
            home.display().to_string(),
        ]),
    );
    assert_eq!(listed.get("ok"), Some(&json!(true)));
    let rows = listed.get("items").and_then(Value::as_array).unwrap();
    assert_eq!(rows.len(), 3);
    assert!(listed.get("watchPaths").is_some());
    assert!(listed.get("usageWatchPaths").is_some());
    let source = skills.join(OsStr::from_bytes(names[0]));
    let row = rows
        .iter()
        .find(|row| {
            fileblade::common::parse_path(row["path"].as_str().unwrap()).unwrap()
                == source.join("SKILL.md")
        })
        .unwrap();
    let identity = row["id"].as_str().unwrap();
    let mutate = |state: &str| {
        backend(
            &home,
            "apply",
            true,
            json!([
                "--json",
                "--exact",
                "--project",
                fileblade::common::path_text(&project),
                "--home",
                home.display().to_string(),
                "--id",
                identity,
                "--agent",
                "codex",
                "--state",
                state,
            ]),
        )
    };
    let applied = mutate("on");
    assert_eq!(applied.get("ok"), Some(&json!(true)), "{applied}");
    let link = project
        .join(".agents/skills")
        .join(OsStr::from_bytes(names[0]));
    assert_eq!(fs::read_link(&link).unwrap(), source);
    assert_eq!(
        applied["results"][0]["touched"],
        json!([fileblade::common::path_text(&link)])
    );
    assert_eq!(mutate("off").get("ok"), Some(&json!(true)));
    assert!(fs::symlink_metadata(&link).is_err());
    for name in names {
        assert_eq!(
            fs::read_to_string(skills.join(OsStr::from_bytes(name)).join("SKILL.md")).unwrap(),
            "# original\n"
        );
    }
}

#[test]
fn the_route_refuses_unknown_arguments() {
    let base = tempfile::Builder::new()
        .prefix("fileblade-skills-route-args-")
        .tempdir()
        .unwrap();
    let output = invoke(
        base.path(),
        "list",
        false,
        json!(["--json", "--unknown-flag"]),
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("unrecognised skills helper arguments"),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}
