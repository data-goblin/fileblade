use serde_json::{Value, json};
use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
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
            "fileblade.core.hooks",
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

fn text(value: &Value) -> &str {
    value.as_str().unwrap_or_default()
}

#[test]
fn the_in_process_route_lists_removes_and_restores_native_byte_sources() {
    let base = tempfile::Builder::new()
        .prefix("fileblade-hooks-route-")
        .tempdir()
        .unwrap();
    let anchor = fs::canonicalize(base.path()).unwrap();
    let home = anchor.join("home");
    let state = anchor.join("state");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&state).unwrap();
    let project = anchor.join(OsStr::from_bytes(b"repo-\xff"));
    let hooks = project.join(".github/hooks");
    fs::create_dir_all(&hooks).unwrap();
    fs::create_dir_all(project.join(".git")).unwrap();
    let names: [&[u8]; 4] = [
        b"\xff.json",
        b"\xfe.json",
        "\u{fffd}.json".as_bytes(),
        b"\\xFF.json",
    ];
    let original = json!({
        "version": 1,
        "hooks": {"preToolUse": [{"hooks": [{"type": "command", "bash": "printf TEST_NOT_EXECUTED"}]}]},
    });
    for name in names {
        fs::write(
            hooks.join(OsStr::from_bytes(name)),
            serde_json::to_string(&original).unwrap(),
        )
        .unwrap();
    }
    let project_text = fileblade::common::path_text(&project);
    let home_text = home.to_string_lossy().into_owned();
    let list = json!([
        "--exact",
        "--project",
        project_text,
        "--home",
        home_text,
        "--json"
    ]);
    let document = backend(&home, &state, "list", false, list.clone(), None);
    assert_eq!(document["ok"], json!(true));
    assert_eq!(text(&document["project"]), project_text);
    let rows: Vec<&Value> = document["items"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|row| row["agent"] == json!("copilot-cli") && row["scope"] == json!("project"))
        .collect();
    assert_eq!(rows.len(), 4);
    let ids: std::collections::BTreeSet<&str> = rows.iter().map(|row| text(&row["id"])).collect();
    assert_eq!(
        ids.len(),
        4,
        "{:?}",
        rows.iter()
            .map(|row| (
                text(&row["id"]).to_string(),
                text(&row["source"]["path"]).to_string(),
                text(&row["source"]["name"]).to_string()
            ))
            .collect::<Vec<_>>()
    );
    let blob = serde_json::to_string(&document).unwrap();
    assert!(!blob.contains("TEST_NOT_EXECUTED"));
    assert!(document.get("watchPaths").is_none());

    let watched = backend(
        &home,
        &state,
        "list",
        false,
        json!([
            "--exact",
            "--project",
            project_text,
            "--home",
            home_text,
            "--watch",
            "--json"
        ]),
        None,
    );
    let paths: Vec<&str> = watched["watchPaths"]
        .as_array()
        .unwrap()
        .iter()
        .map(text)
        .collect();
    assert!(paths.contains(&project_text.as_str()));
    assert!(paths.len() <= 512);

    let source = hooks.join(OsStr::from_bytes(names[0]));
    let source_text = fileblade::common::path_text(&source);
    let before = fs::read(&source).unwrap();
    let row = rows
        .iter()
        .find(|row| text(&row["source"]["path"]) == source_text)
        .unwrap();
    let removal = json!([
        "--exact",
        "--project",
        project_text,
        "--home",
        home_text,
        "--id",
        text(&row["id"]),
        "--json"
    ]);
    let prepared = backend(&home, &state, "prepare-remove", true, removal, None);
    assert_eq!(prepared["ok"], json!(true), "{prepared}");
    assert_eq!(text(&prepared["payload"]["source"]), source_text);
    assert_eq!(fs::read(&source).unwrap(), before);

    let inventory = backend(
        &home,
        &state,
        "recovery-list",
        false,
        json!(["--json"]),
        None,
    );
    assert_eq!(inventory["ok"], json!(true));
    assert_eq!(inventory["records"].as_array().unwrap().len(), 1);

    let record_id = text(&prepared["recordId"]).to_string();
    let restored = backend(
        &home,
        &state,
        "restore",
        true,
        json!(["--record-id", record_id, "--json"]),
        None,
    );
    assert_eq!(restored["ok"], json!(true), "{restored}");
    assert_eq!(restored["results"][0]["changed"], json!(false));
    for name in names {
        let written: Value =
            serde_json::from_slice(&fs::read(hooks.join(OsStr::from_bytes(name))).unwrap())
                .unwrap();
        assert_eq!(written, original);
    }

    let discarded = backend(
        &home,
        &state,
        "discard",
        true,
        json!(["--record-id", record_id, "--json"]),
        None,
    );
    assert_eq!(discarded["ok"], json!(true), "{discarded}");
    let inventory = backend(
        &home,
        &state,
        "recovery-list",
        false,
        json!(["--json"]),
        None,
    );
    assert!(inventory["records"].as_array().unwrap().is_empty());

    let digest = text(&row["summary"]["digest"]).to_string();
    let labelled = backend(
        &home,
        &state,
        "label",
        true,
        json!([
            "--home", home_text, "--digest", digest, "--text", "Guard", "--json"
        ]),
        None,
    );
    assert_eq!(labelled["ok"], json!(true), "{labelled}");
    assert_eq!(labelled["changed"], json!(true));

    let relisted = backend(&home, &state, "list", false, list, None);
    assert!(
        relisted["items"]
            .as_array()
            .unwrap()
            .iter()
            .any(|item| item["summary"]["labelSource"] == json!("user"))
    );
}

#[test]
fn a_refused_removal_still_answers_with_a_document() {
    let base = tempfile::Builder::new()
        .prefix("fileblade-hooks-route-refusal-")
        .tempdir()
        .unwrap();
    let anchor = fs::canonicalize(base.path()).unwrap();
    let home = anchor.join("home");
    let state = anchor.join("state");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&state).unwrap();
    let output = invoke(
        &home,
        &state,
        "prepare-remove",
        true,
        json!([
            "--home",
            home.to_string_lossy(),
            "--id",
            "missing",
            "--json"
        ]),
        None,
    );
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(document["ok"], json!(false));
    assert!(text(&document["message"]).contains("no hook row"));
}
