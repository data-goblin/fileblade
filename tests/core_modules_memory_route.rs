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
            "fileblade.core.memory",
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
fn the_in_process_route_lists_and_links_native_byte_memory() {
    let base = tempfile::Builder::new()
        .prefix("fileblade-memory-route-")
        .tempdir()
        .unwrap();
    let home = base.path().join("home");
    fs::create_dir_all(&home).unwrap();
    let project = base.path().join(OsStr::from_bytes(b"repo-\xff"));
    fs::create_dir_all(project.join(".git")).unwrap();
    let source = project.join("AGENTS.md");
    fs::write(&source, "# original\n").unwrap();
    let rules = project.join(".claude/rules");
    fs::create_dir_all(&rules).unwrap();
    let names: [&[u8]; 4] = [
        b"\xff.md",
        b"\xfe.md",
        "\u{fffd}.md".as_bytes(),
        b"\\xFF.md",
    ];
    for name in names {
        fs::write(rules.join(OsStr::from_bytes(name)), "# rule\n").unwrap();
    }
    let neighbour = base.path().join("repo-\u{fffd}");
    fs::create_dir_all(&neighbour).unwrap();
    fs::write(neighbour.join("AGENTS.md"), "neighbour").unwrap();

    let location = json!([
        "--json",
        "--exact",
        "--project",
        fileblade::common::path_text(&project),
        "--home",
        home.display().to_string(),
    ]);
    let listed = backend(&home, "list", false, location.clone());
    assert_eq!(listed.get("ok"), Some(&json!(true)));
    assert_eq!(
        listed.get("project").and_then(Value::as_str),
        Some(fileblade::common::path_text(&project).as_str())
    );
    assert!(listed.get("watchPaths").is_some());
    let rows = listed.get("items").and_then(Value::as_array).unwrap();
    let rule_rows: Vec<&Value> = rows
        .iter()
        .filter(|row| {
            fileblade::common::parse_path(row["path"].as_str().unwrap())
                .map(|path| path.parent() == Some(rules.as_path()))
                .unwrap_or(false)
        })
        .collect();
    assert_eq!(rule_rows.len(), 4);
    let identities: std::collections::BTreeSet<&str> = rule_rows
        .iter()
        .map(|row| row["id"].as_str().unwrap())
        .collect();
    assert_eq!(
        identities.len(),
        4,
        "every native name has its own identity"
    );
    let displayed: std::collections::BTreeSet<String> = rule_rows
        .iter()
        .map(|row| row["name"].as_str().unwrap().to_string())
        .collect();
    let expected: std::collections::BTreeSet<String> = names
        .iter()
        .map(|name| fileblade::common::display_path(Path::new(OsStr::from_bytes(name))))
        .collect();
    assert_eq!(displayed, expected);

    let row = rows
        .iter()
        .find(|row| fileblade::common::parse_path(row["path"].as_str().unwrap()).unwrap() == source)
        .unwrap();
    let identity = row["id"].as_str().unwrap();
    let mutate = |state: &str| {
        let mut arguments = location.as_array().unwrap().clone();
        arguments.extend([
            json!("--id"),
            json!(identity),
            json!("--agent"),
            json!("claude-code"),
            json!("--state"),
            json!(state),
        ]);
        backend(&home, "apply", true, Value::Array(arguments))
    };
    let applied = mutate("on");
    assert_eq!(applied.get("ok"), Some(&json!(true)), "{applied}");
    let link = project.join("CLAUDE.md");
    assert_eq!(fs::read_link(&link).unwrap(), source);
    assert_eq!(
        applied["results"][0]["touched"],
        json!([fileblade::common::path_text(&link)])
    );
    let removed = mutate("off");
    assert_eq!(removed.get("ok"), Some(&json!(true)), "{removed}");
    assert!(fs::symlink_metadata(&link).is_err());
    assert_eq!(fs::read_to_string(&source).unwrap(), "# original\n");
    assert_eq!(
        fs::read_to_string(neighbour.join("AGENTS.md")).unwrap(),
        "neighbour"
    );
}
