mod support;

use serde_json::json;
use std::ffi::OsStr;
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::symlink;
use support::Resident;
use tempfile::tempdir;

#[test]
fn native_byte_watch_roots_survive_subscription_acknowledgement() {
    use fileblade::common::{parse_path, path_text};

    let temporary = tempdir().unwrap();
    let raw = temporary.path().join(OsStr::from_bytes(b"\xff"));
    let unicode = temporary.path().join("�");
    fs::create_dir(&raw).unwrap();
    fs::create_dir(&unicode).unwrap();
    let mut resident = Resident::start(1);
    resident.send(json!({
        "v": 1, "type": "subscribe", "id": "watch-native", "generation": 1,
        "topic": "filesystem", "paths": [path_text(&raw), path_text(&unicode)]
    }));
    let subscribed = resident.receive_where(|frame| frame["id"] == "watch-native");
    assert_eq!(subscribed["type"], "subscribed", "{subscribed}");
    let watched: Vec<_> = subscribed["paths"]
        .as_array()
        .unwrap()
        .iter()
        .map(|value| parse_path(value.as_str().unwrap()).unwrap())
        .collect();
    assert!(
        watched.contains(&raw) && watched.contains(&unicode),
        "{subscribed}"
    );
    let child = raw.join(OsStr::from_bytes(b"\xfe.txt"));
    fs::write(&child, "native bytes").unwrap();
    let event = resident.receive_where(|frame| frame["type"] == "event");
    assert_eq!(parse_path(event["root"].as_str().unwrap()).unwrap(), raw);
    assert_eq!(parse_path(event["path"].as_str().unwrap()).unwrap(), child);
    assert_eq!(event["name"], "\\xFE.txt");
    resident.send(json!({"v": 1, "type": "cancel", "id": "watch-native", "generation": 1}));
    assert_eq!(resident.response("watch-native")["cancelled"], true);
    resident.finish();
}

#[test]
fn a_subscription_survives_a_missing_path_and_watches_through_a_symlink() {
    let temporary = tempdir().unwrap();
    let root = temporary.path();
    let real = root.join("real");
    fs::create_dir(&real).unwrap();
    let target = root.join("target");
    fs::create_dir(&target).unwrap();
    let link = root.join("link");
    symlink(&target, &link).unwrap();
    let missing = root.join("missing");

    let mut resident = Resident::start(1);
    resident.send(json!({
        "v": 1,
        "type": "subscribe",
        "id": "watch-1",
        "generation": 1,
        "topic": "filesystem",
        "paths": [real, link, missing]
    }));
    let subscribed = resident.receive_where(|frame| frame["id"] == "watch-1");
    assert_eq!(subscribed["type"], "subscribed", "{subscribed}");
    assert_eq!(subscribed["ok"], true, "{subscribed}");
    let skipped = subscribed["skipped"].as_array().unwrap();
    assert_eq!(skipped.len(), 1, "{subscribed}");
    assert_eq!(skipped[0]["path"], missing.to_string_lossy().as_ref());
    assert_eq!(subscribed["paths"].as_array().map(Vec::len), Some(2));

    resident.request(
        "list-while-watching",
        "children-window",
        &["--path".to_string(), real.to_string_lossy().into_owned()],
        5000,
    );
    let listing = resident.response("list-while-watching");
    assert_eq!(
        listing["ok"], true,
        "a standing subscription must not count against a concurrency cap of one: {listing}"
    );

    fs::write(link.join("note.txt"), "hello").unwrap();
    let event =
        resident.receive_where(|frame| frame["type"] == "event" && frame["name"] == "note.txt");
    assert_eq!(event["root"], link.to_string_lossy().as_ref(), "{event}");

    resident.send(json!({"v": 1, "type": "cancel", "id": "watch-1", "generation": 1}));
    let closed = resident.response("watch-1");
    assert_eq!(closed["cancelled"], true, "{closed}");
    resident.finish();
}

#[test]
fn a_subscription_with_no_watchable_path_fails_with_the_first_error() {
    let temporary = tempdir().unwrap();
    let missing = temporary.path().join("gone");
    let mut resident = Resident::start(4);
    resident.send(json!({
        "v": 1,
        "type": "subscribe",
        "id": "watch-2",
        "generation": 1,
        "topic": "filesystem",
        "paths": [missing]
    }));
    let closed = resident.response("watch-2");
    assert_eq!(closed["ok"], false, "{closed}");
    assert!(
        closed["error"]
            .as_str()
            .unwrap_or_default()
            .contains("could not watch any of the 1 requested paths"),
        "{closed}"
    );
    resident.finish();
}

#[test]
fn a_subscription_closes_once_every_watched_directory_is_removed() {
    let temporary = tempdir().unwrap();
    let watched = temporary.path().join("store");
    fs::create_dir(&watched).unwrap();
    let mut resident = Resident::start(1);
    resident.send(json!({
        "v": 1, "type": "subscribe", "id": "watch-3", "generation": 1,
        "topic": "filesystem", "paths": [watched]
    }));
    let subscribed = resident.receive_where(|frame| frame["id"] == "watch-3");
    assert_eq!(subscribed["type"], "subscribed", "{subscribed}");
    fs::remove_dir(&watched).unwrap();
    let closed = resident.response("watch-3");
    assert_eq!(closed["ok"], false, "{closed}");
    assert_eq!(
        closed["error"], "every watched path was removed",
        "{closed}"
    );
    resident.finish();
}

#[test]
fn a_subscription_with_names_reports_only_those_children() {
    let temporary = tempdir().unwrap();
    let home = temporary.path().join("home");
    fs::create_dir(&home).unwrap();
    let mut resident = Resident::start(1);
    resident.send(json!({
        "v": 1, "type": "subscribe", "id": "watch-names", "generation": 1,
        "topic": "filesystem", "paths": [home],
        "names": {home.to_string_lossy(): [".claude.json"]}
    }));
    let subscribed = resident.receive_where(|frame| frame["id"] == "watch-names");
    assert_eq!(subscribed["type"], "subscribed", "{subscribed}");
    fs::write(home.join(".bash_history"), "ls").unwrap();
    fs::write(home.join(".claude.json"), "{}").unwrap();
    let event = resident.receive_where(|frame| frame["type"] == "event");
    assert_eq!(event["name"], ".claude.json", "{event}");
    resident.send(json!({
        "v": 1, "type": "subscribe", "id": "watch-bad", "generation": 1,
        "topic": "filesystem", "paths": [home],
        "names": {home.to_string_lossy(): ["nested/file"]}
    }));
    let refused = resident.receive_where(|frame| frame["id"] == "watch-bad");
    assert_eq!(refused["ok"], false, "{refused}");
    resident.send(json!({"v": 1, "type": "cancel", "id": "watch-names", "generation": 1}));
    assert_eq!(resident.response("watch-names")["cancelled"], true);
    resident.finish();
}
