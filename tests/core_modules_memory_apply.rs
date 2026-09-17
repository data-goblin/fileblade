#[path = "common/memory_fixtures.rs"]
mod memory_fixtures;

use fileblade::core_modules::memory::apply::apply_in;
use fileblade::core_modules::memory::discovery::{Context, build_context};
use memory_fixtures::{environ_with, field, payload, rows, write, write_default};
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

static SERIAL: Mutex<()> = Mutex::new(());

const ALL_AGENTS: [&str; 6] = [
    "claude-code",
    "codex",
    "opencode",
    "pi",
    "copilot-cli",
    "antigravity",
];
const DIRECT_PROJECT_READERS: [&str; 4] = ["codex", "opencode", "pi", "copilot-cli"];

fn serialized() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|error| error.into_inner())
}

struct Fixture {
    _base: tempfile::TempDir,
    root: PathBuf,
    home: PathBuf,
    project: PathBuf,
}

fn fixture() -> Fixture {
    let base = tempfile::Builder::new()
        .prefix("fileblade-memory-apply-")
        .tempdir()
        .unwrap();
    let root = fs::canonicalize(base.path()).unwrap();
    let home = root.join("home");
    let project = root.join("work/repo");
    fs::create_dir_all(project.join(".git")).unwrap();
    fs::create_dir_all(&home).unwrap();
    Fixture {
        _base: base,
        root,
        home,
        project,
    }
}

fn context_for(fixture: &Fixture, extra: &[(&str, &str)]) -> Context {
    build_context(
        fixture.project.to_str().unwrap(),
        fixture.home.to_str().unwrap(),
        environ_with(&fixture.home, extra),
        false,
        "all",
    )
}

fn agents(requested: &[&str]) -> Vec<String> {
    requested.iter().map(|agent| agent.to_string()).collect()
}

fn row_for(fixture: &Fixture, realpath: &Path) -> Value {
    let document = payload(&fixture.home, &fixture.project);
    let resolved = fs::canonicalize(realpath).unwrap();
    rows(&document)
        .into_iter()
        .find(|row| field(row, "realpath") == resolved.to_str().unwrap())
        .unwrap_or_else(|| panic!("no row for {}", realpath.display()))
        .clone()
}

fn readers_of(fixture: &Fixture, realpath: &Path) -> BTreeSet<String> {
    memory_fixtures::readers_of(&row_for(fixture, realpath))
        .into_iter()
        .collect()
}

fn run(fixture: &Fixture, row_id: &str, requested: &[&str], state: &str) -> Map<String, Value> {
    run_with(fixture, row_id, requested, state, &[])
}

fn run_with(
    fixture: &Fixture,
    row_id: &str,
    requested: &[&str],
    state: &str,
    extra: &[(&str, &str)],
) -> Map<String, Value> {
    apply_in(
        &context_for(fixture, extra),
        "",
        row_id,
        &agents(requested),
        state,
    )
}

fn by_agent(document: &Map<String, Value>, agent: &str) -> Value {
    document
        .get("results")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .find(|row| field(row, "agent") == agent)
        .unwrap_or_else(|| panic!("no result for {agent}"))
        .clone()
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

fn identity(row: &Value) -> String {
    field(row, "id").to_string()
}

#[test]
fn a_project_file_links_and_unlinks_for_every_agent() {
    let _guard = serialized();
    let fixture = fixture();
    let source = write(
        &fixture.project.join("AGENTS.md"),
        "# shared instructions\n",
    );
    let row = row_for(&fixture, &source);
    assert_eq!(
        memory_fixtures::readers_of(&row)
            .into_iter()
            .collect::<BTreeSet<_>>(),
        DIRECT_PROJECT_READERS
            .iter()
            .map(|agent| agent.to_string())
            .collect::<BTreeSet<_>>()
    );
    let on = run(&fixture, &identity(&row), &ALL_AGENTS, "on");
    assert_eq!(on.get("ok"), Some(&json!(true)));
    assert_eq!(on.get("schemaVersion"), Some(&json!(1)));
    assert_eq!(
        on.get("project").and_then(Value::as_str),
        Some(fixture.project.to_str().unwrap())
    );
    for agent in DIRECT_PROJECT_READERS {
        let result = by_agent(&on, agent);
        assert_eq!(result.get("ok"), Some(&json!(true)), "{agent}");
        assert_eq!(result.get("changed"), Some(&json!(false)), "{agent}");
        assert!(touched(&result).is_empty(), "{agent}");
    }
    for (agent, link) in [
        ("claude-code", fixture.project.join("CLAUDE.md")),
        (
            "antigravity",
            fixture.project.join(".agents/rules/AGENTS.md"),
        ),
    ] {
        let result = by_agent(&on, agent);
        assert_eq!(result.get("changed"), Some(&json!(true)), "{agent}");
        assert_eq!(touched(&result), vec![link.to_str().unwrap().to_string()]);
        assert!(fs::symlink_metadata(&link).unwrap().is_symlink());
        assert_eq!(
            fs::read_link(&link).unwrap(),
            fs::canonicalize(&source).unwrap()
        );
    }
    assert_eq!(
        readers_of(&fixture, &source),
        ALL_AGENTS.iter().map(|agent| agent.to_string()).collect()
    );
    assert_eq!(identity(&row_for(&fixture, &source)), identity(&row));

    let off = run(&fixture, &identity(&row), &ALL_AGENTS, "off");
    assert_eq!(off.get("ok"), Some(&json!(true)), "{off:?}");
    for agent in ["claude-code", "antigravity"] {
        assert_eq!(
            by_agent(&off, agent).get("changed"),
            Some(&json!(true)),
            "{agent}"
        );
    }
    for agent in DIRECT_PROJECT_READERS {
        let result = by_agent(&off, agent);
        assert_eq!(result.get("ok"), Some(&json!(true)), "{agent}");
        assert_eq!(result.get("changed"), Some(&json!(false)), "{agent}");
    }
    assert!(!fs::symlink_metadata(&source).unwrap().is_symlink());
    assert_eq!(
        fs::read_to_string(&source).unwrap(),
        "# shared instructions\n"
    );
    assert!(fs::symlink_metadata(fixture.project.join("CLAUDE.md")).is_err());
    assert!(fs::symlink_metadata(fixture.project.join(".agents/rules/AGENTS.md")).is_err());
    assert_eq!(
        readers_of(&fixture, &source),
        DIRECT_PROJECT_READERS
            .iter()
            .map(|agent| agent.to_string())
            .collect()
    );
}

#[test]
fn a_user_file_links_into_every_documented_home() {
    let _guard = serialized();
    let fixture = fixture();
    let source = write(
        &fixture.home.join(".claude/CLAUDE.md"),
        "# user instructions\n",
    );
    let row = row_for(&fixture, &source);
    assert_eq!(field(&row, "scope"), "user");
    for missing in [".codex", ".pi", ".copilot", ".gemini", ".config"] {
        assert!(fs::symlink_metadata(fixture.home.join(missing)).is_err());
    }
    let on = run(&fixture, &identity(&row), &ALL_AGENTS, "on");
    assert_eq!(on.get("ok"), Some(&json!(true)), "{on:?}");
    let expected = [
        ("codex", fixture.home.join(".codex/AGENTS.md")),
        ("opencode", fixture.home.join(".config/opencode/AGENTS.md")),
        ("pi", fixture.home.join(".pi/agent/AGENTS.md")),
        (
            "copilot-cli",
            fixture.home.join(".copilot/copilot-instructions.md"),
        ),
        ("antigravity", fixture.home.join(".gemini/GEMINI.md")),
    ];
    for (agent, link) in &expected {
        assert_eq!(
            by_agent(&on, agent).get("changed"),
            Some(&json!(true)),
            "{agent}"
        );
        assert!(fs::symlink_metadata(link).unwrap().is_symlink(), "{agent}");
        assert_eq!(
            fs::canonicalize(link).unwrap(),
            fs::canonicalize(&source).unwrap()
        );
    }
    let claude = by_agent(&on, "claude-code");
    assert_eq!(claude.get("ok"), Some(&json!(true)));
    assert_eq!(claude.get("changed"), Some(&json!(false)));
    assert_eq!(
        readers_of(&fixture, &source),
        ALL_AGENTS.iter().map(|agent| agent.to_string()).collect()
    );

    let off = run(&fixture, &identity(&row), &ALL_AGENTS, "off");
    assert_eq!(off.get("ok"), Some(&json!(true)), "{off:?}");
    assert_eq!(
        by_agent(&off, "antigravity").get("changed"),
        Some(&json!(true))
    );
    for (_, link) in &expected {
        assert!(fs::symlink_metadata(link).is_err());
    }
    assert_eq!(
        fs::read_to_string(&source).unwrap(),
        "# user instructions\n"
    );
    assert_eq!(
        readers_of(&fixture, &source),
        BTreeSet::from(["claude-code".to_string(), "opencode".to_string()])
    );
}

#[test]
fn linking_twice_changes_nothing_the_second_time() {
    let _guard = serialized();
    let fixture = fixture();
    let source = write_default(&fixture.project.join("AGENTS.md"));
    let row = row_for(&fixture, &source);
    let first = run(&fixture, &identity(&row), &["claude-code"], "on");
    let second = run(&fixture, &identity(&row), &["claude-code"], "on");
    assert_eq!(
        by_agent(&first, "claude-code").get("changed"),
        Some(&json!(true))
    );
    let repeat = by_agent(&second, "claude-code");
    assert_eq!(repeat.get("ok"), Some(&json!(true)));
    assert_eq!(repeat.get("changed"), Some(&json!(false)));
    assert!(touched(&repeat).is_empty());
    let link = fixture.project.join("CLAUDE.md");
    assert!(fs::symlink_metadata(&link).unwrap().is_symlink());
    let removed = run(&fixture, &identity(&row), &["claude-code"], "off");
    let again = run(&fixture, &identity(&row), &["claude-code"], "off");
    assert_eq!(
        by_agent(&removed, "claude-code").get("changed"),
        Some(&json!(true))
    );
    assert_eq!(
        by_agent(&again, "claude-code"),
        json!({
            "agent": "claude-code",
            "ok": true,
            "changed": false,
            "message": format!("nothing linked at {}", link.display()),
            "touched": [],
        })
    );
}

#[test]
fn a_real_file_and_a_foreign_link_are_both_refused() {
    let _guard = serialized();
    let fixture = fixture();
    let source = write(&fixture.project.join("AGENTS.md"), "# agents\n");
    let real = write(
        &fixture.project.join("CLAUDE.md"),
        "# separate claude file\n",
    );
    let other = write(&fixture.project.join("docs/other.md"), "# other\n");
    let foreign = fixture.project.join(".agents/rules/AGENTS.md");
    fs::create_dir_all(foreign.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&other, &foreign).unwrap();
    let row = row_for(&fixture, &source);
    let document = run(
        &fixture,
        &identity(&row),
        &["claude-code", "antigravity"],
        "on",
    );
    assert_eq!(document.get("ok"), Some(&json!(false)));
    let claude = by_agent(&document, "claude-code");
    assert_eq!(claude.get("ok"), Some(&json!(false)));
    assert_eq!(claude.get("changed"), Some(&json!(false)));
    assert!(field(&claude, "message").contains("real file"));
    let antigravity = by_agent(&document, "antigravity");
    assert_eq!(antigravity.get("ok"), Some(&json!(false)));
    assert!(field(&antigravity, "message").contains("different file"));
    let joined = document
        .get("message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(joined.contains("claude-code:") && joined.contains("antigravity:"));
    assert_eq!(
        fs::read_to_string(&real).unwrap(),
        "# separate claude file\n"
    );
    assert_eq!(
        fs::canonicalize(&foreign).unwrap(),
        fs::canonicalize(&other).unwrap()
    );
    let off = run(
        &fixture,
        &identity(&row),
        &["claude-code", "antigravity", "codex"],
        "off",
    );
    assert_eq!(off.get("ok"), Some(&json!(true)));
    for result in off.get("results").and_then(Value::as_array).unwrap() {
        assert_eq!(result.get("changed"), Some(&json!(false)));
        assert!(touched(result).is_empty());
    }
    assert!(fs::symlink_metadata(&foreign).unwrap().is_symlink());
}

#[test]
fn rules_and_other_kinds_are_never_linked() {
    let _guard = serialized();
    let fixture = fixture();
    let rule = write(
        &fixture.project.join(".claude/rules/api.md"),
        "# api rule\n",
    );
    let memory = write(
        &fixture.home.join(".claude/projects/repo/memory/MEMORY.md"),
        "# index\n",
    );
    let system = write(&fixture.home.join(".pi/agent/SYSTEM.md"), "# system\n");
    for path in [rule, memory, system] {
        let row = row_for(&fixture, &path);
        let document = run(&fixture, &identity(&row), &["codex", "claude-code"], "on");
        assert_eq!(document.get("ok"), Some(&json!(false)));
        for result in document.get("results").and_then(Value::as_array).unwrap() {
            assert_eq!(result.get("ok"), Some(&json!(false)));
            assert_eq!(result.get("changed"), Some(&json!(false)));
            assert!(touched(result).is_empty());
            assert!(field(result, "message").contains(field(&row, "kind")));
        }
    }
}

#[test]
fn unknown_rows_agents_and_states_are_refused() {
    let _guard = serialized();
    let fixture = fixture();
    let source = write_default(&fixture.project.join("AGENTS.md"));
    let row = row_for(&fixture, &source);
    let missing = run(&fixture, "deadbeefdeadbeef", &["codex"], "on");
    assert_eq!(missing.get("ok"), Some(&json!(false)));
    let first = &missing.get("results").and_then(Value::as_array).unwrap()[0];
    assert_eq!(first.get("ok"), Some(&json!(false)));
    assert!(field(first, "message").contains("deadbeefdeadbeef"));
    let bogus = run(&fixture, &identity(&row), &["codex", "not-an-agent"], "on");
    assert_eq!(bogus.get("ok"), Some(&json!(false)));
    assert_eq!(by_agent(&bogus, "codex").get("ok"), Some(&json!(true)));
    assert_eq!(
        bogus.get("message").and_then(Value::as_str),
        Some("not-an-agent: unknown agent id 'not-an-agent'")
    );
    let bad_state = run(&fixture, &identity(&row), &["codex"], "sideways");
    assert_eq!(bad_state.get("ok"), Some(&json!(false)));
    assert!(
        field(
            &bad_state.get("results").and_then(Value::as_array).unwrap()[0],
            "message"
        )
        .contains("state")
    );
    let without_agents = apply_in(&context_for(&fixture, &[]), "", &identity(&row), &[], "on");
    assert_eq!(without_agents.get("ok"), Some(&json!(false)));
    assert_eq!(
        without_agents.get("message").and_then(Value::as_str),
        Some(": at least one --agent is required")
    );
}

#[test]
fn a_source_behind_a_symlink_links_to_its_realpath() {
    let _guard = serialized();
    let fixture = fixture();
    let vault = write(
        &fixture.root.join("vault/instructions.md"),
        "# vault copy\n",
    );
    let alias = fixture.project.join("AGENTS.md");
    std::os::unix::fs::symlink(&vault, &alias).unwrap();
    let row = row_for(&fixture, &vault);
    assert_eq!(row.get("alias"), Some(&json!(true)));
    let document = run(
        &fixture,
        &identity(&row),
        &["claude-code", "antigravity"],
        "on",
    );
    assert_eq!(document.get("ok"), Some(&json!(true)), "{document:?}");
    assert_eq!(
        fs::read_link(fixture.project.join("CLAUDE.md")).unwrap(),
        fs::canonicalize(&vault).unwrap()
    );
    assert!(
        fs::symlink_metadata(fixture.project.join(".agents/rules/instructions.md"))
            .unwrap()
            .is_symlink()
    );
    let off = run(
        &fixture,
        &identity(&row),
        &["claude-code", "antigravity", "codex"],
        "off",
    );
    assert_eq!(off.get("ok"), Some(&json!(true)), "{off:?}");
    for agent in ["claude-code", "antigravity", "codex"] {
        assert_eq!(
            by_agent(&off, agent).get("changed"),
            Some(&json!(true)),
            "{agent}"
        );
    }
    assert!(fs::symlink_metadata(&alias).is_err());
    assert_eq!(fs::read_to_string(&vault).unwrap(), "# vault copy\n");
}

#[test]
fn relocated_agent_homes_are_honoured() {
    let _guard = serialized();
    let fixture = fixture();
    let source = write_default(&fixture.home.join(".claude/CLAUDE.md"));
    let codex_home = fixture.root.join("relocated/codex home");
    let config_home = fixture.root.join("relocated/xdg");
    let copilot_home = fixture.root.join("relocated/copilot");
    let extra = [
        ("CODEX_HOME", codex_home.to_str().unwrap()),
        ("XDG_CONFIG_HOME", config_home.to_str().unwrap()),
        ("COPILOT_HOME", copilot_home.to_str().unwrap()),
    ];
    let row = {
        let document = fileblade::core_modules::memory::discovery::collect_in(
            &mut fileblade::core_modules::watch::WatchPlan::new(),
            &context_for(&fixture, &extra),
            "",
        );
        rows(&document)
            .into_iter()
            .find(|row| {
                field(row, "realpath") == fs::canonicalize(&source).unwrap().to_str().unwrap()
            })
            .unwrap()
            .clone()
    };
    let document = run_with(
        &fixture,
        &identity(&row),
        &["codex", "opencode", "copilot-cli", "antigravity"],
        "on",
        &extra,
    );
    assert_eq!(document.get("ok"), Some(&json!(true)), "{document:?}");
    for link in [
        codex_home.join("AGENTS.md"),
        config_home.join("opencode/AGENTS.md"),
        copilot_home.join("copilot-instructions.md"),
        fixture.home.join(".gemini/GEMINI.md"),
    ] {
        assert!(
            fs::symlink_metadata(&link).unwrap().is_symlink(),
            "{link:?}"
        );
        assert_eq!(
            fs::canonicalize(&link).unwrap(),
            fs::canonicalize(&source).unwrap()
        );
    }
    assert!(fs::symlink_metadata(fixture.home.join(".codex")).is_err());
    assert!(fs::symlink_metadata(fixture.home.join(".copilot")).is_err());
}
