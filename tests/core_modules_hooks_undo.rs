use fileblade::core_modules::hooks::apply::{self, Locator, Removal};
use fileblade::core_modules::hooks::discovery;
use fileblade::core_modules::hooks::labels::Environ;
use fileblade::core_modules::hooks::safeio::Budget;
use serde_json::{Map, Value, json};
use std::ffi::OsString;
use std::fs;
use std::path::PathBuf;
use std::sync::{Mutex, MutexGuard};

static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|error| error.into_inner())
}

struct Case {
    _base: tempfile::TempDir,
    root: PathBuf,
    home: PathBuf,
    project: PathBuf,
    state: PathBuf,
    source: PathBuf,
    agent: String,
    event: String,
    container: String,
    original: Value,
    project_text: String,
    home_text: String,
}

fn new_case() -> Case {
    let base = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(base.path()).unwrap();
    let home = root.join("home");
    let project = root.join("project");
    let state = root.join("state");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&project).unwrap();
    fs::create_dir_all(&state).unwrap();
    let source = project.join(".claude/settings.json");
    fs::create_dir_all(source.parent().unwrap()).unwrap();
    Case {
        _base: base,
        project_text: project.to_string_lossy().into_owned(),
        home_text: home.to_string_lossy().into_owned(),
        root,
        home,
        project,
        state,
        source,
        agent: "claude-code".to_string(),
        event: "PreToolUse".to_string(),
        container: "hooks".to_string(),
        original: Value::Null,
    }
}

impl Case {
    fn variables(&self) -> Environ {
        vec![
            (OsString::from("HOME"), OsString::from(&self.home)),
            (
                OsString::from("XDG_STATE_HOME"),
                OsString::from(&self.state),
            ),
        ]
    }

    fn etc(&self) -> String {
        self.root.join("etc").to_string_lossy().into_owned()
    }

    fn locator(&self) -> Locator<'_> {
        Locator {
            project: &self.project_text,
            home: &self.home_text,
            environ: self.variables(),
            etc_root: Box::leak(self.etc().into_boxed_str()),
            policy_owner_uid: uid(),
            exact: true,
        }
    }

    fn write(&mut self, definitions: Value) {
        let mut original = Map::new();
        original.insert("other".to_string(), json!({"keep": true}));
        original.insert(
            self.container.clone(),
            json!({self.event.clone(): definitions}),
        );
        if self.agent == "copilot-cli" {
            original.insert("version".to_string(), json!(1));
        }
        self.original = Value::Object(original);
        fs::write(&self.source, serde_json::to_string(&self.original).unwrap()).unwrap();
    }

    fn rows(&self) -> Vec<Map<String, Value>> {
        let mut budget = Budget::default();
        let document = discovery::collect(
            &mut budget,
            &discovery::Query {
                project: &self.project_text,
                home: &self.home_text,
                etc_root: &self.etc(),
                policy_owner_uid: uid(),
                exact: true,
                scope: "all",
            },
            self.variables(),
        );
        document
            .get("items")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_object)
                    .filter(|row| {
                        row.get("agent").and_then(Value::as_str) == Some(self.agent.as_str())
                            && row
                                .get("source")
                                .and_then(Value::as_object)
                                .and_then(|source| source.get("path"))
                                .and_then(Value::as_str)
                                == Some(self.source.to_string_lossy().as_ref())
                    })
                    .cloned()
                    .collect()
            })
            .unwrap_or_default()
    }

    fn remove(&self, row: &Map<String, Value>) -> Map<String, Value> {
        apply::remove(
            &self.locator(),
            row.get("id").and_then(Value::as_str).unwrap_or_default(),
            &Removal {
                prepare: false,
                expected_payload: None,
                transaction_id: "",
            },
        )
    }

    fn prepare(&self, row: &Map<String, Value>) -> Map<String, Value> {
        apply::remove(
            &self.locator(),
            row.get("id").and_then(Value::as_str).unwrap_or_default(),
            &Removal {
                prepare: true,
                expected_payload: None,
                transaction_id: "",
            },
        )
    }

    fn commit(&self, row: &Map<String, Value>, expected: &Value) -> Map<String, Value> {
        apply::remove(
            &self.locator(),
            row.get("id").and_then(Value::as_str).unwrap_or_default(),
            &Removal {
                prepare: false,
                expected_payload: Some(expected),
                transaction_id: "",
            },
        )
    }

    fn restore(&self, record_id: &str, payload: &Value) -> Map<String, Value> {
        apply::restore(
            record_id,
            Some(&serde_json::to_string(payload).unwrap()),
            &self.home_text,
            &self.variables(),
            &self.etc(),
            uid(),
        )
    }

    fn store(&self) -> fileblade::core_modules::recovery_store::RecoveryStore {
        apply::recovery_store(&self.home, &self.variables())
    }

    fn mint(&self, payload: &Value) -> String {
        let context = json!({
            "project": self.project_text,
            "home": self.home_text,
            "etcRoot": self.etc(),
            "policyOwnerUid": uid().to_string(),
        });
        self.store()
            .write(payload, "fixture", &context, "")
            .unwrap()
    }

    fn current(&self) -> Value {
        serde_json::from_slice(&fs::read(&self.source).unwrap()).unwrap()
    }

    fn round_trip(&mut self, index: usize) -> Map<String, Value> {
        let rows = self.rows();
        let removed = self.remove(&rows[index]);
        assert_eq!(removed.get("ok"), Some(&json!(true)), "{removed:?}");
        let payload = removed.get("payload").cloned().unwrap();
        let restored = self.restore("", &payload);
        assert_eq!(restored.get("ok"), Some(&json!(true)), "{restored:?}");
        assert_eq!(self.current(), self.original);
        let again = self.restore("", &payload);
        assert_eq!(again.get("ok"), Some(&json!(true)));
        assert_eq!(
            again["results"][0]["changed"],
            json!(false),
            "a second restore changes nothing"
        );
        removed
    }
}

fn uid() -> u32 {
    rustix::process::geteuid().as_raw()
}

fn command(text: &str) -> Value {
    json!({"type": "command", "command": text})
}

#[test]
fn exact_fields_timeout_group_and_position_survive_the_round_trip() {
    let _guard = serial();
    let mut case = new_case();
    case.write(json!([
        {"hooks": [command("printf before")]},
        {"matcher": "Bash", "groupExtension": {"keep": [1, false]}, "hooks": [
            command("printf first"),
            {"type": "command", "command": "printf selected", "timeout": 0.5,
             "entryExtension": {"keep": true}, "if": "test fixture"},
            command("printf last")]},
        {"hooks": [command("printf after")]},
    ]));
    case.round_trip(2);
}

#[test]
fn a_prepared_removal_preserves_the_source_and_compares_the_whole_record() {
    let _guard = serial();
    let mut case = new_case();
    case.write(json!([{"hooks": [command("printf first"), command("printf selected")]}]));
    let row = case.rows().remove(1);
    let before = fs::read(&case.source).unwrap();
    let prepared = case.prepare(&row);
    assert_eq!(prepared.get("ok"), Some(&json!(true)), "{prepared:?}");
    assert_eq!(fs::read(&case.source).unwrap(), before);
    let mut tampered = prepared.get("payload").cloned().unwrap();
    tampered["source"] = json!(case.root.join("elsewhere").to_string_lossy());
    let refused = case.commit(&row, &tampered);
    assert_eq!(refused.get("ok"), Some(&json!(false)), "{refused:?}");
    assert_eq!(fs::read(&case.source).unwrap(), before);
    let payload = prepared.get("payload").cloned().unwrap();
    let committed = case.commit(&row, &payload);
    assert_eq!(committed.get("ok"), Some(&json!(true)), "{committed:?}");
    assert_eq!(case.restore("", &payload).get("ok"), Some(&json!(true)));
    assert_eq!(case.current(), case.original);
}

#[test]
fn the_last_entry_restores_all_group_metadata() {
    let _guard = serial();
    let mut case = new_case();
    case.write(
        json!([{"matcher": "Bash", "enabled": false, "extra": "keep", "hooks": [
        {"type": "command", "command": "printf selected", "timeout": 0.5}]}]),
    );
    case.round_trip(0);
}

#[test]
fn duplicate_commands_remove_only_the_selected_record() {
    let _guard = serial();
    let mut case = new_case();
    let entry = command("printf duplicate");
    case.write(json!([
        {"matcher": "Bash", "hooks": [entry.clone(), entry.clone()]},
        {"matcher": "Read", "hooks": [entry]},
    ]));
    let row = case.rows().remove(1);
    let removed = case.remove(&row);
    assert_eq!(removed.get("ok"), Some(&json!(true)), "{removed:?}");
    let mut expected = case.original.clone();
    expected["hooks"][&case.event][0]["hooks"]
        .as_array_mut()
        .unwrap()
        .remove(1);
    assert_eq!(case.current(), expected);
    let payload = removed.get("payload").cloned().unwrap();
    assert_eq!(case.restore("", &payload).get("ok"), Some(&json!(true)));
    assert_eq!(case.current(), case.original);
}

#[test]
fn a_group_larger_than_one_hundred_keeps_a_unique_address() {
    let _guard = serial();
    let mut case = new_case();
    let entries: Vec<Value> = (0..101)
        .map(|index| command(&format!("printf first-{index}")))
        .collect();
    case.write(json!([
        {"hooks": entries},
        {"hooks": [command("printf second")]},
    ]));
    let rows = case.rows();
    assert_eq!(rows.len(), 102);
    let unique: std::collections::BTreeSet<&str> = rows
        .iter()
        .filter_map(|row| row.get("id").and_then(Value::as_str))
        .collect();
    assert_eq!(unique.len(), 102);
    let entry = apply::source_entry(&rows[100], case.original.as_object().unwrap()).unwrap();
    assert_eq!(entry.get("command"), Some(&json!("printf first-100")));
    case.round_trip(100);
}

#[test]
fn a_stale_row_cannot_remove_or_copy_its_replacement() {
    let _guard = serial();
    let mut case = new_case();
    case.write(json!([{"hooks": [{"type": "command", "command": "printf same", "timeout": 1}]}]));
    let old = case.rows().remove(0);
    let mut replacement = case.original.clone();
    replacement["hooks"][&case.event][0]["hooks"][0]["timeout"] = json!(2);
    fs::write(&case.source, serde_json::to_string(&replacement).unwrap()).unwrap();
    assert_ne!(old.get("id"), case.rows()[0].get("id"));
    assert_eq!(case.remove(&old).get("ok"), Some(&json!(false)));
    let copied = apply::apply(
        &case.locator(),
        old.get("id").and_then(Value::as_str).unwrap(),
        &["codex".to_string()],
        "on",
    );
    assert_eq!(copied.get("ok"), Some(&json!(false)), "{copied:?}");
    assert!(!case.home.join(".codex/hooks.json").exists());
    assert_eq!(case.current(), replacement);
}

#[test]
fn restore_preserves_other_events_and_refuses_a_changed_event() {
    let _guard = serial();
    let mut case = new_case();
    case.write(json!([{"hooks": [command("printf selected")]}]));
    let removed = case.remove(&case.rows().remove(0));
    assert_eq!(removed.get("ok"), Some(&json!(true)), "{removed:?}");
    let payload = removed.get("payload").cloned().unwrap();
    let mut changed = case.current();
    changed["other"]["added"] = json!(42);
    changed["hooks"]["Stop"] = json!([{"hooks": [{"command": "printf later"}]}]);
    fs::write(&case.source, serde_json::to_string(&changed).unwrap()).unwrap();
    let restored = case.restore("", &payload);
    assert_eq!(restored.get("ok"), Some(&json!(true)), "{restored:?}");
    changed["hooks"][&case.event] = case.original["hooks"][&case.event].clone();
    assert_eq!(case.current(), changed);

    let mut case = new_case();
    case.write(json!([{"hooks": [command("printf selected")]}]));
    let removed = case.remove(&case.rows().remove(0));
    let payload = removed.get("payload").cloned().unwrap();
    let mut changed = case.current();
    changed["hooks"][&case.event] = json!([{"hooks": [{"command": "printf later"}]}]);
    fs::write(&case.source, serde_json::to_string(&changed).unwrap()).unwrap();
    let before = fs::read(&case.source).unwrap();
    let refused = case.restore("", &payload);
    assert_eq!(refused.get("ok"), Some(&json!(false)), "{refused:?}");
    assert!(refused["message"].as_str().unwrap().contains("changed"));
    assert_eq!(fs::read(&case.source).unwrap(), before);
}

#[test]
fn a_flat_copilot_record_round_trips() {
    let _guard = serial();
    let mut case = new_case();
    case.source = case.project.join(".github/hooks/fixture.json");
    fs::create_dir_all(case.source.parent().unwrap()).unwrap();
    case.agent = "copilot-cli".to_string();
    case.event = "preToolUse".to_string();
    case.write(
        json!([{"type": "command", "bash": "printf selected", "timeoutSec": 0.25,
                       "powershell": "Write-Host fixture", "extra": {"keep": true}}]),
    );
    case.round_trip(0);
}

#[test]
fn an_antigravity_named_group_round_trips() {
    let _guard = serial();
    let mut case = new_case();
    case.source = case.project.join(".agents/hooks.json");
    fs::create_dir_all(case.source.parent().unwrap()).unwrap();
    case.agent = "antigravity".to_string();
    case.container = "original group".to_string();
    case.write(json!([{"matcher": "Bash", "extra": "group", "hooks": [
        {"type": "command", "command": "printf selected", "extra": "entry"}]}]));
    case.original["original group"]["enabled"] = json!(false);
    fs::write(&case.source, serde_json::to_string(&case.original).unwrap()).unwrap();
    case.round_trip(0);
}

#[test]
fn a_retargeted_symlink_cannot_restore_into_another_file() {
    let _guard = serial();
    let mut case = new_case();
    case.write(json!([{"hooks": [command("printf selected")]}]));
    let real = case.root.join("real.json");
    fs::rename(&case.source, &real).unwrap();
    std::os::unix::fs::symlink(&real, &case.source).unwrap();
    let removed = case.remove(&case.rows().remove(0));
    assert_eq!(removed.get("ok"), Some(&json!(true)), "{removed:?}");
    let other = case.root.join("other.json");
    fs::write(&other, fs::read(&real).unwrap()).unwrap();
    fs::remove_file(&case.source).unwrap();
    std::os::unix::fs::symlink(&other, &case.source).unwrap();
    let payload = removed.get("payload").cloned().unwrap();
    let refused = case.restore("", &payload);
    assert_eq!(refused.get("ok"), Some(&json!(false)), "{refused:?}");
    assert_eq!(fs::read(&other).unwrap(), fs::read(&real).unwrap());
}

#[test]
fn invalid_restore_records_are_refused_without_writes() {
    let _guard = serial();
    let mut case = new_case();
    case.write(json!([{"hooks": [command("printf selected")]}]));
    let removed = case.remove(&case.rows().remove(0));
    assert_eq!(removed.get("ok"), Some(&json!(true)), "{removed:?}");
    let before = fs::read(&case.source).unwrap();
    let payload = removed.get("payload").cloned().unwrap();
    for (key, value) in [
        ("index", json!(-1)),
        ("index", json!(true)),
        ("entry", json!([])),
        ("grouped", json!("true")),
        ("fields", json!({"hooks": []})),
        ("before", json!("0".repeat(64))),
        ("format", json!(3)),
        ("event", json!([])),
        ("source", json!({})),
        ("group", json!({})),
    ] {
        let mut forged = payload.clone();
        forged[key] = value.clone();
        let record_id = case.mint(&forged);
        let refused = case.restore(&record_id, &forged);
        assert_eq!(
            refused.get("ok"),
            Some(&json!(false)),
            "{key} = {value} was accepted"
        );
        assert_eq!(fs::read(&case.source).unwrap(), before);
    }
}

#[test]
fn a_forged_payload_cannot_write_outside_the_known_hook_files() {
    let _guard = serial();
    let mut case = new_case();
    case.write(json!([{"hooks": [command("printf selected")]}]));
    let removed = case.remove(&case.rows().remove(0));
    assert_eq!(removed.get("ok"), Some(&json!(true)), "{removed:?}");
    let outsider = case.home.join("unrelated.json");
    fs::write(
        &outsider,
        serde_json::to_string(&json!({"keep": true})).unwrap(),
    )
    .unwrap();
    let before = fs::read(&outsider).unwrap();
    let mut forged = removed.get("payload").cloned().unwrap();
    forged["source"] = json!(outsider.to_string_lossy());
    forged["target"] = json!(outsider.to_string_lossy());
    assert_eq!(case.restore("", &forged).get("ok"), Some(&json!(false)));
    assert_eq!(fs::read(&outsider).unwrap(), before);
    let record_id = case.mint(&forged);
    let stored = case.restore(&record_id, &forged);
    assert_eq!(stored.get("ok"), Some(&json!(false)), "{stored:?}");
    assert_eq!(fs::read(&outsider).unwrap(), before);
}

#[test]
fn duplicate_json_members_refuse_the_removal_before_any_write() {
    let _guard = serial();
    let case = new_case();
    fs::write(
        &case.source,
        "{\"other\":1,\"other\":2,\"hooks\":{\"PreToolUse\":[{\"hooks\":[{\"type\":\"command\",\"command\":\"printf fixture\"}]}]}}",
    )
    .unwrap();
    let row = case.rows().remove(0);
    let before = fs::read(&case.source).unwrap();
    assert_eq!(case.remove(&row).get("ok"), Some(&json!(false)));
    assert_eq!(fs::read(&case.source).unwrap(), before);
}

#[test]
fn the_store_refuses_a_new_removal_instead_of_evicting_records() {
    let _guard = serial();
    let mut case = new_case();
    case.write(json!([{"hooks": [command("printf selected")]}]));
    let row = case.rows().remove(0);
    let context = json!({
        "project": case.project_text,
        "home": case.home_text,
        "etcRoot": case.etc(),
        "policyOwnerUid": uid().to_string(),
    });
    let store = case.store();
    for index in 0..64 {
        store
            .write(
                &json!({"format": 2, "filler": index}),
                "fixture",
                &context,
                "",
            )
            .unwrap();
    }
    let before = fs::read(&case.source).unwrap();
    let refused = case.remove(&row);
    assert_eq!(refused.get("ok"), Some(&json!(false)), "{refused:?}");
    assert!(
        refused["message"]
            .as_str()
            .unwrap()
            .contains("undo store is full")
    );
    assert_eq!(fs::read(&case.source).unwrap(), before);
}

#[test]
fn a_restored_record_stops_holding_a_place_in_the_store() {
    let _guard = serial();
    let mut case = new_case();
    case.write(json!([{"hooks": [command("printf selected")]}]));
    let removed = case.remove(&case.rows().remove(0));
    assert_eq!(removed.get("ok"), Some(&json!(true)), "{removed:?}");
    let store = case.store();
    assert_eq!(store.live_records().unwrap(), 1);
    let record_id = removed["recordId"].as_str().unwrap().to_string();
    let payload = removed.get("payload").cloned().unwrap();
    assert_eq!(
        case.restore(&record_id, &payload).get("ok"),
        Some(&json!(true))
    );
    assert_eq!(store.live_records().unwrap(), 0);
    let again = case.restore(&record_id, &payload);
    assert_eq!(again.get("ok"), Some(&json!(true)));
    assert_eq!(again["results"][0]["changed"], json!(false));
}

#[test]
fn a_restore_needs_a_prepared_record_and_reports_its_identifier() {
    let _guard = serial();
    let mut case = new_case();
    case.write(json!([{"hooks": [command("printf selected")]}]));
    let removed = case.remove(&case.rows().remove(0));
    assert_eq!(removed.get("ok"), Some(&json!(true)), "{removed:?}");
    assert_eq!(removed["recordId"].as_str().unwrap().len(), 32);
    let payload = removed.get("payload").cloned().unwrap();
    assert_eq!(
        case.restore(removed["recordId"].as_str().unwrap(), &payload)
            .get("ok"),
        Some(&json!(true))
    );
    let removed_again = case.remove(&case.rows().remove(0));
    assert_eq!(removed_again.get("ok"), Some(&json!(true)));
    case.store()
        .discard(removed_again["recordId"].as_str().unwrap())
        .unwrap();
    case.store()
        .discard(removed["recordId"].as_str().unwrap())
        .unwrap();
    let after = fs::read(&case.source).unwrap();
    let refused = case.restore(
        removed_again["recordId"].as_str().unwrap(),
        removed_again.get("payload").unwrap(),
    );
    assert_eq!(refused.get("ok"), Some(&json!(false)), "{refused:?}");
    assert_eq!(fs::read(&case.source).unwrap(), after);
}

#[test]
fn copilot_settings_and_plugin_sources_round_trip() {
    let _guard = serial();
    for relative in [
        ".copilot/settings.json",
        ".github/copilot/settings.json",
        ".copilot/installed-plugins/market/sample/hooks.json",
    ] {
        let mut case = new_case();
        case.agent = "copilot-cli".to_string();
        case.source = if relative.starts_with(".github") {
            case.project.join(relative)
        } else {
            case.home.join(relative)
        };
        fs::create_dir_all(case.source.parent().unwrap()).unwrap();
        if relative.contains("installed-plugins") {
            fs::write(
                case.source.parent().unwrap().join("plugin.json"),
                serde_json::to_string(&json!({"name": "sample"})).unwrap(),
            )
            .unwrap();
        }
        case.write(json!([{"hooks": [command("printf selected")]}]));
        let rows = case.rows();
        assert!(!rows.is_empty(), "no row for {relative}");
        let removed = case.remove(&rows[0]);
        assert_eq!(
            removed.get("ok"),
            Some(&json!(true)),
            "{relative}: {removed:?}"
        );
        let restored = case.restore(
            removed["recordId"].as_str().unwrap(),
            removed.get("payload").unwrap(),
        );
        assert_eq!(
            restored.get("ok"),
            Some(&json!(true)),
            "{relative}: {restored:?}"
        );
        assert_eq!(case.current(), case.original);
    }
}

#[test]
fn a_project_source_restores_without_project_arguments() {
    let _guard = serial();
    let mut case = new_case();
    case.write(json!([{"hooks": [command("printf selected")]}]));
    let removed = case.remove(&case.rows().remove(0));
    assert_eq!(removed.get("ok"), Some(&json!(true)), "{removed:?}");
    let restored = case.restore(
        removed["recordId"].as_str().unwrap(),
        removed.get("payload").unwrap(),
    );
    assert_eq!(restored.get("ok"), Some(&json!(true)), "{restored:?}");
    assert_eq!(case.current(), case.original);
}
