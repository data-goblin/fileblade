use serde_json::Value;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

fn fileblade() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fileblade"))
}

fn stub(directory: &Path, name: &str, body: &str) {
    let path = directory.join(name);
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).unwrap();
}

fn snapshot(directory: &Path, companion: &str) -> Value {
    let output = fileblade()
        .args(["--output", "json", "host-status", "--companion", companion])
        .env("PATH", directory)
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn rows(host: &str, companion: &str) -> String {
    format!(
        "[{{\"id\":\"data-goblin.fileblade\",\"name\":\"FileBlade\",\"enabled\":{host}}},{{\"id\":\"{companion}\",\"name\":\"FileBlade Weather\",\"enabled\":true}}]"
    )
}

#[test]
fn the_state_follows_the_host_plugin_and_the_shell() {
    let temporary = tempfile::tempdir().unwrap();
    let bin = temporary.path();
    let companion = "acme.fileblade-weather";

    stub(bin, "omarchy", "printf '%s' '[]'");
    let missing = snapshot(bin, companion);
    assert_eq!(missing["state"], "missing");
    assert_eq!(missing["schemaVersion"], 1);
    assert_eq!(missing["plugins"].as_array().unwrap().len(), 0);

    stub(
        bin,
        "omarchy",
        &format!("printf '%s' '{}'", rows("false", companion)),
    );
    let disabled = snapshot(bin, companion);
    assert_eq!(disabled["state"], "disabled");
    assert_eq!(disabled["plugins"][0]["id"], companion);
    assert_eq!(disabled["plugins"][0]["name"], "FileBlade Weather");
    assert_eq!(disabled["plugins"][0]["enabled"], true);

    stub(
        bin,
        "omarchy",
        &format!("printf '%s' '{}'", rows("true", companion)),
    );
    stub(bin, "omarchy-shell", "exit 1");
    assert_eq!(snapshot(bin, companion)["state"], "starting");

    stub(bin, "omarchy-shell", "printf '%s' '{\"open\":true}'");
    assert_eq!(snapshot(bin, companion)["state"], "starting");

    stub(
        bin,
        "omarchy-shell",
        "printf '%s' '{\"open\":false,\"rootPath\":\"/home\"}'",
    );
    assert_eq!(snapshot(bin, companion)["state"], "ready");
}

#[test]
fn a_malformed_or_absent_listing_answers_unknown() {
    let temporary = tempfile::tempdir().unwrap();
    let bin = temporary.path();
    let companion = "acme.fileblade-weather";

    assert_eq!(snapshot(bin, companion)["state"], "unknown");

    stub(bin, "omarchy", "printf '%s' 'not json'");
    assert_eq!(snapshot(bin, companion)["state"], "unknown");

    stub(
        bin,
        "omarchy",
        "printf '%s' '[{\"id\":\"../escape\",\"enabled\":true}]'",
    );
    assert_eq!(snapshot(bin, companion)["state"], "unknown");

    stub(
        bin,
        "omarchy",
        "printf '%s' '[{\"id\":\"a.b\",\"enabled\":true},{\"id\":\"a.b\",\"enabled\":true}]'",
    );
    assert_eq!(snapshot(bin, companion)["state"], "unknown");

    stub(bin, "omarchy", "printf '%s' '[{\"id\":\"a.b\"}]'");
    assert_eq!(snapshot(bin, companion)["state"], "unknown");

    stub(bin, "omarchy", "exit 3");
    assert_eq!(snapshot(bin, companion)["state"], "unknown");
}

#[test]
fn a_companion_id_must_be_an_identifier() {
    let refused = fileblade()
        .args(["host-status", "--companion", "../escape"])
        .output()
        .unwrap();
    assert!(!refused.status.success());
    assert!(String::from_utf8_lossy(&refused.stderr).contains("is not a plugin id"));
}
