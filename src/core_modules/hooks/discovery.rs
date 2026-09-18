use super::adapters::{ADAPTERS, Context, PROJECT_SCOPES};
use super::events::canonical;
use super::labels::{Environ, attach_labels, labels_path, load_labels};
use super::redaction::assert_safe;
use super::safeio::{Budget, expanded, is_plain_dir};
use crate::common::path_text;
use crate::core_modules::watch::lane_rows;
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

pub const SCHEMA_VERSION: i64 = 1;
pub const PROJECT_MARKERS: [&str; 7] = [
    ".git",
    ".claude",
    ".agents",
    ".codex",
    ".opencode",
    ".pi",
    ".github",
];
pub const EVENT_ORDER: [&str; 3] = ["documented", "undocumented-event", "code-hosted"];
const MAX_ANCESTORS: usize = 24;

pub fn project_root(budget: &mut Budget, start: &Path, home: &Path) -> PathBuf {
    if start.as_os_str().is_empty() {
        return PathBuf::new();
    }
    let mut current = crate::common::expanded_os_path(start);
    if !is_plain_dir(budget, &current) {
        current = current.parent().map(Path::to_path_buf).unwrap_or(current);
    }
    let boundary = home.parent().map(Path::to_path_buf);
    for candidate in current.ancestors().take(MAX_ANCESTORS) {
        if boundary.as_deref() == Some(candidate) {
            break;
        }
        for marker in PROJECT_MARKERS {
            if candidate.join(marker).exists() {
                return candidate.to_path_buf();
            }
        }
    }
    PathBuf::new()
}

fn sharing_key(row: &Map<String, Value>) -> (String, String) {
    let digest = row
        .get("summary")
        .and_then(Value::as_object)
        .and_then(|summary| summary.get("digest"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let agent = row.get("agent").and_then(Value::as_str).unwrap_or_default();
    let event = row.get("event").and_then(Value::as_str).unwrap_or_default();
    (digest, canonical(agent, event))
}

fn annotate_applied_agents(rows: &mut [Value]) {
    let mut groups: BTreeMap<(String, String), BTreeSet<String>> = BTreeMap::new();
    for row in rows.iter() {
        let Some(entry) = row.as_object() else {
            continue;
        };
        let key = sharing_key(entry);
        if key.0.is_empty() {
            continue;
        }
        groups.entry(key).or_default().insert(
            entry
                .get("agent")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_string(),
        );
    }
    for row in rows.iter_mut() {
        let Some(entry) = row.as_object_mut() else {
            continue;
        };
        let key = sharing_key(entry);
        let agents: Vec<String> = if key.0.is_empty() {
            vec![
                entry
                    .get("agent")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            ]
        } else {
            groups
                .get(&key)
                .map(|names| names.iter().cloned().collect())
                .unwrap_or_default()
        };
        entry.insert("appliedAgents".to_string(), json!(agents));
    }
}

fn order_key(row: &Value) -> (usize, String, String, String, u64) {
    let Some(entry) = row.as_object() else {
        return (9, String::new(), String::new(), String::new(), 0);
    };
    let support = entry
        .get("supportStatus")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let rank = EVENT_ORDER
        .iter()
        .position(|candidate| *candidate == support)
        .unwrap_or(9);
    (
        rank,
        entry
            .get("agent")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        entry
            .get("event")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        entry
            .get("source")
            .and_then(Value::as_object)
            .and_then(|source| source.get("path"))
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        entry
            .get("index")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
    )
}

pub struct Query<'a> {
    pub project: &'a str,
    pub home: &'a str,
    pub etc_root: &'a str,
    pub policy_owner_uid: u32,
    pub exact: bool,
    pub scope: &'a str,
}

impl<'a> Query<'a> {
    pub fn new(project: &'a str, home: &'a str) -> Self {
        Self {
            project,
            home,
            etc_root: "/etc",
            policy_owner_uid: 0,
            exact: false,
            scope: "all",
        }
    }
}

pub fn build_context(budget: &mut Budget, query: &Query<'_>, environ: Environ) -> Context {
    let Query {
        project,
        home,
        etc_root,
        policy_owner_uid,
        exact,
        scope,
    } = *query;
    let home_path = if home.is_empty() {
        crate::common::expanded_os_path(Path::new("~"))
    } else {
        expanded(home)
    };
    let root = if scope == "user" {
        PathBuf::new()
    } else if exact && !project.is_empty() {
        expanded(project)
    } else {
        let start = if project.is_empty() {
            home_path.clone()
        } else {
            expanded(project)
        };
        project_root(budget, &start, &home_path)
    };
    Context {
        home: home_path,
        project_root: root,
        environ,
        etc_root: expanded(etc_root),
        policy_owner_uid,
        scope: scope.to_string(),
    }
}

pub fn collect(budget: &mut Budget, query: &Query<'_>, environ: Environ) -> Map<String, Value> {
    let scope = query.scope;
    let context = build_context(budget, query, environ);
    let mut rows: Vec<Value> = Vec::new();
    let mut agents = Map::new();
    for (name, adapter) in ADAPTERS {
        let produced = adapter(&context, budget);
        agents.insert(
            name.to_string(),
            json!(if name == "opencode" || name == "pi" {
                "code-hosted"
            } else {
                "documented"
            }),
        );
        for mut entry in produced {
            assert_safe(&mut entry);
            rows.push(Value::Object(entry));
        }
    }
    let mut rows = lane_rows(rows, scope, &PROJECT_SCOPES, "scope");
    annotate_applied_agents(&mut rows);
    let labels = load_labels(budget, &labels_path(&context.home, &context.environ));
    attach_labels(&mut rows, &labels);
    rows.sort_by_key(order_key);
    let mut document = Map::new();
    document.insert("schemaVersion".to_string(), json!(SCHEMA_VERSION));
    document.insert("ok".to_string(), json!(true));
    document.insert(
        "project".to_string(),
        json!(path_text(&context.project_root)),
    );
    document.insert("home".to_string(), json!(path_text(&context.home)));
    document.insert("count".to_string(), json!(rows.len()));
    document.insert("truncated".to_string(), json!(budget.truncated));
    document.insert("agents".to_string(), Value::Object(agents));
    document.insert("items".to_string(), Value::Array(rows));
    document
}
