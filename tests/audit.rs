use fileblade::audit;
use serde_json::{Value, json};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::time::Instant;
use tempfile::tempdir;

fn event<'a>(
    command: &'a str,
    arguments: &'a [String],
    outcome: &'a fileblade::AppResult<Value>,
) -> audit::Event<'a> {
    audit::Event {
        via: "cli",
        actor: "cli",
        command,
        arguments,
        outcome,
        started: Instant::now(),
    }
}

#[test]
fn audit_log_records_mutations_privately_and_reads_back_filtered() {
    let temporary = tempdir().unwrap();
    let state = temporary.path().join("state");
    unsafe { std::env::set_var("XDG_STATE_HOME", &state) };
    let ok = Ok(json!({"ok": true, "operation": "trash", "paths": ["/tmp/a"]}));
    let failed: fileblade::AppResult<Value> = Err(fileblade::AppError::command("refused"));
    let trash_arguments = vec!["--path".to_string(), "/tmp/a".to_string()];
    let environment_secret = "ENV_SECRET_DO_NOT_LOG";
    let header_secret = "HEADER_SECRET_DO_NOT_LOG";
    let item = json!({
        "payload": {
            "env": {"TOKEN": environment_secret},
            "headers": {"Authorization": header_secret}
        }
    });
    let bin_arguments = vec![
        "--module".to_string(),
        "mcp".to_string(),
        "--item".to_string(),
        item.to_string(),
    ];
    let bin_outcome = Ok(json!({"ok": true, "entry": "bin:opaque", "payload": item["payload"]}));
    let bin_failure = Ok(json!({
        "ok": false,
        "error": environment_secret,
        "message": header_secret,
        "results": [{"ok": false, "changed": false, "message": environment_secret}],
        "payload": item["payload"],
    }));
    audit::record(&event("trash", &trash_arguments, &ok)).unwrap();
    audit::record(&event("undo", &[], &failed)).unwrap();
    audit::record(&event("bin-put", &bin_arguments, &bin_outcome)).unwrap();
    audit::record(&event(
        "bin-restore",
        &["--id".to_string(), "bin:opaque".to_string()],
        &bin_outcome,
    ))
    .unwrap();
    audit::record(&event("bin-put", &[format!("--item={item}")], &bin_failure)).unwrap();
    audit::record(&event(
        "helper-write",
        &[
            "--provider".to_string(),
            "test.inventory".to_string(),
            "--helper=inventory".to_string(),
            "--method".to_string(),
            "restore".to_string(),
            "--arguments".to_string(),
            item.to_string(),
        ],
        &bin_failure,
    ))
    .unwrap();
    audit::record(&event("search", &[], &ok)).unwrap();
    let path = audit::path();
    assert_eq!(path, state.join("omarchy/fileblade/audit.jsonl"));
    assert_eq!(
        fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    let everything = audit::read(50, "", "");
    let entries = everything["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 6, "{everything}");
    assert_eq!(entries[0]["command"], "trash");
    assert_eq!(entries[0]["arguments"][1], "/tmp/a");
    assert_eq!(entries[0]["ok"], true);
    assert_eq!(entries[1]["command"], "undo");
    assert_eq!(entries[1]["ok"], false);
    assert_eq!(entries[1]["error"], "refused");
    assert_eq!(entries[2]["command"], "bin-put");
    assert_eq!(entries[2]["arguments"][3], "<redacted>");
    assert_eq!(entries[2]["result"]["payload"], "<redacted>");
    assert_eq!(entries[3]["command"], "bin-restore");
    assert_eq!(entries[3]["arguments"][1], "bin:opaque");
    assert_eq!(entries[3]["result"]["payload"], "<redacted>");
    assert_eq!(entries[4]["command"], "bin-put");
    assert_eq!(entries[4]["arguments"][0], "--item=<redacted>");
    assert_eq!(entries[4]["error"], "bin operation failed");
    assert_eq!(entries[4]["result"]["results"]["failed"], 1);
    assert_eq!(entries[5]["command"], "helper-write");
    assert_eq!(
        entries[5]["arguments"],
        json!([
            "--provider",
            "test.inventory",
            "--helper=inventory",
            "--method",
            "restore"
        ])
    );
    assert_eq!(entries[5]["error"], "helper operation failed");
    assert_eq!(entries[5]["result"], json!({"ok":false}));
    let audit_text = fs::read_to_string(&path).unwrap();
    assert!(!audit_text.contains(environment_secret));
    assert!(!audit_text.contains(header_secret));
    assert!(entries[0]["ts"].as_str().unwrap().ends_with('Z'));
    let only_undo = audit::read(50, "", "undo");
    assert_eq!(only_undo["entries"].as_array().unwrap().len(), 1);
    let future = audit::read(50, "2999-01-01T00:00:00Z", "");
    assert!(future["entries"].as_array().unwrap().is_empty());
    let last = audit::read(1, "", "");
    assert_eq!(last["entries"][0]["command"], "helper-write");
    let idle_prune = Ok(
        json!({"ok": true, "operation": "trash-prune", "eligible": 0, "completed": 0, "failed": 0}),
    );
    let idle_recover = Ok(
        json!({"ok": true, "operation": "recover", "restored": [], "removed": [], "conflicts": [], "errors": []}),
    );
    let pruned = Ok(
        json!({"ok": true, "operation": "trash-prune", "eligible": 2, "completed": 2, "failed": 0}),
    );
    let days = vec!["--days".to_string(), "7".to_string()];
    audit::record(&event("trash-prune", &days, &idle_prune)).unwrap();
    audit::record(&event("recover", &[], &idle_recover)).unwrap();
    audit::record(&event("trash-prune", &days, &pruned)).unwrap();
    let maintenance = audit::read(50, "", "");
    let entries = maintenance["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 7, "{maintenance}");
    assert_eq!(entries[6]["result"]["completed"], 2);
}

#[test]
fn audit_log_rotates_once_at_the_cap() {
    let temporary = tempdir().unwrap();
    let path = temporary.path().join("nested/audit.jsonl");
    fileblade::lease::durable::append(&path, "first", 16).unwrap();
    fileblade::lease::durable::append(&path, "second-line-past-cap", 16).unwrap();
    fileblade::lease::durable::append(&path, "third", 16).unwrap();
    assert_eq!(fs::read_to_string(&path).unwrap(), "third\n");
    assert_eq!(
        fs::read_to_string(path.with_extension("1.jsonl")).unwrap(),
        "first\nsecond-line-past-cap\n"
    );
}

#[test]
fn concurrent_appenders_publish_complete_private_records() {
    let temporary = tempdir().unwrap();
    let path = temporary.path().join("audit.jsonl");
    std::thread::scope(|scope| {
        for worker in 0..16 {
            let path = &path;
            scope.spawn(move || {
                for index in 0..100 {
                    fileblade::lease::durable::append(
                        path,
                        &json!({"worker": worker, "index": index}).to_string(),
                        u64::MAX,
                    )
                    .unwrap();
                }
            });
        }
    });
    let records = fs::read_to_string(&path).unwrap();
    let values: Vec<Value> = records
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    assert_eq!(values.len(), 1600);
    let unique: std::collections::BTreeSet<_> = values
        .iter()
        .map(|value| {
            (
                value["worker"].as_u64().unwrap(),
                value["index"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(unique.len(), values.len());
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    assert!(fileblade::lease::durable::append(&path, "{}", u64::MAX).is_err());
}
