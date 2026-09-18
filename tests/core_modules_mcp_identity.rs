#[path = "common/mcp_case.rs"]
mod mcp_case;

use fileblade::core_modules::mcp::apply::{Applier, Removal};
use fileblade::core_modules::mcp::model::{digest_bytes, fs_encode};
use fileblade::core_modules::mcp::safeio::{Deadline, bounded_directories};
use fileblade::core_modules::watch::WatchPlan;
use mcp_case::{Case, serial, text};
use serde_json::{Value, json};
use std::fs;
use std::os::unix::fs::symlink;

#[test]
fn non_utf8_path_bytes_hash_the_surrogate_encoding() {
    assert_eq!(fs_encode(b"repo-\xff"), b"repo-\xed\xb3\xbf".to_vec());
    assert_eq!(fs_encode("repo".as_bytes()), b"repo".to_vec());
    let path = b"/home/u/repo-\xff/.mcp.json";
    let identifier = digest_bytes(&[
        b"definition-v2",
        b"claude",
        b"x",
        b"<project>/.mcp.json",
        &fs_encode(path),
        b"n",
        b"f",
    ]);
    assert_eq!(identifier, "9eb9345dbefc524352a37f3f");
}

#[test]
fn a_dangling_config_symlink_reads_as_missing() {
    let _guard = serial();
    let bare = Case::new();
    let expected = bare.scan();
    let case = Case::new();
    symlink(
        case.project.join("absent.json"),
        case.project.join(".mcp.json"),
    )
    .unwrap();
    let document = case.scan();
    assert_eq!(document.get("warnings"), expected.get("warnings"));
    assert_eq!(document.get("sources"), expected.get("sources"));
    assert_eq!(document.get("definitions"), expected.get("definitions"));
}

#[test]
fn a_lone_surrogate_lists_and_refuses_removal() {
    let _guard = serial();
    let case = Case::new();
    let source = case.project.join(".mcp.json");
    fs::write(
        &source,
        r#"{"mcpServers":{"tool":{"command":"printf","args":["fixture"],"env":{"TOKEN":"\udcff"}}}}"#,
    )
    .unwrap();
    let rows = case.definitions();
    let row = rows
        .iter()
        .find(|row| text(row, "name") == "tool")
        .expect("the server lists despite the lone surrogate");
    let identifier = text(row, "id").to_string();
    let outcome = Applier::new(case.inventory()).remove(
        &identifier,
        &Removal {
            prepare: false,
            expected_payload: None,
            transaction_id: "",
        },
    );
    assert_eq!(outcome.get("ok"), Some(&json!(false)));
    assert_eq!(
        outcome.get("message").and_then(Value::as_str),
        Some(
            "the definition cannot be preserved as a Unicode undo record; edit the source directly"
        )
    );
    assert_eq!(
        fs::read_to_string(&source).unwrap(),
        r#"{"mcpServers":{"tool":{"command":"printf","args":["fixture"],"env":{"TOKEN":"\udcff"}}}}"#
    );
}

#[test]
fn an_expired_deadline_inside_a_directory_walk_is_reported() {
    let case = Case::new();
    let root = case.base.join("walk");
    fs::create_dir_all(root.join("child")).unwrap();
    let mut plan = WatchPlan::default();
    let mut deadline = Deadline::new(0);
    std::thread::sleep(std::time::Duration::from_millis(5));
    assert!(bounded_directories(&mut plan, &root, 16, &mut deadline).is_err());
    let mut fresh = Deadline::new(2000);
    let found = bounded_directories(&mut plan, &root, 16, &mut fresh).unwrap();
    assert_eq!(found, vec![root.join("child")]);
}
