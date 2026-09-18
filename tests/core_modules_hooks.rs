#[path = "common/hook_fixtures.rs"]
mod hook_fixtures;

use fileblade::core_modules::hooks::adapters;
use fileblade::core_modules::hooks::discovery;
use fileblade::core_modules::hooks::events;
use fileblade::core_modules::hooks::labels;
use fileblade::core_modules::hooks::safeio::{self, Budget};
use fileblade::core_modules::watch::WatchPlan;
use hook_fixtures::{
    CANARY_PATH, CANARY_TOKEN, badges, build_all, canary_command, collect_with, environ,
    environ_with, rows, rows_for, source_path, summary, text, write_json, write_text,
};
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;
use std::ffi::OsStr;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::{Mutex, MutexGuard};

static SERIAL: Mutex<()> = Mutex::new(());

fn serial() -> MutexGuard<'static, ()> {
    SERIAL.lock().unwrap_or_else(|error| error.into_inner())
}

struct Sandbox {
    _base: tempfile::TempDir,
    home: PathBuf,
    project: PathBuf,
    etc: PathBuf,
}

fn sandbox() -> Sandbox {
    let base = tempfile::tempdir().unwrap();
    let anchor = fs::canonicalize(base.path()).unwrap();
    let home = anchor.join("home");
    let project = anchor.join("work/repo");
    let etc = anchor.join("etc");
    build_all(&home, &project, Some(&etc));
    Sandbox {
        _base: base,
        home,
        project,
        etc,
    }
}

fn uid() -> u32 {
    rustix::process::geteuid().as_raw()
}

fn payload(sandbox: &Sandbox) -> Map<String, Value> {
    collect_with(
        &sandbox.home,
        &sandbox.project,
        environ(&sandbox.home),
        &sandbox.etc,
        uid(),
        false,
        "all",
    )
    .0
}

fn serialized(document: &Map<String, Value>) -> String {
    serde_json::to_string(&Value::Object(document.clone())).unwrap()
}

#[test]
fn the_schema_names_every_agent_and_its_support_class() {
    let sandbox = sandbox();
    let document = payload(&sandbox);
    assert_eq!(document.get("schemaVersion"), Some(&json!(1)));
    assert_eq!(document.get("ok"), Some(&json!(true)));
    assert_eq!(
        document.get("project").and_then(Value::as_str),
        Some(sandbox.project.to_string_lossy().as_ref())
    );
    let agents = document.get("agents").and_then(Value::as_object).unwrap();
    let names: BTreeSet<&str> = agents.keys().map(String::as_str).collect();
    assert_eq!(
        names,
        BTreeSet::from([
            "claude-code",
            "codex",
            "copilot-cli",
            "antigravity",
            "opencode",
            "pi"
        ])
    );
    assert_eq!(agents.get("opencode"), Some(&json!("code-hosted")));
    assert_eq!(agents.get("pi"), Some(&json!("code-hosted")));
}

#[test]
fn metrics_are_bounded_unicode_aware_and_dated() {
    let sandbox = sandbox();
    let source = sandbox
        .home
        .join("atypical/\u{6302}\u{94a9}-\u{d6c4}\u{d06c}.json");
    let text_value = "\u{732b} \u{3ba}\u{3cd}\u{3bc}\u{3b1} \u{41f}\u{440}\u{438} \u{d55c}\u{ae00}";
    write_text(&source, text_value);
    let mut budget = Budget::default();
    let metrics = safeio::artifact_metrics(&mut budget, &source, text_value);
    assert_eq!(metrics.get("bytes"), Some(&json!(text_value.len())));
    assert_eq!(
        metrics.get("characters"),
        Some(&json!(text_value.chars().count()))
    );
    assert_eq!(metrics.get("words"), Some(&json!(4)));
    assert_eq!(
        metrics.get("tokens"),
        Some(&json!(text_value.len().div_ceil(4)))
    );
    assert_eq!(
        metrics
            .get("updated")
            .and_then(Value::as_str)
            .unwrap()
            .len(),
        16
    );

    let oversized = "\u{732b}".repeat(safeio::MAX_FILE_BYTES + 1);
    let metrics = safeio::artifact_metrics(&mut budget, &source, &oversized);
    assert!(metrics.get("bytes").and_then(Value::as_u64).unwrap() > safeio::MAX_FILE_BYTES as u64);
    for key in ["characters", "words", "tokens"] {
        assert_eq!(metrics.get(key), Some(&Value::Null));
    }
    let missing =
        safeio::artifact_metrics(&mut budget, &source.with_file_name("gone.json"), text_value);
    assert!(missing.is_empty());
}

#[test]
fn discovered_metrics_count_the_redacted_payload_not_the_source() {
    let sandbox = sandbox();
    let document = payload(&sandbox);
    let row = rows(&document)
        .into_iter()
        .find(|row| {
            text(row, "supportStatus") != "code-hosted"
                && summary(row, "payloadBytes").as_u64().unwrap_or_default() > 0
        })
        .expect("a payload-bearing row");
    let metrics = row.get("metrics").and_then(Value::as_object).unwrap();
    assert_eq!(metrics.get("bytes"), Some(&summary(&row, "payloadBytes")));
    for key in ["characters", "words", "tokens"] {
        assert!(metrics.get(key).and_then(Value::as_u64).unwrap_or_default() >= 1);
    }
    assert_eq!(
        metrics
            .get("updated")
            .and_then(Value::as_str)
            .unwrap()
            .len(),
        16
    );
}

#[test]
fn every_documented_agent_produces_rows() {
    let sandbox = sandbox();
    let document = payload(&sandbox);
    for agent in ["claude-code", "codex", "copilot-cli", "antigravity"] {
        let found = rows_for(&document, agent);
        assert!(!found.is_empty(), "{agent}");
        assert!(found.iter().all(|row| {
            matches!(
                text(row, "supportStatus").as_str(),
                "documented" | "undocumented-event"
            )
        }));
    }
}

#[test]
fn an_exact_root_skips_the_project_walk() {
    let sandbox = sandbox();
    let nested = sandbox.project.join("packages/web");
    fs::create_dir_all(&nested).unwrap();
    let walked = collect_with(
        &sandbox.home,
        &nested,
        environ(&sandbox.home),
        &sandbox.etc,
        uid(),
        false,
        "all",
    )
    .0;
    let exact = collect_with(
        &sandbox.home,
        &nested,
        environ(&sandbox.home),
        &sandbox.etc,
        uid(),
        true,
        "all",
    )
    .0;
    assert_eq!(
        walked.get("project").and_then(Value::as_str),
        Some(sandbox.project.to_string_lossy().as_ref())
    );
    assert_eq!(
        exact.get("project").and_then(Value::as_str),
        Some(nested.to_string_lossy().as_ref())
    );
}

#[test]
fn codex_root_events_and_wrapped_events_both_list() {
    let sandbox = sandbox();
    let document = payload(&sandbox);
    let codex = rows_for(&document, "codex");
    assert!(
        codex
            .iter()
            .any(|row| text(row, "event") == "Stop" && text(row, "scope") == "user")
    );
    assert!(
        codex
            .iter()
            .any(|row| text(row, "event") == "SubagentStop" && text(row, "scope") == "project")
    );
}

#[test]
fn scope_lanes_split_user_and_project_sources_and_watches() {
    let sandbox = sandbox();
    let mut lanes: Vec<(String, Map<String, Value>, Vec<PathBuf>)> = Vec::new();
    for scope in ["user", "project"] {
        let mut budget = Budget::default();
        let mut document = discovery::collect(
            &mut budget,
            &discovery::Query {
                project: &sandbox.project.to_string_lossy(),
                home: &sandbox.home.to_string_lossy(),
                etc_root: &sandbox.etc.to_string_lossy(),
                policy_owner_uid: uid(),
                exact: false,
                scope,
            },
            environ(&sandbox.home),
        );
        let mut plan: WatchPlan = std::mem::take(&mut budget.plan);
        let watched = plan.paths();
        plan.finish(&mut document);
        lanes.push((scope.to_string(), document, watched));
    }
    let both = payload(&sandbox);
    let user = &lanes[0].1;
    let project = &lanes[1].1;
    let user_scopes: BTreeSet<String> = rows(user).iter().map(|row| text(row, "scope")).collect();
    assert!(!user_scopes.contains("project") && !user_scopes.contains("local"));
    let project_scopes: BTreeSet<String> =
        rows(project).iter().map(|row| text(row, "scope")).collect();
    assert_eq!(
        project_scopes,
        BTreeSet::from(["project".to_string(), "local".to_string()])
    );
    assert_eq!(user.get("project"), Some(&json!("")));
    assert_eq!(
        project.get("project").and_then(Value::as_str),
        Some(sandbox.project.to_string_lossy().as_ref())
    );
    assert_eq!(rows(&both).len(), rows(user).len() + rows(project).len());
    assert!(
        !lanes[0]
            .2
            .iter()
            .any(|path| path.starts_with(&sandbox.project)),
        "user lane watches nothing under the project"
    );
    let user_roots = [
        ".copilot",
        ".pi",
        ".claude/plugins",
        ".config/opencode",
        ".gemini/config",
    ];
    assert!(!lanes[1].2.iter().any(|path| {
        user_roots
            .iter()
            .any(|part| path.starts_with(sandbox.home.join(part)))
    }));
}

#[test]
fn claude_scopes_and_undocumented_events_are_listed() {
    let sandbox = sandbox();
    let document = payload(&sandbox);
    let claude = rows_for(&document, "claude-code");
    let scopes: BTreeSet<String> = claude.iter().map(|row| text(row, "scope")).collect();
    for wanted in ["user", "project", "local", "plugin"] {
        assert!(scopes.contains(wanted), "{wanted} missing from {scopes:?}");
    }
    assert!(claude.iter().any(|row| text(row, "event") == "PreToolUse"));
    assert!(claude.iter().any(|row| {
        text(row, "supportStatus") == "undocumented-event" && text(row, "event") == "NotARealEvent"
    }));
}

#[test]
fn enabled_and_disabled_states_are_reported() {
    let sandbox = sandbox();
    let document = payload(&sandbox);
    let antigravity = rows_for(&document, "antigravity");
    assert!(
        antigravity
            .iter()
            .any(|row| row.get("enabled") == Some(&json!(true)))
    );
    assert!(
        antigravity
            .iter()
            .any(|row| row.get("enabled") == Some(&json!(false)))
    );
    let copilot = rows_for(&document, "copilot-cli");
    assert!(copilot.iter().any(|row| {
        row.get("enabled") == Some(&json!(false))
            && badges(row).contains(&"file-disabled".to_string())
    }));
    assert!(
        copilot
            .iter()
            .any(|row| row.get("enabled") == Some(&Value::Null))
    );
}

#[test]
fn codex_lists_documented_sources_with_the_feature_flag_honoured() {
    let sandbox = sandbox();
    let document = payload(&sandbox);
    let codex = rows_for(&document, "codex");
    assert!(!codex.is_empty());
    let events: BTreeSet<String> = codex.iter().map(|row| text(row, "event")).collect();
    for wanted in ["Stop", "PreCompact", "SubagentStop"] {
        assert!(events.contains(wanted), "{wanted} missing");
    }
    assert!(
        codex
            .iter()
            .all(|row| text(row, "supportStatus") == "documented")
    );
    assert!(
        !codex
            .iter()
            .any(|row| badges(row).contains(&"feature-off".to_string()))
    );

    let config = sandbox.home.join(".codex/config.toml");
    let text_value = fs::read_to_string(&config).unwrap();
    fs::write(&config, text_value.replace("hooks = true", "hooks = false")).unwrap();
    let document = payload(&sandbox);
    let active: Vec<Map<String, Value>> = rows_for(&document, "codex")
        .into_iter()
        .filter(|row| text(row, "scope") != "profile")
        .collect();
    assert!(!active.is_empty());
    assert!(
        active
            .iter()
            .all(|row| badges(row).contains(&"feature-off".to_string()))
    );
    let profiles: Vec<Map<String, Value>> = rows_for(&document, "codex")
        .into_iter()
        .filter(|row| text(row, "scope") == "profile")
        .collect();
    assert!(!profiles.is_empty());
    assert!(
        profiles
            .iter()
            .all(|row| !badges(row).contains(&"feature-off".to_string()))
    );
    assert!(
        profiles
            .iter()
            .all(|row| row.get("enabled") == Some(&Value::Null))
    );
    assert!(
        profiles
            .iter()
            .all(|row| text(row, "note") == "applies only when codex runs with --profile")
    );
}

#[test]
fn the_documented_event_tables_keep_their_shape() {
    assert!(adapters::CLAUDE_EVENTS.contains(&"InstructionsLoaded"));
    assert_eq!(adapters::CLAUDE_EVENTS.len(), 33);
    assert_eq!(adapters::CODEX_EVENTS.len(), 12);
    assert!(adapters::CODEX_EVENTS.contains(&"Interrupt"));
    assert_eq!(adapters::COPILOT_EVENTS.len(), 14);
    assert_eq!(events::COPILOT_PASCAL_ALIASES.len(), 12);
    assert!(
        events::COPILOT_PASCAL_ALIASES
            .iter()
            .all(|(_, native)| adapters::COPILOT_EVENTS.contains(native))
    );
    assert_eq!(events::CANONICAL_EVENTS.len(), 13);
}

#[test]
fn unsupported_locations_and_rejected_versions_never_reach_the_payload() {
    let sandbox = sandbox();
    let document = payload(&sandbox);
    let blob = serialized(&document);
    for forbidden in [
        "REJECTED-BAD-VERSION",
        "POLICY-BAD-VERSION",
        "POLICY-GROUP-WRITABLE",
        "antigravity-cli",
        "UNDOCUMENTED-PLUGIN-PATH",
        ".cursor",
        "credentials",
        ".netrc",
    ] {
        assert!(!blob.contains(forbidden), "{forbidden} leaked");
    }
    assert!(
        rows_for(&document, "copilot-cli")
            .iter()
            .any(|row| text(row, "event") == "subagentStart")
    );
}

#[test]
fn the_payload_is_bounded_to_one_mebibyte() {
    let sandbox = sandbox();
    let document = payload(&sandbox);
    let template = rows(&document).first().cloned().unwrap();
    let mut bulk = document.clone();
    let items: Vec<Value> = (0..6000)
        .map(|index| {
            let mut row = template.clone();
            row.insert("id".to_string(), json!(format!("row-{index:06}")));
            row.insert("note".to_string(), json!("N".repeat(400)));
            Value::Object(row)
        })
        .collect();
    bulk.insert("count".to_string(), json!(items.len()));
    bulk.insert("truncated".to_string(), json!(false));
    bulk.insert("items".to_string(), Value::Array(items));
    assert!(serialized(&bulk).len() > safeio::MAX_STDOUT_BYTES);
    let encoded = fileblade::core_modules::emit::encoded_items(&bulk);
    assert!(encoded.len() <= safeio::MAX_STDOUT_BYTES);
    let reparsed: Value = serde_json::from_slice(&encoded).unwrap();
    assert_eq!(reparsed.get("truncated"), Some(&json!(true)));
    let count = reparsed.get("count").and_then(Value::as_u64).unwrap();
    assert!(count > 0 && count < 6000);
    assert_eq!(
        count as usize,
        reparsed
            .get("items")
            .and_then(Value::as_array)
            .unwrap()
            .len()
    );
    let small = fileblade::core_modules::emit::encoded_items(&document);
    let reparsed: Value = serde_json::from_slice(&small).unwrap();
    assert_eq!(reparsed.get("count"), document.get("count"));
}

#[test]
fn code_hosted_agents_are_listed_as_locations_only() {
    let sandbox = sandbox();
    let document = payload(&sandbox);
    for agent in ["opencode", "pi"] {
        let found = rows_for(&document, agent);
        assert!(!found.is_empty(), "{agent}");
        for row in found {
            assert_eq!(text(&row, "supportStatus"), "code-hosted");
            assert_eq!(text(&row, "event"), "");
            assert_eq!(summary(&row, "type"), json!("code"));
            assert_eq!(summary(&row, "digest"), json!(""));
            assert!(badges(&row).contains(&"not-inspected".to_string()));
            assert!(row.get("entries").and_then(Value::as_u64).unwrap() >= 1);
        }
    }
}

#[test]
fn row_identities_are_stable_and_distinct() {
    let sandbox = sandbox();
    let first: Vec<String> = rows(&payload(&sandbox))
        .iter()
        .map(|row| text(row, "id"))
        .collect();
    let second: Vec<String> = rows(&payload(&sandbox))
        .iter()
        .map(|row| text(row, "id"))
        .collect();
    assert_eq!(first, second);
    let unique: BTreeSet<&String> = first.iter().collect();
    assert_eq!(unique.len(), first.len());
}

#[test]
fn a_symlinked_config_reports_its_target() {
    let sandbox = sandbox();
    let target = write_json(
        &sandbox.home.join("elsewhere/hooks.json"),
        &json!({"hooks": {"Stop": [{"hooks": [{"type": "command", "command": "linked"}]}]}}),
    );
    let real = sandbox.home.join(".codex/hooks.json");
    fs::remove_file(&real).unwrap();
    std::os::unix::fs::symlink(&target, &real).unwrap();
    let document = payload(&sandbox);
    let linked: Vec<Map<String, Value>> = rows_for(&document, "codex")
        .into_iter()
        .filter(|row| source_path(row) == real.to_string_lossy())
        .collect();
    assert!(!linked.is_empty());
    assert_eq!(text(&linked[0], "event"), "Stop");
    assert_eq!(
        linked[0]
            .get("source")
            .and_then(Value::as_object)
            .and_then(|source| source.get("realpath"))
            .and_then(Value::as_str),
        Some(target.to_string_lossy().as_ref())
    );
}

#[test]
fn oversized_and_overdeep_documents_are_refused() {
    let sandbox = sandbox();
    let oversize = sandbox.home.join(".claude/big.json");
    let mut data = vec![b'{'];
    data.extend(std::iter::repeat_n(b' ', safeio::MAX_FILE_BYTES + 10));
    data.push(b'}');
    fs::write(&oversize, &data).unwrap();
    let mut budget = Budget::default();
    assert!(safeio::read_bytes(&mut budget, &oversize, None).is_none());
    let mut deep = json!("leaf");
    for _ in 0..(safeio::MAX_JSON_DEPTH + 3) {
        deep = json!({"nested": deep});
    }
    assert!(!safeio::bounded_depth(&deep));
}

#[test]
fn irregular_and_malformed_files_do_not_break_the_scan() {
    let sandbox = sandbox();
    let fifo = sandbox.home.join(".copilot/hooks/pipe.json");
    let name = std::ffi::CString::new(fifo.to_string_lossy().as_bytes()).unwrap();
    assert_eq!(unsafe { libc::mkfifo(name.as_ptr(), 0o600) }, 0);
    let broken = sandbox.home.join(".copilot/hooks/broken.json");
    fs::write(&broken, "{ not json at all").unwrap();
    let document = payload(&sandbox);
    assert!(
        !rows(&document)
            .iter()
            .any(|row| source_path(row).ends_with("pipe.json"))
    );
    assert_eq!(document.get("ok"), Some(&json!(true)));
    assert!(document.get("count").and_then(Value::as_u64).unwrap() > 0);
}

#[test]
fn a_missing_home_lists_nothing() {
    let base = tempfile::tempdir().unwrap();
    let home = base.path().join("nothing");
    let mut budget = Budget::default();
    let document = discovery::collect(
        &mut budget,
        &discovery::Query::new("", &home.to_string_lossy()),
        Vec::new(),
    );
    assert_eq!(document.get("ok"), Some(&json!(true)));
    assert_eq!(document.get("count"), Some(&json!(0)));
}

#[test]
fn copilot_policy_files_are_ordered_owner_checked_and_gated() {
    let sandbox = sandbox();
    let document = payload(&sandbox);
    let policy: Vec<Map<String, Value>> = rows_for(&document, "copilot-cli")
        .into_iter()
        .filter(|row| text(row, "scope") == "policy")
        .collect();
    let events: BTreeSet<String> = policy.iter().map(|row| text(row, "event")).collect();
    assert_eq!(
        events,
        BTreeSet::from(["sessionStart".to_string(), "preToolUse".to_string()])
    );
    let order: Vec<String> = policy
        .iter()
        .map(|row| {
            Path::new(&source_path(row))
                .file_name()
                .unwrap()
                .to_string_lossy()
                .to_string()
        })
        .collect();
    assert!(order.contains(&"10-first.json".to_string()));
    assert!(
        policy
            .iter()
            .all(|row| badges(row).contains(&"policy".to_string()))
    );

    let wrong_owner = collect_with(
        &sandbox.home,
        &sandbox.project,
        environ(&sandbox.home),
        &sandbox.etc,
        uid() + 1,
        false,
        "all",
    )
    .0;
    assert!(
        !rows_for(&wrong_owner, "copilot-cli")
            .iter()
            .any(|row| text(row, "scope") == "policy")
    );
}

#[test]
fn copilot_files_settings_and_plugins_follow_their_own_gates() {
    let sandbox = sandbox();
    let document = payload(&sandbox);
    let copilot = rows_for(&document, "copilot-cli");
    let user: Vec<Map<String, Value>> = copilot
        .iter()
        .filter(|row| {
            text(row, "scope") == "user" && !badges(row).contains(&"settings-inline".to_string())
        })
        .cloned()
        .collect();
    assert!(user.iter().any(|row| text(row, "event") == "preToolUse"));
    let off: Vec<&Map<String, Value>> = user
        .iter()
        .filter(|row| badges(row).contains(&"file-disabled".to_string()))
        .collect();
    assert!(!off.is_empty());
    assert!(
        off.iter()
            .all(|row| row.get("enabled") == Some(&json!(false)))
    );
    assert!(
        user.iter()
            .any(|row| row.get("enabled") != Some(&json!(false)))
    );

    let inline: Vec<&Map<String, Value>> = copilot
        .iter()
        .filter(|row| badges(row).contains(&"settings-inline".to_string()))
        .collect();
    let events: BTreeSet<String> = inline.iter().map(|row| text(row, "event")).collect();
    for wanted in ["userPromptSubmitted", "subagentStart", "PreToolUse"] {
        assert!(events.contains(wanted), "{wanted} missing from {events:?}");
    }
    assert!(
        inline
            .iter()
            .all(|row| text(row, "supportStatus") == "documented")
    );
    let shared: BTreeSet<String> = inline
        .iter()
        .filter(|row| source_path(row).contains("/.claude/"))
        .map(|row| text(row, "event"))
        .collect();
    for wanted in ["Stop", "PostToolUse"] {
        assert!(shared.contains(wanted), "{wanted} missing from {shared:?}");
    }

    let plugins: Vec<&Map<String, Value>> = copilot
        .iter()
        .filter(|row| text(row, "scope") == "plugin")
        .collect();
    let names: BTreeSet<String> = plugins.iter().map(|row| text(row, "plugin")).collect();
    assert_eq!(
        names,
        BTreeSet::from(["guard-plugin".to_string(), "direct-plugin".to_string()])
    );
    assert!(
        plugins
            .iter()
            .all(|row| row.get("enabled") == Some(&Value::Null))
    );
    assert!(
        plugins
            .iter()
            .all(|row| badges(row).contains(&"enable-state-undocumented".to_string()))
    );

    let aliased: Vec<&Map<String, Value>> = copilot
        .iter()
        .filter(|row| badges(row).contains(&"vscode-alias".to_string()))
        .collect();
    assert!(!aliased.is_empty());
    for row in aliased {
        assert_eq!(text(row, "supportStatus"), "documented");
        assert!(adapters::COPILOT_EVENTS.contains(&text(row, "canonicalEvent").as_str()));
    }
}

#[test]
fn a_repository_disable_all_hooks_spares_policy_rows() {
    let sandbox = sandbox();
    write_json(
        &sandbox.project.join(".github/copilot/settings.json"),
        &json!({"disableAllHooks": true}),
    );
    let document = payload(&sandbox);
    let copilot = rows_for(&document, "copilot-cli");
    let policy: Vec<&Map<String, Value>> = copilot
        .iter()
        .filter(|row| text(row, "scope") == "policy")
        .collect();
    let others: Vec<&Map<String, Value>> = copilot
        .iter()
        .filter(|row| text(row, "scope") != "policy")
        .collect();
    assert!(!policy.is_empty());
    assert!(
        policy
            .iter()
            .all(|row| row.get("enabled") != Some(&json!(false)))
    );
    assert!(!others.is_empty());
    assert!(
        others
            .iter()
            .all(|row| row.get("enabled") == Some(&json!(false)))
    );
    assert!(
        others
            .iter()
            .all(|row| badges(row).contains(&"repository-disabled".to_string()))
    );
}

#[test]
fn codex_profiles_are_never_active_and_config_toml_is_not_one() {
    let sandbox = sandbox();
    let document = payload(&sandbox);
    let profiles: Vec<Map<String, Value>> = rows_for(&document, "codex")
        .into_iter()
        .filter(|row| text(row, "scope") == "profile")
        .collect();
    let names: BTreeSet<String> = profiles.iter().map(|row| text(row, "profile")).collect();
    assert_eq!(
        names,
        BTreeSet::from([
            "t\u{fb}rk\u{e7}e-pr\u{f6}fil".to_string(),
            "plain".to_string()
        ])
    );
    assert!(
        profiles
            .iter()
            .all(|row| row.get("enabled") == Some(&Value::Null))
    );
    assert!(
        profiles
            .iter()
            .all(|row| badges(row).contains(&"profile-unselected".to_string()))
    );
    let unicode_row = profiles
        .iter()
        .find(|row| text(row, "profile") == "t\u{fb}rk\u{e7}e-pr\u{f6}fil")
        .unwrap();
    assert!(source_path(unicode_row).contains("t\u{fb}rk\u{e7}e-pr\u{f6}fil.config.toml"));
    assert_eq!(
        unicode_row
            .get("source")
            .and_then(Value::as_object)
            .and_then(|source| source.get("name"))
            .and_then(Value::as_str),
        Some("t\u{fb}rk\u{e7}e-pr\u{f6}fil.config.toml")
    );
    let mut budget = Budget::default();
    let mut discovered: Vec<String> =
        adapters::codex_profile_paths(&mut budget, &sandbox.home.join(".codex"))
            .into_iter()
            .map(|path| path.file_name().unwrap().to_string_lossy().to_string())
            .collect();
    discovered.sort();
    assert_eq!(
        discovered,
        vec![
            "plain.config.toml".to_string(),
            "t\u{fb}rk\u{e7}e-pr\u{f6}fil.config.toml".to_string()
        ]
    );
}

#[test]
fn antigravity_grouped_shapes_carry_their_group_and_state() {
    let sandbox = sandbox();
    write_json(
        &sandbox.home.join(".gemini/config/hooks.json"),
        &json!({
            "my-linter-hook": {"PostToolUse": [{"matcher": "run_command",
                "hooks": [{"type": "command", "command": "./lint.sh", "timeout": 10}]}]},
            "safety-gate": {"enabled": false, "PreToolUse": [{"matcher": "run_command",
                "hooks": [{"command": "./safety.sh"}]}]},
        }),
    );
    let document = payload(&sandbox);
    let found: Vec<Map<String, Value>> = rows_for(&document, "antigravity")
        .into_iter()
        .filter(|row| text(row, "scope") == "user")
        .collect();
    let groups: BTreeSet<String> = found.iter().map(|row| text(row, "group")).collect();
    assert_eq!(
        groups,
        BTreeSet::from(["my-linter-hook".to_string(), "safety-gate".to_string()])
    );
    let linter = found
        .iter()
        .find(|row| text(row, "group") == "my-linter-hook")
        .unwrap();
    assert_eq!(summary(linter, "matcher"), json!("literal"));
    assert_eq!(summary(linter, "timeoutSeconds"), json!(10));
    let gate = found
        .iter()
        .find(|row| text(row, "group") == "safety-gate")
        .unwrap();
    assert_eq!(gate.get("enabled"), Some(&json!(false)));
    let unique: BTreeSet<String> = found.iter().map(|row| text(row, "id")).collect();
    assert_eq!(unique.len(), found.len());
}

#[test]
fn labels_come_from_status_message_then_script_basename() {
    let sandbox = sandbox();
    let settings = sandbox.home.join(".claude/settings.json");
    let mut document: Value = serde_json::from_slice(&fs::read(&settings).unwrap()).unwrap();
    document["hooks"]["PreCompact"] = json!([{"hooks": [
        {"type": "command", "command": "/home/someone/.claude/hooks/tidy-context.sh --fast",
         "statusMessage": "  Tidying   context "},
        {"type": "command", "command": "~/.claude/hooks/guard-secrets.py"},
        {"type": "command", "command": "bun test"},
    ]}]);
    fs::write(&settings, serde_json::to_string(&document).unwrap()).unwrap();
    let listed = payload(&sandbox);
    let found: Vec<Map<String, Value>> = rows_for(&listed, "claude-code")
        .into_iter()
        .filter(|row| text(row, "event") == "PreCompact")
        .collect();
    let labels: BTreeSet<(String, String)> = found
        .iter()
        .map(|row| {
            (
                summary(row, "label")
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
                summary(row, "labelSource")
                    .as_str()
                    .unwrap_or_default()
                    .to_string(),
            )
        })
        .collect();
    assert_eq!(
        labels,
        BTreeSet::from([
            ("Tidying context".to_string(), "statusMessage".to_string()),
            ("guard-secrets.py".to_string(), "command".to_string()),
            (String::new(), String::new()),
        ])
    );
    let blob = serde_json::to_string(&found).unwrap();
    for forbidden in ["/home/someone", "--fast", "bun test"] {
        assert!(!blob.contains(forbidden), "{forbidden} leaked");
    }
}

#[test]
fn user_labels_override_and_clear() {
    let _guard = serial();
    let sandbox = sandbox();
    let config = sandbox.home.join(".config");
    let variables = environ_with(&sandbox.home, &[("XDG_CONFIG_HOME", OsStr::new(&config))]);
    let listed = collect_with(
        &sandbox.home,
        &sandbox.project,
        variables.clone(),
        &sandbox.etc,
        uid(),
        false,
        "all",
    )
    .0;
    let row = rows_for(&listed, "claude-code").first().cloned().unwrap();
    let digest = summary(&row, "digest").as_str().unwrap().to_string();
    let result = labels::set_label(
        &sandbox.home,
        &variables,
        &digest,
        "  Block dangerous  shell ",
    );
    assert_eq!(result.get("ok"), Some(&json!(true)), "{result}");
    assert_eq!(result.get("changed"), Some(&json!(true)));
    assert_eq!(result.get("label"), Some(&json!("Block dangerous shell")));
    assert_eq!(
        result.get("touched"),
        Some(&json!([config
            .join("omarchy/fileblade/hooks/labels.json")
            .to_string_lossy()]))
    );
    let relisted = collect_with(
        &sandbox.home,
        &sandbox.project,
        variables.clone(),
        &sandbox.etc,
        uid(),
        false,
        "all",
    )
    .0;
    let again = rows(&relisted)
        .into_iter()
        .find(|item| text(item, "id") == text(&row, "id"))
        .unwrap();
    assert_eq!(summary(&again, "label"), json!("Block dangerous shell"));
    assert_eq!(summary(&again, "labelSource"), json!("user"));
    let repeat = labels::set_label(&sandbox.home, &variables, &digest, "Block dangerous shell");
    assert_eq!(repeat.get("changed"), Some(&json!(false)));
    assert_eq!(repeat.get("touched"), Some(&json!([])));
    assert_eq!(
        labels::set_label(&sandbox.home, &variables, "not-hex!", "x").get("ok"),
        Some(&json!(false))
    );
    let cleared = labels::set_label(&sandbox.home, &variables, &digest, "");
    assert_eq!(cleared.get("ok"), Some(&json!(true)));
    assert_eq!(cleared.get("changed"), Some(&json!(true)));
    let restored = collect_with(
        &sandbox.home,
        &sandbox.project,
        variables,
        &sandbox.etc,
        uid(),
        false,
        "all",
    )
    .0;
    let final_row = rows(&restored)
        .into_iter()
        .find(|item| text(item, "id") == text(&row, "id"))
        .unwrap();
    assert_ne!(summary(&final_row, "labelSource"), json!("user"));
}

#[test]
fn missing_sources_and_nested_code_directories_are_watched() {
    let base = tempfile::tempdir().unwrap();
    let anchor = fs::canonicalize(base.path()).unwrap();
    let home = anchor.join("home");
    let project = anchor.join("project");
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&project).unwrap();
    let scan = |home: &Path, project: &Path| {
        let mut budget = Budget::default();
        let document = discovery::collect(
            &mut budget,
            &discovery::Query {
                project: &project.to_string_lossy(),
                home: &home.to_string_lossy(),
                etc_root: &anchor.join("etc").to_string_lossy(),
                policy_owner_uid: 0,
                exact: true,
                scope: "all",
            },
            Vec::new(),
        );
        let paths = budget.plan.paths();
        (document, paths)
    };
    let (_, paths) = scan(&home, &project);
    assert!(paths.contains(&project));
    assert!(paths.contains(&home));
    let source = project.join(".pi/extensions/nested");
    fs::create_dir_all(&source).unwrap();
    fs::write(source.join("hook.ts"), "SHOULD_NOT_BE_EXECUTED").unwrap();
    let (document, paths) = scan(&home, &project);
    assert!(paths.contains(&source));
    assert!(paths.contains(&source.parent().unwrap().to_path_buf()));
    assert!(!serialized(&document).contains("SHOULD_NOT_BE_EXECUTED"));
    assert!(rows(&document).iter().any(|row| text(row, "agent") == "pi"));

    let settings = project.join(".claude/settings.json");
    let target = anchor.join("vault/settings.json");
    fs::create_dir_all(settings.parent().unwrap()).unwrap();
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::write(
        &target,
        "{\"hooks\":{\"Stop\":[{\"command\":\"private-command\"}]}}",
    )
    .unwrap();
    std::os::unix::fs::symlink(&target, &settings).unwrap();
    let (document, paths) = scan(&home, &project);
    assert!(paths.contains(&settings.parent().unwrap().to_path_buf()));
    assert!(paths.contains(&target.parent().unwrap().to_path_buf()));
    assert!(!rows(&document).is_empty());
    assert!(!serialized(&document).contains("private-command"));
}

#[test]
fn an_overdeep_source_is_skipped_without_abandoning_other_agents() {
    let base = tempfile::tempdir().unwrap();
    let anchor = fs::canonicalize(base.path()).unwrap();
    let home = anchor.join("home");
    let project = anchor.join("project");
    fs::create_dir_all(&home).unwrap();
    let source = project.join(".claude/settings.json");
    write_text(
        &source,
        &format!("{{\"hooks\":{}0{}}}", "[".repeat(2000), "]".repeat(2000)),
    );
    let other = project.join(".codex/hooks.json");
    write_text(
        &other,
        "{\"hooks\":{\"Stop\":[{\"command\":\"private-command\"}]}}",
    );
    let mut budget = Budget::default();
    let document = discovery::collect(
        &mut budget,
        &discovery::Query {
            project: &project.to_string_lossy(),
            home: &home.to_string_lossy(),
            etc_root: &anchor.join("etc").to_string_lossy(),
            policy_owner_uid: 0,
            exact: true,
            scope: "all",
        },
        Vec::new(),
    );
    assert!(
        rows(&document)
            .iter()
            .any(|row| text(row, "agent") == "codex")
    );
    assert!(
        budget
            .plan
            .paths()
            .contains(&source.parent().unwrap().to_path_buf())
    );

    let mut budget = Budget::default();
    let deep_json = project.join("deep.json");
    write_text(
        &deep_json,
        &format!("{{\"value\":{}0{}}}", "[".repeat(2000), "]".repeat(2000)),
    );
    assert!(safeio::load_json(&mut budget, &deep_json, false, None).is_none());
    let deep_toml = project.join("deep.toml");
    write_text(
        &deep_toml,
        &format!("value = {}0{}", "[".repeat(2000), "]".repeat(2000)),
    );
    assert!(safeio::load_toml(&mut budget, &deep_toml).is_none());
}

#[test]
fn the_canary_command_never_appears_in_a_summary() {
    let sandbox = sandbox();
    let document = payload(&sandbox);
    let blob = serialized(&document);
    assert!(!blob.contains(&canary_command()));
    assert!(!blob.contains(CANARY_TOKEN));
    assert!(!blob.contains(CANARY_PATH));
}
