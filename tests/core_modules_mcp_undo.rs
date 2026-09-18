#[path = "common/mcp_case.rs"]
mod mcp_case;

use fileblade::core_modules::mcp::apply::{Applier, Removal};
use fileblade::core_modules::mcp::inventory::{Inventory, Settings};
use fileblade::core_modules::mcp::model::core_agent_id;
use fileblade::core_modules::recovery_store::RecoveryStore;
use mcp_case::{Case, serial, text};
use serde_json::{Value, json};
use std::fs;
use std::path::PathBuf;

struct Undo {
    case: Case,
    source: PathBuf,
    agent: String,
}

fn undo() -> Undo {
    let case = Case::new();
    let source = case.home.join(".codex").join("config.toml");
    Undo {
        case,
        source,
        agent: "codex".to_string(),
    }
}

impl Undo {
    fn write(&self, text: &str) {
        fs::create_dir_all(self.source.parent().unwrap()).unwrap();
        fs::write(&self.source, text).unwrap();
    }

    fn inventory(&self) -> Inventory {
        self.case.inventory()
    }

    fn identifier(&self) -> String {
        let mut store = self.inventory();
        store.scan();
        store
            .definitions
            .iter()
            .find(|row| {
                row.raw_name == "tool"
                    && core_agent_id(&row.agent) == self.agent
                    && row.absolute_path.as_deref() == Some(self.source.as_path())
            })
            .map(|row| row.id.clone())
            .unwrap_or_else(|| panic!("no removable definition for {}", self.agent))
    }

    fn remove_id(&self, identifier: &str) -> Value {
        Applier::new(self.inventory()).remove(
            identifier,
            &Removal {
                prepare: false,
                expected_payload: None,
                transaction_id: "",
            },
        )
    }

    fn remove(&self) -> Value {
        self.remove_id(&self.identifier())
    }

    fn prepare(&self, identifier: &str) -> Value {
        Applier::new(self.inventory()).remove(
            identifier,
            &Removal {
                prepare: true,
                expected_payload: None,
                transaction_id: "",
            },
        )
    }

    fn commit(&self, identifier: &str, expected: &Value) -> Value {
        Applier::new(self.inventory()).remove(
            identifier,
            &Removal {
                prepare: false,
                expected_payload: Some(expected),
                transaction_id: "",
            },
        )
    }

    fn restore(&self, payload: &Value, record_id: &str) -> Value {
        Applier::new(self.inventory()).restore(record_id, Some(&payload.to_string()))
    }

    fn mint(&self, payload: &Value) -> String {
        let applier = Applier::new(self.inventory());
        let context = applier.recovery_context();
        applier
            .recovery
            .write(payload, "fixture", &context, "")
            .unwrap()
    }

    fn store(&self) -> RecoveryStore {
        RecoveryStore::new(self.inventory().recovery_directory())
    }

    fn text(&self) -> String {
        fs::read_to_string(&self.source).unwrap()
    }

    fn bytes(&self) -> Vec<u8> {
        fs::read(&self.source).unwrap()
    }

    fn json(&self) -> Value {
        serde_json::from_slice(&fs::read(&self.source).unwrap()).unwrap()
    }
}

fn payload_of(document: &Value) -> Value {
    document.get("payload").cloned().unwrap()
}

fn ok(document: &Value) -> bool {
    document.get("ok") == Some(&json!(true))
}

fn parsed_toml(text: &str) -> toml::Table {
    text.parse().unwrap()
}

#[test]
fn a_codex_round_trip_preserves_unknown_fields_and_source_text() {
    let _guard = serial();
    let case = undo();
    let original = concat!(
        "model = \"fixture\"\n\n# retained preface\n[mcp_servers.tool]\n",
        "command = \"printf\" # keep this comment\nargs = [\"fixture\"]\n",
        "enabled = false\nstartup_timeout_sec = 0.5\ntool_timeout_sec = 42\n",
        "enabled_tools = [\"first\"]\nunknown = { enabled = true, weight = 1.25 }\n",
        "[mcp_servers.tool.env]\nPRIVATE = \"fixture-private-value\"\n\n",
        "[mcp_servers.neighbor]\ncommand = \"unchanged\"\n"
    );
    case.write(original);
    let removed = case.remove();
    assert!(ok(&removed), "{removed:?}");
    assert!(
        parsed_toml(&case.text())["mcp_servers"]
            .get("tool")
            .is_none()
    );
    let restored = case.restore(&payload_of(&removed), "");
    assert!(ok(&restored), "{restored:?}");
    assert_eq!(case.text(), original);
}

#[test]
fn a_prepared_removal_preserves_the_source_and_stale_records_are_refused() {
    let _guard = serial();
    let case = undo();
    case.write("[mcp_servers.tool]\ncommand = \"printf\"\n");
    let before = case.bytes();
    let identifier = case.identifier();
    let prepared = case.prepare(&identifier);
    assert!(ok(&prepared), "{prepared:?}");
    assert_eq!(case.bytes(), before);
    case.write(&format!(
        "model = \"later\"\n{}",
        String::from_utf8_lossy(&before)
    ));
    let refused = case.commit(&identifier, &payload_of(&prepared));
    assert!(!ok(&refused), "{refused:?}");
    assert!(case.text().starts_with("model = \"later\""));
    fs::write(&case.source, &before).unwrap();
    let committed = case.commit(&identifier, &payload_of(&prepared));
    assert!(ok(&committed), "{committed:?}");
    assert!(ok(&case.restore(&payload_of(&prepared), "")));
    assert_eq!(case.bytes(), before);
}

#[test]
fn a_codex_restore_preserves_other_later_settings() {
    let _guard = serial();
    let case = undo();
    case.write(
        "[mcp_servers.tool]\ncommand = \"printf\"\nenabled = false\nstartup_timeout_sec = 0.5\n",
    );
    let original = parsed_toml(&case.text());
    let removed = case.remove();
    assert!(ok(&removed), "{removed:?}");
    case.write("model = \"later\"\n[mcp_servers.neighbor]\ncommand = \"keep\"\n");
    let restored = case.restore(&payload_of(&removed), "");
    assert!(ok(&restored), "{restored:?}");
    let after = parsed_toml(&case.text());
    assert_eq!(after["model"].as_str(), Some("later"));
    assert_eq!(
        after["mcp_servers"]["neighbor"]["command"].as_str(),
        Some("keep")
    );
    assert_eq!(
        after["mcp_servers"]["tool"].as_table(),
        original["mcp_servers"]["tool"].as_table()
    );
}

#[test]
fn a_stale_id_cannot_remove_or_copy_a_changed_definition() {
    let _guard = serial();
    let case = undo();
    case.write("[mcp_servers.tool]\ncommand = \"printf\"\nstartup_timeout_sec = 1\n");
    let identifier = case.identifier();
    case.write("[mcp_servers.tool]\ncommand = \"printf\"\nstartup_timeout_sec = 2\n");
    let before = case.bytes();
    assert_ne!(identifier, case.identifier());
    assert!(!ok(&case.remove_id(&identifier)));
    assert!(!ok(&Applier::new(case.inventory()).apply(
        &identifier,
        &["claude-code".to_string()],
        "on"
    )));
    assert_eq!(case.bytes(), before);
    assert!(!case.case.home.join(".claude.json").exists());
}

#[test]
fn the_last_opencode_v1_entry_restores_into_its_original_map() {
    let _guard = serial();
    let mut case = undo();
    case.agent = "opencode".to_string();
    case.source = case.case.config.join("opencode").join("opencode.json");
    let original = json!({"other": true, "mcp": {"tool": {"type": "local", "command": ["printf", "fixture"], "enabled": false}}});
    case.write(&original.to_string());
    let removed = case.remove();
    assert!(ok(&removed), "{removed:?}");
    assert_eq!(case.json(), json!({"other": true, "mcp": {}}));
    assert!(ok(&case.restore(&payload_of(&removed), "")));
    assert_eq!(case.json(), original);
}

#[test]
fn a_flat_copilot_source_is_removed_and_restored_in_place() {
    let _guard = serial();
    let mut case = undo();
    case.agent = "copilot-cli".to_string();
    case.source = case.case.project.join(".github").join("mcp.json");
    let original = json!({
        "tool": {"command": "printf", "tools": ["one"], "timeout": 0.5},
        "neighbor": {"command": "keep"},
    });
    case.write(&original.to_string());
    let removed = case.remove();
    assert!(ok(&removed), "{removed:?}");
    assert_eq!(removed.get("changed"), Some(&json!(true)));
    assert_eq!(case.json(), json!({"neighbor": {"command": "keep"}}));
    assert!(ok(&case.restore(&payload_of(&removed), "")));
    assert_eq!(case.json(), original);
}

#[test]
fn a_retargeted_symlink_refuses_restore_without_writing_either_file() {
    let _guard = serial();
    let mut case = undo();
    case.agent = "claude-code".to_string();
    case.source = case.case.project.join(".mcp.json");
    case.write(&json!({"mcpServers": {"tool": {"command": "printf"}}}).to_string());
    let target = case.case.base.join("original.json");
    fs::rename(&case.source, &target).unwrap();
    std::os::unix::fs::symlink(&target, &case.source).unwrap();
    let removed = case.remove();
    assert!(ok(&removed), "{removed:?}");
    let replacement = case.case.base.join("replacement.json");
    fs::write(&replacement, fs::read(&target).unwrap()).unwrap();
    fs::remove_file(&case.source).unwrap();
    std::os::unix::fs::symlink(&replacement, &case.source).unwrap();
    let before = fs::read(&replacement).unwrap();
    assert!(!ok(&case.restore(&payload_of(&removed), "")));
    assert_eq!(fs::read(&replacement).unwrap(), before);
    assert_eq!(fs::read(&target).unwrap(), before);
}

#[test]
fn json_restores_its_original_position_and_preserves_later_values() {
    let _guard = serial();
    let mut case = undo();
    case.agent = "claude-code".to_string();
    case.source = case.case.project.join(".mcp.json");
    let original = json!({
        "mcpServers": {
            "first": {"command": "one"},
            "tool": {"command": "printf", "enabled": false},
            "last": {"command": "three"},
        },
        "other": true,
    });
    case.write(&original.to_string());
    let removed = case.remove();
    assert!(ok(&removed), "{removed:?}");
    let mut after = case.json();
    after
        .as_object_mut()
        .unwrap()
        .insert("later".to_string(), json!({"keep": true}));
    case.write(&after.to_string());
    assert!(ok(&case.restore(&payload_of(&removed), "")));
    let restored = case.json();
    let keys: Vec<&String> = restored["mcpServers"].as_object().unwrap().keys().collect();
    assert_eq!(keys, ["first", "tool", "last"]);
    assert_eq!(restored["later"], json!({"keep": true}));
    assert_eq!(restored["other"], json!(true));
    let before = case.bytes();
    let again = case.restore(&payload_of(&removed), "");
    assert!(ok(&again), "{again:?}");
    assert_eq!(again.get("changed"), Some(&json!(false)));
    assert_eq!(case.bytes(), before);
}

#[test]
fn a_restore_conflict_keeps_the_current_definition_and_does_not_echo_secrets() {
    let _guard = serial();
    let mut case = undo();
    case.agent = "claude-code".to_string();
    case.source = case.case.project.join(".mcp.json");
    case.write(
        &json!({"mcpServers": {"tool": {"command": "printf", "env": {"KEY": "PRIVATE_SENTINEL"}}}})
            .to_string(),
    );
    let removed = case.remove();
    assert!(ok(&removed), "{removed:?}");
    case.write(&json!({"mcpServers": {"tool": {"command": "newer"}}}).to_string());
    let before = case.bytes();
    let restored = case.restore(&payload_of(&removed), "");
    assert!(!ok(&restored));
    assert!(!restored.to_string().contains("PRIVATE_SENTINEL"));
    assert_eq!(case.bytes(), before);
}

#[test]
fn a_flat_pi_record_preserves_the_adapter_settings() {
    let _guard = serial();
    let mut case = undo();
    case.agent = "pi".to_string();
    case.source = case
        .case
        .home
        .join(".pi")
        .join("agent")
        .join("settings.json");
    case.write(&json!({"packages": ["pi-mcp-adapter"]}).to_string());
    case.source = case.case.project.join(".pi").join("mcp.json");
    let original = json!({
        "settings": {"keep": true},
        "$schema": "fixture",
        "tool": {"command": "printf", "enabled": false},
    });
    case.write(&original.to_string());
    let removed = case.remove();
    assert!(ok(&removed), "{removed:?}");
    assert_eq!(removed.get("changed"), Some(&json!(true)));
    assert_eq!(
        case.json(),
        json!({"settings": {"keep": true}, "$schema": "fixture"})
    );
    assert!(ok(&case.restore(&payload_of(&removed), "")));
    assert_eq!(case.json(), original);
}

#[test]
fn toml_dates_nested_values_crlf_and_a_missing_final_newline_round_trip() {
    let _guard = serial();
    let case = undo();
    let original = concat!(
        "[mcp_servers.\"tool\"]\r\ncommand = \"printf\"\r\n",
        "date = 2026-09-05\r\ntime = 12:30:00\r\nstamp = 2026-09-05T12:30:00Z\r\n",
        "limits = [1, 0.5, true]\r\n[mcp_servers.tool.extra]\r\nempty = {}"
    );
    fs::create_dir_all(case.source.parent().unwrap()).unwrap();
    fs::write(&case.source, original).unwrap();
    let removed = case.remove();
    assert!(ok(&removed), "{removed:?}");
    assert!(ok(&case.restore(&payload_of(&removed), "")));
    assert_eq!(case.bytes(), original.as_bytes());
    let again = case.restore(&payload_of(&removed), "");
    assert!(ok(&again), "{again:?}");
    assert_eq!(again.get("changed"), Some(&json!(false)));
}

#[test]
fn a_toml_conflict_or_a_dispersed_table_is_refused_without_writes() {
    let _guard = serial();
    let case = undo();
    case.write(concat!(
        "[mcp_servers.tool]\ncommand = \"printf\"\n[mcp_servers.other]\ncommand = \"keep\"\n",
        "[mcp_servers.tool.env]\nKEY = \"private\"\n"
    ));
    let before = case.bytes();
    assert!(!ok(&case.remove()));
    assert_eq!(case.bytes(), before);
    case.write("[mcp_servers.tool]\ncommand = \"printf\"\n");
    let removed = case.remove();
    assert!(ok(&removed), "{removed:?}");
    case.write("[mcp_servers.tool]\ncommand = \"replacement\"\n");
    let before = case.bytes();
    assert!(!ok(&case.restore(&payload_of(&removed), "")));
    assert_eq!(case.bytes(), before);
}

#[test]
fn duplicate_json_keys_and_nonfinite_numbers_refuse_before_removal() {
    let _guard = serial();
    let mut case = undo();
    case.agent = "claude-code".to_string();
    case.source = case.case.project.join(".mcp.json");
    for malformed in [
        r#"{"mcpServers":{"tool":{"command":"one","command":"two"}}}"#,
        r#"{"mcpServers":{"tool":{"command":"printf","timeout":NaN}}}"#,
    ] {
        case.write(r#"{"mcpServers":{"tool":{"command":"printf"}}}"#);
        let identifier = case.identifier();
        case.write(malformed);
        assert!(!ok(&case.remove_id(&identifier)));
        assert_eq!(case.text(), malformed);
    }
}

#[test]
fn malformed_json_records_are_refused_without_source_changes() {
    let _guard = serial();
    let mut case = undo();
    case.agent = "claude-code".to_string();
    case.source = case.case.project.join(".mcp.json");
    case.write(r#"{"mcpServers":{"tool":{"command":"printf"}}}"#);
    let removed = case.remove();
    assert!(ok(&removed), "{removed:?}");
    let before = case.bytes();
    let payload = payload_of(&removed);
    let mutations: [(&str, Value); 10] = [
        ("format", json!("2")),
        ("format", json!(3)),
        ("kind", json!("toml")),
        ("path", json!([])),
        ("target", json!("relative")),
        ("name", json!([])),
        ("container", json!(["elsewhere"])),
        ("position", json!(true)),
        ("raw", json!([])),
        ("definition", json!("bad")),
    ];
    for (key, value) in mutations {
        let mut forged = payload.clone();
        forged
            .as_object_mut()
            .unwrap()
            .insert(key.to_string(), value);
        let record_id = case.mint(&forged);
        assert!(!ok(&case.restore(&forged, &record_id)), "{key}");
        assert_eq!(case.bytes(), before, "{key}");
    }
}

#[test]
fn malformed_toml_records_are_refused_without_source_changes() {
    let _guard = serial();
    let case = undo();
    case.write("[mcp_servers.tool]\ncommand = \"printf\"\n");
    let removed = case.remove();
    assert!(ok(&removed), "{removed:?}");
    let before = case.bytes();
    let payload = payload_of(&removed);
    let fragment = payload
        .get("text")
        .and_then(Value::as_str)
        .unwrap()
        .to_string();
    let mutations: [(&str, Value); 5] = [
        ("offset", json!(true)),
        ("offset", json!(10000)),
        ("text", json!([])),
        ("after", json!([])),
        (
            "text",
            json!(format!("{fragment}[unrelated]\nkeep = false\n")),
        ),
    ];
    for (key, value) in mutations {
        let mut forged = payload.clone();
        forged
            .as_object_mut()
            .unwrap()
            .insert(key.to_string(), value);
        let record_id = case.mint(&forged);
        assert!(!ok(&case.restore(&forged, &record_id)), "{key}");
        assert_eq!(case.bytes(), before, "{key}");
    }
}

#[test]
fn a_forged_payload_cannot_write_outside_the_known_configuration_files() {
    let _guard = serial();
    let mut case = undo();
    case.agent = "claude-code".to_string();
    case.source = case.case.project.join(".mcp.json");
    case.write(&json!({"mcpServers": {"tool": {"command": "printf"}}}).to_string());
    let removed = case.remove();
    assert!(ok(&removed), "{removed:?}");
    let outsider = case.case.home.join("unrelated.json");
    fs::write(&outsider, json!({"keep": true}).to_string()).unwrap();
    let before = fs::read(&outsider).unwrap();
    let mut forged = payload_of(&removed);
    let entries = forged.as_object_mut().unwrap();
    entries.insert("path".to_string(), json!(outsider.to_string_lossy()));
    entries.insert("target".to_string(), json!(outsider.to_string_lossy()));
    assert!(!ok(&case.restore(&forged, "")));
    assert_eq!(fs::read(&outsider).unwrap(), before);
    let record_id = case.mint(&forged);
    assert!(!ok(&case.restore(&forged, &record_id)));
    assert_eq!(fs::read(&outsider).unwrap(), before);
}

#[test]
fn a_project_source_restores_without_a_project_argument() {
    let _guard = serial();
    let mut case = undo();
    case.agent = "claude-code".to_string();
    case.source = case.case.project.join(".mcp.json");
    let original = json!({"mcpServers": {"tool": {"command": "printf"}}});
    case.write(&original.to_string());
    let removed = case.remove();
    assert!(ok(&removed), "{removed:?}");
    let elsewhere = Inventory::new(Settings {
        project: case.case.home.clone(),
        ..case.case.settings()
    });
    let record_id = removed
        .get("recordId")
        .and_then(Value::as_str)
        .unwrap()
        .to_string();
    let restored =
        Applier::new(elsewhere).restore(&record_id, Some(&payload_of(&removed).to_string()));
    assert!(ok(&restored), "{restored:?}");
    assert_eq!(case.json(), original);
}

#[test]
fn a_restored_record_stops_holding_a_place_in_the_store() {
    let _guard = serial();
    let mut case = undo();
    case.agent = "claude-code".to_string();
    case.source = case.case.project.join(".mcp.json");
    case.write(&json!({"mcpServers": {"tool": {"command": "printf"}}}).to_string());
    let removed = case.remove();
    assert!(ok(&removed), "{removed:?}");
    let store = case.store();
    assert_eq!(store.live_records().unwrap(), 1);
    let record_id = removed.get("recordId").and_then(Value::as_str).unwrap();
    let restored = case.restore(&payload_of(&removed), record_id);
    assert!(ok(&restored), "{restored:?}");
    assert_eq!(store.live_records().unwrap(), 0);
    let again = case.restore(&payload_of(&removed), record_id);
    assert!(ok(&again), "{again:?}");
    assert_eq!(again.get("changed"), Some(&json!(false)));
}

#[test]
fn the_store_refuses_a_new_removal_instead_of_evicting_undo_records() {
    let _guard = serial();
    let mut case = undo();
    case.agent = "claude-code".to_string();
    case.source = case.case.project.join(".mcp.json");
    case.write(&json!({"mcpServers": {"tool": {"command": "printf"}}}).to_string());
    let applier = Applier::new(case.inventory());
    let context = applier.recovery_context();
    for index in 0..64 {
        applier
            .recovery
            .write(
                &json!({"format": 2, "filler": index}),
                "fixture",
                &context,
                "",
            )
            .unwrap();
    }
    let before = case.bytes();
    let refused = case.remove();
    assert!(!ok(&refused), "{refused:?}");
    assert!(
        refused
            .get("message")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .contains("undo store is full")
    );
    assert_eq!(case.bytes(), before);
}

#[test]
fn preparing_then_removing_reuses_one_record() {
    let _guard = serial();
    let mut case = undo();
    case.agent = "claude-code".to_string();
    case.source = case.case.project.join(".mcp.json");
    case.write(&json!({"mcpServers": {"tool": {"command": "printf"}}}).to_string());
    let prepared = case.prepare(&case.identifier());
    assert!(ok(&prepared), "{prepared:?}");
    let removed = case.remove();
    assert!(ok(&removed), "{removed:?}");
    assert_eq!(prepared.get("recordId"), removed.get("recordId"));
    assert_eq!(case.store().live_records().unwrap(), 1);
}

#[test]
fn restore_needs_a_prepared_record_and_reports_its_identifier() {
    let _guard = serial();
    let mut case = undo();
    case.agent = "claude-code".to_string();
    case.source = case.case.project.join(".mcp.json");
    case.write(&json!({"mcpServers": {"tool": {"command": "printf"}}}).to_string());
    let prepared = case.prepare(&case.identifier());
    assert!(ok(&prepared), "{prepared:?}");
    assert_eq!(text(&prepared, "recordId").len(), 32);
    let removed = case.remove();
    assert!(ok(&removed), "{removed:?}");
    assert_eq!(text(&removed, "recordId").len(), 32);
    let unknown = case.restore(&payload_of(&removed), &"0".repeat(32));
    assert!(ok(&unknown), "{unknown:?}");
    case.store().discard(text(&removed, "recordId")).unwrap();
    case.store().discard(text(&prepared, "recordId")).unwrap();
    let after = case.bytes();
    let refused = case.restore(&payload_of(&removed), "");
    assert!(!ok(&refused), "{refused:?}");
    assert_eq!(case.bytes(), after);
}

#[test]
fn identical_project_aliases_do_not_share_an_actionable_identity() {
    let _guard = serial();
    let mut case = undo();
    case.agent = "claude-code".to_string();
    case.source = case.case.project.join(".mcp.json");
    let original = r#"{"mcpServers":{"tool":{"command":"printf"}}}"#;
    case.write(original);
    let identifier = case.identifier();
    let mut seen = vec![identifier.clone()];
    let spellings: [&std::ffi::OsStr; 3] = [
        std::ffi::OsStr::new("other"),
        std::os::unix::ffi::OsStrExt::from_bytes(b"other-\xff".as_slice()),
        std::ffi::OsStr::new("other-\u{fffd}"),
    ];
    for spelling in spellings {
        let other = case.case.base.join(spelling);
        fs::create_dir_all(&other).unwrap();
        let source = other.join(".mcp.json");
        fs::write(&source, original).unwrap();
        let mut store = Inventory::new(Settings {
            project: other.clone(),
            ..case.case.settings()
        });
        store.scan();
        let row = store
            .definitions
            .iter()
            .find(|row| row.agent == "claude" && row.raw_name == "tool")
            .unwrap();
        assert!(!seen.contains(&row.id), "{:?}", spelling);
        seen.push(row.id.clone());
        let elsewhere = Inventory::new(Settings {
            project: other.clone(),
            ..case.case.settings()
        });
        assert!(!ok(&Applier::new(elsewhere).remove(
            &identifier,
            &Removal {
                prepare: false,
                expected_payload: None,
                transaction_id: "",
            }
        )));
        assert_eq!(fs::read_to_string(&source).unwrap(), original);
    }
    assert_eq!(case.text(), original);
}
