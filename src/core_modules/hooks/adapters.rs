use super::events::COPILOT_PASCAL_ALIASES;
use super::labels::{Environ, environ_value};
use super::records::identity;
use super::redaction::{enabled_state, payload_material, safe_summary};
use super::safeio::{
    Budget, MAX_ITEMS_PER_SOURCE, absolute, artifact_metrics, existing_file, expanded_os,
    load_json, load_toml, scan_dir, scan_dir_directories, stable_id, suffix_of,
};
use crate::common::{display_path, path_text};
use serde_json::{Map, Value, json};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

pub const CLAUDE_EVENTS: [&str; 33] = [
    "SessionStart",
    "Setup",
    "UserPromptSubmit",
    "UserPromptExpansion",
    "PreToolUse",
    "PermissionRequest",
    "PermissionDenied",
    "PostToolUse",
    "PostToolUseFailure",
    "PostToolBatch",
    "Notification",
    "MessageDisplay",
    "SubagentStart",
    "SubagentStop",
    "TaskCreated",
    "TaskCompleted",
    "Stop",
    "StopFailure",
    "TeammateIdle",
    "InstructionsLoaded",
    "ConfigChange",
    "CwdChanged",
    "DirectoryAdded",
    "FileChanged",
    "WorktreeCreate",
    "WorktreeRemove",
    "PreCompact",
    "PostCompact",
    "PreModelSwitch",
    "PostModelSwitch",
    "Elicitation",
    "ElicitationResult",
    "SessionEnd",
];

pub const CODEX_EVENTS: [&str; 12] = [
    "SessionStart",
    "SessionEnd",
    "PreToolUse",
    "PermissionRequest",
    "PostToolUse",
    "PreCompact",
    "PostCompact",
    "UserPromptSubmit",
    "SubagentStart",
    "SubagentStop",
    "Stop",
    "Interrupt",
];

pub const COPILOT_EVENTS: [&str; 14] = [
    "sessionStart",
    "sessionEnd",
    "userPromptSubmitted",
    "userPromptTransformed",
    "preToolUse",
    "postToolUse",
    "postToolUseFailure",
    "agentStop",
    "subagentStart",
    "subagentStop",
    "errorOccurred",
    "preCompact",
    "permissionRequest",
    "notification",
];

pub const ANTIGRAVITY_EVENTS: [&str; 5] = [
    "PreToolUse",
    "PostToolUse",
    "PreInvocation",
    "PostInvocation",
    "Stop",
];

pub const COPILOT_MANIFESTS: [&[&str]; 4] = [
    &[".plugin", "plugin.json"],
    &["plugin.json"],
    &[".github", "plugin", "plugin.json"],
    &[".claude-plugin", "plugin.json"],
];

pub const PROJECT_SCOPES: [&str; 2] = ["project", "local"];

pub const AGENT_IDS: [&str; 6] = [
    "claude-code",
    "codex",
    "copilot-cli",
    "antigravity",
    "opencode",
    "pi",
];

pub fn copilot_known(event: &str) -> bool {
    COPILOT_EVENTS.contains(&event)
        || COPILOT_PASCAL_ALIASES
            .iter()
            .any(|(alias, _)| *alias == event)
}

pub struct Context {
    pub home: PathBuf,
    pub project_root: PathBuf,
    pub environ: Environ,
    pub etc_root: PathBuf,
    pub policy_owner_uid: u32,
    pub scope: String,
}

impl Context {
    pub fn user_lane(&self) -> bool {
        self.scope != "project"
    }

    pub fn project_path(&self, parts: &[&str]) -> Option<PathBuf> {
        if self.project_root.as_os_str().is_empty() {
            return None;
        }
        let mut path = self.project_root.clone();
        for part in parts {
            path = path.join(part);
        }
        Some(path)
    }

    pub fn codex_home(&self) -> PathBuf {
        let override_value = environ_value(&self.environ, "CODEX_HOME");
        if override_value.is_empty() {
            self.home.join(".codex")
        } else {
            expanded_os(&override_value)
        }
    }

    pub fn copilot_home(&self) -> PathBuf {
        let override_value = environ_value(&self.environ, "COPILOT_HOME");
        if override_value.is_empty() {
            self.home.join(".copilot")
        } else {
            expanded_os(&override_value)
        }
    }

    pub fn config_home(&self) -> PathBuf {
        let override_value = environ_value(&self.environ, "XDG_CONFIG_HOME");
        if override_value.is_empty() {
            self.home.join(".config")
        } else {
            expanded_os(&override_value)
        }
    }
}

pub type Source = (PathBuf, String);
pub type Loader<'a> = &'a dyn Fn(&mut Budget, &Path) -> Option<Map<String, Value>>;

fn truncated(text: &str, limit: usize) -> String {
    text.chars().take(limit).collect()
}

fn source_map(budget: &mut Budget, path: &Path) -> Map<String, Value> {
    let suffix = suffix_of(path);
    let kind = suffix.trim_start_matches('.');
    let kind = if kind.is_empty() { "file" } else { kind };
    let mut source = Map::new();
    source.insert("kind".to_string(), json!(kind));
    source.insert("path".to_string(), json!(path_text(path)));
    source.insert(
        "name".to_string(),
        json!(display_path(Path::new(
            path.file_name().unwrap_or_default()
        ))),
    );
    let _ = budget;
    if let Ok(target) = std::fs::canonicalize(path)
        && target != absolute(path)
    {
        source.insert("realpath".to_string(), json!(path_text(&target)));
    }
    source
}

pub struct RowSpec<'a> {
    pub agent: &'a str,
    pub path: &'a Path,
    pub scope: &'a str,
    pub event: &'a str,
    pub index: usize,
    pub record_id: &'a str,
    pub support: &'a str,
    pub enabled: Option<bool>,
    pub note: &'a str,
}

pub fn row(
    budget: &mut Budget,
    spec: &RowSpec<'_>,
    entry: &Map<String, Value>,
) -> Map<String, Value> {
    let RowSpec {
        agent,
        path,
        scope,
        event,
        index,
        record_id,
        support,
        enabled,
        note,
    } = *spec;
    let source = source_map(budget, path);
    let metrics = artifact_metrics(budget, path, &payload_material(entry));
    let mut item = Map::new();
    item.insert("precedence".to_string(), Value::Null);
    item.insert("id".to_string(), json!(record_id));
    item.insert("agent".to_string(), json!(agent));
    item.insert("source".to_string(), Value::Object(source));
    item.insert("scope".to_string(), json!(scope));
    item.insert("supportStatus".to_string(), json!(support));
    item.insert("event".to_string(), json!(event));
    item.insert("index".to_string(), json!(index));
    item.insert(
        "enabled".to_string(),
        match enabled {
            Some(flag) => json!(flag),
            None => Value::Null,
        },
    );
    item.insert("summary".to_string(), Value::Object(safe_summary(entry)));
    item.insert("metrics".to_string(), Value::Object(metrics));
    item.insert("note".to_string(), json!(truncated(note, 80)));
    item.insert("badges".to_string(), Value::Array(Vec::new()));
    item
}

pub fn location_row(
    budget: &mut Budget,
    agent: &str,
    path: &Path,
    scope: &str,
    note: &str,
    count: usize,
) -> Map<String, Value> {
    let mut source = Map::new();
    source.insert("kind".to_string(), json!("location"));
    source.insert("path".to_string(), json!(path_text(path)));
    source.insert(
        "name".to_string(),
        json!(display_path(Path::new(
            path.file_name().unwrap_or_default()
        ))),
    );
    if let Ok(target) = std::fs::canonicalize(path)
        && target != absolute(path)
    {
        source.insert("realpath".to_string(), json!(path_text(&target)));
    }
    let metrics = artifact_metrics(budget, path, "");
    let mut item = Map::new();
    item.insert(
        "id".to_string(),
        json!(stable_id(&[
            agent.as_bytes(),
            path.as_os_str().as_bytes(),
            b"location",
        ])),
    );
    item.insert("agent".to_string(), json!(agent));
    item.insert("source".to_string(), Value::Object(source));
    item.insert("scope".to_string(), json!(scope));
    item.insert("supportStatus".to_string(), json!("code-hosted"));
    item.insert("event".to_string(), json!(""));
    item.insert("index".to_string(), json!(0));
    item.insert("enabled".to_string(), Value::Null);
    let mut summary = Map::new();
    summary.insert("label".to_string(), json!(""));
    summary.insert("labelSource".to_string(), json!(""));
    summary.insert("type".to_string(), json!("code"));
    summary.insert("fields".to_string(), Value::Array(Vec::new()));
    summary.insert("digest".to_string(), json!(""));
    summary.insert("payloadBytes".to_string(), json!(0));
    summary.insert("envCount".to_string(), json!(0));
    summary.insert("matcher".to_string(), json!("none"));
    summary.insert("timeoutSeconds".to_string(), Value::Null);
    summary.insert("hasCondition".to_string(), json!(false));
    item.insert("summary".to_string(), Value::Object(summary));
    item.insert("metrics".to_string(), Value::Object(metrics));
    item.insert("note".to_string(), json!(truncated(note, 80)));
    item.insert("entries".to_string(), json!(count));
    item.insert("badges".to_string(), json!(["not-inspected"]));
    item
}

pub struct MapSpec<'a> {
    pub agent: &'a str,
    pub path: &'a Path,
    pub scope: &'a str,
    pub known: &'a dyn Fn(&str) -> bool,
    pub container: &'a str,
    pub enabled_default: Option<bool>,
    pub namespace: &'a str,
}

impl<'a> MapSpec<'a> {
    pub fn new(
        agent: &'a str,
        path: &'a Path,
        scope: &'a str,
        known: &'a dyn Fn(&str) -> bool,
    ) -> Self {
        Self {
            agent,
            path,
            scope,
            known,
            container: "hooks",
            enabled_default: None,
            namespace: "",
        }
    }
}

pub fn event_map_rows(
    budget: &mut Budget,
    spec: &MapSpec<'_>,
    mapping: &Map<String, Value>,
) -> Vec<Map<String, Value>> {
    let MapSpec {
        agent,
        path,
        scope,
        known,
        container,
        enabled_default,
        namespace,
    } = *spec;
    let mut rows: Vec<Map<String, Value>> = Vec::new();
    for (event, definitions) in mapping {
        let Some(definitions) = definitions.as_array() else {
            continue;
        };
        let support = if known(event) {
            "documented"
        } else {
            "undocumented-event"
        };
        for (group_index, group) in definitions.iter().take(MAX_ITEMS_PER_SOURCE).enumerate() {
            let Some(group) = group.as_object() else {
                continue;
            };
            let candidates: Vec<&Value> = match group.get(container) {
                Some(Value::Array(entries)) => entries.iter().collect(),
                _ => vec![definitions.get(group_index).expect("group in range")],
            };
            for (entry_index, candidate) in candidates.iter().take(MAX_ITEMS_PER_SOURCE).enumerate()
            {
                let Some(entry) = candidate.as_object() else {
                    break;
                };
                if !budget.take_row() {
                    break;
                }
                let mut merged = group.clone();
                merged.shift_remove(container);
                for (key, value) in entry {
                    merged.insert(key.clone(), value.clone());
                }
                let index = group_index * MAX_ITEMS_PER_SOURCE + entry_index;
                let record_id = identity(agent, path, event, index, group, entry, namespace);
                let enabled = enabled_state(&merged, enabled_default);
                rows.push(row(
                    budget,
                    &RowSpec {
                        agent,
                        path,
                        scope,
                        event,
                        index,
                        record_id: &record_id,
                        support,
                        enabled,
                        note: "",
                    },
                    &merged,
                ));
            }
        }
    }
    rows
}

pub fn documents(
    budget: &mut Budget,
    agent: &str,
    sources: &[Source],
    loader: Loader<'_>,
) -> Vec<(PathBuf, String, Map<String, Value>)> {
    let mut found = Vec::new();
    for (path, scope) in sources {
        if !existing_file(budget, path) {
            continue;
        }
        let noted = absolute(path);
        budget.note_source(agent, &noted);
        if !budget.take_source() {
            continue;
        }
        if let Some(document) = loader(budget, path) {
            found.push((path.clone(), scope.clone(), document));
        }
    }
    found
}

pub fn hook_mapping(document: &Map<String, Value>) -> Option<Map<String, Value>> {
    document.get("hooks").and_then(Value::as_object).cloned()
}

pub fn root_event_mapping(document: &Map<String, Value>) -> Option<Map<String, Value>> {
    if let Some(mapping) = hook_mapping(document) {
        return Some(mapping);
    }
    let events: Map<String, Value> = document
        .iter()
        .filter(|(_, value)| value.is_array())
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect();
    (!events.is_empty()).then_some(events)
}

pub fn lane_sources(context: &Context, sources: Vec<Source>) -> Vec<Source> {
    match context.scope.as_str() {
        "project" => sources
            .into_iter()
            .filter(|(_, scope)| PROJECT_SCOPES.contains(&scope.as_str()))
            .collect(),
        "user" => sources
            .into_iter()
            .filter(|(_, scope)| !PROJECT_SCOPES.contains(&scope.as_str()))
            .collect(),
        _ => sources,
    }
}

pub fn scoped(
    context: &Context,
    home_path: PathBuf,
    project_parts: &[&str],
    lane: bool,
) -> Vec<Source> {
    let mut sources: Vec<Source> = vec![(home_path, "user".to_string())];
    if let Some(candidate) = context.project_path(project_parts) {
        sources.push((candidate, "project".to_string()));
    }
    if lane {
        lane_sources(context, sources)
    } else {
        sources
    }
}

pub fn settings_rows(
    budget: &mut Budget,
    agent: &str,
    sources: &[Source],
    known: &dyn Fn(&str) -> bool,
    allow_comments: bool,
    root_events: bool,
) -> Vec<Map<String, Value>> {
    let mut rows = Vec::new();
    let loader = |budget: &mut Budget, path: &Path| load_json(budget, path, allow_comments, None);
    for (path, scope, document) in documents(budget, agent, sources, &loader) {
        let mapping = if root_events {
            root_event_mapping(&document)
        } else {
            hook_mapping(&document)
        };
        if let Some(mapping) = mapping {
            rows.extend(event_map_rows(
                budget,
                &MapSpec::new(agent, &path, &scope, known),
                &mapping,
            ));
        }
    }
    rows
}

pub fn mark(rows: &mut [Map<String, Value>], badge: &str, note: &str) {
    for entry in rows {
        let badges = entry
            .entry("badges".to_string())
            .or_insert_with(|| Value::Array(Vec::new()));
        if let Some(list) = badges.as_array_mut()
            && !list.iter().any(|item| item.as_str() == Some(badge))
        {
            list.push(json!(badge));
        }
        if !note.is_empty()
            && entry
                .get("note")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .is_empty()
        {
            entry.insert("note".to_string(), json!(truncated(note, 80)));
        }
    }
}

pub fn alias_badges(rows: &mut [Map<String, Value>]) {
    for entry in rows {
        let event = entry
            .get("event")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let Some((_, canonical)) = COPILOT_PASCAL_ALIASES
            .iter()
            .find(|(alias, _)| *alias == event)
        else {
            continue;
        };
        if let Some(list) = entry.get_mut("badges").and_then(Value::as_array_mut) {
            list.push(json!("vscode-alias"));
        }
        entry.insert("canonicalEvent".to_string(), json!(*canonical));
    }
}

pub fn claude_code(context: &Context, budget: &mut Budget) -> Vec<Map<String, Value>> {
    let mut sources: Vec<Source> = vec![
        (
            PathBuf::from("/etc/claude-code/managed-settings.json"),
            "managed".to_string(),
        ),
        (
            context.home.join(".claude").join("settings.json"),
            "user".to_string(),
        ),
    ];
    for (name, scope) in [
        ("settings.json", "project"),
        ("settings.local.json", "local"),
    ] {
        if let Some(candidate) = context.project_path(&[".claude", name]) {
            sources.push((candidate, scope.to_string()));
        }
    }
    let known = |event: &str| CLAUDE_EVENTS.contains(&event);
    let mut rows = settings_rows(
        budget,
        "claude-code",
        &lane_sources(context, sources),
        &known,
        false,
        false,
    );
    if !context.user_lane() {
        return rows;
    }
    let cache = context.home.join(".claude").join("plugins").join("cache");
    let plugin_files: Vec<Source> = scan_dir(budget, &cache, ".json", 5)
        .into_iter()
        .filter(|path| {
            path.file_name().is_some_and(|name| name == "hooks.json")
                && path
                    .parent()
                    .and_then(Path::file_name)
                    .is_some_and(|name| name == "hooks")
        })
        .map(|path| (path, "plugin".to_string()))
        .collect();
    rows.extend(settings_rows(
        budget,
        "claude-code",
        &plugin_files,
        &known,
        false,
        false,
    ));
    rows
}

pub fn codex_profile_paths(budget: &mut Budget, agent_home: &Path) -> Vec<PathBuf> {
    scan_dir(budget, agent_home, ".toml", 1)
        .into_iter()
        .filter(|path| {
            let name = path
                .file_name()
                .unwrap_or_default()
                .to_string_lossy()
                .to_string();
            name.ends_with(".config.toml") && name != "config.toml"
        })
        .collect()
}

fn codex_profile_rows(budget: &mut Budget, agent_home: &Path) -> Vec<Map<String, Value>> {
    let known = |event: &str| CODEX_EVENTS.contains(&event);
    let mut rows = Vec::new();
    for path in codex_profile_paths(budget, agent_home) {
        budget.note_source("codex", &absolute(&path));
        if !budget.take_source() {
            break;
        }
        let Some(document) = load_toml(budget, &path) else {
            continue;
        };
        let Some(mapping) = hook_mapping(&document) else {
            continue;
        };
        let mut entries = event_map_rows(
            budget,
            &MapSpec::new("codex", &path, "profile", &known),
            &mapping,
        );
        let name = path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let profile = name.trim_end_matches(".config.toml").to_string();
        for entry in &mut entries {
            entry.insert("enabled".to_string(), Value::Null);
            entry.insert("profile".to_string(), json!(profile));
        }
        mark(
            &mut entries,
            "profile-unselected",
            "applies only when codex runs with --profile",
        );
        rows.extend(entries);
    }
    rows
}

fn codex_enabled(budget: &mut Budget, sources: &[Source]) -> bool {
    for (path, _) in sources {
        if !existing_file(budget, path) {
            continue;
        }
        let Some(document) = load_toml(budget, path) else {
            continue;
        };
        let Some(Value::Object(features)) = document.get("features") else {
            continue;
        };
        for key in ["hooks", "codex_hooks"] {
            if features.get(key) == Some(&Value::Bool(true)) {
                return true;
            }
        }
    }
    false
}

pub fn codex(context: &Context, budget: &mut Budget) -> Vec<Map<String, Value>> {
    let codex_home = context.codex_home();
    let known = |event: &str| CODEX_EVENTS.contains(&event);
    let json_sources = scoped(
        context,
        codex_home.join("hooks.json"),
        &[".codex", "hooks.json"],
        true,
    );
    let mut rows = settings_rows(budget, "codex", &json_sources, &known, false, true);
    let toml_sources = scoped(
        context,
        codex_home.join("config.toml"),
        &[".codex", "config.toml"],
        false,
    );
    let loader = |budget: &mut Budget, path: &Path| load_toml(budget, path);
    for (path, scope, document) in documents(
        budget,
        "codex",
        &lane_sources(context, toml_sources.clone()),
        &loader,
    ) {
        if let Some(mapping) = hook_mapping(&document) {
            rows.extend(event_map_rows(
                budget,
                &MapSpec::new("codex", &path, &scope, &known),
                &mapping,
            ));
        }
    }
    if !rows.is_empty() && !codex_enabled(budget, &toml_sources) {
        mark(
            &mut rows,
            "feature-off",
            "codex hooks feature is not enabled",
        );
    }
    if !context.user_lane() {
        return rows;
    }
    rows.extend(codex_profile_rows(budget, &codex_home));
    rows
}

fn copilot_policy_rows(context: &Context, budget: &mut Budget) -> Vec<Map<String, Value>> {
    let directory = context.etc_root.join("github-copilot").join("policy.d");
    let owner = context.policy_owner_uid;
    let mut rows: Vec<Map<String, Value>> = Vec::new();
    for path in scan_dir(budget, &directory, ".json", 1) {
        budget.note_source("copilot-cli", &absolute(&path));
        if !budget.take_source() {
            break;
        }
        let Some(document) = load_json(budget, &path, false, Some(owner)) else {
            continue;
        };
        if document.get("version") != Some(&json!(1)) {
            continue;
        }
        let Some(mapping) = hook_mapping(&document) else {
            continue;
        };
        let mut entries = event_map_rows(
            budget,
            &MapSpec::new("copilot-cli", &path, "policy", &copilot_known),
            &mapping,
        );
        mark(
            &mut entries,
            "policy",
            "policy hooks ignore disableAllHooks",
        );
        rows.extend(entries);
    }
    alias_badges(&mut rows);
    rows
}

fn copilot_file_rows(context: &Context, budget: &mut Budget) -> Vec<Map<String, Value>> {
    let mut files: Vec<Source> = Vec::new();
    if context.user_lane() {
        let directory = context.copilot_home().join("hooks");
        files.extend(
            scan_dir(budget, &directory, ".json", 1)
                .into_iter()
                .map(|path| (path, "user".to_string())),
        );
    }
    if let Some(project) = context.project_path(&[".github", "hooks"]) {
        files.extend(
            scan_dir(budget, &project, ".json", 1)
                .into_iter()
                .map(|path| (path, "project".to_string())),
        );
    }
    let loader = |budget: &mut Budget, path: &Path| load_json(budget, path, false, None);
    let mut rows: Vec<Map<String, Value>> = Vec::new();
    for (path, scope, document) in documents(budget, "copilot-cli", &files, &loader) {
        if document.get("version") != Some(&json!(1)) {
            continue;
        }
        let Some(mapping) = hook_mapping(&document) else {
            continue;
        };
        let mut entries = event_map_rows(
            budget,
            &MapSpec::new("copilot-cli", &path, &scope, &copilot_known),
            &mapping,
        );
        if document.get("disableAllHooks") == Some(&Value::Bool(true)) {
            for entry in &mut entries {
                entry.insert("enabled".to_string(), json!(false));
            }
            mark(
                &mut entries,
                "file-disabled",
                "disableAllHooks in this hook file",
            );
        }
        rows.extend(entries);
    }
    alias_badges(&mut rows);
    rows
}

fn copilot_settings_sources(context: &Context) -> Vec<(PathBuf, String, bool)> {
    let mut sources = vec![(
        context.copilot_home().join("settings.json"),
        "user".to_string(),
        false,
    )];
    for parts in [
        [".github", "copilot", "settings.json"].as_slice(),
        [".github", "copilot", "settings.local.json"].as_slice(),
        [".claude", "settings.json"].as_slice(),
        [".claude", "settings.local.json"].as_slice(),
    ] {
        if let Some(candidate) = context.project_path(parts) {
            sources.push((candidate, "project".to_string(), true));
        }
    }
    sources
}

fn copilot_settings_rows(
    context: &Context,
    budget: &mut Budget,
) -> (Vec<Map<String, Value>>, bool) {
    let mut rows: Vec<Map<String, Value>> = Vec::new();
    let mut repository_disabled = false;
    let lane: Vec<(PathBuf, String, bool)> = copilot_settings_sources(context)
        .into_iter()
        .filter(|(_, scope, _)| match context.scope.as_str() {
            "project" => PROJECT_SCOPES.contains(&scope.as_str()),
            "user" => !PROJECT_SCOPES.contains(&scope.as_str()),
            _ => true,
        })
        .collect();
    for (path, scope, repository) in lane {
        if !existing_file(budget, &path) {
            continue;
        }
        budget.note_source("copilot-cli", &absolute(&path));
        if !budget.take_source() {
            continue;
        }
        let Some(document) = load_json(budget, &path, false, None) else {
            continue;
        };
        let disabled = document.get("disableAllHooks") == Some(&Value::Bool(true));
        if disabled && repository {
            repository_disabled = true;
        }
        let Some(mapping) = hook_mapping(&document) else {
            continue;
        };
        let mut entries = event_map_rows(
            budget,
            &MapSpec::new("copilot-cli", &path, &scope, &copilot_known),
            &mapping,
        );
        mark(&mut entries, "settings-inline", "");
        if disabled && !repository {
            for entry in &mut entries {
                entry.insert("enabled".to_string(), json!(false));
            }
            mark(
                &mut entries,
                "file-disabled",
                "disableAllHooks in user settings",
            );
        }
        rows.extend(entries);
    }
    alias_badges(&mut rows);
    (rows, repository_disabled)
}

fn copilot_plugin_directories(context: &Context, budget: &mut Budget) -> Vec<PathBuf> {
    let installed = context.copilot_home().join("installed-plugins");
    let mut found = Vec::new();
    for marketplace in scan_dir_directories(budget, &installed) {
        found.extend(scan_dir_directories(budget, &marketplace));
    }
    found.truncate(MAX_ITEMS_PER_SOURCE);
    found
}

fn copilot_plugin_name(budget: &mut Budget, plugin: &Path) -> String {
    for parts in COPILOT_MANIFESTS {
        let mut manifest_path = plugin.to_path_buf();
        for part in parts {
            manifest_path = manifest_path.join(part);
        }
        if !existing_file(budget, &manifest_path) || !budget.take_source() {
            continue;
        }
        if let Some(manifest) = load_json(budget, &manifest_path, false, None)
            && let Some(name) = manifest.get("name").and_then(Value::as_str)
        {
            return name.to_string();
        }
    }
    plugin
        .file_name()
        .unwrap_or_default()
        .to_string_lossy()
        .to_string()
}

fn copilot_plugin_rows(context: &Context, budget: &mut Budget) -> Vec<Map<String, Value>> {
    let mut rows: Vec<Map<String, Value>> = Vec::new();
    for plugin in copilot_plugin_directories(context, budget) {
        let name = copilot_plugin_name(budget, &plugin);
        for parts in [
            ["hooks.json"].as_slice(),
            ["hooks", "hooks.json"].as_slice(),
        ] {
            let mut path = plugin.clone();
            for part in parts {
                path = path.join(part);
            }
            if !existing_file(budget, &path) {
                continue;
            }
            budget.note_source("copilot-cli", &absolute(&path));
            if !budget.take_source() {
                continue;
            }
            let Some(document) = load_json(budget, &path, false, None) else {
                continue;
            };
            let Some(mapping) = hook_mapping(&document) else {
                continue;
            };
            let mut entries = event_map_rows(
                budget,
                &MapSpec::new("copilot-cli", &path, "plugin", &copilot_known),
                &mapping,
            );
            for entry in &mut entries {
                entry.insert("enabled".to_string(), Value::Null);
                entry.insert("plugin".to_string(), json!(name));
            }
            mark(
                &mut entries,
                "enable-state-undocumented",
                "plugin enable state has no documented on-disk location",
            );
            rows.extend(entries);
            break;
        }
    }
    alias_badges(&mut rows);
    rows
}

pub fn copilot_cli(context: &Context, budget: &mut Budget) -> Vec<Map<String, Value>> {
    let policy = if context.user_lane() {
        copilot_policy_rows(context, budget)
    } else {
        Vec::new()
    };
    let files = copilot_file_rows(context, budget);
    let (settings, repository_disabled) = copilot_settings_rows(context, budget);
    let plugins = if context.user_lane() {
        copilot_plugin_rows(context, budget)
    } else {
        Vec::new()
    };
    let mut others: Vec<Map<String, Value>> = Vec::new();
    others.extend(files);
    others.extend(settings);
    others.extend(plugins);
    if repository_disabled {
        for entry in &mut others {
            entry.insert("enabled".to_string(), json!(false));
        }
        mark(
            &mut others,
            "repository-disabled",
            "disableAllHooks in repository settings",
        );
    }
    let mut rows = policy;
    rows.extend(others);
    rows
}

pub fn antigravity(context: &Context, budget: &mut Budget) -> Vec<Map<String, Value>> {
    let sources = scoped(
        context,
        context
            .home
            .join(".gemini")
            .join("config")
            .join("hooks.json"),
        &[".agents", "hooks.json"],
        true,
    );
    let loader = |budget: &mut Budget, path: &Path| load_json(budget, path, false, None);
    let known = |event: &str| ANTIGRAVITY_EVENTS.contains(&event);
    let mut rows: Vec<Map<String, Value>> = Vec::new();
    for (path, scope, document) in documents(budget, "antigravity", &sources, &loader) {
        for (name, definition) in document.iter().take(MAX_ITEMS_PER_SOURCE) {
            let Some(definition) = definition.as_object() else {
                continue;
            };
            let enabled_default = match definition.get("enabled") {
                Some(Value::Bool(flag)) => Some(*flag),
                _ => None,
            };
            let mapping: Map<String, Value> = ANTIGRAVITY_EVENTS
                .iter()
                .filter_map(|event| {
                    definition
                        .get(*event)
                        .filter(|value| value.is_array())
                        .map(|value| ((*event).to_string(), value.clone()))
                })
                .collect();
            let mut entries = event_map_rows(
                budget,
                &MapSpec {
                    enabled_default,
                    namespace: name,
                    ..MapSpec::new("antigravity", &path, &scope, &known)
                },
                &mapping,
            );
            for entry in &mut entries {
                entry.insert("group".to_string(), json!(name));
                entry.insert("note".to_string(), json!(truncated(name, 80)));
            }
            rows.extend(entries);
        }
    }
    rows
}

fn code_hosted(
    budget: &mut Budget,
    agent: &str,
    directories: &[Source],
    suffixes: &[&str],
    depth: usize,
    note: &str,
) -> Vec<Map<String, Value>> {
    let mut rows = Vec::new();
    for (directory, scope) in directories {
        let mut files: Vec<PathBuf> = Vec::new();
        for suffix in suffixes {
            files.extend(scan_dir(budget, directory, suffix, depth));
        }
        if files.is_empty() || !budget.take_source() || !budget.take_row() {
            continue;
        }
        rows.push(location_row(
            budget,
            agent,
            directory,
            scope,
            note,
            files.len(),
        ));
    }
    rows
}

fn declared_rows(
    budget: &mut Budget,
    agent: &str,
    sources: &[Source],
    keys: &[&str],
    note: &str,
) -> Vec<Map<String, Value>> {
    let loader = |budget: &mut Budget, path: &Path| load_json(budget, path, true, None);
    let mut rows = Vec::new();
    for (path, scope, document) in documents(budget, agent, sources, &loader) {
        let mut declared = 0usize;
        for key in keys {
            if let Some(Value::Array(items)) = document.get(*key) {
                declared += items.len();
            }
        }
        if declared > 0 && budget.take_row() {
            rows.push(location_row(budget, agent, &path, &scope, note, declared));
        }
    }
    rows
}

pub fn opencode(context: &Context, budget: &mut Budget) -> Vec<Map<String, Value>> {
    let config_home = context.config_home();
    let directories = scoped(
        context,
        config_home.join("opencode").join("plugins"),
        &[".opencode", "plugins"],
        true,
    );
    let mut rows = code_hosted(
        budget,
        "opencode",
        &directories,
        &[".js", ".ts"],
        1,
        "plugin directory, hooks defined in code",
    );
    let declared = scoped(
        context,
        config_home.join("opencode").join("opencode.json"),
        &["opencode.json"],
        true,
    );
    rows.extend(declared_rows(
        budget,
        "opencode",
        &declared,
        &["plugin"],
        "declared plugin packages",
    ));
    rows
}

pub fn pi(context: &Context, budget: &mut Budget) -> Vec<Map<String, Value>> {
    let directories = scoped(
        context,
        context.home.join(".pi").join("agent").join("extensions"),
        &[".pi", "extensions"],
        true,
    );
    let mut rows = code_hosted(
        budget,
        "pi",
        &directories,
        &[".ts", ".js"],
        2,
        "extension directory, hooks defined in code",
    );
    let declared = scoped(
        context,
        context.home.join(".pi").join("agent").join("settings.json"),
        &[".pi", "settings.json"],
        true,
    );
    rows.extend(declared_rows(
        budget,
        "pi",
        &declared,
        &["extensions", "packages"],
        "declared extensions and packages",
    ));
    rows
}

pub type Adapter = fn(&Context, &mut Budget) -> Vec<Map<String, Value>>;

pub const ADAPTERS: [(&str, Adapter); 6] = [
    ("claude-code", claude_code),
    ("codex", codex),
    ("copilot-cli", copilot_cli),
    ("antigravity", antigravity),
    ("opencode", opencode),
    ("pi", pi),
];
