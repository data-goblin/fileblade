use super::discovery;
use super::events::mapped_event;
use super::labels::{Environ, environ_value};
use super::records;
use super::redaction::{payload_digest, safe_type};
use super::safeio::{
    Budget, MAX_FILE_BYTES, bounded_depth, document_kind, expanded, expanded_os, refuse_update,
};
use crate::common::{parse_path, path_text};
use crate::core_modules::canonical::UniqueValue;
use crate::core_modules::recovery_store::{RecoveryError, RecoveryStore};
use crate::core_modules::snapshot::Snapshot;
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: i64 = 1;
pub const MAX_RESTORE_PAYLOAD_BYTES: usize = 1024 * 1024;
pub const WRITER_AGENTS: [&str; 4] = ["claude-code", "codex", "copilot-cli", "antigravity"];
pub const CODE_HOSTED_AGENTS: [&str; 2] = ["opencode", "pi"];

pub fn recovery_store(home: &Path, environ: &Environ) -> RecoveryStore {
    let configured = environ_value(environ, "XDG_STATE_HOME");
    let base = if configured.is_empty() {
        home.join(".local").join("state")
    } else {
        expanded_os(&configured)
    };
    RecoveryStore::new(base.join("fileblade").join("hooks-recovery"))
}

pub fn target_path(agent: &str, home: &Path, environ: &Environ) -> PathBuf {
    match agent {
        "claude-code" => home.join(".claude").join("settings.json"),
        "codex" => {
            let override_value = environ_value(environ, "CODEX_HOME");
            let base = if override_value.is_empty() {
                home.join(".codex")
            } else {
                expanded_os(&override_value)
            };
            base.join("hooks.json")
        }
        "copilot-cli" => {
            let override_value = environ_value(environ, "COPILOT_HOME");
            let base = if override_value.is_empty() {
                home.join(".copilot")
            } else {
                expanded_os(&override_value)
            };
            base.join("hooks").join("hooks.json")
        }
        _ => home.join(".gemini").join("config").join("hooks.json"),
    }
}

pub struct Loaded {
    pub document: Map<String, Value>,
    pub snapshot: Snapshot,
}

pub fn load_target(path: &Path) -> Result<Loaded, String> {
    let snapshot = match Snapshot::read(path, MAX_FILE_BYTES) {
        Ok(snapshot) => snapshot,
        Err(error) => return Err(format!("cannot read {}: {error}", path.display())),
    };
    let Some(raw) = snapshot.data.clone() else {
        return Ok(Loaded {
            document: Map::new(),
            snapshot,
        });
    };
    let Ok(text) = std::str::from_utf8(&raw) else {
        return Err(format!(
            "{} is not strict JSON (comments or trailing commas are not rewritten)",
            path.display()
        ));
    };
    let Ok(UniqueValue(parsed)) = serde_json::from_str::<UniqueValue>(text) else {
        return Err(format!(
            "{} is not strict JSON (comments or trailing commas are not rewritten)",
            path.display()
        ));
    };
    let Some(document) = parsed.as_object() else {
        return Err(format!("{} does not hold a JSON object", path.display()));
    };
    if !bounded_depth(&parsed) {
        return Err(format!("{} exceeds the JSON nesting limit", path.display()));
    }
    Ok(Loaded {
        document: document.clone(),
        snapshot,
    })
}

fn rendered(document: &Map<String, Value>) -> Result<Vec<u8>, String> {
    let mut text = serde_json::to_string_pretty(&Value::Object(document.clone()))
        .map_err(|_| "the updated hook file cannot be represented as UTF-8 JSON".to_string())?;
    text.push('\n');
    Ok(text.into_bytes())
}

pub fn write_snapshot(
    snapshot: &Snapshot,
    document: &Map<String, Value>,
    removals: &[String],
) -> Result<(), String> {
    let kind = document_kind(&[&snapshot.logical, &snapshot.resolved]);
    if kind != "json" {
        return Err(
            "TOML hook files are never rewritten as JSON; edit the source directly".to_string(),
        );
    }
    let payload = rendered(document)?;
    let reason = refuse_update(snapshot.data.as_deref(), &payload, kind, removals);
    if !reason.is_empty() {
        return Err(reason);
    }
    snapshot.write(&payload).map_err(|error| error.to_string())
}

pub fn write_path(
    path: &Path,
    document: &Map<String, Value>,
    removals: &[String],
) -> Result<(), String> {
    let snapshot = Snapshot::read(path, MAX_FILE_BYTES).map_err(|error| error.to_string())?;
    write_snapshot(&snapshot, document, removals)
}

pub fn merged_entry(group: &Map<String, Value>, entry: &Map<String, Value>) -> Map<String, Value> {
    let mut merged = group.clone();
    merged.shift_remove("hooks");
    for (key, value) in entry {
        merged.insert(key.clone(), value.clone());
    }
    merged
}

pub fn source_document(
    budget: &mut Budget,
    row: &Map<String, Value>,
) -> Option<Map<String, Value>> {
    let path = row_source_path(row)?;
    if super::safeio::has_suffix(&path, ".toml") {
        super::safeio::load_toml(budget, &path)
    } else {
        super::safeio::load_json(budget, &path, true, None)
    }
}

pub fn source_entry(
    row: &Map<String, Value>,
    document: &Map<String, Value>,
) -> Option<Map<String, Value>> {
    let found = records::locate(row, document)?;
    Some(merged_entry(&found.group, &found.entry))
}

pub fn row_source_path(row: &Map<String, Value>) -> Option<PathBuf> {
    let text = row
        .get("source")
        .and_then(Value::as_object)
        .and_then(|source| source.get("path"))
        .and_then(Value::as_str)?;
    Some(parse_path(text).unwrap_or_else(|_| PathBuf::from(text)))
}

fn timeout_seconds(agent: &str, entry: &Map<String, Value>) -> Option<i64> {
    let field = if agent == "copilot-cli" {
        "timeoutSec"
    } else {
        "timeout"
    };
    let Some(Value::Number(number)) = entry.get(field) else {
        return None;
    };
    let value = number.as_f64()?;
    if value <= 0.0 {
        return None;
    }
    Some((value.round_ties_even() as i64).max(1))
}

pub struct Hook {
    pub command: String,
    pub matcher: Option<String>,
    pub timeout: Option<i64>,
    pub condition: Option<String>,
    pub digest: String,
}

pub fn portable_hook(agent: &str, entry: &Map<String, Value>) -> Result<Hook, String> {
    if safe_type(entry) != "command" {
        return Err("only command hooks can be copied between agents".to_string());
    }
    let command = match entry.get("command") {
        Some(Value::String(value)) => Some(value.clone()),
        _ => entry
            .get("bash")
            .and_then(Value::as_str)
            .map(str::to_string),
    };
    let Some(command) = command.filter(|value| !value.trim().is_empty()) else {
        return Err("the source hook has no shell command to copy".to_string());
    };
    let text = |key: &str| {
        entry
            .get(key)
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .map(str::to_string)
    };
    Ok(Hook {
        command,
        matcher: text("matcher"),
        timeout: timeout_seconds(agent, entry),
        condition: text("if"),
        digest: payload_digest(entry),
    })
}

fn hook_entry(agent: &str, hook: &Hook) -> Map<String, Value> {
    let mut entry = Map::new();
    entry.insert("type".to_string(), json!("command"));
    if agent == "copilot-cli" {
        entry.insert("bash".to_string(), json!(hook.command));
        if let Some(timeout) = hook.timeout {
            entry.insert("timeoutSec".to_string(), json!(timeout));
        }
        return entry;
    }
    entry.insert("command".to_string(), json!(hook.command));
    if let Some(timeout) = hook.timeout {
        entry.insert("timeout".to_string(), json!(timeout));
    }
    if agent == "claude-code"
        && let Some(condition) = &hook.condition
    {
        entry.insert("if".to_string(), json!(condition));
    }
    entry
}

fn group_entry(agent: &str, hook: &Hook) -> Map<String, Value> {
    let mut group = Map::new();
    if let Some(matcher) = &hook.matcher {
        group.insert("matcher".to_string(), json!(matcher));
    }
    group.insert(
        "hooks".to_string(),
        Value::Array(vec![Value::Object(hook_entry(agent, hook))]),
    );
    group
}

fn entry_matches(group: &Map<String, Value>, entry: &Map<String, Value>, digest: &str) -> bool {
    payload_digest(&merged_entry(group, entry)) == digest
}

fn contains_digest(definitions: &[Value], digest: &str) -> bool {
    for group in definitions {
        let Some(group) = group.as_object() else {
            continue;
        };
        match group.get("hooks") {
            Some(Value::Array(entries)) => {
                if entries.iter().any(|entry| {
                    entry
                        .as_object()
                        .is_some_and(|entry| entry_matches(group, entry, digest))
                }) {
                    return true;
                }
            }
            _ => {
                if entry_matches(&Map::new(), group, digest) {
                    return true;
                }
            }
        }
    }
    false
}

fn without_digest(definitions: &[Value], digest: &str) -> (Vec<Value>, bool) {
    let mut kept: Vec<Value> = Vec::new();
    let mut changed = false;
    for group in definitions {
        let Some(object) = group.as_object() else {
            kept.push(group.clone());
            continue;
        };
        if let Some(Value::Array(entries)) = object.get("hooks") {
            let remaining: Vec<Value> = entries
                .iter()
                .filter(|entry| {
                    !entry
                        .as_object()
                        .is_some_and(|entry| entry_matches(object, entry, digest))
                })
                .cloned()
                .collect();
            if remaining.len() != entries.len() {
                changed = true;
            }
            if !remaining.is_empty() {
                let mut trimmed = object.clone();
                trimmed.insert("hooks".to_string(), Value::Array(remaining));
                kept.push(Value::Object(trimmed));
            }
            continue;
        }
        if entry_matches(&Map::new(), object, digest) {
            changed = true;
            continue;
        }
        kept.push(group.clone());
    }
    (kept, changed)
}

fn repr(value: Option<&Value>) -> String {
    match value {
        Some(Value::String(text)) => format!("'{text}'"),
        Some(Value::Bool(true)) => "True".to_string(),
        Some(Value::Bool(false)) => "False".to_string(),
        Some(Value::Null) | None => "None".to_string(),
        Some(other) => other.to_string(),
    }
}

fn hooks_container<'a>(
    document: &'a mut Map<String, Value>,
    agent: &str,
) -> Result<&'a mut Map<String, Value>, String> {
    if agent == "copilot-cli" {
        let version = document.get("version").cloned().unwrap_or(json!(1));
        if version != json!(1) {
            return Err(format!(
                "copilot hook file version {} is not 1",
                repr(Some(&version))
            ));
        }
        document.insert("version".to_string(), json!(1));
    }
    if !document.contains_key("hooks") || document.get("hooks") == Some(&Value::Null) {
        document.insert("hooks".to_string(), Value::Object(Map::new()));
    }
    document
        .get_mut("hooks")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| "the target's hooks key is not an object".to_string())
}

fn event_list<'a>(
    container: &'a mut Map<String, Value>,
    event: &str,
) -> Result<&'a mut Vec<Value>, String> {
    if !container.contains_key(event) || container.get(event) == Some(&Value::Null) {
        container.insert(event.to_string(), Value::Array(Vec::new()));
    }
    container
        .get_mut(event)
        .and_then(Value::as_array_mut)
        .ok_or_else(|| format!("the target's {event} entry is not an array"))
}

fn antigravity_group_names(document: &Map<String, Value>) -> Vec<String> {
    document
        .iter()
        .filter(|(_, value)| value.is_object())
        .map(|(key, _)| key.clone())
        .collect()
}

fn turn_on(
    agent: &str,
    event: &str,
    document: &mut Map<String, Value>,
    hook: &Hook,
) -> Result<(bool, Vec<String>), String> {
    if agent == "antigravity" {
        for name in antigravity_group_names(document) {
            let present = document
                .get(&name)
                .and_then(Value::as_object)
                .and_then(|definition| definition.get(event))
                .and_then(Value::as_array)
                .is_some_and(|definitions| contains_digest(definitions, &hook.digest));
            if present {
                return Ok((false, Vec::new()));
            }
        }
        let key = format!("hook-{}", hook.digest);
        if !document.contains_key(&key) {
            document.insert(key.clone(), Value::Object(Map::new()));
        }
        let Some(definition) = document.get_mut(&key).and_then(Value::as_object_mut) else {
            return Err("the target's hook group is not an object".to_string());
        };
        let definitions = event_list(definition, event)?;
        definitions.push(Value::Object(group_entry(agent, hook)));
        return Ok((true, Vec::new()));
    }
    let container = hooks_container(document, agent)?;
    let definitions = event_list(container, event)?;
    if contains_digest(definitions, &hook.digest) {
        return Ok((false, Vec::new()));
    }
    let added = if agent == "copilot-cli" {
        hook_entry(agent, hook)
    } else {
        group_entry(agent, hook)
    };
    definitions.push(Value::Object(added));
    Ok((true, Vec::new()))
}

fn turn_off(
    agent: &str,
    event: &str,
    document: &mut Map<String, Value>,
    digest: &str,
) -> Result<(bool, Vec<String>), String> {
    if agent == "antigravity" {
        let mut changed = false;
        let mut dropped: Vec<String> = Vec::new();
        for name in antigravity_group_names(document) {
            let Some(definitions) = document
                .get(&name)
                .and_then(Value::as_object)
                .and_then(|definition| definition.get(event))
                .and_then(Value::as_array)
                .cloned()
            else {
                continue;
            };
            let (kept, removed) = without_digest(&definitions, digest);
            if !removed {
                continue;
            }
            changed = true;
            let Some(definition) = document.get_mut(&name).and_then(Value::as_object_mut) else {
                continue;
            };
            if kept.is_empty() {
                definition.shift_remove(event);
            } else {
                definition.insert(event.to_string(), Value::Array(kept));
            }
            if definition.keys().all(|key| key == "enabled") {
                document.shift_remove(&name);
                dropped.push(name);
            }
        }
        return Ok((changed, dropped));
    }
    let container = hooks_container(document, agent)?;
    let Some(Value::Array(definitions)) = container.get(event).cloned() else {
        return Ok((false, Vec::new()));
    };
    let (kept, changed) = without_digest(&definitions, digest);
    if changed {
        if kept.is_empty() {
            container.shift_remove(event);
        } else {
            container.insert(event.to_string(), Value::Array(kept));
        }
    }
    Ok((changed, Vec::new()))
}

fn result(agent: &str, ok: bool, changed: bool, message: &str, touched: Vec<String>) -> Value {
    json!({
        "agent": agent,
        "ok": ok,
        "changed": changed,
        "message": message,
        "touched": touched,
    })
}

fn refusal(agent: &str, message: &str) -> Value {
    result(agent, false, false, message, Vec::new())
}

fn apply_to_agent(
    agent: &str,
    row: &Map<String, Value>,
    hook: &Hook,
    state: &str,
    home: &Path,
    environ: &Environ,
) -> Value {
    if CODE_HOSTED_AGENTS.contains(&agent) {
        return refusal(
            agent,
            &format!("{agent} hooks live in code and have no hook file to write"),
        );
    }
    if !WRITER_AGENTS.contains(&agent) {
        return refusal(agent, &format!("unknown agent '{agent}'"));
    }
    let row_agent = row.get("agent").and_then(Value::as_str).unwrap_or_default();
    let row_event = row.get("event").and_then(Value::as_str).unwrap_or_default();
    if state == "off" && agent == row_agent {
        return refusal(
            agent,
            "the row's own agent keeps its hook; edit the source file directly",
        );
    }
    let Some(event) = mapped_event(row_agent, row_event, agent) else {
        return refusal(
            agent,
            &format!("{agent} has no event equivalent to {row_agent} {row_event}"),
        );
    };
    let path = target_path(agent, home, environ);
    let mut loaded = match load_target(&path) {
        Ok(loaded) => loaded,
        Err(message) => return refusal(agent, &message),
    };
    let outcome = if state == "on" {
        turn_on(agent, &event, &mut loaded.document, hook)
    } else {
        turn_off(agent, &event, &mut loaded.document, &hook.digest)
    };
    let (changed, removals) = match outcome {
        Ok(outcome) => outcome,
        Err(message) => return refusal(agent, &message),
    };
    if !changed {
        let verb = if state == "on" {
            "already present under"
        } else {
            "no matching hook under"
        };
        return result(
            agent,
            true,
            false,
            &format!("{verb} {event} in {}", path.display()),
            Vec::new(),
        );
    }
    if let Err(message) = write_snapshot(&loaded.snapshot, &loaded.document, &removals) {
        return refusal(
            agent,
            &format!("could not write {}: {message}", path.display()),
        );
    }
    let verb = if state == "on" {
        "added under"
    } else {
        "removed from"
    };
    result(
        agent,
        true,
        true,
        &format!("{verb} {event} in {}", path.display()),
        vec![path_text(&path)],
    )
}

pub fn expand_agents(agents: &[String], source_agent: &str, state: &str) -> Vec<String> {
    let mut expanded_agents: Vec<String> = Vec::new();
    for agent in agents {
        if agent == "all" {
            for name in WRITER_AGENTS {
                if state == "off" && name == source_agent {
                    continue;
                }
                expanded_agents.push(name.to_string());
            }
        } else {
            expanded_agents.push(agent.clone());
        }
    }
    let mut unique: Vec<String> = Vec::new();
    for agent in expanded_agents {
        if !unique.contains(&agent) {
            unique.push(agent);
        }
    }
    unique
}

pub fn failure(project: &str, message: &str) -> Map<String, Value> {
    let mut document = Map::new();
    document.insert("ok".to_string(), json!(false));
    document.insert("schemaVersion".to_string(), json!(SCHEMA_VERSION));
    document.insert("project".to_string(), json!(project));
    document.insert("message".to_string(), json!(message));
    document.insert("results".to_string(), Value::Array(Vec::new()));
    document
}

pub struct Locator<'a> {
    pub project: &'a str,
    pub home: &'a str,
    pub environ: Environ,
    pub etc_root: &'a str,
    pub policy_owner_uid: u32,
    pub exact: bool,
}

impl Locator<'_> {
    fn home_path(&self) -> PathBuf {
        if self.home.is_empty() {
            crate::common::expanded_os_path(Path::new("~"))
        } else {
            expanded(self.home)
        }
    }

    fn query(&self) -> discovery::Query<'_> {
        discovery::Query {
            project: self.project,
            home: self.home,
            etc_root: self.etc_root,
            policy_owner_uid: self.policy_owner_uid,
            exact: self.exact,
            scope: "all",
        }
    }

    fn inventory(&self, budget: &mut Budget) -> Map<String, Value> {
        discovery::collect(budget, &self.query(), self.environ.clone())
    }
}

fn located(
    locator: &Locator<'_>,
    budget: &mut Budget,
    row_id: &str,
) -> (String, Option<Map<String, Value>>) {
    let inventory = locator.inventory(budget);
    let root = inventory
        .get("project")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let row = inventory
        .get("items")
        .and_then(Value::as_array)
        .and_then(|items| {
            items
                .iter()
                .find(|item| item.get("id").and_then(Value::as_str) == Some(row_id))
        })
        .and_then(Value::as_object)
        .cloned();
    (root, row)
}

pub fn apply(
    locator: &Locator<'_>,
    row_id: &str,
    agents: &[String],
    state: &str,
) -> Map<String, Value> {
    if state != "on" && state != "off" {
        return failure("", &format!("state must be on or off, not '{state}'"));
    }
    let mut budget = Budget::default();
    let (root, row) = located(locator, &mut budget, row_id);
    let Some(row) = row else {
        return failure(&root, &format!("no hook row with id '{row_id}'"));
    };
    if row.get("supportStatus").and_then(Value::as_str) == Some("code-hosted") {
        return failure(
            &root,
            "code-hosted rows describe a directory, not a hook, and cannot be copied",
        );
    }
    let document = source_document(&mut budget, &row);
    let entry = document
        .as_ref()
        .and_then(|document| source_entry(&row, document));
    let Some(entry) = entry else {
        let path = row
            .get("source")
            .and_then(Value::as_object)
            .and_then(|source| source.get("path"))
            .and_then(Value::as_str)
            .unwrap_or_default();
        return failure(
            &root,
            &format!("the source hook is no longer readable at {path}"),
        );
    };
    let row_agent = row.get("agent").and_then(Value::as_str).unwrap_or_default();
    let hook = match portable_hook(row_agent, &entry) {
        Ok(hook) => hook,
        Err(message) => return failure(&root, &message),
    };
    let document = document.expect("source document present");
    if !records::matches(&row, &document) {
        return failure(
            &root,
            "the source hook changed since it was listed; refresh and retry",
        );
    }
    let home = locator.home_path();
    let results: Vec<Value> = expand_agents(agents, row_agent, state)
        .iter()
        .map(|agent| apply_to_agent(agent, &row, &hook, state, &home, &locator.environ))
        .collect();
    let failures: Vec<String> = results
        .iter()
        .filter(|outcome| outcome.get("ok") != Some(&json!(true)))
        .map(|outcome| {
            format!(
                "{}: {}",
                outcome
                    .get("agent")
                    .and_then(Value::as_str)
                    .unwrap_or_default(),
                outcome
                    .get("message")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
            )
        })
        .collect();
    let mut document = Map::new();
    document.insert(
        "ok".to_string(),
        json!(!results.is_empty() && failures.is_empty()),
    );
    document.insert("schemaVersion".to_string(), json!(SCHEMA_VERSION));
    document.insert("project".to_string(), json!(root));
    document.insert("message".to_string(), json!(failures.join("; ")));
    document.insert("results".to_string(), Value::Array(results));
    document
}

pub struct Removal<'a> {
    pub prepare: bool,
    pub expected_payload: Option<&'a Value>,
    pub transaction_id: &'a str,
}

pub fn remove(locator: &Locator<'_>, row_id: &str, removal: &Removal<'_>) -> Map<String, Value> {
    let mut budget = Budget::default();
    let (root, row) = located(locator, &mut budget, row_id);
    let Some(row) = row else {
        return failure(&root, &format!("no hook row with id '{row_id}'"));
    };
    let kind = row
        .get("source")
        .and_then(Value::as_object)
        .and_then(|source| source.get("kind"))
        .and_then(Value::as_str)
        .unwrap_or_default();
    if row.get("supportStatus").and_then(Value::as_str) == Some("code-hosted") || kind != "json" {
        return failure(
            &root,
            "only JSON hook definitions can be removed here; edit the source directly",
        );
    }
    let Some(path) = row_source_path(&row) else {
        return failure(
            &root,
            "the source hook path is unreadable; refresh and retry",
        );
    };
    let mut loaded = match load_target(&path) {
        Ok(loaded) => loaded,
        Err(message) => return failure(&root, &message),
    };
    if !records::matches(&row, &loaded.document) {
        return failure(
            &root,
            "the source hook changed since it was listed; refresh and retry",
        );
    }
    let row_agent = row
        .get("agent")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let Some(entry) = source_entry(&row, &loaded.document) else {
        return failure(&root, "the source hook is no longer readable");
    };
    if let Err(message) = portable_hook(&row_agent, &entry) {
        return failure(&root, &message);
    }
    let Some(mut payload) = records::detach(&row, &mut loaded.document) else {
        return failure(&root, "the source hook is no longer readable");
    };
    payload.insert("source".to_string(), json!(path_text(&path)));
    let Ok(resolved) = std::fs::canonicalize(&path) else {
        return failure(&root, "the source hook path changed; refresh and retry");
    };
    payload.insert("target".to_string(), json!(path_text(&resolved)));
    let payload = Value::Object(payload);
    let Ok(encoded) = serde_json::to_vec(&payload) else {
        return failure(
            &root,
            "the hook cannot be preserved as a Unicode undo record; edit the source directly",
        );
    };
    if encoded.len() > 1024 * 1024 {
        return failure(
            &root,
            "the hook exceeds the undo record size limit; edit the source directly",
        );
    }
    let home = locator.home_path();
    let context = json!({
        "project": root,
        "home": path_text(&home),
        "etcRoot": path_text(Path::new(locator.etc_root)),
        "policyOwnerUid": locator.policy_owner_uid.to_string(),
    });
    if let Some(expected) = removal.expected_payload
        && *expected != payload
    {
        return failure(
            &root,
            "the source hook changed after recovery was prepared; nothing was changed",
        );
    }
    let store = recovery_store(&home, &locator.environ);
    let record_id = match store.write(&payload, row_id, &context, removal.transaction_id) {
        Ok(record_id) => record_id,
        Err(RecoveryError::Full(message)) => return failure(&root, &message),
        Err(error) => {
            return failure(
                &root,
                &format!("the recovery record could not be stored: {error}"),
            );
        }
    };
    if removal.prepare {
        let mut document = Map::new();
        document.insert("ok".to_string(), json!(true));
        document.insert("schemaVersion".to_string(), json!(SCHEMA_VERSION));
        document.insert("project".to_string(), json!(root));
        document.insert("results".to_string(), Value::Array(Vec::new()));
        document.insert("payload".to_string(), payload);
        document.insert("recordId".to_string(), json!(record_id));
        return document;
    }
    if let Err(message) = write_snapshot(&loaded.snapshot, &loaded.document, &[]) {
        return failure(
            &root,
            &format!("could not write the source hook: {message}"),
        );
    }
    let event = row.get("event").and_then(Value::as_str).unwrap_or_default();
    let outcome = result(
        &row_agent,
        true,
        true,
        &format!("removed from {event}"),
        vec![path_text(&path)],
    );
    let mut document = Map::new();
    document.insert("ok".to_string(), json!(true));
    document.insert("schemaVersion".to_string(), json!(SCHEMA_VERSION));
    document.insert("project".to_string(), json!(root));
    document.insert("message".to_string(), json!(""));
    document.insert("results".to_string(), Value::Array(vec![outcome]));
    document.insert("payload".to_string(), payload);
    document.insert("recordId".to_string(), json!(record_id));
    document
}

fn restored(agent: &str, changed: bool, source: &str) -> Map<String, Value> {
    let outcome = result(
        agent,
        true,
        changed,
        if changed {
            "restored original hook"
        } else {
            "already restored"
        },
        if changed {
            vec![source.to_string()]
        } else {
            Vec::new()
        },
    );
    let mut document = Map::new();
    document.insert("ok".to_string(), json!(true));
    document.insert("schemaVersion".to_string(), json!(SCHEMA_VERSION));
    document.insert("project".to_string(), json!(""));
    document.insert("message".to_string(), json!(""));
    document.insert("results".to_string(), Value::Array(vec![outcome]));
    document
}

pub fn restore(
    record_id: &str,
    raw_payload: Option<&str>,
    home: &str,
    environ: &Environ,
    etc_root: &str,
    policy_owner_uid: u32,
) -> Map<String, Value> {
    let home_path = if home.is_empty() {
        crate::common::expanded_os_path(Path::new("~"))
    } else {
        expanded(home)
    };
    let store = recovery_store(&home_path, environ);
    let payload = match raw_payload {
        Some(text) => match serde_json::from_str::<Value>(text) {
            Ok(value) => value,
            Err(_) => {
                return failure(
                    "",
                    "restore payload is not JSON or its recovery record is unavailable",
                );
            }
        },
        None => match store.read(record_id) {
            Some(record) => record.payload().clone(),
            None => {
                return failure(
                    "",
                    "restore payload is not JSON or its recovery record is unavailable",
                );
            }
        },
    };
    if !payload.is_object() {
        return failure("", "restore payload is not a record");
    }
    let mut record = store.read(record_id);
    if record
        .as_ref()
        .is_some_and(|found| found.payload() != &payload)
    {
        record = None;
    }
    if record.is_none() {
        record = store.find(&payload);
    }
    let Some(record) = record else {
        return failure("", "no prepared recovery record matches this payload");
    };
    let prepared = record.payload().clone();
    let prepared = prepared.as_object().cloned().unwrap_or_default();
    let context = record.context().as_object().cloned().unwrap_or_default();
    let missing = ["agent", "event", "source"]
        .iter()
        .any(|key| !prepared.get(*key).is_some_and(Value::is_string));
    if prepared.get("format") != Some(&json!(2)) || missing {
        return failure("", "restore payload is incomplete");
    }
    let agent = prepared
        .get("agent")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let event = prepared
        .get("event")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let raw_source = prepared
        .get("source")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let source = match parse_path(raw_source) {
        Ok(path) => path,
        Err(error) => return failure("", &error.to_string()),
    };
    if !WRITER_AGENTS.contains(&agent.as_str()) || event.is_empty() || !source.is_absolute() {
        return failure("", "restore payload is incomplete");
    }
    let recorded_target = prepared
        .get("target")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let target = match parse_path(recorded_target) {
        Ok(path) => path,
        Err(error) => return failure("", &error.to_string()),
    };
    let resolved = std::fs::canonicalize(&source).ok();
    if !target.is_absolute() || resolved.as_deref() != Some(target.as_path()) {
        return failure("", "the source hook path changed; restore it manually");
    }
    let recorded_project = context
        .get("project")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| source.parent().map(path_text).unwrap_or_default());
    let recorded_home = context
        .get("home")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| home.to_string());
    let recorded_etc = context
        .get("etcRoot")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .unwrap_or_else(|| etc_root.to_string());
    let recorded_owner = context
        .get("policyOwnerUid")
        .and_then(Value::as_str)
        .and_then(|value| value.parse::<u32>().ok())
        .unwrap_or(policy_owner_uid);
    let mut budget = Budget::default();
    discovery::collect(
        &mut budget,
        &discovery::Query {
            project: &recorded_project,
            home: &recorded_home,
            etc_root: &recorded_etc,
            policy_owner_uid: recorded_owner,
            exact: true,
            scope: "all",
        },
        environ.clone(),
    );
    if !budget.knows(&agent, &super::safeio::absolute(&source)) {
        return failure(
            "",
            "the recorded source is not a known hook configuration file for that agent",
        );
    }
    let mut loaded = match load_target(&source) {
        Ok(loaded) => loaded,
        Err(message) => return failure("", &message),
    };
    let changed = match records::attach(&prepared, &mut loaded.document) {
        Ok(changed) => changed,
        Err(message) => return failure("", &message),
    };
    if changed && let Err(message) = write_snapshot(&loaded.snapshot, &loaded.document, &[]) {
        return failure("", &message);
    }
    store.mark_restored(&record.record_id);
    restored(&agent, changed, &path_text(&source))
}
