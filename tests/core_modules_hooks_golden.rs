#[path = "common/hook_fixtures.rs"]
mod hook_fixtures;

use fileblade::core_modules::hooks::apply::{self, Locator, Removal};
use fileblade::core_modules::hooks::discovery;
use fileblade::core_modules::hooks::labels::Environ;
use fileblade::core_modules::hooks::safeio::Budget;
use hook_fixtures::build_all;
use serde_json::{Map, Value};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

const BASELINE_ROOT: &str = "/tmp/fileblade-python-baseline";
const TRANSACTION_ID: &str = "0123456789abcdef0123456789abcdef";
const VOLATILE: [&str; 2] = ["updated", "created"];

fn fixture(name: &str) -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/python-baseline/hooks")
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
                .filter(|(key, _)| !VOLATILE.contains(&key.as_str()))
                .map(|(key, item)| (key.clone(), stable(item)))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn environ(home: &Path, state: &Path) -> Environ {
    vec![
        ("HOME", OsString::from(home)),
        ("TZ", OsString::from("UTC")),
        ("XDG_CONFIG_HOME", OsString::from(home.join(".config"))),
        ("XDG_STATE_HOME", OsString::from(state)),
        ("XDG_CACHE_HOME", OsString::from(home.join(".cache"))),
        ("CODEX_HOME", OsString::from(home.join(".codex"))),
    ]
    .into_iter()
    .map(|(key, value)| (OsString::from(key), value))
    .collect()
}

fn prepared_root() -> (PathBuf, PathBuf, PathBuf) {
    let root = PathBuf::from(BASELINE_ROOT).join("hooks");
    let _ = fs::remove_dir_all(&root);
    let home = root.join("home");
    let project = root.join("work/project");
    let state = root.join("state");
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&state).unwrap();
    build_all(&home, &project, None);
    (home, project, state)
}

fn listing(home: &Path, project: &Path, state: &Path) -> Map<String, Value> {
    let mut budget = Budget::default();
    discovery::collect(
        &mut budget,
        &discovery::Query::new(&project.to_string_lossy(), &home.to_string_lossy()),
        environ(home, state),
    )
}

#[test]
fn the_fixture_listing_and_removal_match_the_recorded_python_baseline() {
    let (home, project, state) = prepared_root();
    let document = listing(&home, &project, &state);
    let recorded = fixture("list.json");
    let expected = expanded(recorded.get("items").unwrap(), BASELINE_ROOT);
    let produced = Value::Array(
        document
            .get("items")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default(),
    );
    assert_eq!(stable(&produced), stable(&expected));
    assert_eq!(
        document.get("count").and_then(Value::as_u64),
        recorded.get("count").and_then(Value::as_u64)
    );
    assert_eq!(
        document.get("project"),
        recorded_expanded(&recorded, "project").as_ref()
    );
    assert_eq!(document.get("truncated"), recorded.get("truncated"));
    assert_eq!(document.get("agents"), recorded.get("agents"));

    let removed_id = "b07479258dd9089a";
    let locator = Locator {
        project: &project.to_string_lossy(),
        home: &home.to_string_lossy(),
        environ: environ(&home, &state),
        etc_root: "/etc",
        policy_owner_uid: 0,
        exact: false,
    };
    let prepared = apply::remove(
        &locator,
        removed_id,
        &Removal {
            prepare: true,
            expected_payload: None,
            transaction_id: TRANSACTION_ID,
        },
    );
    let recorded_prepared = expanded(&fixture("prepare-remove.json"), BASELINE_ROOT);
    assert_eq!(Value::Object(prepared.clone()), recorded_prepared);

    let whole = Value::Object(prepared.clone());
    let refused = apply::remove(
        &locator,
        removed_id,
        &Removal {
            prepare: false,
            expected_payload: Some(&whole),
            transaction_id: TRANSACTION_ID,
        },
    );
    assert_eq!(
        Value::Object(refused),
        expanded(&fixture("remove-prepared.json"), BASELINE_ROOT)
    );

    let store_path = state.join("fileblade/hooks-recovery");
    let mut records: Vec<PathBuf> = fs::read_dir(&store_path)
        .unwrap()
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    records.sort();
    assert_eq!(records.len(), 1);
    let mut record: Value = serde_json::from_slice(&fs::read(&records[0]).unwrap()).unwrap();
    record
        .as_object_mut()
        .unwrap()
        .insert("createdAt".to_string(), Value::from("{CREATED_AT}"));
    let mut recorded_record = fixture("recovery-record.json");
    recorded_record = expanded(&recorded_record, BASELINE_ROOT);
    assert_eq!(record, recorded_record);

    let payload = prepared.get("payload").cloned().unwrap();
    let committed = apply::remove(
        &locator,
        removed_id,
        &Removal {
            prepare: false,
            expected_payload: Some(&payload),
            transaction_id: TRANSACTION_ID,
        },
    );
    assert_eq!(committed.get("ok"), Some(&Value::Bool(true)));

    let restored = apply::restore(
        TRANSACTION_ID,
        Some(&serde_json::to_string(&payload).unwrap()),
        &home.to_string_lossy(),
        &environ(&home, &state),
        "/etc",
        0,
    );
    assert_eq!(restored.get("ok"), Some(&Value::Bool(true)), "{restored:?}");
    let after = listing(&home, &project, &state);
    assert_eq!(
        stable(&Value::Array(
            after
                .get("items")
                .and_then(Value::as_array)
                .cloned()
                .unwrap()
        )),
        stable(&expected)
    );
}

fn recorded_expanded(recorded: &Value, key: &str) -> Option<Value> {
    recorded
        .get(key)
        .map(|value| expanded(value, BASELINE_ROOT))
}
