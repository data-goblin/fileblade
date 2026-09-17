#[path = "common/skill_fixtures.rs"]
mod skill_fixtures;

use serde_json::Value;
use skill_fixtures::{collect, environment, link_skill, write_skill, write_skill_raw};
use std::fs;
use std::path::{Path, PathBuf};

const BASELINE_ROOT: &str = "/tmp/fileblade-python-baseline";
const VOLATILE: [&str; 7] = [
    "updated",
    "created",
    "uses",
    "usesAgent",
    "usesUser",
    "usesScheduled",
    "failed",
];

fn build(home: &Path, project: &Path) {
    write_skill(&home.join(".claude/skills"), "claude-user", "A user skill");
    write_skill(
        &home.join(".agents/skills"),
        "agents-user",
        "A shared skill",
    );
    write_skill(&home.join(".pi/agent/skills"), "pi-user", "A pi skill");
    write_skill(
        &home.join(".copilot/skills"),
        "copilot-user",
        "A copilot skill",
    );
    write_skill(
        &home.join(".config/opencode/skills"),
        "opencode-user",
        "An opencode skill",
    );
    write_skill(
        &home.join(".gemini/antigravity-cli/skills"),
        "antigravity-user",
        "An antigravity skill",
    );
    write_skill(
        &project.join(".claude/skills"),
        "project-only",
        "A project skill",
    );
    write_skill_raw(
        &project.join(".claude/skills"),
        &skill_fixtures::native_name(0xff),
        "native-byte",
        "A native-byte skill",
    );
    let descriptor = write_skill(&home.join("vault/skills"), "linked", "A linked skill");
    let target = descriptor.parent().unwrap();
    link_skill(&home.join(".claude/skills"), "linked", target);
    link_skill(&home.join(".agents/skills"), "linked", target);
}

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
                    let scope = entry
                        .get("scope")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    let path = entry
                        .get("path")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_string();
                    entry.insert("id".to_string(), Value::from(format!("{scope}\u{0}{path}")));
                    Value::Object(entry)
                }
                other => other,
            })
            .collect(),
    )
}

fn digests(items: &Value) -> Vec<String> {
    items
        .as_array()
        .cloned()
        .unwrap_or_default()
        .iter()
        .filter_map(|row| row.get("id").and_then(Value::as_str).map(str::to_string))
        .collect()
}

fn baseline() -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/golden/python-baseline/skills/list.json");
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn the_fixture_listing_matches_the_recorded_python_baseline() {
    let base = tempfile::tempdir().unwrap();
    let anchor = fs::canonicalize(base.path()).unwrap();
    let root = anchor.join("skills");
    let home = root.join("home");
    let project = root.join("work/project");
    fs::create_dir_all(&project).unwrap();
    build(&home, &project);
    let document = collect(&environment(&home).anchored(&project));
    let recorded = baseline();
    let expected = normalized(recorded.get("items").unwrap(), BASELINE_ROOT);
    let produced = normalized(
        document.get("items").unwrap(),
        &anchor.display().to_string(),
    );
    let ids = digests(&produced);
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
        document.get("agents"),
        recorded.get("agents"),
        "the agent roster is part of the contract"
    );
    assert_eq!(document.get("count"), recorded.get("count"));
    assert_eq!(document.get("truncated"), recorded.get("truncated"));
    assert_eq!(document.get("schemaVersion"), recorded.get("schemaVersion"));
}
