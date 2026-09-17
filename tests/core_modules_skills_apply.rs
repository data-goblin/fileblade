#[path = "common/skill_fixtures.rs"]
mod skill_fixtures;

use fileblade::core_modules::skills::apply::{apply, valid_name};
use fileblade::core_modules::skills::discovery::Environment;
use fileblade::core_modules::skills::registry;
use serde_json::{Map, Value, json};
use skill_fixtures::{agents_of, environment, link_skill, row_id, write_skill};
use std::fs;
use std::path::Path;
use std::sync::{Mutex, MutexGuard};
use tempfile::TempDir;

static SERIAL: Mutex<()> = Mutex::new(());

fn serialized() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|error| error.into_inner())
}

fn sandbox() -> TempDir {
    tempfile::Builder::new()
        .prefix("fileblade-skills-apply-")
        .tempdir()
        .unwrap()
}

fn agents(requested: &[&str]) -> Vec<String> {
    requested.iter().map(|agent| agent.to_string()).collect()
}

fn results(document: &Map<String, Value>) -> &Vec<Value> {
    document.get("results").and_then(Value::as_array).unwrap()
}

fn first(document: &Map<String, Value>) -> &Value {
    &results(document)[0]
}

fn message(row: &Value) -> &str {
    row.get("message")
        .and_then(Value::as_str)
        .unwrap_or_default()
}

fn touched(row: &Value) -> Vec<String> {
    row.get("touched")
        .and_then(Value::as_array)
        .map(|paths| {
            paths
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn holds(environment: &Environment, name: &str, agent: &str) -> bool {
    agents_of(environment, name).contains(&agent.to_string())
}

#[test]
fn linking_and_unlinking_one_agent_round_trips() {
    let _serial = serialized();
    let base = sandbox();
    let home = base.path().join("home");
    write_skill(
        &home.join(".claude/skills"),
        "shared-skill",
        "A fixture skill",
    );
    let environment = environment(&home);
    let id = row_id(&environment, "shared-skill");
    let document = apply(&environment, &id, &agents(&["codex"]), "on");
    let row = first(&document);
    let link = home.join(".agents/skills/shared-skill");
    assert_eq!(document.get("ok"), Some(&json!(true)));
    assert_eq!(row.get("changed"), Some(&json!(true)));
    assert!(fs::symlink_metadata(&link).unwrap().is_symlink());
    assert_eq!(
        fs::canonicalize(&link).unwrap(),
        fs::canonicalize(home.join(".claude/skills/shared-skill")).unwrap()
    );
    assert_eq!(touched(row), vec![link.display().to_string()]);
    assert!(message(row).contains("; also applies to opencode, pi, copilot-cli"));
    assert!(holds(&environment, "shared-skill", "codex"));
    assert!(holds(&environment, "shared-skill", "pi"));
    let again = apply(&environment, &id, &agents(&["codex"]), "on");
    assert_eq!(first(&again).get("changed"), Some(&json!(false)));
    assert!(touched(first(&again)).is_empty());
    let off = apply(&environment, &id, &agents(&["pi"]), "off");
    assert_eq!(first(&off).get("ok"), Some(&json!(true)));
    assert_eq!(first(&off).get("changed"), Some(&json!(true)));
    assert!(fs::symlink_metadata(&link).is_err());
    assert!(message(first(&off)).contains("; also applies to codex, opencode, copilot-cli"));
    assert!(!holds(&environment, "shared-skill", "codex"));
    assert!(home.join(".claude/skills/shared-skill/SKILL.md").is_file());
    let idle = apply(&environment, &id, &agents(&["pi"]), "off");
    assert_eq!(first(&idle).get("ok"), Some(&json!(true)));
    assert_eq!(first(&idle).get("changed"), Some(&json!(false)));
}

#[test]
fn a_project_row_links_under_the_project_root() {
    let _serial = serialized();
    let base = sandbox();
    let home = base.path().join("home");
    let repo = base.path().join("repo");
    let nested = repo.join("pkg");
    fs::create_dir_all(repo.join(".git")).unwrap();
    fs::create_dir_all(&nested).unwrap();
    write_skill(
        &repo.join(".claude/skills"),
        "repo-skill",
        "A fixture skill",
    );
    let environment = environment(&home).anchored(&nested);
    let id = row_id(&environment, "repo-skill");
    let document = apply(&environment, &id, &agents(&["antigravity"]), "on");
    assert!(
        fs::symlink_metadata(repo.join(".agents/skills/repo-skill"))
            .unwrap()
            .is_symlink()
    );
    assert!(
        message(first(&document)).contains("; also applies to codex, opencode, pi, copilot-cli")
    );
    assert!(fs::symlink_metadata(home.join(".agents")).is_err());
    let homeless = apply(
        &skill_fixtures::environment(&home),
        &id,
        &agents(&["antigravity"]),
        "on",
    );
    assert_eq!(first(&homeless).get("ok"), Some(&json!(false)));
}

#[test]
fn the_all_keyword_expands_to_every_agent_once() {
    let _serial = serialized();
    let base = sandbox();
    let home = base.path().join("home");
    write_skill(
        &home.join(".claude/skills"),
        "everywhere",
        "A fixture skill",
    );
    let environment = environment(&home);
    let id = row_id(&environment, "everywhere");
    let document = apply(&environment, &id, &agents(&["all"]), "on");
    let listed: Vec<&str> = results(&document)
        .iter()
        .filter_map(|row| row.get("agent").and_then(Value::as_str))
        .collect();
    assert_eq!(listed, registry::agents());
    assert_eq!(document.get("ok"), Some(&json!(true)));
    let mut applied = agents_of(&environment, "everywhere");
    applied.sort();
    let mut expected: Vec<String> = registry::agents().iter().map(|a| a.to_string()).collect();
    expected.sort();
    assert_eq!(applied, expected);
    let by_agent = |agent: &str| {
        results(&document)
            .iter()
            .find(|row| row.get("agent").and_then(Value::as_str) == Some(agent))
            .cloned()
            .unwrap()
    };
    assert_eq!(by_agent("claude-code").get("changed"), Some(&json!(false)));
    assert_eq!(by_agent("codex").get("changed"), Some(&json!(true)));
    assert_eq!(by_agent("copilot-cli").get("changed"), Some(&json!(false)));
    assert_eq!(by_agent("pi").get("changed"), Some(&json!(false)));
    let off = apply(
        &environment,
        &id,
        &agents(&["codex", "pi", "antigravity"]),
        "off",
    );
    assert_eq!(off.get("ok"), Some(&json!(true)));
    assert!(!holds(&environment, "everywhere", "antigravity"));
}

#[test]
fn apply_refuses_real_directories_clashes_and_foreign_links() {
    let _serial = serialized();
    let base = sandbox();
    let home = base.path().join("home");
    write_skill(&home.join(".claude/skills"), "guarded", "A fixture skill");
    let environment = environment(&home);
    let id = row_id(&environment, "guarded");
    let real = apply(&environment, &id, &agents(&["claude-code"]), "off");
    assert_eq!(first(&real).get("ok"), Some(&json!(false)));
    assert_eq!(first(&real).get("changed"), Some(&json!(false)));
    assert!(home.join(".claude/skills/guarded/SKILL.md").is_file());
    assert!(
        message(first(&real)).contains(&home.join(".claude/skills/guarded").display().to_string())
    );
    assert!(message(first(&real)).ends_with("the only copy of this skill; refusing to delete it"));
    write_skill(
        &home.join(".agents/skills"),
        "guarded",
        "an unrelated real skill",
    );
    let clash = apply(&environment, &id, &agents(&["codex"]), "on");
    assert_eq!(first(&clash).get("ok"), Some(&json!(false)));
    assert!(touched(first(&clash)).is_empty());
    assert!(message(first(&clash)).ends_with("exists and is not a symlink; nothing was changed"));
    assert!(
        !fs::symlink_metadata(home.join(".agents/skills/guarded"))
            .unwrap()
            .is_symlink()
    );
    let elsewhere = base.path().join("elsewhere");
    write_skill(&elsewhere, "other", "A fixture skill");
    link_skill(
        &home.join(".pi/agent/skills"),
        "guarded",
        &elsewhere.join("other"),
    );
    let foreign = apply(&environment, &id, &agents(&["pi"]), "on");
    assert_eq!(first(&foreign).get("ok"), Some(&json!(false)));
    assert!(message(first(&foreign)).ends_with("not to this skill; nothing was changed"));
    assert_eq!(
        fs::read_link(home.join(".pi/agent/skills/guarded")).unwrap(),
        elsewhere.join("other")
    );
    let unknown_agent = apply(&environment, &id, &agents(&["nobody"]), "on");
    assert_eq!(unknown_agent.get("ok"), Some(&json!(false)));
    assert_eq!(message(first(&unknown_agent)), "unknown agent id nobody");
    let unknown_row = apply(&environment, "0000000000000000", &agents(&["codex"]), "on");
    assert_eq!(unknown_row.get("ok"), Some(&json!(false)));
    assert_eq!(message(first(&unknown_row)), "unknown row id");
    assert!(!valid_name(Path::new("../x")));
    assert!(!valid_name(Path::new("a/b")));
    assert!(!valid_name(Path::new("")));
    assert!(valid_name(Path::new("plain-name")));
}

#[test]
fn only_user_and_project_rows_can_be_applied() {
    let _serial = serialized();
    let base = sandbox();
    let home = base.path().join("home");
    let prefix = base.path().join("prefix");
    write_skill(
        &prefix.join("etc/codex/skills"),
        "system-skill",
        "A fixture skill",
    );
    let environment = environment(&home).prefixed(&prefix);
    let id = row_id(&environment, "system-skill");
    let document = apply(&environment, &id, &agents(&["codex"]), "on");
    assert_eq!(first(&document).get("ok"), Some(&json!(false)));
    assert_eq!(
        message(first(&document)),
        "system rows cannot be applied; only user and project rows can"
    );
    assert!(fs::symlink_metadata(home.join(".agents")).is_err());
}

#[test]
fn turning_a_skill_off_unlinks_every_matching_root() {
    let _serial = serialized();
    let base = sandbox();
    let home = base.path().join("home");
    let real = write_skill(&home.join(".pi/agent/skills"), "multi", "A fixture skill")
        .parent()
        .unwrap()
        .to_path_buf();
    link_skill(&home.join(".config/opencode/skills"), "multi", &real);
    link_skill(&home.join(".agents/skills"), "multi", &real);
    let environment = environment(&home);
    let id = row_id(&environment, "multi");
    assert!(holds(&environment, "multi", "opencode"));
    let document = apply(&environment, &id, &agents(&["opencode"]), "off");
    let row = first(&document);
    assert_eq!(row.get("ok"), Some(&json!(true)));
    assert_eq!(row.get("changed"), Some(&json!(true)));
    let mut removed = touched(row);
    removed.sort();
    assert_eq!(
        removed,
        vec![
            home.join(".agents/skills/multi").display().to_string(),
            home.join(".config/opencode/skills/multi")
                .display()
                .to_string(),
        ]
    );
    assert!(real.join("SKILL.md").is_file());
    assert!(message(row).contains("; also applies to codex, pi, copilot-cli"));
    assert!(!holds(&environment, "multi", "opencode"));
    write_skill(&home.join(".claude/skills"), "anchored", "A fixture skill");
    link_skill(
        &home.join(".config/opencode/skills"),
        "anchored",
        &home.join(".claude/skills/anchored"),
    );
    let blocked = apply(
        &environment,
        &row_id(&environment, "anchored"),
        &agents(&["opencode"]),
        "off",
    );
    assert_eq!(first(&blocked).get("ok"), Some(&json!(false)));
    assert_eq!(first(&blocked).get("changed"), Some(&json!(false)));
    assert!(
        fs::symlink_metadata(home.join(".config/opencode/skills/anchored"))
            .unwrap()
            .is_symlink()
    );
}

#[test]
fn an_incomplete_root_scan_refuses_before_anything_changes() {
    let _serial = serialized();
    let base = sandbox();
    let home = base.path().join("home");
    write_skill(&home.join(".claude/skills"), "crowded", "A fixture skill");
    let shared = home.join(".agents/skills");
    fs::create_dir_all(&shared).unwrap();
    for index in 0..513 {
        fs::create_dir_all(shared.join(format!("filler-{index:04}"))).unwrap();
    }
    let environment = environment(&home);
    let id = row_id(&environment, "crowded");
    let document = apply(&environment, &id, &agents(&["opencode"]), "off");
    assert_eq!(first(&document).get("ok"), Some(&json!(false)));
    assert_eq!(first(&document).get("changed"), Some(&json!(false)));
    assert_eq!(
        message(first(&document)),
        "skill root exceeds the entry limit; nothing was changed"
    );
    assert!(home.join(".claude/skills/crowded/SKILL.md").is_file());
}
