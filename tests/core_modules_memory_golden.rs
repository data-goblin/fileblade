#[path = "common/memory_fixtures.rs"]
mod memory_fixtures;

use memory_fixtures::{build_all, payload, rows};
use serde_json::Value;
use std::fs;
use std::path::PathBuf;

const BASELINE_ROOT: &str = "/tmp/fileblade-python-baseline";
const VOLATILE: [&str; 2] = ["updated", "created"];

fn normalized(value: &Value, root: &str) -> Value {
    match value {
        Value::String(text) => Value::from(text.replace(root, "{ROOT}")),
        Value::Array(items) => {
            Value::Array(items.iter().map(|item| normalized(item, root)).collect())
        }
        Value::Object(entries) => Value::Object(
            entries
                .iter()
                .filter(|(key, _)| !VOLATILE.contains(&key.as_str()))
                .map(|(key, item)| (key.replace(root, "{ROOT}"), normalized(item, root)))
                .collect(),
        ),
        other => other.clone(),
    }
}

fn identified(items: &Value) -> Value {
    Value::Array(
        items
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|row| match row {
                Value::Object(mut entry) => {
                    let realpath = entry
                        .get("realpath")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    entry.insert("id".to_string(), Value::from(realpath));
                    Value::Object(entry)
                }
                other => other,
            })
            .collect(),
    )
}

fn baseline() -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/python-baseline/memory/list.json");
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn the_fixture_listing_matches_the_recorded_python_baseline() {
    let base = tempfile::tempdir().unwrap();
    let anchor = fs::canonicalize(base.path()).unwrap();
    let home = anchor.join("memory/home");
    let project = anchor.join("memory/work/project");
    fs::create_dir_all(&project).unwrap();
    build_all(&home, &project);
    let document = payload(&home, &project);
    let recorded = baseline();
    let expected = normalized(recorded.get("items").unwrap(), BASELINE_ROOT);
    let produced = normalized(
        document.get("items").unwrap(),
        &anchor.display().to_string(),
    );
    let ids: Vec<String> = rows(&document)
        .into_iter()
        .filter_map(|row| row.get("id").and_then(Value::as_str))
        .map(str::to_string)
        .collect();
    assert!(
        ids.iter()
            .all(|id| id.len() == 16 && id.bytes().all(|byte| byte.is_ascii_hexdigit())),
        "every row carries a sixteen character hex identity"
    );
    let mut unique = ids.clone();
    unique.sort();
    unique.dedup();
    assert_eq!(unique.len(), ids.len(), "row identities are distinct");
    assert_eq!(identified(&produced), identified(&expected));
    assert_eq!(
        document.get("count").and_then(Value::as_u64),
        recorded.get("count").and_then(Value::as_u64)
    );
    assert_eq!(document.get("truncated"), recorded.get("truncated"));
    assert_eq!(document.get("schemaVersion"), recorded.get("schemaVersion"));
    assert_eq!(document.get("excludedKinds"), recorded.get("excludedKinds"));
}
