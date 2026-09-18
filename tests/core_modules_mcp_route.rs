use serde_json::{Value, json};
use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn invoke(
    home: &Path,
    state: &Path,
    method: &str,
    write: bool,
    arguments: Value,
    input: Option<&str>,
) -> std::process::Output {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR"));
    let mut command = Command::new(env!("CARGO_BIN_EXE_fileblade"));
    command
        .args([
            "_backend",
            if write { "helper-write" } else { "helper-read" },
            "--provider",
            "fileblade.core.mcp",
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
        .env("XDG_STATE_HOME", state)
        .env("XDG_CACHE_HOME", home.join(".cache"))
        .env("XDG_DATA_HOME", home.join(".local/share"))
        .env("CODEX_HOME", home.join(".codex"))
        .current_dir(repository)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    let mut child = command.spawn().unwrap();
    if let Some(input) = input {
        use std::io::Write;
        child
            .stdin
            .as_mut()
            .unwrap()
            .write_all(input.as_bytes())
            .unwrap();
    }
    drop(child.stdin.take());
    child.wait_with_output().unwrap()
}

fn backend(
    home: &Path,
    state: &Path,
    method: &str,
    write: bool,
    arguments: Value,
    input: Option<&str>,
) -> Value {
    let output = invoke(home, state, method, write, arguments, input);
    assert!(
        !output.stdout.is_empty(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn prepared_home() -> (tempfile::TempDir, PathBuf, PathBuf, PathBuf) {
    let base = tempfile::Builder::new()
        .prefix("fileblade-mcp-route-")
        .tempdir()
        .unwrap();
    let anchor = fs::canonicalize(base.path()).unwrap();
    let home = anchor.join("home");
    let state = anchor.join("state");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&state).unwrap();
    let project = anchor.join(OsStr::from_bytes(b"repo-\xff"));
    fs::create_dir_all(&project).unwrap();
    (base, home, state, project)
}

#[test]
fn the_in_process_route_lists_removes_and_restores_native_byte_sources() {
    let (_base, home, state, project) = prepared_home();
    let source = project.join(".mcp.json");
    let original = json!({
        "mcpServers": {
            "native-test": {
                "command": "printf",
                "args": ["TEST_NOT_EXECUTED"],
                "env": {"TOKEN": "SECRET_NOT_EMITTED"},
            }
        }
    });
    fs::write(&source, original.to_string()).unwrap();
    let neighbor = _base.path().join("repo-\u{fffd}");
    fs::create_dir_all(&neighbor).unwrap();
    fs::write(neighbor.join(".mcp.json"), "neighbor").unwrap();

    let project_text = fileblade::common::path_text(&project);
    let listing = backend(
        &home,
        &state,
        "list",
        false,
        json!(["--project", project_text, "--json", "--watch"]),
        None,
    );
    assert_eq!(listing.get("ok"), Some(&json!(true)), "{listing:?}");
    assert_eq!(
        listing.get("healthBasis"),
        Some(&json!("configuration-only"))
    );
    let encoded = listing.to_string();
    assert!(!encoded.contains("SECRET_NOT_EMITTED"));
    assert!(!encoded.contains("TEST_NOT_EXECUTED"));
    assert!(!encoded.contains("\\udcff"));
    let watched = listing
        .get("watchPaths")
        .and_then(Value::as_array)
        .cloned()
        .unwrap();
    assert!(watched.contains(&json!(project_text)));
    assert!(watched.len() <= 512);

    let row = listing
        .get("definitions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap()
        .into_iter()
        .find(|row| row.get("name") == Some(&json!("native-test")))
        .unwrap();
    assert_eq!(
        row.get("source").and_then(|source| source.get("path")),
        Some(&json!("<project>/.mcp.json"))
    );
    let id = row.get("id").and_then(Value::as_str).unwrap().to_string();

    let plain = backend(
        &home,
        &state,
        "list",
        false,
        json!(["--project", project_text, "--json"]),
        None,
    );
    assert!(plain.get("watchPaths").is_none());

    let prepared = backend(
        &home,
        &state,
        "prepare-remove",
        true,
        json!(["--project", project_text, "--id", id, "--json"]),
        None,
    );
    assert_eq!(prepared.get("ok"), Some(&json!(true)), "{prepared:?}");
    let payload = prepared.get("payload").cloned().unwrap();
    assert_eq!(
        payload.get("path"),
        Some(&json!(fileblade::common::path_text(&source)))
    );
    let record_id = prepared
        .get("recordId")
        .and_then(Value::as_str)
        .unwrap()
        .to_string();

    let before = fs::read(&source).unwrap();
    let refused = backend(
        &home,
        &state,
        "remove-prepared",
        true,
        json!([
            "--project",
            project_text,
            "--id",
            id,
            "--json",
            "--payload-stdin"
        ]),
        Some(&format!("{payload}\n")),
    );
    assert_eq!(refused.get("ok"), Some(&json!(false)), "{refused:?}");
    assert_eq!(
        refused.get("message"),
        Some(&json!("prepared recovery payload is missing or invalid"))
    );
    assert_eq!(fs::read(&source).unwrap(), before);
    assert!(!record_id.is_empty());
    let after: Value = serde_json::from_slice(&fs::read(&source).unwrap()).unwrap();
    assert_eq!(after, original);
    assert_eq!(
        fs::read_to_string(neighbor.join(".mcp.json")).unwrap(),
        "neighbor"
    );
}

#[test]
fn the_recovery_listing_and_the_usage_methods_answer_on_the_same_route() {
    let (_base, home, state, project) = prepared_home();
    let project_text = fileblade::common::path_text(&project);
    let recovery = backend(
        &home,
        &state,
        "recovery-list",
        false,
        json!(["--json"]),
        None,
    );
    assert_eq!(recovery.get("ok"), Some(&json!(true)), "{recovery:?}");
    let usage = backend(&home, &state, "usage", false, json!(["--json"]), None);
    assert!(usage.get("ok").is_some());
    let apply = backend(
        &home,
        &state,
        "apply",
        true,
        json!([
            "--project",
            project_text,
            "--id",
            "0".repeat(24),
            "--agent",
            "codex",
            "--state",
            "on",
            "--json"
        ]),
        None,
    );
    assert_eq!(apply.get("ok"), Some(&json!(false)));
    assert_eq!(apply.get("message"), Some(&json!("unknown definition id")));
}

#[test]
fn the_mcp_module_depends_on_no_process_or_network_facility() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("src/core_modules/mcp");
    let banned = [
        "std::process",
        "std::net",
        "std::os::unix::net",
        "Command::new",
        "CommandSpec",
        "TcpStream",
        "UdpSocket",
        "UnixStream",
        "reqwest",
        "ureq",
        "curl",
        "hyper",
        "tokio",
        "zbus",
        "libc::fork",
        "libc::execvp",
        "rustix::process::execve",
    ];
    let mut seen = 0usize;
    for entry in fs::read_dir(&root).unwrap() {
        let path = entry.unwrap().path();
        if path.extension().and_then(OsStr::to_str) != Some("rs") {
            continue;
        }
        seen += 1;
        let text = fs::read_to_string(&path).unwrap();
        for needle in banned {
            assert!(
                !text.contains(needle),
                "{} must not reach for {needle}",
                path.display()
            );
        }
        for line in text.lines() {
            let trimmed = line.trim_start();
            assert!(
                !trimmed.starts_with("//") && !trimmed.starts_with("/*"),
                "{} carries a comment: {trimmed}",
                path.display()
            );
        }
    }
    assert!(seen >= 8, "the module should hold every ported file");

    let manifest = fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml"))
        .unwrap()
        .parse::<toml::Table>()
        .unwrap();
    let declared: Vec<String> = manifest["dependencies"]
        .as_table()
        .unwrap()
        .keys()
        .cloned()
        .collect();
    for crate_name in [
        "chrono",
        "libc",
        "regex",
        "rustix",
        "serde",
        "serde_json",
        "sha2",
        "toml",
        "unicode-general-category",
    ] {
        assert!(
            declared.contains(&crate_name.to_string()),
            "{crate_name} must stay a declared dependency"
        );
    }
    for absent in ["rusqlite", "turso", "reqwest", "ureq", "cc", "toml_edit"] {
        assert!(
            !declared.contains(&absent.to_string()),
            "{absent} must not be a dependency"
        );
    }
}
