#[path = "common/mcp_fixtures.rs"]
mod mcp_fixtures;

use fileblade::core_modules::mcp::apply::{Applier, Removal};
use fileblade::core_modules::mcp::inventory::{Environ, Inventory, Settings};
use serde_json::{Map, Value};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

const BASELINE_ROOT: &str = "/tmp/fileblade-python-baseline";
const TRANSACTION_ID: &str = "0123456789abcdef0123456789abcdef";
const REMOVED_ID: &str = "c3747448dbffddc5c56d9269";
const VOLATILE: [&str; 2] = ["updated", "created"];
const USAGE_KEYS: [&str; 13] = [
    "uses",
    "usesAgent",
    "usesScheduled",
    "usesUser",
    "agentCalls",
    "userCalls",
    "scheduledCalls",
    "failed",
    "lastUsed",
    "usageAmbiguous",
    "observed",
    "calls",
    "usageIdentity",
];

fn fixture(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/python-baseline/mcp")
        .join(name);
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn expanded(value: &Value, root: &str) -> Value {
    match value {
        Value::String(text) => Value::from(text.replace("{ROOT}", root)),
        Value::Array(items) => {
            Value::Array(items.iter().map(|item| expanded(item, root)).collect())
        }
        Value::Object(entries) => Value::Object(
            entries
                .iter()
                .map(|(key, item)| (key.replace("{ROOT}", root), expanded(item, root)))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn stable(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(stable).collect()),
        Value::Object(entries) => Value::Object(
            entries
                .iter()
                .filter(|(key, _)| {
                    !VOLATILE.contains(&key.as_str()) && !USAGE_KEYS.contains(&key.as_str())
                })
                .map(|(key, item)| (key.clone(), stable(item)))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn structural(value: &Value) -> Value {
    match value {
        Value::Array(items) => Value::Array(items.iter().map(structural).collect()),
        Value::Object(entries) => Value::Object(
            entries
                .iter()
                .filter(|(key, _)| key.as_str() != "metrics")
                .map(|(key, item)| (key.clone(), structural(item)))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn environ(home: &Path, state: &Path) -> Environ {
    [
        ("HOME", OsString::from(home)),
        ("TZ", OsString::from("UTC")),
        ("XDG_CONFIG_HOME", OsString::from(home.join(".config"))),
        ("XDG_STATE_HOME", OsString::from(state)),
        ("CODEX_HOME", OsString::from(home.join(".codex"))),
    ]
    .into_iter()
    .map(|(key, value)| (OsString::from(key), value))
    .collect()
}

fn settings(home: &Path, project: &Path, state: &Path) -> Settings {
    Settings {
        project: project.to_path_buf(),
        home: home.to_path_buf(),
        config_home: home.join(".config"),
        etc_root: PathBuf::from("/etc"),
        codex_home: home.join(".codex"),
        system_owner_uid: 0,
        scope: "all".to_string(),
        environment: environ(home, state),
    }
}

fn prepared_root() -> (PathBuf, PathBuf, PathBuf) {
    let root = PathBuf::from(BASELINE_ROOT).join("mcp");
    let _ = fs::remove_dir_all(&root);
    let home = root.join("home");
    let project = root.join("work/project");
    let state = root.join("state");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&state).unwrap();
    mcp_fixtures::build_all(&root, &project);
    (home, project, state)
}

fn listing(home: &Path, project: &Path, state: &Path) -> Map<String, Value> {
    Inventory::new(settings(home, project, state)).scan()
}

#[test]
fn the_fixture_listing_and_removal_match_the_recorded_python_baseline() {
    let (home, project, state) = prepared_root();
    let document = listing(&home, &project, &state);
    let recorded = fixture("list.json");
    let expected = expanded(recorded.get("definitions").unwrap(), BASELINE_ROOT);
    let produced = document.get("definitions").cloned().unwrap();
    if std::env::var_os("MCP_GOLDEN_DUMP").is_some() {
        fs::write(
            "/tmp/mcp-produced.json",
            serde_json::to_string_pretty(&stable(&produced)).unwrap(),
        )
        .unwrap();
        fs::write(
            "/tmp/mcp-expected.json",
            serde_json::to_string_pretty(&stable(&expected)).unwrap(),
        )
        .unwrap();
    }
    assert_eq!(stable(&produced), stable(&expected));
    assert_eq!(document.get("agents"), recorded.get("agents"));
    assert_eq!(document.get("warnings"), recorded.get("warnings"));
    assert_eq!(document.get("truncated"), recorded.get("truncated"));
    assert_eq!(document.get("healthBasis"), recorded.get("healthBasis"));
    assert_eq!(document.get("schemaVersion"), recorded.get("schemaVersion"));
    assert_eq!(document.get("limits"), recorded.get("limits"));

    let prepared = Applier::new(Inventory::new(settings(&home, &project, &state))).remove(
        REMOVED_ID,
        &Removal {
            prepare: true,
            expected_payload: None,
            transaction_id: TRANSACTION_ID,
        },
    );
    assert_eq!(
        prepared,
        expanded(&fixture("prepare-remove.json"), BASELINE_ROOT),
        "{prepared:?}"
    );

    let refused = Applier::new(Inventory::new(settings(&home, &project, &state))).remove(
        REMOVED_ID,
        &Removal {
            prepare: false,
            expected_payload: Some(&prepared),
            transaction_id: TRANSACTION_ID,
        },
    );
    assert_eq!(
        refused,
        expanded(&fixture("remove-prepared.json"), BASELINE_ROOT)
    );

    let store_path = state.join("fileblade/mcp-recovery");
    let mut records: Vec<PathBuf> = fs::read_dir(&store_path)
        .unwrap()
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.extension().is_some_and(|value| value == "json"))
        .collect();
    records.sort();
    assert_eq!(records.len(), 1);
    let mut record: Value = serde_json::from_slice(&fs::read(&records[0]).unwrap()).unwrap();
    record
        .as_object_mut()
        .unwrap()
        .insert("createdAt".to_string(), Value::from("{CREATED_AT}"));
    assert_eq!(
        record,
        expanded(&fixture("recovery-record.json"), BASELINE_ROOT)
    );

    let payload = prepared.get("payload").cloned().unwrap();
    let committed = Applier::new(Inventory::new(settings(&home, &project, &state))).remove(
        REMOVED_ID,
        &Removal {
            prepare: false,
            expected_payload: Some(&payload),
            transaction_id: TRANSACTION_ID,
        },
    );
    assert_eq!(
        committed.get("ok"),
        Some(&Value::Bool(true)),
        "{committed:?}"
    );

    let restored = Applier::new(Inventory::new(settings(&home, &project, &state))).restore(
        TRANSACTION_ID,
        Some(&serde_json::to_string(&payload).unwrap()),
    );
    assert_eq!(restored.get("ok"), Some(&Value::Bool(true)), "{restored:?}");
    let after = listing(&home, &project, &state);
    assert_eq!(
        structural(&stable(after.get("definitions").unwrap())),
        structural(&stable(&expected))
    );
}

#[test]
fn a_python_written_toml_recovery_record_restores_at_its_recorded_offset() {
    let recorded = fixture("toml-recovery-record.json");
    let original = recorded.get("original").and_then(Value::as_str).unwrap();
    let updated = recorded.get("updated").and_then(Value::as_str).unwrap();
    let record = recorded.get("record").cloned().unwrap();

    let parsed = fileblade::core_modules::mcp::records::validate_toml_record(&record).unwrap();
    assert_eq!(parsed.name, "alpha");

    let restored = fileblade::core_modules::mcp::records::attach_toml(updated, &record)
        .unwrap()
        .unwrap();
    assert_eq!(restored, original);

    let already = fileblade::core_modules::mcp::records::attach_toml(original, &record).unwrap();
    assert!(already.is_none());

    let mut conflicting = record.clone();
    conflicting
        .as_object_mut()
        .unwrap()
        .insert("after".to_string(), Value::from("0".repeat(64)));
    let appended = fileblade::core_modules::mcp::records::attach_toml(updated, &conflicting)
        .unwrap()
        .unwrap();
    assert_ne!(appended, original);
    assert!(appended.contains("[mcp_servers.alpha]"));
}
