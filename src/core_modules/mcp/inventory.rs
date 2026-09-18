use super::model::{Definition, SCHEMA_VERSION, core_agent_id, digest_bytes};
use super::parsers::{ParseFailure, parse_json, parse_jsonc, parse_toml};
use super::safeio::{
    Deadline, Options, absolute, artifact_metrics, bounded_directories, bounded_files,
    bounded_read, safe_relative_file,
};
use super::value::Cfg;
use crate::core_modules::canonical::canonical_json;
use crate::core_modules::watch::WatchPlan;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};

pub const MAX_CLAUDE_BYTES: usize = 4 * 1024 * 1024;
pub const MAX_CONFIG_BYTES: usize = 512 * 1024;
pub const MAX_REGISTRY_BYTES: usize = 1024 * 1024;
pub const MAX_SOURCES: usize = 256;
pub const MAX_DEFINITIONS: usize = 1024;
pub const MAX_WARNINGS: usize = 128;
pub const MAX_PLUGIN_DIRS: usize = 128;
pub const MAX_PROFILE_FILES: usize = 128;
pub const MAX_OUTPUT_BYTES: usize = 512 * 1024;
pub const MAX_ENVIRONMENT_PATH_CHARS: usize = 4096;
pub const DEADLINE_MILLISECONDS: u64 = 2000;

pub const PROJECT_SCOPES: [&str; 2] = ["project", "local"];

const USER_SOURCE_KINDS: [&str; 14] = [
    "codex-profile",
    "codex-plugin-manifest",
    "claude-plugin-registry",
    "claude-plugin-manifest",
    "claude-plugin",
    "copilot-plugin-manifest",
    "copilot-plugin",
    "copilot-user",
    "copilot-managed-settings",
    "antigravity-cli-plugin",
    "antigravity-user",
    "opencode-user",
    "pi-shared-global",
    "pi-agents-global",
];

#[derive(Clone, Copy)]
pub struct PluginSource<'a> {
    pub agent: &'a str,
    pub plugin: &'a Path,
    pub manifest: &'a Cfg,
    pub manifest_path: &'a Path,
    pub source_kind: &'a str,
    pub support: &'a str,
    pub priority: i64,
    pub enabled: Option<bool>,
    pub precedence_known: bool,
}

pub struct Timeout;

type Timed<T> = Result<T, Timeout>;

pub type Environ = BTreeMap<OsString, OsString>;

fn project_source_kind(source_kind: &str) -> bool {
    source_kind == "project"
        || source_kind.ends_with("-project")
        || source_kind.ends_with("-project-override")
}

fn user_source_kind(source_kind: &str) -> bool {
    USER_SOURCE_KINDS.contains(&source_kind)
}

pub fn read_environment_path(environment: &Environ, name: &str) -> Option<String> {
    let value = environment.get(&OsString::from(name))?;
    let text = value.to_str()?;
    if text.is_empty() || text.contains('\0') || text.chars().count() > MAX_ENVIRONMENT_PATH_CHARS {
        return None;
    }
    Some(text.to_string())
}

fn string_list(value: Option<&Cfg>) -> Vec<String> {
    let Some(Cfg::Array(items)) = value else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(Cfg::as_str)
        .map(str::to_string)
        .take(1024)
        .collect()
}

fn python_text(value: &Cfg) -> String {
    match value {
        Cfg::Str(text) => text.clone(),
        Cfg::Bool(true) => "True".to_string(),
        Cfg::Bool(false) => "False".to_string(),
        Cfg::Null => "None".to_string(),
        Cfg::Num(number) => {
            if number.is_f64() {
                crate::core_modules::canonical::python_float_repr(
                    number.as_f64().unwrap_or_default(),
                )
            } else {
                number.to_string()
            }
        }
        Cfg::Stamp { text, .. } => text.clone(),
        _ => String::new(),
    }
}

fn truthy_text(value: Option<&Cfg>) -> Option<String> {
    let value = value?;
    let text = python_text(value);
    if matches!(value, Cfg::Null | Cfg::Bool(false)) || text.is_empty() {
        return None;
    }
    Some(text)
}

fn enabled_of(config: &Cfg) -> (Option<bool>, bool) {
    if !config.is_table() {
        return (None, false);
    }
    if let Some(value) = config.get("disabled") {
        return match value.as_bool() {
            Some(flag) => (Some(!flag), true),
            None => (None, false),
        };
    }
    if let Some(value) = config.get("enabled") {
        return match value.as_bool() {
            Some(flag) => (Some(flag), true),
            None => (None, false),
        };
    }
    (Some(true), true)
}

fn transport_of(config: &Cfg) -> (String, bool) {
    if !config.is_table() {
        return ("unknown".to_string(), false);
    }
    let kind = config
        .get("type")
        .map(python_text)
        .unwrap_or_default()
        .to_lowercase();
    let named = |value: &str| (value.to_string(), true);
    if config.contains("socket") {
        return named("unix");
    }
    if config.contains("httpUrl") {
        return named("streamable-http");
    }
    if config.contains("serverUrl") {
        if matches!(
            kind.as_str(),
            "sse" | "websocket" | "ws" | "streamable-http" | "http"
        ) {
            if kind == "websocket" || kind == "ws" {
                return named("websocket");
            }
            return named(if kind == "sse" {
                "sse"
            } else {
                "streamable-http"
            });
        }
        return named("unknown");
    }
    if config.contains("url") {
        return named(if kind == "sse" {
            "sse"
        } else {
            "streamable-http"
        });
    }
    if config.contains("command") {
        return named("stdio");
    }
    if kind == "local" || kind == "stdio" {
        return ("stdio".to_string(), false);
    }
    if kind == "remote" || kind == "http" {
        return ("streamable-http".to_string(), false);
    }
    if kind == "sse" {
        return ("sse".to_string(), false);
    }
    ("unknown".to_string(), false)
}

fn endpoint_signature(config: &Cfg) -> Option<String> {
    if !config.is_table() {
        return None;
    }
    for key in ["httpUrl", "serverUrl", "url"] {
        if let Some(Cfg::Str(value)) = config.get(key) {
            return Some(super::model::digest(&["endpoint-v1", "remote", value]));
        }
    }
    let command = config.get("command");
    let arguments = config.get("args");
    if let Some(Cfg::Array(items)) = command
        && items.iter().all(|item| item.as_str().is_some())
    {
        let mut parts: Vec<&str> = vec!["endpoint-v1", "stdio"];
        let texts: Vec<&str> = items.iter().filter_map(Cfg::as_str).collect();
        parts.extend(texts);
        return Some(super::model::digest(&parts));
    }
    if let Some(Cfg::Str(text)) = command {
        let items = match arguments {
            None => Vec::new(),
            Some(Cfg::Array(items)) => {
                if !items.iter().all(|item| item.as_str().is_some()) {
                    return None;
                }
                items.iter().filter_map(Cfg::as_str).collect()
            }
            Some(_) => return None,
        };
        let mut parts: Vec<&str> = vec!["endpoint-v1", "stdio", text];
        parts.extend(items);
        return Some(super::model::digest(&parts));
    }
    None
}

fn secret_presence(config: &Cfg) -> (bool, bool, bool) {
    if !config.is_table() {
        return (false, false, false);
    }
    let keys: BTreeSet<String> = config.keys().iter().map(|key| key.to_lowercase()).collect();
    let any = |names: &[&str]| names.iter().any(|name| keys.contains(*name));
    (
        any(&["env", "environment", "env_vars", "envvars"]),
        any(&[
            "headers",
            "http_headers",
            "env_http_headers",
            "headershelper",
        ]),
        any(&[
            "oauth",
            "auth",
            "bearertoken",
            "bearer_token",
            "bearertokenenv",
            "bearer_token_env_var",
            "clientsecret",
            "client_secret",
            "authprovidertype",
        ]),
    )
}

fn lane_selects(scope: &str, lane: &str) -> bool {
    let project = PROJECT_SCOPES.contains(&scope);
    match lane {
        "project" => project,
        "user" => !project,
        _ => true,
    }
}

pub struct Settings {
    pub project: PathBuf,
    pub home: PathBuf,
    pub config_home: PathBuf,
    pub etc_root: PathBuf,
    pub codex_home: PathBuf,
    pub system_owner_uid: u32,
    pub scope: String,
    pub environment: Environ,
}

pub struct Inventory {
    pub settings: Settings,
    pub state_home: PathBuf,
    pub deadline: Deadline,
    pub plan: WatchPlan,
    pub config_paths: BTreeMap<PathBuf, BTreeSet<String>>,
    pub definitions: Vec<Definition>,
    pub warnings: Vec<Value>,
    pub agent_status: BTreeMap<String, Value>,
    pub sources: usize,
    pub source_metrics: BTreeMap<Vec<u8>, Map<String, Value>>,
    pub truncated: bool,
}

pub struct ServerBatch<'a> {
    pub agent: &'a str,
    pub mapping: Option<&'a Cfg>,
    pub scope: &'a str,
    pub source_kind: &'a str,
    pub path: &'a Path,
    pub support: &'a str,
    pub priority: i64,
    pub trusted: Option<bool>,
    pub enabled_override: Option<bool>,
    pub enabled_known: bool,
    pub precedence_known: bool,
    pub trust_from_config: bool,
    pub trust_gates_effective: bool,
    pub trust_role: &'a str,
}

impl<'a> ServerBatch<'a> {
    pub fn new(
        agent: &'a str,
        mapping: Option<&'a Cfg>,
        scope: &'a str,
        source_kind: &'a str,
        path: &'a Path,
        support: &'a str,
        priority: i64,
    ) -> Self {
        Self {
            agent,
            mapping,
            scope,
            source_kind,
            path,
            support,
            priority,
            trusted: Some(true),
            enabled_override: None,
            enabled_known: true,
            precedence_known: true,
            trust_from_config: false,
            trust_gates_effective: true,
            trust_role: "source-approval",
        }
    }
}

impl Inventory {
    pub fn new(settings: Settings) -> Self {
        let state_home = read_environment_path(&settings.environment, "XDG_STATE_HOME")
            .map(|value| absolute(Path::new(&value)))
            .unwrap_or_else(|| settings.home.join(".local").join("state"));
        Self {
            settings,
            state_home,
            deadline: Deadline::new(DEADLINE_MILLISECONDS),
            plan: WatchPlan::new(),
            config_paths: BTreeMap::new(),
            definitions: Vec::new(),
            warnings: Vec::new(),
            agent_status: BTreeMap::new(),
            sources: 0,
            source_metrics: BTreeMap::new(),
            truncated: false,
        }
    }

    pub fn recovery_directory(&self) -> PathBuf {
        self.state_home.join("fileblade").join("mcp-recovery")
    }

    fn check(&self) -> Timed<()> {
        if self.deadline.expired() {
            return Err(Timeout);
        }
        Ok(())
    }

    pub fn logical_path(&self, path: &Path) -> Vec<u8> {
        let target = absolute(path);
        let anchors: [(&Path, &[u8]); 3] = [
            (&self.settings.project, b"<project>"),
            (&self.settings.home, b"~"),
            (&self.settings.codex_home, b"<codex-home>"),
        ];
        for (root, label) in anchors {
            let Ok(relative) = target.strip_prefix(root) else {
                continue;
            };
            let bytes = relative.as_os_str().as_bytes();
            if bytes.is_empty() {
                return label.to_vec();
            }
            let mut result = label.to_vec();
            result.push(b'/');
            result.extend_from_slice(bytes);
            return result;
        }
        target.as_os_str().as_bytes().to_vec()
    }

    fn warn(&mut self, agent: &str, source_kind: &str, path: &Path, code: &str) {
        if self.warnings.len() >= MAX_WARNINGS {
            self.truncated = true;
            return;
        }
        let logical = self.logical_path(path);
        let trimmed: String = code.chars().take(64).collect();
        self.warnings.push(json!({
            "code": trimmed,
            "sourceId": digest_bytes(&[b"source-v1", agent.as_bytes(), source_kind.as_bytes(), &logical]),
        }));
    }

    fn read_document(
        &mut self,
        agent: &str,
        source_kind: &str,
        path: &Path,
        parser: fn(&[u8]) -> Result<Cfg, ParseFailure>,
        limit: usize,
        secure_managed: bool,
    ) -> Timed<Option<Cfg>> {
        self.check()?;
        if self.settings.scope == "user" && project_source_kind(source_kind) {
            return Ok(None);
        }
        if self.settings.scope == "project" && user_source_kind(source_kind) {
            return Ok(None);
        }
        self.config_paths
            .entry(absolute(path))
            .or_default()
            .insert(core_agent_id(agent));
        if self.sources >= MAX_SOURCES {
            self.truncated = true;
            return Ok(None);
        }
        let options = if secure_managed {
            Options::managed(self.settings.system_owner_uid)
        } else {
            Options::default()
        };
        let result = bounded_read(&mut self.plan, path, limit, &options);
        if result.error == Some("missing") {
            return Ok(None);
        }
        self.sources += 1;
        let logical = self.logical_path(path);
        if let Some(data) = &result.data {
            let metrics = artifact_metrics(path, data);
            self.source_metrics.insert(logical, metrics);
        }
        let Some(data) = result.data else {
            self.warn(
                agent,
                source_kind,
                path,
                result.error.unwrap_or("unreadable"),
            );
            return Ok(None);
        };
        match parser(&data) {
            Ok(value) => Ok(Some(value)),
            Err(error) => {
                self.warn(agent, source_kind, path, error.code());
                Ok(None)
            }
        }
    }

    fn add_servers(&mut self, batch: ServerBatch<'_>) -> Timed<()> {
        let Some(mapping) = batch.mapping else {
            return Ok(());
        };
        if !mapping.is_table() || !lane_selects(batch.scope, &self.settings.scope) {
            return Ok(());
        }
        let logical = self.logical_path(batch.path);
        let metrics = self
            .source_metrics
            .get(&logical)
            .cloned()
            .unwrap_or_default();
        let mut names = mapping.keys();
        names.sort();
        for name in names {
            self.check()?;
            if self.definitions.len() >= MAX_DEFINITIONS {
                self.truncated = true;
                return Ok(());
            }
            let config = mapping.get(&name).cloned().unwrap_or(Cfg::Null);
            let (mut enabled, enabled_valid) = enabled_of(&config);
            if !batch.enabled_known {
                enabled = None;
            }
            if let Some(override_value) = batch.enabled_override {
                enabled = match enabled {
                    Some(value) => Some(value && override_value),
                    None => Some(override_value),
                };
            }
            let mut trusted = batch.trusted;
            if batch.trust_from_config && config.is_table() {
                trusted = match config.get("trust") {
                    None => Some(false),
                    Some(value) => value.as_bool(),
                };
            }
            let (transport, transport_valid) = transport_of(&config);
            let definition = Definition {
                agent: batch.agent.to_string(),
                raw_name: name.clone(),
                scope: batch.scope.to_string(),
                source_kind: batch.source_kind.to_string(),
                source_path: logical.clone(),
                support: batch.support.to_string(),
                transport,
                enabled,
                trusted,
                valid: config.is_table() && enabled_valid && transport_valid,
                priority: batch.priority,
                precedence_known: batch.precedence_known,
                trust_gates_effective: batch.trust_gates_effective,
                trust_role: batch.trust_role.to_string(),
                endpoint_signature: endpoint_signature(&config),
                secret_presence: secret_presence(&config),
                metrics: metrics.clone(),
                raw_config: config.is_table().then(|| config.clone()),
                absolute_path: Some(absolute(batch.path)),
                ..Definition::default()
            }
            .finish();
            self.definitions.push(definition);
        }
        Ok(())
    }

    pub fn ancestor_directories(&mut self) -> Vec<PathBuf> {
        let mut current = self.settings.project.clone();
        let mut chain = vec![current.clone()];
        while current.parent().is_some_and(|parent| parent != current) && chain.len() < 64 {
            if self.settings.scope != "user" {
                self.plan.watch_path(&current.join(".git"), false);
            }
            if current.join(".git").exists() {
                break;
            }
            current = current.parent().unwrap_or(Path::new("/")).to_path_buf();
            chain.push(current.clone());
        }
        chain.reverse();
        chain
    }

    pub fn filesystem_ancestors(&self) -> Vec<PathBuf> {
        let mut current = self.settings.project.clone();
        let mut chain = vec![current.clone()];
        while current.parent().is_some_and(|parent| parent != current) && chain.len() < 64 {
            current = current.parent().unwrap_or(Path::new("/")).to_path_buf();
            chain.push(current.clone());
        }
        chain.reverse();
        chain
    }

    fn nested_directories(&mut self, root: &Path, depth: usize) -> Vec<PathBuf> {
        let mut current = vec![root.to_path_buf()];
        for _ in 0..depth {
            let mut following: Vec<PathBuf> = Vec::new();
            for parent in &current {
                let remaining = MAX_PLUGIN_DIRS.saturating_sub(following.len());
                if remaining == 0 {
                    self.truncated = true;
                    break;
                }
                let found =
                    bounded_directories(&mut self.plan, parent, remaining, &mut self.deadline);
                following.extend(found);
            }
            current = following;
            if current.is_empty() {
                break;
            }
        }
        current
    }
}

impl Inventory {
    fn plugin_source(&mut self, plugin: &PluginSource<'_>) -> Timed<()> {
        let PluginSource {
            agent,
            plugin: plugin_root,
            manifest,
            manifest_path,
            source_kind,
            support,
            priority,
            enabled,
            precedence_known,
        } = *plugin;
        let plugin = plugin_root;
        let declared = manifest.get("mcpServers").cloned();
        if let Some(mapping) = declared.as_ref().filter(|value| value.is_table()) {
            let inline = format!("{source_kind}-inline");
            let mut batch = ServerBatch::new(
                agent,
                Some(mapping),
                "plugin",
                &inline,
                manifest_path,
                support,
                priority,
            );
            batch.enabled_override = enabled;
            batch.precedence_known = precedence_known;
            return self.add_servers(batch);
        }
        let mut candidates: Vec<PathBuf> = Vec::new();
        match declared.as_ref() {
            Some(Cfg::Str(relative)) => match safe_relative_file(plugin, relative) {
                Some(candidate) => candidates.push(candidate),
                None => {
                    self.warn(agent, source_kind, manifest_path, "unsafe-plugin-path");
                    return Ok(());
                }
            },
            Some(Cfg::Array(items)) => {
                for value in items.iter().take(16) {
                    let candidate = value
                        .as_str()
                        .and_then(|relative| safe_relative_file(plugin, relative));
                    match candidate {
                        Some(candidate) => candidates.push(candidate),
                        None => self.warn(agent, source_kind, manifest_path, "unsafe-plugin-path"),
                    }
                }
            }
            _ => {
                candidates.push(plugin.join(".mcp.json"));
                candidates.push(plugin.join(".github").join("mcp.json"));
            }
        }
        for path in candidates {
            let document = self.read_document(
                agent,
                source_kind,
                &path,
                parse_json,
                MAX_CONFIG_BYTES,
                false,
            )?;
            let Some(document) = document.filter(|value| !value.keys().is_empty()) else {
                continue;
            };
            let mapping = match document.get("mcpServers") {
                Some(value) if value.is_table() => value.clone(),
                _ => match document.get("mcp_servers") {
                    Some(value) if value.is_table() => value.clone(),
                    _ => document.clone(),
                },
            };
            let mut batch = ServerBatch::new(
                agent,
                Some(&mapping),
                "plugin",
                source_kind,
                &path,
                support,
                priority,
            );
            batch.enabled_override = enabled;
            batch.precedence_known = precedence_known;
            return self.add_servers(batch);
        }
        Ok(())
    }

    fn settings_plugin_states(
        &mut self,
        paths: &[PathBuf],
        agent: &str,
    ) -> Timed<BTreeMap<String, bool>> {
        let mut states: BTreeMap<String, bool> = BTreeMap::new();
        let kind = format!("{agent}-settings");
        for path in paths {
            let document =
                self.read_document(agent, &kind, path, parse_json, MAX_CONFIG_BYTES, false)?;
            let Some(document) = document.filter(|value| !value.keys().is_empty()) else {
                continue;
            };
            if let Some(Cfg::Table(entries)) = document.get("enabledPlugins") {
                for (key, value) in entries {
                    if let Some(flag) = value.as_bool() {
                        states.insert(key.clone(), flag);
                    }
                }
            }
            for key in string_list(document.get("disabledPlugins")) {
                states.insert(key, false);
            }
        }
        Ok(states)
    }

    fn apply_name_policy(&mut self, agent: &str, document: Option<&Cfg>) {
        let Some(document) = document.filter(|value| !value.keys().is_empty()) else {
            return;
        };
        let allowed_value = document.get("allowedMcpServers");
        let denied_value = document.get("deniedMcpServers");
        let names = |value: Option<&Cfg>| -> (BTreeSet<String>, bool) {
            let mut result: BTreeSet<String> = BTreeSet::new();
            let mut ambiguous = false;
            let Some(Cfg::Array(items)) = value else {
                return (result, false);
            };
            for entry in items.iter().take(1024) {
                if !entry.is_table() {
                    ambiguous = true;
                    continue;
                }
                let name = entry.get("serverName").and_then(Cfg::as_str);
                let other = entry.keys().into_iter().any(|key| key != "serverName");
                match name {
                    Some(name) if !other => {
                        result.insert(name.to_string());
                    }
                    _ => ambiguous = true,
                }
            }
            (result, ambiguous)
        };
        let (allowed, allowed_ambiguous) = names(allowed_value);
        let (denied, denied_ambiguous) = names(denied_value);
        let allowed_list = matches!(allowed_value, Some(Cfg::Array(_)));
        let allowed_empty = matches!(allowed_value, Some(Cfg::Array(items)) if items.is_empty());
        for definition in &mut self.definitions {
            if definition.agent != agent {
                continue;
            }
            if denied.contains(&definition.raw_name) && !denied_ambiguous {
                definition.enabled = Some(false);
                continue;
            }
            if allowed_list {
                if allowed_empty || (!allowed_ambiguous && !allowed.contains(&definition.raw_name))
                {
                    definition.enabled = Some(false);
                } else if allowed_ambiguous && definition.enabled != Some(false) {
                    definition.enabled = None;
                }
            }
            if denied_ambiguous && definition.enabled != Some(false) {
                definition.enabled = None;
            }
        }
    }

    fn installed_plugin_paths(&self, document: &Cfg, root: &Path) -> Vec<(String, PathBuf)> {
        let mut found: Vec<(String, PathBuf)> = Vec::new();
        walk_plugins(document, "", root, &mut found);
        let mut unique: BTreeMap<Vec<u8>, (String, PathBuf)> = BTreeMap::new();
        for (name, path) in found {
            unique.insert(path.as_os_str().as_bytes().to_vec(), (name, path));
        }
        unique.into_values().collect()
    }

    fn discover_claude(&mut self) -> Timed<()> {
        let agent = "claude";
        let user_path = self.settings.home.join(".claude.json");
        let user = self.read_document(
            agent,
            "claude-user-local",
            &user_path,
            parse_json,
            MAX_CLAUDE_BYTES,
            false,
        )?;
        let mut project_trust = Cfg::table();
        if let Some(user) = user.as_ref().filter(|value| !value.keys().is_empty()) {
            self.add_servers(ServerBatch::new(
                agent,
                user.get("mcpServers"),
                "user",
                "claude-user",
                &user_path,
                "documented",
                30,
            ))?;
            let projects = user.get("projects").cloned().unwrap_or(Cfg::Null);
            let key = self.settings.project.to_string_lossy().into_owned();
            let mut local_mapping: Option<Cfg> = None;
            if let Some(record) = projects.get(&key).filter(|value| value.is_table()) {
                local_mapping = record.get("mcpServers").cloned();
                project_trust = record.clone();
            }
            self.add_servers(ServerBatch::new(
                agent,
                local_mapping.as_ref(),
                "local",
                "claude-local",
                &user_path,
                "documented",
                50,
            ))?;
        }

        let project_path = self.settings.project.join(".mcp.json");
        let project = self.read_document(
            agent,
            "claude-project",
            &project_path,
            parse_json,
            MAX_CONFIG_BYTES,
            false,
        )?;
        let mut approvals: Vec<Cfg> = vec![project_trust];
        let settings_sources: [(&str, PathBuf); 3] = [
            (
                "claude-user-settings",
                self.settings.home.join(".claude").join("settings.json"),
            ),
            (
                "claude-settings-project",
                self.settings.project.join(".claude").join("settings.json"),
            ),
            (
                "claude-settings-local-project",
                self.settings
                    .project
                    .join(".claude")
                    .join("settings.local.json"),
            ),
        ];
        for (kind, path) in &settings_sources {
            let document =
                self.read_document(agent, kind, path, parse_json, MAX_CONFIG_BYTES, false)?;
            if let Some(document) = document.filter(|value| !value.keys().is_empty()) {
                approvals.push(document);
            }
        }
        let approved: BTreeSet<String> = approvals
            .iter()
            .flat_map(|record| string_list(record.get("enabledMcpjsonServers")))
            .collect();
        let denied: BTreeSet<String> = approvals
            .iter()
            .flat_map(|record| string_list(record.get("disabledMcpjsonServers")))
            .collect();
        let approve_all = approvals.iter().any(|record| {
            record
                .get("enableAllProjectMcpServers")
                .and_then(Cfg::as_bool)
                == Some(true)
        });
        if let Some(project) = project.as_ref().filter(|value| !value.keys().is_empty())
            && let Some(mapping) = project.get("mcpServers").filter(|value| value.is_table())
        {
            for (name, config) in mapping.as_table().cloned().unwrap_or_default() {
                let trust = if denied.contains(&name) {
                    Some(false)
                } else if approve_all || approved.contains(&name) {
                    Some(true)
                } else {
                    None
                };
                let single = Cfg::Table(vec![(name.clone(), config.clone())]);
                let mut batch = ServerBatch::new(
                    agent,
                    Some(&single),
                    "project",
                    "claude-project",
                    &project_path,
                    "documented",
                    40,
                );
                batch.trusted = trust;
                self.add_servers(batch)?;
            }
        }

        let plugin_root = self.settings.home.join(".claude").join("plugins");
        let registry_path = plugin_root.join("installed_plugins.json");
        let registry = self.read_document(
            agent,
            "claude-plugin-registry",
            &registry_path,
            parse_json,
            MAX_REGISTRY_BYTES,
            false,
        )?;
        let registry = registry.filter(|value| !value.keys().is_empty());
        let mut plugin_states = if registry.is_some() {
            self.settings_plugin_states(
                &[
                    self.settings.home.join(".claude").join("settings.json"),
                    self.settings.project.join(".claude").join("settings.json"),
                    self.settings
                        .project
                        .join(".claude")
                        .join("settings.local.json"),
                ],
                agent,
            )?
        } else {
            BTreeMap::new()
        };
        let managed_settings_path = self
            .settings
            .etc_root
            .join("claude-code")
            .join("managed-settings.json");
        let managed_settings = self.read_document(
            agent,
            "claude-managed-settings",
            &managed_settings_path,
            parse_json,
            MAX_CONFIG_BYTES,
            true,
        )?;
        let managed_settings = managed_settings.filter(|value| !value.keys().is_empty());
        if let Some(document) = managed_settings.as_ref()
            && let Some(Cfg::Table(entries)) = document.get("enabledPlugins")
        {
            for (key, value) in entries {
                if let Some(flag) = value.as_bool() {
                    plugin_states.insert(key.clone(), flag);
                }
            }
        }
        if let Some(registry) = registry.as_ref() {
            for (plugin_id, plugin_path) in self.installed_plugin_paths(registry, &plugin_root) {
                let first = self.definitions.len();
                let enabled = plugin_states.get(&plugin_id).copied();
                let manifest_path = plugin_path.join(".claude-plugin").join("plugin.json");
                let manifest = self.read_document(
                    agent,
                    "claude-plugin-manifest",
                    &manifest_path,
                    parse_json,
                    MAX_CONFIG_BYTES,
                    false,
                )?;
                let manifest = manifest.filter(|value| !value.keys().is_empty());
                if let Some(manifest) = manifest
                    .as_ref()
                    .filter(|value| value.contains("mcpServers"))
                {
                    self.plugin_source(&PluginSource {
                        agent,
                        plugin: &plugin_path,
                        manifest,
                        manifest_path: &manifest_path,
                        source_kind: "claude-plugin",
                        support: "documented",
                        priority: 20,
                        enabled,
                        precedence_known: true,
                    })?;
                } else {
                    let mcp_path = plugin_path.join(".mcp.json");
                    let mcp = self.read_document(
                        agent,
                        "claude-plugin",
                        &mcp_path,
                        parse_json,
                        MAX_CONFIG_BYTES,
                        false,
                    )?;
                    if let Some(mcp) = mcp.filter(|value| !value.keys().is_empty()) {
                        let mut batch = ServerBatch::new(
                            agent,
                            mcp.get("mcpServers"),
                            "plugin",
                            "claude-plugin",
                            &mcp_path,
                            "documented",
                            20,
                        );
                        batch.enabled_override = enabled;
                        self.add_servers(batch)?;
                    }
                }
                let label = manifest
                    .as_ref()
                    .and_then(|value| truthy_text(value.get("name")))
                    .unwrap_or_else(|| {
                        plugin_id
                            .split_once('@')
                            .map(|(head, _)| head.to_string())
                            .unwrap_or_else(|| plugin_id.clone())
                    });
                for definition in &mut self.definitions[first..] {
                    definition.plugin_name = label.clone();
                }
            }
        }

        let managed_path = self
            .settings
            .etc_root
            .join("claude-code")
            .join("managed-mcp.json");
        let managed = self.read_document(
            agent,
            "claude-managed",
            &managed_path,
            parse_json,
            MAX_CONFIG_BYTES,
            true,
        )?;
        if let Some(managed) = managed.filter(|value| !value.keys().is_empty()) {
            self.add_servers(ServerBatch::new(
                agent,
                managed.get("mcpServers"),
                "managed",
                "claude-managed",
                &managed_path,
                "documented",
                100,
            ))?;
            for definition in &mut self.definitions {
                if definition.agent == agent && definition.scope != "managed" {
                    definition.selected = Some(false);
                }
            }
        }
        self.apply_name_policy(agent, managed_settings.as_ref());
        Ok(())
    }
}

fn walk_plugins(value: &Cfg, label: &str, root: &Path, found: &mut Vec<(String, PathBuf)>) {
    if found.len() >= MAX_PLUGIN_DIRS {
        return;
    }
    match value {
        Cfg::Table(entries) => {
            let install = value
                .get("installPath")
                .and_then(|item| truthy_text(Some(item)))
                .or_else(|| value.get("path").and_then(|item| truthy_text(Some(item))));
            if let Some(install) = install {
                let candidate = Path::new(&install);
                let candidate = if candidate.is_absolute() {
                    candidate.to_path_buf()
                } else {
                    root.join(candidate)
                };
                let candidate = absolute(&candidate);
                if candidate.strip_prefix(absolute(root)).is_err() {
                    return;
                }
                let name = truthy_text(value.get("id"))
                    .or_else(|| truthy_text(value.get("name")))
                    .or_else(|| {
                        if label.is_empty() {
                            None
                        } else {
                            Some(label.to_string())
                        }
                    })
                    .unwrap_or_else(|| {
                        candidate
                            .file_name()
                            .map(|name| name.to_string_lossy().into_owned())
                            .unwrap_or_default()
                    });
                found.push((name, candidate));
                return;
            }
            for (key, child) in entries.iter().take(MAX_PLUGIN_DIRS) {
                walk_plugins(child, key, root, found);
            }
        }
        Cfg::Array(items) => {
            for child in items.iter().take(MAX_PLUGIN_DIRS) {
                walk_plugins(child, label, root, found);
            }
        }
        _ => {}
    }
}

impl Inventory {
    fn discover_codex(&mut self) -> Timed<()> {
        let agent = "codex";
        let user_path = self.settings.home.join(".codex").join("config.toml");
        let user = self
            .read_document(
                agent,
                "codex-user",
                &user_path,
                parse_toml,
                MAX_CONFIG_BYTES,
                false,
            )?
            .filter(|value| !value.keys().is_empty());
        if let Some(user) = user.as_ref() {
            self.add_servers(ServerBatch::new(
                agent,
                user.get("mcp_servers"),
                "user",
                "codex-user",
                &user_path,
                "documented",
                20,
            ))?;
        }
        let mut trust: Option<bool> = None;
        if let Some(user) = user.as_ref() {
            let key = self.settings.project.to_string_lossy().into_owned();
            if let Some(record) = user
                .get("projects")
                .and_then(|projects| projects.get(&key))
                .filter(|value| value.is_table())
            {
                trust = match record.get("trust_level").and_then(Cfg::as_str) {
                    Some("trusted") => Some(true),
                    Some("untrusted") => Some(false),
                    _ => None,
                };
            }
        }
        for (index, directory) in self.ancestor_directories().into_iter().enumerate() {
            let path = directory.join(".codex").join("config.toml");
            let document = self
                .read_document(
                    agent,
                    "codex-project",
                    &path,
                    parse_toml,
                    MAX_CONFIG_BYTES,
                    false,
                )?
                .filter(|value| !value.keys().is_empty());
            if let Some(document) = document {
                let mut batch = ServerBatch::new(
                    agent,
                    document.get("mcp_servers"),
                    "project",
                    "codex-project",
                    &path,
                    "documented",
                    40 + index as i64,
                );
                batch.trusted = trust;
                self.add_servers(batch)?;
            }
        }

        let profile_suffix = ".config.toml";
        let profiles = if self.settings.scope != "project" {
            let codex_home = self.settings.codex_home.clone();
            bounded_files(
                &mut self.plan,
                &codex_home,
                MAX_PROFILE_FILES,
                &mut self.deadline,
            )
        } else {
            Vec::new()
        };
        for path in profiles {
            let name = path.file_name().unwrap_or_default().as_bytes().to_vec();
            if name.len() <= profile_suffix.len() || !name.ends_with(profile_suffix.as_bytes()) {
                continue;
            }
            let document = self
                .read_document(
                    agent,
                    "codex-profile",
                    &path,
                    parse_toml,
                    MAX_CONFIG_BYTES,
                    false,
                )?
                .filter(|value| !value.keys().is_empty());
            if let Some(document) = document {
                let mut batch = ServerBatch::new(
                    agent,
                    document.get("mcp_servers"),
                    "profile",
                    "codex-profile",
                    &path,
                    "documented",
                    0,
                );
                batch.enabled_known = false;
                batch.trusted = None;
                batch.precedence_known = false;
                self.add_servers(batch)?;
            }
        }

        let plugin_controls = user
            .as_ref()
            .and_then(|value| value.get("plugins"))
            .filter(|value| value.is_table())
            .cloned()
            .unwrap_or_else(Cfg::table);
        let cache_root = self
            .settings
            .home
            .join(".codex")
            .join("plugins")
            .join("cache");
        let versions = if self.settings.scope != "project" {
            self.nested_directories(&cache_root, 3)
        } else {
            Vec::new()
        };
        for version in versions {
            let plugin = version.parent().unwrap_or(Path::new("/")).to_path_buf();
            let marketplace = plugin.parent().unwrap_or(Path::new("/")).to_path_buf();
            let manifest_path = version.join(".codex-plugin").join("plugin.json");
            let manifest = self
                .read_document(
                    agent,
                    "codex-plugin-manifest",
                    &manifest_path,
                    parse_json,
                    MAX_CONFIG_BYTES,
                    false,
                )?
                .filter(|value| !value.keys().is_empty());
            let Some(manifest) = manifest else { continue };
            let plugin_name = truthy_text(manifest.get("name")).unwrap_or_else(|| {
                plugin
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default()
            });
            let market = marketplace
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            let plugin_id = format!("{plugin_name}@{market}");
            let control = plugin_controls
                .get(&plugin_id)
                .filter(|value| value.is_table())
                .cloned()
                .unwrap_or_else(Cfg::table);
            let plugin_enabled = control.get("enabled").and_then(Cfg::as_bool);
            let before = self.definitions.len();
            self.plugin_source(&PluginSource {
                agent,
                plugin: &version,
                manifest: &manifest,
                manifest_path: &manifest_path,
                source_kind: "codex-plugin",
                support: "documented",
                priority: 10,
                enabled: plugin_enabled,
                precedence_known: false,
            })?;
            let server_controls = control
                .get("mcp_servers")
                .filter(|value| value.is_table())
                .cloned()
                .unwrap_or_else(Cfg::table);
            for definition in &mut self.definitions[before..] {
                let server = server_controls
                    .get(&definition.raw_name)
                    .filter(|value| value.is_table())
                    .cloned()
                    .unwrap_or_else(Cfg::table);
                if let Some(flag) = server.get("enabled").and_then(Cfg::as_bool) {
                    definition.enabled = Some(flag && definition.enabled != Some(false));
                }
            }
        }
        self.agent_status.insert(
            agent.to_string(),
            json!({
                "id": agent,
                "support": "partial",
                "reason": "plugin-version-profile-selection-and-cli-overrides-unobserved",
            }),
        );
        Ok(())
    }

    fn config_candidates(&self, directory: &Path, name: &str) -> Vec<PathBuf> {
        vec![
            directory.join(format!("{name}.json")),
            directory.join(format!("{name}.jsonc")),
        ]
    }

    fn discover_opencode(&mut self) -> Timed<()> {
        let agent = "opencode";
        let ancestors = self.filesystem_ancestors();
        let mut candidates: Vec<(PathBuf, &str, i64, bool)> = Vec::new();
        let user_directory = self.settings.config_home.join("opencode");
        for path in self.config_candidates(&user_directory, "opencode") {
            candidates.push((path, "user", 100, false));
        }
        for (index, directory) in ancestors.iter().enumerate() {
            for path in self.config_candidates(directory, "opencode") {
                candidates.push((path, "project", 200 + index as i64, false));
            }
        }
        for (index, directory) in ancestors.iter().enumerate() {
            for path in self.config_candidates(&directory.join(".opencode"), "opencode") {
                candidates.push((path, "project", 300 + index as i64, true));
            }
        }
        let mut v1_paths: BTreeSet<PathBuf> = BTreeSet::new();
        for directory in self.ancestor_directories() {
            for path in self.config_candidates(&directory, "opencode") {
                v1_paths.insert(path);
            }
        }
        for (path, scope, priority, dot_directory) in candidates {
            let kind = format!("opencode-{scope}");
            let document = self
                .read_document(agent, &kind, &path, parse_jsonc, MAX_CONFIG_BYTES, false)?
                .filter(|value| !value.keys().is_empty());
            let Some(document) = document else { continue };
            let Some(mcp) = document.get("mcp").filter(|value| value.is_table()) else {
                continue;
            };
            let (mapping, support, source_kind) =
                if mcp.get("servers").is_some_and(|value| value.is_table()) {
                    (
                        mcp.get("servers").cloned().unwrap_or_else(Cfg::table),
                        "beta",
                        format!("opencode-v2-{scope}"),
                    )
                } else {
                    if dot_directory || (scope == "project" && !v1_paths.contains(&path)) {
                        continue;
                    }
                    (mcp.clone(), "documented", format!("opencode-v1-{scope}"))
                };
            self.add_servers(ServerBatch::new(
                agent,
                Some(&mapping),
                scope,
                &source_kind,
                &path,
                support,
                priority,
            ))?;
        }
        self.agent_status.insert(
            agent.to_string(),
            json!({"id": agent, "support": "partial", "reason": "remote-inline-plugin-state-unobserved"}),
        );
        Ok(())
    }

    pub fn pi_packages(&self, document: &Cfg) -> BTreeSet<String> {
        let mut result: BTreeSet<String> = BTreeSet::new();
        let Some(Cfg::Array(packages)) = document.get("packages") else {
            return result;
        };
        for entry in packages.iter().take(MAX_PLUGIN_DIRS) {
            match entry {
                Cfg::Str(value) => {
                    let text = value.strip_prefix("npm:").unwrap_or(value);
                    if text.starts_with('@') {
                        let slash = text.find('/');
                        let version = match slash {
                            Some(slash) => text[slash + 1..].find('@').map(|at| slash + 1 + at),
                            None => None,
                        };
                        result.insert(match version {
                            Some(at) => text[..at].to_string(),
                            None => text.to_string(),
                        });
                    } else {
                        result.insert(
                            text.split_once('@')
                                .map(|(head, _)| head.to_string())
                                .unwrap_or_else(|| text.to_string()),
                        );
                    }
                }
                Cfg::Table(_) => {
                    if entry.get("enabled").and_then(Cfg::as_bool) == Some(false) {
                        continue;
                    }
                    let value =
                        truthy_text(entry.get("name")).or_else(|| truthy_text(entry.get("source")));
                    if let Some(value) = value {
                        let text = value.strip_prefix("npm:").unwrap_or(&value).to_string();
                        result.insert(
                            text.split_once('@')
                                .map(|(head, _)| head.to_string())
                                .unwrap_or(text),
                        );
                    }
                }
                _ => {}
            }
        }
        result
    }

    fn pi_server_map(&self, document: &Cfg) -> Cfg {
        if let Some(value) = document.get("mcpServers").filter(|value| value.is_table()) {
            return value.clone();
        }
        Cfg::Table(
            document
                .as_table()
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter(|(key, _)| !matches!(key.as_str(), "settings" | "imports" | "$schema"))
                .collect(),
        )
    }

    fn discover_pi(&mut self) -> Timed<()> {
        let agent = "pi";
        let agent_dir = self.settings.home.join(".pi").join("agent");
        let settings_path = agent_dir.join("settings.json");
        let settings = self
            .read_document(
                agent,
                "pi-settings",
                &settings_path,
                parse_json,
                MAX_CONFIG_BYTES,
                false,
            )?
            .filter(|value| !value.keys().is_empty());
        let mut packages = self.pi_packages(&settings.unwrap_or_else(Cfg::table));
        let project_settings = self
            .read_document(
                agent,
                "pi-settings-project",
                &self.settings.project.join(".pi").join("settings.json"),
                parse_json,
                MAX_CONFIG_BYTES,
                false,
            )?
            .filter(|value| !value.keys().is_empty());
        packages.extend(self.pi_packages(&project_settings.unwrap_or_else(Cfg::table)));
        let adapter = packages.contains("pi-mcp-adapter");
        let codemode = packages.contains("pi-codemode-mcp");
        if !adapter && !codemode {
            self.agent_status.insert(
                agent.to_string(),
                json!({"id": agent, "support": "unsupported", "reason": "pi-core-has-no-native-mcp"}),
            );
            return Ok(());
        }
        self.agent_status.insert(
            agent.to_string(),
            json!({"id": agent, "support": "extension", "reason": "explicit-supported-adapter"}),
        );
        if codemode && !adapter {
            let sources: [(PathBuf, &str, i64, Option<bool>); 4] = [
                (agent_dir.join("mcp.json"), "user", 10, Some(true)),
                (agent_dir.join(".mcp.json"), "user", 20, Some(true)),
                (
                    self.settings.project.join(".pi").join("mcp.json"),
                    "project",
                    30,
                    None,
                ),
                (self.settings.project.join(".mcp.json"), "project", 40, None),
            ];
            for (path, scope, priority, trusted) in sources {
                let document = self
                    .read_document(
                        agent,
                        "pi-codemode-mcp",
                        &path,
                        parse_json,
                        MAX_CONFIG_BYTES,
                        false,
                    )?
                    .filter(|value| !value.keys().is_empty());
                if let Some(document) = document {
                    let mapping = self.pi_server_map(&document);
                    let mut batch = ServerBatch::new(
                        agent,
                        Some(&mapping),
                        scope,
                        "pi-codemode-mcp",
                        &path,
                        "extension",
                        priority,
                    );
                    batch.trusted = trusted;
                    self.add_servers(batch)?;
                }
            }
            return Ok(());
        }
        let sources: [(PathBuf, &str, &str, i64, Option<bool>); 6] = [
            (
                self.settings.config_home.join("mcp").join("mcp.json"),
                "user",
                "pi-shared-global",
                10,
                Some(true),
            ),
            (
                self.settings.home.join(".agents").join("mcp.json"),
                "user",
                "pi-agents-global",
                20,
                Some(true),
            ),
            (
                self.settings
                    .home
                    .join(".agents")
                    .join("mcp")
                    .join("mcp.json"),
                "user",
                "pi-agents-global",
                30,
                Some(true),
            ),
            (
                agent_dir.join("mcp.json"),
                "user",
                "pi-user-override",
                40,
                Some(true),
            ),
            (
                self.settings.project.join(".mcp.json"),
                "project",
                "pi-shared-project",
                50,
                None,
            ),
            (
                self.settings.project.join(".pi").join("mcp.json"),
                "project",
                "pi-project-override",
                60,
                None,
            ),
        ];
        for (path, scope, kind, priority, trusted) in sources {
            let document = self
                .read_document(agent, kind, &path, parse_json, MAX_CONFIG_BYTES, false)?
                .filter(|value| !value.keys().is_empty());
            if let Some(document) = document {
                let mapping = self.pi_server_map(&document);
                let mut batch = ServerBatch::new(
                    agent,
                    Some(&mapping),
                    scope,
                    kind,
                    &path,
                    "extension",
                    priority,
                );
                batch.trusted = trusted;
                self.add_servers(batch)?;
            }
        }
        Ok(())
    }
}

impl Inventory {
    fn discover_copilot(&mut self) -> Timed<()> {
        let agent = "github-copilot-cli";
        let user_path = self.settings.home.join(".copilot").join("mcp-config.json");
        let user = self
            .read_document(
                agent,
                "copilot-user",
                &user_path,
                parse_json,
                MAX_CONFIG_BYTES,
                false,
            )?
            .filter(|value| !value.keys().is_empty());
        if let Some(user) = user {
            self.add_servers(ServerBatch::new(
                agent,
                user.get("mcpServers"),
                "user",
                "copilot-user",
                &user_path,
                "documented",
                10,
            ))?;
        }
        for (index, directory) in self.ancestor_directories().into_iter().enumerate() {
            let paths = [
                directory.join(".github").join("mcp.json"),
                directory.join(".mcp.json"),
            ];
            for (offset, path) in paths.into_iter().enumerate() {
                let document = self
                    .read_document(
                        agent,
                        "copilot-project",
                        &path,
                        parse_json,
                        MAX_CONFIG_BYTES,
                        false,
                    )?
                    .filter(|value| !value.keys().is_empty());
                if let Some(document) = document {
                    let mapping = match document.get("mcpServers") {
                        Some(value) if value.is_table() => value.clone(),
                        _ => document.clone(),
                    };
                    let mut batch = ServerBatch::new(
                        agent,
                        Some(&mapping),
                        "project",
                        "copilot-project",
                        &path,
                        "documented",
                        30 + index as i64 * 2 + offset as i64,
                    );
                    batch.trusted = None;
                    self.add_servers(batch)?;
                }
            }
        }

        let mut settings_paths: Vec<PathBuf> = Vec::new();
        if self.settings.scope != "project" {
            settings_paths.push(self.settings.home.join(".copilot").join("settings.json"));
        }
        settings_paths.push(
            self.settings
                .project
                .join(".github")
                .join("copilot")
                .join("settings.json"),
        );
        settings_paths.push(
            self.settings
                .project
                .join(".github")
                .join("copilot")
                .join("settings.local.json"),
        );
        let mut plugin_states: BTreeMap<String, bool> = BTreeMap::new();
        let mut disabled_servers: BTreeSet<String> = BTreeSet::new();
        for settings_path in settings_paths {
            let settings = self
                .read_document(
                    agent,
                    "copilot-settings",
                    &settings_path,
                    parse_jsonc,
                    MAX_CONFIG_BYTES,
                    false,
                )?
                .filter(|value| !value.keys().is_empty());
            let Some(settings) = settings else { continue };
            if let Some(Cfg::Table(entries)) = settings.get("enabledPlugins") {
                for (key, value) in entries {
                    if let Some(flag) = value.as_bool() {
                        plugin_states.insert(key.clone(), flag);
                    }
                }
            }
            disabled_servers.extend(string_list(settings.get("disabledMcpServers")));
        }
        let managed_settings_path = self
            .settings
            .etc_root
            .join("github-copilot")
            .join("managed-settings.json");
        let managed_settings = self
            .read_document(
                agent,
                "copilot-managed-settings",
                &managed_settings_path,
                parse_json,
                MAX_CONFIG_BYTES,
                true,
            )?
            .filter(|value| !value.keys().is_empty());
        if let Some(document) = managed_settings.as_ref()
            && let Some(Cfg::Table(entries)) = document.get("enabledPlugins")
        {
            for (key, value) in entries {
                if let Some(flag) = value.as_bool() {
                    plugin_states.insert(key.clone(), flag);
                }
            }
        }
        let installed_root = self
            .settings
            .home
            .join(".copilot")
            .join("installed-plugins");
        let plugins = if self.settings.scope != "project" {
            self.nested_directories(&installed_root, 2)
        } else {
            Vec::new()
        };
        for plugin in plugins {
            let marketplace = plugin.parent().unwrap_or(Path::new("/")).to_path_buf();
            let mut manifest: Option<Cfg> = None;
            let mut manifest_path = plugin.join("plugin.json");
            for candidate in [
                plugin.join(".plugin").join("plugin.json"),
                plugin.join("plugin.json"),
                plugin.join(".github").join("plugin").join("plugin.json"),
                plugin.join(".claude-plugin").join("plugin.json"),
            ] {
                let document = self
                    .read_document(
                        agent,
                        "copilot-plugin-manifest",
                        &candidate,
                        parse_json,
                        MAX_CONFIG_BYTES,
                        false,
                    )?
                    .filter(|value| !value.keys().is_empty());
                if let Some(document) = document {
                    manifest = Some(document);
                    manifest_path = candidate;
                    break;
                }
            }
            let Some(manifest) = manifest else { continue };
            let plugin_name = truthy_text(manifest.get("name")).unwrap_or_else(|| {
                plugin
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default()
            });
            let market = marketplace
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            let plugin_id = if market == "_direct" {
                plugin_name.clone()
            } else {
                format!("{plugin_name}@{market}")
            };
            let enabled = plugin_states
                .get(&plugin_id)
                .or_else(|| plugin_states.get(&plugin_name))
                .copied()
                .unwrap_or(true);
            self.plugin_source(&PluginSource {
                agent,
                plugin: &plugin,
                manifest: &manifest,
                manifest_path: &manifest_path,
                source_kind: "copilot-plugin",
                support: "documented",
                priority: 100,
                enabled: Some(enabled),
                precedence_known: true,
            })?;
        }
        for definition in &mut self.definitions {
            if definition.agent == agent && disabled_servers.contains(&definition.raw_name) {
                definition.enabled = Some(false);
            }
        }
        self.apply_name_policy(agent, managed_settings.as_ref());
        self.agent_status.insert(
            agent.to_string(),
            json!({"id": agent, "support": "partial", "reason": "session-and-organization-state-unobserved"}),
        );
        Ok(())
    }

    fn discover_antigravity(&mut self) -> Timed<()> {
        let agent = "google-antigravity";
        let sources: [(PathBuf, &str); 2] = [
            (
                self.settings
                    .home
                    .join(".gemini")
                    .join("config")
                    .join("mcp_config.json"),
                "user",
            ),
            (
                self.settings
                    .project
                    .join(".agents")
                    .join("mcp_config.json"),
                "project",
            ),
        ];
        for (path, scope) in sources {
            let kind = format!("antigravity-{scope}");
            let document = self
                .read_document(agent, &kind, &path, parse_json, MAX_CONFIG_BYTES, false)?
                .filter(|value| !value.keys().is_empty());
            if let Some(document) = document {
                let mut batch = ServerBatch::new(
                    agent,
                    document.get("mcpServers"),
                    scope,
                    &kind,
                    &path,
                    "documented",
                    10,
                );
                batch.trusted = if scope == "user" { Some(true) } else { None };
                batch.precedence_known = false;
                self.add_servers(batch)?;
            }
        }
        let plugin_root = self
            .settings
            .home
            .join(".gemini")
            .join("antigravity-cli")
            .join("plugins");
        let plugins = if self.settings.scope != "project" {
            bounded_directories(
                &mut self.plan,
                &plugin_root,
                MAX_PLUGIN_DIRS,
                &mut self.deadline,
            )
        } else {
            Vec::new()
        };
        for plugin in plugins {
            let path = plugin.join("mcp_config.json");
            let document = self
                .read_document(
                    agent,
                    "antigravity-cli-plugin",
                    &path,
                    parse_json,
                    MAX_CONFIG_BYTES,
                    false,
                )?
                .filter(|value| !value.keys().is_empty());
            if let Some(document) = document {
                let mut batch = ServerBatch::new(
                    agent,
                    document.get("mcpServers"),
                    "plugin",
                    "antigravity-cli-plugin",
                    &path,
                    "documented",
                    10,
                );
                batch.enabled_known = false;
                batch.trusted = None;
                batch.precedence_known = false;
                self.add_servers(batch)?;
            }
        }
        self.agent_status.insert(
            agent.to_string(),
            json!({
                "id": agent,
                "support": "partial",
                "reason": "plugin-enable-and-source-precedence-unobserved",
            }),
        );
        Ok(())
    }

    fn finalize(&mut self) {
        let mut order: Vec<(String, String)> = Vec::new();
        let mut duplicate_groups: BTreeMap<(String, String), Vec<usize>> = BTreeMap::new();
        for (index, definition) in self.definitions.iter().enumerate() {
            if definition.selected == Some(false) {
                continue;
            }
            let collision_name = if definition.agent == "claude" && definition.scope == "plugin" {
                format!("plugin:{}", definition.id)
            } else {
                definition.raw_name.clone()
            };
            let key = (definition.agent.clone(), collision_name);
            if !duplicate_groups.contains_key(&key) {
                order.push(key.clone());
            }
            duplicate_groups.entry(key).or_default().push(index);
        }
        for key in &order {
            let group = duplicate_groups.get(key).cloned().unwrap_or_default();
            if group.len() > 1 {
                let mut ids: Vec<String> = group
                    .iter()
                    .map(|index| self.definitions[*index].id.clone())
                    .collect();
                ids.sort();
                let mut parts: Vec<&str> = vec!["duplicate-v1", &self.definitions[group[0]].agent];
                parts.extend(ids.iter().map(String::as_str));
                let duplicate = super::model::digest(&parts);
                for index in &group {
                    self.definitions[*index].duplicate_group = Some(duplicate.clone());
                }
            }
        }

        for key in &order {
            let group: Vec<usize> = duplicate_groups
                .get(key)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter(|index| self.definitions[*index].source_kind != "codex-profile")
                .collect();
            if group.is_empty() {
                continue;
            }
            if group.len() == 1 {
                let index = group[0];
                if self.definitions[index].selected.is_none() {
                    self.definitions[index].selected = Some(true);
                }
                continue;
            }
            if !group
                .iter()
                .all(|index| self.definitions[*index].precedence_known)
            {
                for index in &group {
                    self.definitions[*index].selected = None;
                }
                continue;
            }
            let highest = group
                .iter()
                .map(|index| self.definitions[*index].priority)
                .max()
                .unwrap_or_default();
            let winners: Vec<usize> = group
                .iter()
                .copied()
                .filter(|index| self.definitions[*index].priority == highest)
                .collect();
            if winners.len() != 1 {
                for index in &group {
                    self.definitions[*index].selected =
                        if self.definitions[*index].priority == highest {
                            None
                        } else {
                            Some(false)
                        };
                }
                continue;
            }
            let winner = winners[0];
            self.definitions[winner].selected = Some(true);
            let winner_id = self.definitions[winner].id.clone();
            for index in &group {
                if *index != winner {
                    self.definitions[*index].selected = Some(false);
                    self.definitions[*index].shadowed_by = Some(winner_id.clone());
                }
            }
        }

        let manual: Vec<usize> = (0..self.definitions.len())
            .filter(|index| {
                let item = &self.definitions[*index];
                item.agent == "claude"
                    && item.scope != "plugin"
                    && item.selected == Some(true)
                    && item.enabled != Some(false)
            })
            .collect();
        let mut plugins: Vec<usize> = (0..self.definitions.len())
            .filter(|index| {
                let item = &self.definitions[*index];
                item.agent == "claude" && item.scope == "plugin" && item.enabled != Some(false)
            })
            .collect();
        plugins.sort_by(|left, right| {
            let first = &self.definitions[*left];
            let second = &self.definitions[*right];
            (&first.source_path, &first.raw_name).cmp(&(&second.source_path, &second.raw_name))
        });
        let mut endpoint_winners: BTreeMap<String, usize> = BTreeMap::new();
        for index in manual {
            if let Some(signature) = self.definitions[index].endpoint_signature.clone() {
                endpoint_winners.insert(signature, index);
            }
        }
        for plugin in plugins {
            let Some(signature) = self.definitions[plugin].endpoint_signature.clone() else {
                continue;
            };
            match endpoint_winners.get(&signature).copied() {
                Some(winner) => {
                    let mut ids = [
                        self.definitions[winner].id.clone(),
                        self.definitions[plugin].id.clone(),
                    ];
                    ids.sort();
                    let duplicate = super::model::digest(&["duplicate-v1", &ids[0], &ids[1]]);
                    if self.definitions[winner].duplicate_group.is_none() {
                        self.definitions[winner].duplicate_group = Some(duplicate.clone());
                    }
                    self.definitions[plugin].duplicate_group = Some(duplicate);
                    self.definitions[plugin].selected = Some(false);
                    self.definitions[plugin].shadowed_by =
                        Some(self.definitions[winner].id.clone());
                }
                None => {
                    if self.definitions[plugin].selected == Some(true) {
                        endpoint_winners.insert(signature, plugin);
                    }
                }
            }
        }

        let mut applied: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
        for definition in &self.definitions {
            let Some(signature) = &definition.endpoint_signature else {
                continue;
            };
            applied
                .entry((definition.raw_name.clone(), signature.clone()))
                .or_default()
                .insert(core_agent_id(&definition.agent));
        }
        for definition in &mut self.definitions {
            match &definition.endpoint_signature {
                None => definition.applied_agents = vec![core_agent_id(&definition.agent)],
                Some(signature) => {
                    definition.applied_agents = applied
                        .get(&(definition.raw_name.clone(), signature.clone()))
                        .cloned()
                        .unwrap_or_default()
                        .into_iter()
                        .collect();
                }
            }
        }
    }

    pub fn definition_by_id(&self, identifier: &str) -> Option<&Definition> {
        self.definitions
            .iter()
            .find(|definition| definition.id == identifier)
    }

    pub fn scan(&mut self) -> Map<String, Value> {
        let discoverers: [fn(&mut Inventory) -> Timed<()>; 6] = [
            Inventory::discover_claude,
            Inventory::discover_codex,
            Inventory::discover_opencode,
            Inventory::discover_pi,
            Inventory::discover_copilot,
            Inventory::discover_antigravity,
        ];
        for discover in discoverers {
            if self.check().is_err() || discover(self).is_err() {
                self.truncated = true;
                self.warnings
                    .push(json!({"code": "deadline", "sourceId": "inventory"}));
                break;
            }
        }
        self.finalize();
        for agent in ["claude", "codex"] {
            self.agent_status.entry(agent.to_string()).or_insert_with(|| {
                json!({"id": agent, "support": "documented", "reason": "filesystem-configuration"})
            });
        }
        let mut sorted: Vec<&Definition> = self.definitions.iter().collect();
        sorted.sort_by(|left, right| {
            (
                &left.agent,
                &left.scope,
                &left.raw_name,
                &left.source_path,
                &left.id,
            )
                .cmp(&(
                    &right.agent,
                    &right.scope,
                    &right.raw_name,
                    &right.source_path,
                    &right.id,
                ))
        });
        let definitions: Vec<Value> = sorted.iter().map(|item| item.public()).collect();
        let mut document = Map::new();
        document.insert("ok".to_string(), json!(true));
        document.insert("schemaVersion".to_string(), json!(SCHEMA_VERSION));
        document.insert("healthBasis".to_string(), json!("configuration-only"));
        document.insert("project".to_string(), json!("<project>"));
        document.insert(
            "agents".to_string(),
            Value::Array(self.agent_status.values().cloned().collect()),
        );
        document.insert("definitions".to_string(), Value::Array(definitions));
        document.insert("warnings".to_string(), Value::Array(self.warnings.clone()));
        document.insert(
            "truncated".to_string(),
            json!(self.truncated || self.deadline.truncated),
        );
        document.insert(
            "limits".to_string(),
            json!({
                "sources": MAX_SOURCES,
                "definitions": MAX_DEFINITIONS,
                "warnings": MAX_WARNINGS,
                "profileFiles": MAX_PROFILE_FILES,
                "outputBytes": MAX_OUTPUT_BYTES,
                "deadlineMilliseconds": DEADLINE_MILLISECONDS,
            }),
        );
        document
    }
}

pub fn bounded_json(document: &Map<String, Value>) -> String {
    let mut definitions: Vec<Value> = document
        .get("definitions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    let mut working = document.clone();
    loop {
        working.insert("definitions".to_string(), Value::Array(definitions.clone()));
        let encoded = canonical_json(&Value::Object(working.clone()));
        if encoded.len() <= MAX_OUTPUT_BYTES {
            return encoded;
        }
        working.insert("truncated".to_string(), json!(true));
        if definitions.pop().is_some() {
            continue;
        }
        let fallback = json!({
            "ok": true,
            "schemaVersion": SCHEMA_VERSION,
            "healthBasis": "configuration-only",
            "definitions": [],
            "warnings": [{"code": "output-limit", "sourceId": "inventory"}],
            "truncated": true,
        });
        return canonical_json(&fallback);
    }
}

pub fn environ() -> Environ {
    std::env::vars_os().collect()
}

pub fn expanded(value: &str) -> PathBuf {
    match crate::common::parse_path(value) {
        Ok(parsed) => crate::common::expanded_os_path(&parsed),
        Err(_) => crate::common::expanded_os_path(Path::new(value)),
    }
}

pub fn os_from_bytes(bytes: &[u8]) -> OsString {
    OsString::from_vec(bytes.to_vec())
}
