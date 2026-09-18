#[path = "common/hook_fixtures.rs"]
mod hook_fixtures;

use fileblade::core_modules::hooks::apply::{self, Locator, Removal};
use fileblade::core_modules::hooks::events;
use fileblade::core_modules::hooks::labels::Environ;
use fileblade::core_modules::hooks::safeio;
use hook_fixtures::{
    CANARY_TOKEN, build_all, canary_command, collect_with, environ, rows, source_path, summary,
    text, write_json, write_text,
};
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|error| error.into_inner())
}

const TARGET_AGENTS: [&str; 3] = ["codex", "copilot-cli", "antigravity"];

struct Sandbox {
    _base: tempfile::TempDir,
    root: PathBuf,
    home: PathBuf,
    project: PathBuf,
    etc: PathBuf,
    state: PathBuf,
    project_text: String,
    home_text: String,
}

fn sandbox() -> Sandbox {
    let base = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(base.path()).unwrap();
    let home = root.join("home");
    let project = root.join("work/repo");
    let etc = root.join("etc");
    let state = root.join("state");
    fs::create_dir_all(&state).unwrap();
    build_all(&home, &project, None);
    let project_text = project.to_string_lossy().into_owned();
    let home_text = home.to_string_lossy().into_owned();
    let sandbox = Sandbox {
        _base: base,
        root,
        home,
        project,
        etc,
        state,
        project_text,
        home_text,
    };
    for agent in TARGET_AGENTS {
        let path = apply::target_path(agent, &sandbox.home, &sandbox.variables());
        if path.exists() {
            fs::remove_file(path).unwrap();
        }
    }
    sandbox
}

impl Sandbox {
    fn variables(&self) -> Environ {
        let mut variables = environ(&self.home);
        variables.push((
            std::ffi::OsString::from("XDG_STATE_HOME"),
            std::ffi::OsString::from(&self.state),
        ));
        variables
    }

    fn locator(&self) -> Locator<'_> {
        Locator {
            project: self.project_text(),
            home: self.home_text(),
            environ: self.variables(),
            etc_root: "/nonexistent-etc",
            policy_owner_uid: 0,
            exact: false,
        }
    }

    fn project_text(&self) -> &str {
        &self.project_text
    }

    fn home_text(&self) -> &str {
        &self.home_text
    }

    fn inventory(&self) -> Map<String, Value> {
        collect_with(
            &self.home,
            &self.project,
            self.variables(),
            &self.etc,
            0,
            false,
            "all",
        )
        .0
    }

    fn target(&self, agent: &str) -> PathBuf {
        apply::target_path(agent, &self.home, &self.variables())
    }

    fn apply(&self, row_id: &str, agents: &[&str], state: &str) -> Map<String, Value> {
        let names: Vec<String> = agents.iter().map(|agent| agent.to_string()).collect();
        apply::apply(&self.locator(), row_id, &names, state)
    }

    fn remove(&self, row_id: &str) -> Map<String, Value> {
        apply::remove(
            &self.locator(),
            row_id,
            &Removal {
                prepare: false,
                expected_payload: None,
                transaction_id: "",
            },
        )
    }

    fn restore(&self, record_id: &str, payload: &Value) -> Map<String, Value> {
        apply::restore(
            record_id,
            Some(&serde_json::to_string(payload).unwrap()),
            self.home_text(),
            &self.variables(),
            "/nonexistent-etc",
            0,
        )
    }
}

fn find_row(document: &Map<String, Value>, wanted: &[(&str, Value)]) -> Map<String, Value> {
    rows(document)
        .into_iter()
        .find(|row| {
            wanted
                .iter()
                .all(|(key, value)| row.get(*key) == Some(value))
        })
        .unwrap_or_else(|| panic!("no row matching {wanted:?}"))
}

fn source_row(document: &Map<String, Value>) -> Map<String, Value> {
    rows(document)
        .into_iter()
        .find(|row| {
            text(row, "agent") == "claude-code"
                && text(row, "scope") == "user"
                && text(row, "event") == "PreToolUse"
                && summary(row, "timeoutSeconds") == json!(30)
        })
        .expect("claude PreToolUse fixture row")
}

fn written(path: &Path) -> Map<String, Value> {
    let text_value = fs::read_to_string(path).unwrap();
    assert!(text_value.ends_with('\n'));
    assert!(
        text_value == "{}\n" || text_value.starts_with("{\n  "),
        "{text_value:.40}"
    );
    assert!(!text_value.contains('\t'));
    serde_json::from_str::<Value>(&text_value)
        .unwrap()
        .as_object()
        .cloned()
        .unwrap()
}

fn results(document: &Map<String, Value>) -> Vec<Map<String, Value>> {
    document
        .get("results")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_object().cloned())
                .collect()
        })
        .unwrap_or_default()
}

fn message(outcome: &Map<String, Value>) -> String {
    text(outcome, "message")
}

#[test]
fn a_hook_copies_on_and_off_for_every_writer_agent() {
    let _guard = serial();
    let sandbox = sandbox();
    let row = source_row(&sandbox.inventory());
    let digest = summary(&row, "digest").as_str().unwrap().to_string();
    let id = text(&row, "id");
    let applied: BTreeSet<String> = row
        .get("appliedAgents")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect();
    assert_eq!(
        applied,
        BTreeSet::from(["claude-code".to_string(), "copilot-cli".to_string()])
    );
    for agent in TARGET_AGENTS {
        let payload = sandbox.apply(&id, &[agent], "on");
        assert_eq!(payload.get("ok"), Some(&json!(true)), "{payload:?}");
        assert_eq!(payload.get("schemaVersion"), Some(&json!(1)));
        let outcome = results(&payload).remove(0);
        let path = sandbox.target(agent);
        assert_eq!(text(&outcome, "agent"), agent);
        assert_eq!(outcome.get("changed"), Some(&json!(true)), "{outcome:?}");
        assert_eq!(
            outcome.get("touched"),
            Some(&json!([path.to_string_lossy()]))
        );
        let document = written(&path);
        let target_event = events::mapped_event("claude-code", "PreToolUse", agent).unwrap();
        if agent == "antigravity" {
            let group = document.get(&format!("hook-{digest}")).unwrap();
            let entry = &group[&target_event][0];
            assert_eq!(entry["matcher"], json!("Bash"));
            assert_eq!(
                entry["hooks"][0],
                json!({"type": "command", "command": canary_command(), "timeout": 30})
            );
        } else if agent == "copilot-cli" {
            assert_eq!(document.get("version"), Some(&json!(1)));
            assert_eq!(
                document["hooks"][&target_event][0],
                json!({"type": "command", "bash": canary_command(), "timeoutSec": 30})
            );
        } else {
            let entry = &document["hooks"][&target_event][0];
            assert_eq!(entry["matcher"], json!("Bash"));
            assert_eq!(entry["hooks"][0]["command"], json!(canary_command()));
            assert_eq!(entry["hooks"][0]["timeout"], json!(30));
            assert!(entry["hooks"][0].get("if").is_none());
        }
        let repeated = sandbox.apply(&id, &[agent], "on");
        assert_eq!(repeated.get("ok"), Some(&json!(true)));
        assert_eq!(repeated.get("message"), Some(&json!("")));
        let again = results(&repeated).remove(0);
        assert_eq!(again.get("changed"), Some(&json!(false)));
        assert_eq!(again.get("touched"), Some(&json!([])));
    }
    let after_on = find_row(&sandbox.inventory(), &[("id", json!(id))]);
    let applied: BTreeSet<String> = after_on
        .get("appliedAgents")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect();
    assert_eq!(
        applied,
        BTreeSet::from([
            "claude-code".to_string(),
            "codex".to_string(),
            "copilot-cli".to_string(),
            "antigravity".to_string()
        ])
    );
    for agent in TARGET_AGENTS {
        let outcome = results(&sandbox.apply(&id, &[agent], "off")).remove(0);
        let path = sandbox.target(agent);
        assert_eq!(outcome.get("ok"), Some(&json!(true)), "{outcome:?}");
        assert_eq!(outcome.get("changed"), Some(&json!(true)));
        let document = written(&path);
        let blob = serde_json::to_string(&document).unwrap();
        assert!(!blob.contains(&digest));
        assert!(!blob.contains(&canary_command()));
        if agent == "antigravity" {
            assert!(document.get(&format!("hook-{digest}")).is_none());
        } else {
            assert_eq!(document.get("hooks"), Some(&json!({})));
        }
        let repeat = results(&sandbox.apply(&id, &[agent], "off")).remove(0);
        assert_eq!(repeat.get("ok"), Some(&json!(true)));
        assert_eq!(repeat.get("changed"), Some(&json!(false)));
    }
}

#[test]
fn a_condition_survives_only_where_it_is_supported() {
    let _guard = serial();
    let sandbox = sandbox();
    let row = find_row(
        &sandbox.inventory(),
        &[
            ("agent", json!("claude-code")),
            ("scope", json!("user")),
            ("event", json!("PreToolUse")),
            ("index", json!(1)),
        ],
    );
    assert_eq!(summary(&row, "hasCondition"), json!(true));
    let outcome = results(&sandbox.apply(&text(&row, "id"), &["codex"], "on")).remove(0);
    assert_eq!(outcome.get("changed"), Some(&json!(true)), "{outcome:?}");
    let hook = written(&sandbox.target("codex"))["hooks"]["PreToolUse"][0]["hooks"][0].clone();
    assert_eq!(hook["command"], json!(format!("echo {CANARY_TOKEN}")));
    assert!(hook.get("if").is_none() && hook.get("timeout").is_none());
    let claude = results(&sandbox.apply(&text(&row, "id"), &["claude-code"], "on")).remove(0);
    assert_eq!(claude.get("ok"), Some(&json!(true)));
    assert_eq!(claude.get("changed"), Some(&json!(false)));
}

#[test]
fn existing_content_and_permissions_are_preserved() {
    let _guard = serial();
    let sandbox = sandbox();
    let row = source_row(&sandbox.inventory());
    let target = sandbox.target("codex");
    write_json(
        &target,
        &json!({"model": "keep-me", "hooks": {"Stop": [{"hooks": [{"type": "command", "command": "stay"}]}]}}),
    );
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
    let outcome = results(&sandbox.apply(&text(&row, "id"), &["codex"], "on")).remove(0);
    assert_eq!(outcome.get("changed"), Some(&json!(true)), "{outcome:?}");
    let document = written(&target);
    assert_eq!(document["model"], json!("keep-me"));
    assert_eq!(
        document["hooks"]["Stop"][0]["hooks"][0]["command"],
        json!("stay")
    );
    assert_eq!(
        document["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
        json!(canary_command())
    );
    assert_eq!(
        fs::metadata(&target).unwrap().permissions().mode() & 0o777,
        0o600
    );
}

#[test]
fn events_without_an_equivalent_are_refused() {
    let _guard = serial();
    let sandbox = sandbox();
    let unknown = find_row(
        &sandbox.inventory(),
        &[
            ("agent", json!("claude-code")),
            ("event", json!("NotARealEvent")),
        ],
    );
    for agent in TARGET_AGENTS {
        let outcome = results(&sandbox.apply(&text(&unknown, "id"), &[agent], "on")).remove(0);
        assert_eq!(outcome.get("ok"), Some(&json!(false)));
        assert!(
            message(&outcome).contains("no event equivalent"),
            "{outcome:?}"
        );
        assert!(!sandbox.target(agent).exists());
    }
    let invocation = find_row(
        &sandbox.inventory(),
        &[
            ("agent", json!("antigravity")),
            ("event", json!("PreInvocation")),
        ],
    );
    let outcome =
        results(&sandbox.apply(&text(&invocation, "id"), &["claude-code"], "on")).remove(0);
    assert_eq!(outcome.get("ok"), Some(&json!(false)));
    assert!(message(&outcome).contains("no event equivalent"));
    assert_eq!(
        events::mapped_event("claude-code", "PreCompact", "copilot-cli").as_deref(),
        Some("preCompact")
    );
    assert_eq!(
        events::mapped_event("copilot-cli", "PreToolUse", "codex").as_deref(),
        Some("PreToolUse")
    );
    assert_eq!(events::canonical("copilot-cli", "Stop"), "Stop");
    assert_eq!(events::canonical("copilot-cli", "agentStop"), "Stop");
    assert_eq!(
        events::canonical("antigravity", "PreInvocation"),
        "antigravity:PreInvocation"
    );
}

#[test]
fn a_jsonc_or_trailing_comma_target_is_refused_untouched() {
    let _guard = serial();
    let sandbox = sandbox();
    let row = source_row(&sandbox.inventory());
    let target = sandbox.target("codex");
    let original = format!(
        "// comment\n{}\n",
        serde_json::to_string_pretty(&json!({"hooks": {}})).unwrap()
    );
    write_text(&target, &original);
    let outcome = results(&sandbox.apply(&text(&row, "id"), &["codex"], "on")).remove(0);
    assert_eq!(outcome.get("ok"), Some(&json!(false)));
    assert!(message(&outcome).contains("strict JSON"), "{outcome:?}");
    assert_eq!(fs::read_to_string(&target).unwrap(), original);
    let trailing = serde_json::to_string_pretty(&json!({"hooks": {}}))
        .unwrap()
        .replace("{}", "{},");
    write_text(&target, &trailing);
    let outcome = results(&sandbox.apply(&text(&row, "id"), &["codex"], "on")).remove(0);
    assert_eq!(outcome.get("ok"), Some(&json!(false)));
    assert_eq!(fs::read_to_string(&target).unwrap(), trailing);
}

#[test]
fn symlinked_targets_are_updated_and_irregular_targets_refused() {
    let _guard = serial();
    let sandbox = sandbox();
    let row = source_row(&sandbox.inventory());
    let id = text(&row, "id");
    let target = sandbox.target("codex");
    let real = write_json(
        &sandbox.home.join("elsewhere/hooks.json"),
        &json!({"hooks": {}}),
    );
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&real, &target).unwrap();
    let outcome = results(&sandbox.apply(&id, &["codex"], "on")).remove(0);
    assert_eq!(outcome.get("ok"), Some(&json!(true)), "{outcome:?}");
    assert_eq!(outcome.get("changed"), Some(&json!(true)));
    assert!(fs::symlink_metadata(&target).unwrap().is_symlink());
    let written_real: Value = serde_json::from_slice(&fs::read(&real).unwrap()).unwrap();
    assert!(written_real["hooks"]["PreToolUse"].is_array());

    fs::remove_file(&target).unwrap();
    write_json(&target, &json!({"hooks": []}));
    let outcome = results(&sandbox.apply(&id, &["codex"], "on")).remove(0);
    assert_eq!(outcome.get("ok"), Some(&json!(false)));
    assert!(message(&outcome).contains("not an object"), "{outcome:?}");

    let copilot = sandbox.target("copilot-cli");
    write_json(&copilot, &json!({"version": 7, "hooks": {}}));
    let outcome = results(&sandbox.apply(&id, &["copilot-cli"], "on")).remove(0);
    assert_eq!(outcome.get("ok"), Some(&json!(false)));
    assert!(message(&outcome).contains("version"), "{outcome:?}");
    let kept: Value = serde_json::from_slice(&fs::read(&copilot).unwrap()).unwrap();
    assert_eq!(kept, json!({"version": 7, "hooks": {}}));
}

#[test]
fn unknown_ids_agents_and_code_hosted_rows_are_refused() {
    let _guard = serial();
    let sandbox = sandbox();
    let payload = sandbox.apply("not-a-row", &["codex"], "on");
    assert_eq!(payload.get("ok"), Some(&json!(false)));
    assert!(results(&payload).is_empty());
    assert!(text(&payload, "message").starts_with("no hook row"));

    let row = source_row(&sandbox.inventory());
    let payload = sandbox.apply(&text(&row, "id"), &["cursor", "opencode", "pi"], "on");
    assert_eq!(payload.get("ok"), Some(&json!(false)));
    let joined: Vec<String> = results(&payload)
        .iter()
        .map(|outcome| format!("{}: {}", text(outcome, "agent"), message(outcome)))
        .collect();
    assert_eq!(text(&payload, "message"), joined.join("; "));
    let outcomes: Vec<Map<String, Value>> = results(&payload);
    assert!(message(&outcomes[0]).contains("unknown agent"));
    for outcome in &outcomes[1..3] {
        assert!(message(outcome).contains("live in code"));
    }
    for agent in TARGET_AGENTS {
        assert!(!sandbox.target(agent).exists());
    }
    let location = rows(&sandbox.inventory())
        .into_iter()
        .find(|row| text(row, "agent") == "opencode")
        .unwrap();
    let payload = sandbox.apply(&text(&location, "id"), &["codex"], "on");
    assert_eq!(payload.get("ok"), Some(&json!(false)));
    assert!(text(&payload, "message").contains("code-hosted"));
}

#[test]
fn turning_a_row_off_on_its_own_agent_is_refused() {
    let _guard = serial();
    let sandbox = sandbox();
    let row = source_row(&sandbox.inventory());
    let id = text(&row, "id");
    let outcome = results(&sandbox.apply(&id, &["claude-code"], "off")).remove(0);
    assert_eq!(outcome.get("ok"), Some(&json!(false)));
    assert!(message(&outcome).contains("own agent"));
    assert!(
        fs::read_to_string(sandbox.home.join(".claude/settings.json"))
            .unwrap()
            .contains(&canary_command())
    );
    let expanded = sandbox.apply(&id, &["all"], "off");
    let agents: Vec<String> = results(&expanded)
        .iter()
        .map(|outcome| text(outcome, "agent"))
        .collect();
    assert_eq!(agents, TARGET_AGENTS.map(str::to_string).to_vec());
    let everyone = sandbox.apply(&id, &["all", "codex"], "on");
    let agents: Vec<String> = results(&everyone)
        .iter()
        .map(|outcome| text(outcome, "agent"))
        .collect();
    assert_eq!(
        agents,
        vec![
            "claude-code".to_string(),
            "codex".to_string(),
            "copilot-cli".to_string(),
            "antigravity".to_string()
        ]
    );
}

#[test]
fn only_command_hooks_travel_between_agents() {
    let _guard = serial();
    let sandbox = sandbox();
    let prompt = find_row(
        &sandbox.inventory(),
        &[
            ("agent", json!("copilot-cli")),
            ("event", json!("postToolUse")),
            ("scope", json!("project")),
        ],
    );
    assert_eq!(summary(&prompt, "type"), json!("prompt"));
    let payload = sandbox.apply(&text(&prompt, "id"), &["claude-code"], "on");
    assert_eq!(payload.get("ok"), Some(&json!(false)));
    assert!(text(&payload, "message").contains("only command hooks"));

    let bash = find_row(
        &sandbox.inventory(),
        &[
            ("agent", json!("copilot-cli")),
            ("event", json!("preToolUse")),
            ("scope", json!("user")),
        ],
    );
    let payload = sandbox.apply(&text(&bash, "id"), &["codex"], "on");
    assert_eq!(
        results(&payload)[0].get("changed"),
        Some(&json!(true)),
        "{payload:?}"
    );
    let hook = written(&sandbox.target("codex"))["hooks"]["PreToolUse"][0].clone();
    assert_eq!(hook["matcher"], json!("shell"));
    assert_eq!(
        hook["hooks"][0],
        json!({"type": "command", "command": canary_command(), "timeout": 15})
    );
}

#[test]
fn the_top_level_result_is_the_and_of_its_outcomes() {
    let _guard = serial();
    let sandbox = sandbox();
    let row = source_row(&sandbox.inventory());
    let id = text(&row, "id");
    let mixed = sandbox.apply(&id, &["codex", "opencode", "cursor"], "on");
    let states: Vec<Value> = results(&mixed)
        .iter()
        .map(|outcome| outcome.get("ok").cloned().unwrap())
        .collect();
    assert_eq!(states, vec![json!(true), json!(false), json!(false)]);
    assert_eq!(mixed.get("ok"), Some(&json!(false)));
    assert_eq!(
        text(&mixed, "message"),
        "opencode: opencode hooks live in code and have no hook file to write; cursor: unknown agent 'cursor'"
    );
    let clean = sandbox.apply(&id, &["codex", "copilot-cli"], "off");
    assert_eq!(clean.get("ok"), Some(&json!(true)));
    assert_eq!(clean.get("message"), Some(&json!("")));
    let empty = sandbox.apply(&id, &[], "on");
    assert_eq!(empty.get("ok"), Some(&json!(false)));
    assert!(results(&empty).is_empty());
    assert_eq!(empty.get("message"), Some(&json!("")));
    let missing = sandbox.apply("nope", &["codex"], "on");
    assert_eq!(missing.get("ok"), Some(&json!(false)));
    assert!(text(&missing, "message").starts_with("no hook row"));
    let bad_state = sandbox.apply(&id, &["codex"], "sideways");
    assert_eq!(bad_state.get("ok"), Some(&json!(false)));
    assert!(text(&bad_state, "message").contains("state must be on or off"));
}

#[test]
fn a_removal_restores_through_its_record_and_its_payload() {
    let _guard = serial();
    let sandbox = sandbox();
    let row = source_row(&sandbox.inventory());
    let source = PathBuf::from(source_path(&row));
    let before: Value = serde_json::from_slice(&fs::read(&source).unwrap()).unwrap();
    let removed = sandbox.remove(&text(&row, "id"));
    assert_eq!(removed.get("ok"), Some(&json!(true)), "{removed:?}");
    let payload = removed.get("payload").cloned().unwrap();
    assert_eq!(payload["agent"], json!("claude-code"));
    assert_eq!(payload["event"], json!("PreToolUse"));
    assert_eq!(payload["source"], json!(source.to_string_lossy()));
    assert_eq!(payload["format"], json!(2));
    assert_eq!(payload["entry"]["command"], json!(canary_command()));
    let after: Value = serde_json::from_slice(&fs::read(&source).unwrap()).unwrap();
    assert_ne!(after, before);
    assert_eq!(results(&removed)[0].get("changed"), Some(&json!(true)));
    assert_eq!(text(&removed, "recordId").len(), 32);
    let digest = summary(&row, "digest").as_str().unwrap().to_string();
    assert!(!rows(&sandbox.inventory()).iter().any(|entry| {
        summary(entry, "digest") == json!(digest) && source_path(entry) == source.to_string_lossy()
    }));
    let again = sandbox.remove("ffffffffffffffff");
    assert_eq!(again.get("ok"), Some(&json!(false)));
    assert!(text(&again, "message").contains("no hook row"));

    let restored = sandbox.restore(&text(&removed, "recordId"), &payload);
    assert_eq!(restored.get("ok"), Some(&json!(true)), "{restored:?}");
    assert!(
        !serde_json::to_string(&restored)
            .unwrap()
            .contains(&canary_command())
    );
    assert!(restored.get("payload").is_none());
    let twice = sandbox.restore(&text(&removed, "recordId"), &payload);
    assert_eq!(twice.get("ok"), Some(&json!(true)));
    assert_eq!(results(&twice)[0].get("changed"), Some(&json!(false)));
    assert_eq!(
        sandbox.restore("", &json!({})).get("ok"),
        Some(&json!(false))
    );
    let mut forged = payload.clone();
    forged["agent"] = json!("nobody");
    assert_eq!(sandbox.restore("", &forged).get("ok"), Some(&json!(false)));
    let current: Value = serde_json::from_slice(&fs::read(&source).unwrap()).unwrap();
    assert_eq!(current, before);
}

#[test]
fn a_symlinked_source_keeps_its_link_through_removal_and_restore() {
    let _guard = serial();
    let sandbox = sandbox();
    let source = sandbox.home.join(".claude/settings.json");
    let target = sandbox.root.join("vault/settings.json");
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::rename(&source, &target).unwrap();
    std::os::unix::fs::symlink(&target, &source).unwrap();
    let row = source_row(&sandbox.inventory());
    assert_eq!(
        row.get("source")
            .and_then(Value::as_object)
            .and_then(|entry| entry.get("realpath"))
            .and_then(Value::as_str),
        Some(target.to_string_lossy().as_ref())
    );
    let before: Value = serde_json::from_slice(&fs::read(&target).unwrap()).unwrap();
    let removed = sandbox.remove(&text(&row, "id"));
    assert_eq!(removed.get("ok"), Some(&json!(true)), "{removed:?}");
    assert!(fs::symlink_metadata(&source).unwrap().is_symlink());
    let after: Value = serde_json::from_slice(&fs::read(&target).unwrap()).unwrap();
    assert_ne!(after, before);
    let restored = sandbox.restore(&text(&removed, "recordId"), removed.get("payload").unwrap());
    assert_eq!(restored.get("ok"), Some(&json!(true)), "{restored:?}");
    assert!(fs::symlink_metadata(&source).unwrap().is_symlink());
    let final_value: Value = serde_json::from_slice(&fs::read(&target).unwrap()).unwrap();
    assert_eq!(final_value, before);
}

#[test]
fn dropping_or_retyping_a_top_level_key_is_refused() {
    let _guard = serial();
    let sandbox = sandbox();
    let row = source_row(&sandbox.inventory());
    let target = sandbox.target("codex");
    write_json(
        &target,
        &json!({"model": "keep-me", "profile": {"name": "work"}, "hooks": {}}),
    );
    let before = fs::read(&target).unwrap();
    let loaded = apply::load_target(&target).unwrap();
    let mut trimmed = loaded.document.clone();
    trimmed.shift_remove("model");
    trimmed.shift_remove("profile");
    let refused = apply::write_snapshot(&loaded.snapshot, &trimmed, &[]).unwrap_err();
    assert!(
        refused.contains("would drop these top-level keys: model, profile"),
        "{refused}"
    );
    assert_eq!(fs::read(&target).unwrap(), before);

    let mut reshaped = loaded.document.clone();
    reshaped.insert("profile".to_string(), json!([]));
    let refused = apply::write_snapshot(&loaded.snapshot, &reshaped, &[]).unwrap_err();
    assert!(
        refused.contains("would change the type of these top-level keys: profile"),
        "{refused}"
    );
    assert_eq!(fs::read(&target).unwrap(), before);

    let mut declared = loaded.document.clone();
    declared.shift_remove("profile");
    apply::write_snapshot(&loaded.snapshot, &declared, &["profile".to_string()]).unwrap();
    let keys: BTreeSet<String> = written(&target).keys().cloned().collect();
    assert_eq!(
        keys,
        BTreeSet::from(["model".to_string(), "hooks".to_string()])
    );
    let outcome = results(&sandbox.apply(&text(&row, "id"), &["codex"], "on")).remove(0);
    assert_eq!(outcome.get("ok"), Some(&json!(true)), "{outcome:?}");
    let document = written(&target);
    assert_eq!(document["model"], json!("keep-me"));
    assert_eq!(
        document["hooks"]["PreToolUse"][0]["hooks"][0]["command"],
        json!(canary_command())
    );
}

#[test]
fn toml_sources_are_never_rewritten_as_json() {
    let _guard = serial();
    let sandbox = sandbox();
    let source = sandbox.home.join(".codex/config.toml");
    let before = fs::read(&source).unwrap();
    let refused = apply::write_path(&source, &Map::new(), &[]).unwrap_err();
    assert!(
        refused.contains("TOML hook files are never rewritten as JSON"),
        "{refused}"
    );
    assert_eq!(fs::read(&source).unwrap(), before);
    let linked = sandbox.home.join(".codex/linked.json");
    std::os::unix::fs::symlink(&source, &linked).unwrap();
    let refused = apply::write_path(&linked, &Map::new(), &[]).unwrap_err();
    assert!(
        refused.contains("TOML hook files are never rewritten"),
        "{refused}"
    );
    assert_eq!(fs::read(&source).unwrap(), before);
    assert_eq!(
        safeio::document_kind(&[Path::new("a.json"), Path::new("b.toml")]),
        "toml"
    );
    assert_eq!(safeio::document_kind(&[Path::new("a.json")]), "json");
}

#[test]
fn the_update_guard_reads_each_format_as_itself() {
    let toml_before = b"model = \"gpt-5\"\n\n[hooks]\nStop = []\n".as_slice();
    assert_eq!(
        safeio::refuse_update(
            Some(toml_before),
            b"model = \"gpt-5\"\n[hooks]\nStop = [\"x\"]\n",
            "toml",
            &[]
        ),
        ""
    );
    let swallowed = safeio::refuse_update(
        Some(toml_before),
        b"[hooks]\nStop = []\nmodel = \"gpt-5\"\n",
        "toml",
        &[],
    );
    assert!(
        swallowed.contains("would drop these top-level keys: model"),
        "{swallowed}"
    );
    let dropped = safeio::refuse_update(Some(toml_before), b"[hooks]\nStop = []\n", "toml", &[]);
    assert!(
        dropped.contains("would drop these top-level keys: model"),
        "{dropped}"
    );
    let as_json = safeio::refuse_update(
        Some(toml_before),
        b"{\"model\": \"gpt-5\", \"hooks\": {}}",
        "toml",
        &[],
    );
    assert!(as_json.contains("does not parse as TOML"), "{as_json}");

    let json_before = b"{\"model\": \"gpt-5\", \"hooks\": {}}".as_slice();
    assert_eq!(
        safeio::refuse_update(
            Some(json_before),
            b"{\"hooks\": {}, \"model\": \"gpt-5\"}",
            "json",
            &[]
        ),
        ""
    );
    for candidate in [
        b"model = \"gpt-5\"\n".as_slice(),
        b"{\"model\": \"gpt-5\", \"hooks\": {}".as_slice(),
        b"[1, 2]".as_slice(),
        b"{\"model\": \"\xff\"}".as_slice(),
    ] {
        let refused = safeio::refuse_update(Some(json_before), candidate, "json", &[]);
        assert!(refused.contains("does not parse as JSON"), "{refused}");
    }
    assert_eq!(
        safeio::refuse_update(None, b"{\"hooks\": {}}", "json", &[]),
        ""
    );
    let stale = safeio::refuse_update(Some(b"// jsonc\n{}"), b"{\"hooks\": {}}", "json", &[]);
    assert!(
        stale.contains("the file on disk is not valid JSON"),
        "{stale}"
    );
    let deep = format!("{{\"hooks\": {}{}}}", "[".repeat(20), "]".repeat(20));
    let refused = safeio::refuse_update(Some(json_before), deep.as_bytes(), "json", &[]);
    assert!(refused.contains("nesting limit"), "{refused}");
}
