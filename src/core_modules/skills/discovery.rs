use super::bounds::{
    MAX_CONFIG_BYTES, MAX_DETAIL, MAX_ENTRIES_PER_ROOT, MAX_ITEMS, MAX_NAME, MAX_PATH, MAX_PLUGINS,
    MAX_PROJECT_WALK, MAX_ROOTS, MAX_TOTAL_ENTRIES, artifact_metrics, bounded_names, read_document,
    realpath, under,
};
use super::registry::{self, Root};
use crate::common::{display_path, expanded_os_path, path_text};
use crate::core_modules::frontmatter;
use crate::core_modules::text::clean;
use crate::core_modules::watch::{WatchPlan, lane_rows};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

pub const SKILL_FILE: &str = "SKILL.md";
const PROJECT_SCOPES: [&str; 1] = ["project"];
const PLUGIN_ORDER: usize = 1 << 20;

#[derive(Clone, Debug)]
pub struct Environment {
    pub home: PathBuf,
    pub anchor: PathBuf,
    pub exact: bool,
    pub platform: String,
    pub prefix: PathBuf,
    pub scope: String,
}

impl Environment {
    pub fn new(home: &Path) -> Self {
        Self {
            home: home.to_path_buf(),
            anchor: PathBuf::new(),
            exact: false,
            platform: "linux".to_string(),
            prefix: PathBuf::new(),
            scope: "all".to_string(),
        }
    }

    pub fn anchored(mut self, anchor: &Path) -> Self {
        self.anchor = anchor.to_path_buf();
        self
    }

    pub fn exactly(mut self) -> Self {
        self.exact = true;
        self
    }

    pub fn prefixed(mut self, prefix: &Path) -> Self {
        self.prefix = prefix.to_path_buf();
        self
    }

    pub fn on_platform(mut self, platform: &str) -> Self {
        self.platform = platform.to_string();
        self
    }

    pub fn scoped(mut self, scope: &str) -> Self {
        self.scope = scope.to_string();
        self
    }
}

#[derive(Default)]
pub struct Budget {
    pub entries: usize,
    pub truncated: bool,
}

impl Budget {
    fn spend(&mut self) -> bool {
        if self.entries + 1 > MAX_TOTAL_ENTRIES {
            self.truncated = true;
            return false;
        }
        self.entries += 1;
        true
    }
}

#[derive(Clone)]
pub struct Candidate {
    pub path: PathBuf,
    pub target: PathBuf,
    pub scope: String,
    pub source: String,
    pub agents: BTreeSet<&'static str>,
    pub roots: BTreeSet<PathBuf>,
    pub precedence: Option<i64>,
    pub order: usize,
    pub enabled: Option<bool>,
}

pub type Collected = BTreeMap<(String, PathBuf), Candidate>;

pub fn chain_for(environment: &Environment) -> (PathBuf, Vec<PathBuf>) {
    if environment.scope == "user" {
        return (PathBuf::new(), Vec::new());
    }
    if environment.exact {
        exact_chain(&environment.anchor)
    } else {
        project_chain(&environment.anchor)
    }
}

fn anchor_directory(anchor: &Path) -> PathBuf {
    let start = expanded_os_path(anchor);
    if start.is_file() {
        start.parent().map(Path::to_path_buf).unwrap_or(start)
    } else {
        start
    }
}

pub fn exact_chain(anchor: &Path) -> (PathBuf, Vec<PathBuf>) {
    if anchor.as_os_str().is_empty() {
        return (PathBuf::new(), Vec::new());
    }
    let root = anchor_directory(anchor);
    (root.clone(), vec![root])
}

pub fn project_chain(anchor: &Path) -> (PathBuf, Vec<PathBuf>) {
    if anchor.as_os_str().is_empty() {
        return (PathBuf::new(), Vec::new());
    }
    let start = anchor_directory(anchor);
    let mut chain: Vec<PathBuf> = Vec::new();
    let mut root = PathBuf::new();
    let mut current = start.clone();
    for _ in 0..MAX_PROJECT_WALK {
        chain.push(current.clone());
        if registry::PROJECT_MARKERS
            .iter()
            .any(|marker| current.join(marker).exists())
        {
            root = current.clone();
            break;
        }
        let Some(parent) = current.parent() else {
            break;
        };
        if parent == current {
            break;
        }
        current = parent.to_path_buf();
    }
    if root.as_os_str().is_empty() {
        return (start.clone(), vec![start]);
    }
    (root, chain)
}

pub fn resolve_roots(environment: &Environment) -> Vec<(&'static Root, PathBuf)> {
    let (_, chain) = chain_for(environment);
    let mut resolved: Vec<(&'static Root, PathBuf)> = Vec::new();
    for entry in registry::ROOTS.iter() {
        if resolved.len() >= MAX_ROOTS {
            break;
        }
        if entry.anchor != "walk" && environment.scope == "project" {
            continue;
        }
        match entry.anchor {
            "home" => resolved.push((entry, environment.home.join(entry.path))),
            "absolute" => resolved.push((entry, under(&environment.prefix, entry.path))),
            "managed-claude" => {
                if let Some(managed) = registry::managed_claude(&environment.platform) {
                    resolved.push((entry, under(&environment.prefix, managed)));
                }
            }
            _ => {
                for directory in &chain {
                    if resolved.len() >= MAX_ROOTS {
                        break;
                    }
                    resolved.push((entry, directory.join(entry.path)));
                }
            }
        }
    }
    resolved
}

fn load_json(plan: &mut WatchPlan, path: &Path) -> Option<Value> {
    let text = read_document(plan, path, MAX_CONFIG_BYTES);
    if text.is_empty() {
        return None;
    }
    serde_json::from_str(&text).ok()
}

pub fn claude_plugins(
    plan: &mut WatchPlan,
    environment: &Environment,
) -> Vec<(String, PathBuf, Option<bool>)> {
    let installed = load_json(
        plan,
        &environment
            .home
            .join(".claude/plugins/installed_plugins.json"),
    );
    let settings = load_json(plan, &environment.home.join(".claude/settings.json"));
    let enabled_map = settings
        .as_ref()
        .and_then(|value| value.get("enabledPlugins"))
        .and_then(Value::as_object)
        .cloned()
        .unwrap_or_default();
    let Some(plugins) = installed
        .as_ref()
        .and_then(|value| value.get("plugins"))
        .and_then(Value::as_object)
    else {
        return Vec::new();
    };
    let mut keys: Vec<&String> = plugins.keys().collect();
    keys.sort();
    let mut found = Vec::new();
    for key in keys {
        if found.len() >= MAX_PLUGINS {
            break;
        }
        let Some(first) = plugins
            .get(key)
            .and_then(Value::as_array)
            .and_then(|entries| entries.first())
            .and_then(Value::as_object)
        else {
            continue;
        };
        let Some(install) = first
            .get("installPath")
            .and_then(Value::as_str)
            .filter(|value| !value.is_empty())
        else {
            continue;
        };
        let state = enabled_map.get(key).and_then(Value::as_bool);
        found.push((key.clone(), PathBuf::from(install), state));
    }
    found
}

pub fn codex_switches(plan: &mut WatchPlan, environment: &Environment) -> BTreeMap<PathBuf, bool> {
    let mut switches = BTreeMap::new();
    let text = read_document(
        plan,
        &environment.home.join(".codex/config.toml"),
        MAX_CONFIG_BYTES,
    );
    if text.is_empty() {
        return switches;
    }
    let Ok(parsed) = text.parse::<toml::Table>() else {
        return switches;
    };
    let Some(rows) = parsed
        .get("skills")
        .and_then(toml::Value::as_table)
        .and_then(|section| section.get("config"))
        .and_then(toml::Value::as_array)
    else {
        return switches;
    };
    for row in rows.iter().take(MAX_PLUGINS) {
        let Some(table) = row.as_table() else {
            continue;
        };
        let path = table.get("path").and_then(toml::Value::as_str);
        let state = table.get("enabled").and_then(toml::Value::as_bool);
        if let (Some(path), Some(state)) = (path, state) {
            switches.insert(realpath(&expanded_os_path(Path::new(path))), state);
        }
    }
    switches
}

pub fn stable_id(scope: &str, target: &Path) -> String {
    crate::core_modules::canonical::scoped_path_id(scope, target)
}

pub fn descriptor(plan: &mut WatchPlan, skill_dir: &Path) -> Option<PathBuf> {
    plan.watch_path(skill_dir, true);
    let candidate = skill_dir.join(SKILL_FILE);
    std::fs::metadata(&candidate)
        .ok()
        .filter(std::fs::Metadata::is_file)
        .map(|_| candidate)
}

fn classify(target: &Path, environment: &Environment, source: &str) -> String {
    if !source.is_empty() {
        return source.to_string();
    }
    let system = under(&environment.prefix, "/usr/share");
    if target == system || target.starts_with(&system) {
        return "system".to_string();
    }
    "loose".to_string()
}

struct Placement<'a> {
    entry: &'a Root,
    root_path: PathBuf,
    order: usize,
    source: String,
    enabled: Option<bool>,
}

fn add_candidate(
    collected: &mut Collected,
    placement: &Placement<'_>,
    path: PathBuf,
    environment: &Environment,
) {
    if path.as_os_str().as_bytes().len() > MAX_PATH {
        return;
    }
    let target = realpath(&path);
    let scope = if placement.source.starts_with("plugin:") {
        "plugin".to_string()
    } else {
        placement.entry.kind.to_string()
    };
    let key = (scope.clone(), target.clone());
    let known = collected.contains_key(&key);
    let candidate = collected.entry(key).or_insert_with(|| Candidate {
        path: path.clone(),
        target: target.clone(),
        scope: scope.clone(),
        source: classify(&target, environment, &placement.source),
        agents: BTreeSet::new(),
        roots: BTreeSet::new(),
        precedence: placement.entry.precedence,
        order: placement.order,
        enabled: placement.enabled,
    });
    if known
        && ((placement.entry.precedence.is_some() && candidate.precedence.is_none())
            || placement.order < candidate.order)
    {
        candidate.path = path;
        candidate.precedence = placement.entry.precedence;
        candidate.order = placement.order;
    }
    candidate.agents.insert(placement.entry.agent);
    candidate.roots.insert(placement.root_path.clone());
    if let Some(state) = placement.enabled {
        candidate.enabled = Some(state);
    }
}

fn bounded_child_dirs(
    plan: &mut WatchPlan,
    root_path: &Path,
    budget: &mut Budget,
) -> Vec<(OsString, PathBuf)> {
    let limit =
        (MAX_ENTRIES_PER_ROOT as isize).min(MAX_TOTAL_ENTRIES as isize - budget.entries as isize);
    let (names, truncated) = bounded_names(plan, root_path, limit);
    budget.truncated |= truncated;
    let mut found = Vec::new();
    let mut seen = 0usize;
    for name in names {
        if seen >= MAX_ENTRIES_PER_ROOT || !budget.spend() {
            budget.truncated = true;
            return found;
        }
        let child = root_path.join(&name);
        if !std::fs::metadata(&child).is_ok_and(|metadata| metadata.is_dir()) {
            continue;
        }
        seen += 1;
        found.push((name, child));
    }
    found
}

fn scan_root(
    collected: &mut Collected,
    plan: &mut WatchPlan,
    placement: &Placement<'_>,
    environment: &Environment,
    budget: &mut Budget,
) {
    let reserved_allowed = placement.entry.agent == "claude-code"
        && matches!(placement.entry.kind, "managed" | "user" | "project");
    for (name, child) in bounded_child_dirs(plan, &placement.root_path, budget) {
        match descriptor(plan, &child) {
            Some(path) => add_candidate(collected, placement, path, environment),
            None => {
                if reserved_allowed
                    && name
                        .as_bytes()
                        .eq_ignore_ascii_case(registry::RESERVED_CLAUDE_SUBDIR.as_bytes())
                {
                    scan_reserved(collected, plan, placement, &child, environment, budget);
                }
            }
        }
    }
}

fn scan_reserved(
    collected: &mut Collected,
    plan: &mut WatchPlan,
    placement: &Placement<'_>,
    root_path: &Path,
    environment: &Environment,
    budget: &mut Budget,
) {
    let reserved = Placement {
        entry: placement.entry,
        root_path: root_path.to_path_buf(),
        order: placement.order,
        source: "synced".to_string(),
        enabled: None,
    };
    for (_, child) in bounded_child_dirs(plan, root_path, budget) {
        if let Some(path) = descriptor(plan, &child) {
            add_candidate(collected, &reserved, path, environment);
        }
    }
}

const PLUGIN_ROOT: Root = Root {
    agent: "claude-code",
    kind: "plugin",
    anchor: "absolute",
    path: "",
    precedence: None,
};

fn scan_plugins(
    collected: &mut Collected,
    plan: &mut WatchPlan,
    environment: &Environment,
    budget: &mut Budget,
) {
    for (key, install_path, enabled) in claude_plugins(plan, environment) {
        let label = format!("plugin:{key}");
        let placement = Placement {
            entry: &PLUGIN_ROOT,
            root_path: install_path.clone(),
            order: PLUGIN_ORDER,
            source: label.clone(),
            enabled,
        };
        if let Some(path) = descriptor(plan, &install_path)
            && budget.spend()
        {
            add_candidate(collected, &placement, path, environment);
        }
        let nested = Placement {
            entry: &PLUGIN_ROOT,
            root_path: install_path.join("skills"),
            order: PLUGIN_ORDER,
            source: label,
            enabled,
        };
        scan_root(collected, plan, &nested, environment, budget);
    }
}

fn scope_order(scope: &str) -> usize {
    match scope {
        "project" => 0,
        "user" => 1,
        "extension" => 2,
        "plugin" => 3,
        "managed" => 4,
        "system" => 5,
        _ => 9,
    }
}

fn emit_row(plan: &mut WatchPlan, candidate: &Candidate) -> Value {
    let text = read_document(plan, &candidate.path, super::bounds::MAX_DESCRIPTOR_BYTES);
    let named = frontmatter::value(&text, "name");
    let name = if named.is_empty() {
        candidate
            .path
            .parent()
            .and_then(Path::file_name)
            .map(|value| display_path(Path::new(value)))
            .unwrap_or_default()
    } else {
        display_path(Path::new(OsStr::new(&named)))
    };
    let description = frontmatter::value(&text, "description");
    let agents: Vec<&str> = candidate
        .agents
        .iter()
        .copied()
        .take(registry::AGENT_LABELS.len())
        .collect();
    let mut badges = vec![Value::from(candidate.scope.clone())];
    if agents.len() > 1 {
        badges.push(Value::from(format!("{} agents", agents.len())));
    }
    if candidate.enabled == Some(false) {
        badges.push(Value::from("disabled"));
    }
    if candidate.scope == "extension" {
        badges.push(Value::from("extension"));
    }
    let link_target = if candidate.target == candidate.path {
        String::new()
    } else {
        path_text(&candidate.target)
    };
    let roots: Vec<Value> = candidate
        .roots
        .iter()
        .take(8)
        .map(|root| Value::from(path_text(root)))
        .collect();
    json!({
        "id": stable_id(&candidate.scope, &candidate.target),
        "name": clean(&name, MAX_NAME),
        "detail": clean(&description, MAX_DETAIL),
        "metrics": artifact_metrics(&candidate.path, &text, &description),
        "scope": candidate.scope,
        "root_kind": candidate.scope,
        "source": candidate.source,
        "path": path_text(&candidate.path),
        "link_target": link_target,
        "agents": agents,
        "agent_labels": agents.iter().filter_map(|agent| registry::label(agent)).collect::<Vec<_>>(),
        "roots": roots,
        "precedence": candidate.precedence,
        "enabled": candidate.enabled,
        "badges": badges,
    })
}

pub fn candidates(
    plan: &mut WatchPlan,
    environment: &Environment,
    budget: &mut Budget,
) -> Collected {
    let mut collected = Collected::new();
    for (order, (entry, root_path)) in resolve_roots(environment).into_iter().enumerate() {
        let placement = Placement {
            entry,
            root_path,
            order,
            source: String::new(),
            enabled: None,
        };
        scan_root(&mut collected, plan, &placement, environment, budget);
    }
    if environment.scope != "project" {
        scan_plugins(&mut collected, plan, environment, budget);
    }
    collected
}

pub fn collect(plan: &mut WatchPlan, environment: &Environment) -> Map<String, Value> {
    let (project_root, _) = chain_for(environment);
    let mut budget = Budget::default();
    let mut collected = candidates(plan, environment, &mut budget);
    let switches = codex_switches(plan, environment);
    for candidate in collected.values_mut() {
        if candidate.enabled.is_none() && candidate.agents.contains("codex") {
            candidate.enabled = switches.get(&candidate.target).copied();
        }
    }
    let rows: Vec<Value> = collected
        .values()
        .map(|candidate| emit_row(plan, candidate))
        .collect();
    let mut rows = lane_rows(rows, &environment.scope, &PROJECT_SCOPES, "scope");
    rows.sort_by_key(sort_key);
    let truncated = budget.truncated || rows.len() > MAX_ITEMS;
    let count = rows.len().min(MAX_ITEMS);
    rows.truncate(MAX_ITEMS);
    let mut document = Map::new();
    document.insert("ok".to_string(), json!(true));
    document.insert("schemaVersion".to_string(), json!(1));
    document.insert("project".to_string(), json!(path_text(&project_root)));
    document.insert("count".to_string(), json!(count));
    document.insert("truncated".to_string(), json!(truncated));
    document.insert("agents".to_string(), json!(registry::agents()));
    document.insert("items".to_string(), Value::Array(rows));
    document
}

fn sort_key(row: &Value) -> (usize, String, String, String) {
    let text = |key: &str| {
        row.get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let scope = text("scope");
    (
        scope_order(&scope),
        text("source"),
        text("name").to_lowercase(),
        text("id"),
    )
}
