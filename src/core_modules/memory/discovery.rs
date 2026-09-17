use super::adapters::{ADAPTERS, Claim, claim};
use super::common::{
    Budget, Environ, MAX_EXTRA_ROOTS, MAX_NAME_CHARS, SCHEMA_VERSION, descriptor_head,
    env_path_value, expanded, expanded_os, expanded_path, listed_files, project_root, realpath_of,
    stable_id, summary_line,
};
use crate::common::{display_path, path_text};
use crate::core_modules::frontmatter::frontmatter_field;
use crate::core_modules::metrics::artifact_metrics;
use crate::core_modules::watch::{WatchPlan, lane_rows};
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};

pub const EXCLUDED_KINDS: [&str; 3] = ["session-transcript", "private-database", "configuration"];
pub const KIND_ORDER: [&str; 5] = [
    "instructions",
    "rules",
    "auto-memory",
    "system-prompt",
    "extra",
];
pub const PROJECT_SCOPES: [&str; 2] = ["project", "local"];

pub struct Context {
    pub home: PathBuf,
    pub cwd: PathBuf,
    pub anchor: PathBuf,
    pub project_root: PathBuf,
    pub environ: Environ,
    pub scope: String,
}

pub fn build_context(
    project: &str,
    home: &str,
    environ: Environ,
    exact: bool,
    scope: &str,
) -> Context {
    let home_path = if home.is_empty() {
        crate::common::expanded_os_path(Path::new("~"))
    } else {
        expanded(home)
    };
    let start = if project.is_empty() {
        home_path.clone()
    } else {
        expanded(project)
    };
    let mut root = if exact && !project.is_empty() {
        if start.is_dir() {
            start.clone()
        } else {
            start.parent().map(Path::to_path_buf).unwrap_or_default()
        }
    } else {
        project_root(&start, &home_path)
    };
    if scope == "user" {
        root = PathBuf::new();
    }
    let anchor = if start.is_file() {
        start
            .parent()
            .map(Path::to_path_buf)
            .unwrap_or(start.clone())
    } else {
        start.clone()
    };
    let cwd = if root.as_os_str().is_empty() {
        start
    } else {
        expanded_path(&root)
    };
    Context {
        home: home_path,
        cwd,
        anchor,
        project_root: root,
        environ,
        scope: scope.to_string(),
    }
}

pub fn config_path(home: &Path, environ: &Environ, override_path: &str) -> PathBuf {
    if !override_path.is_empty() {
        return expanded(override_path);
    }
    let base = env_path_value("XDG_CONFIG_HOME", environ);
    let root = if base.is_empty() {
        home.join(".config")
    } else {
        expanded_os(&base)
    };
    let current = root
        .join("data-goblin.fileblade-memory")
        .join("config.json");
    let legacy = root.join("kurt.agent-memory").join("config.json");
    if current.exists() || !legacy.exists() {
        current
    } else {
        legacy
    }
}

struct ExtraRoot {
    path: String,
    label: String,
}

fn extra_roots(plan: &mut WatchPlan, path: &Path) -> Vec<ExtraRoot> {
    let Some((head, _)) = descriptor_head(plan, path) else {
        return Vec::new();
    };
    let Ok(parsed) = serde_json::from_str::<Value>(&head) else {
        return Vec::new();
    };
    let Some(Value::Array(entries)) = parsed.get("extraRoots") else {
        return Vec::new();
    };
    let mut roots = Vec::new();
    for entry in entries.iter().take(MAX_EXTRA_ROOTS) {
        match entry {
            Value::String(path) => roots.push(ExtraRoot {
                path: path.clone(),
                label: String::new(),
            }),
            Value::Object(fields) => {
                if let Some(Value::String(path)) = fields.get("path") {
                    let label = fields
                        .get("label")
                        .and_then(Value::as_str)
                        .unwrap_or_default();
                    roots.push(ExtraRoot {
                        path: path.clone(),
                        label: label.to_string(),
                    });
                }
            }
            _ => continue,
        }
    }
    roots
}

fn extra_claims(plan: &mut WatchPlan, roots: Vec<ExtraRoot>, budget: &mut Budget) -> Vec<Claim> {
    let mut claims = Vec::new();
    for entry in roots {
        if !budget.take_source() {
            break;
        }
        let target = expanded(&entry.path);
        let label: String = entry.label.chars().take(MAX_NAME_CHARS).collect();
        let files = if std::fs::metadata(&target).is_ok_and(|data| data.is_file()) {
            vec![target]
        } else {
            listed_files(plan, &target, &[".md"])
        };
        for candidate in files {
            let mut row = claim(candidate, "user", "extra", "extra", 90, &label);
            row.discovery = "user-configured";
            claims.push(row);
        }
    }
    claims
}

struct Described {
    detail: String,
    bytes: u64,
    memory_type: String,
    paths: String,
    modified: String,
    metrics: Map<String, Value>,
}

fn describe(plan: &mut WatchPlan, path: &Path) -> Option<Described> {
    let (text, size) = descriptor_head(plan, path)?;
    let description = frontmatter_field(&text, "description");
    Some(Described {
        detail: if description.is_empty() {
            summary_line(&text)
        } else {
            description
        },
        bytes: size,
        memory_type: frontmatter_field(&text, "type"),
        paths: frontmatter_field(&text, "paths"),
        modified: frontmatter_field(&text, "modified"),
        metrics: artifact_metrics(path, &text, size),
    })
}

struct Reader {
    agent: &'static str,
    load_order: i64,
    scope: &'static str,
    kind: &'static str,
    discovery: &'static str,
    excluded: bool,
}

impl Reader {
    fn row(&self) -> Value {
        json!({
            "agent": self.agent,
            "loadOrder": self.load_order,
            "scope": self.scope,
            "kind": self.kind,
            "discovery": self.discovery,
            "excluded": self.excluded,
        })
    }
}

struct Row {
    id: String,
    name: String,
    path: PathBuf,
    realpath: PathBuf,
    aliases: Vec<PathBuf>,
    kind: &'static str,
    scope: &'static str,
    note: String,
    readers: Vec<Reader>,
    excluded: bool,
    inline_bytes: Option<u64>,
    detail: String,
    bytes: u64,
    memory_type: String,
    activation: &'static str,
    modified: String,
    metrics: Map<String, Value>,
    badges: Vec<&'static str>,
}

impl Row {
    fn value(&self) -> Value {
        let alias = self.path != self.realpath;
        json!({
            "id": self.id,
            "name": self.name,
            "path": path_text(&self.path),
            "realpath": path_text(&self.realpath),
            "alias": alias,
            "aliases": self.aliases.iter().map(|path| Value::from(path_text(path))).collect::<Vec<_>>(),
            "kind": self.kind,
            "scope": self.scope,
            "note": self.note,
            "readers": self.readers.iter().map(Reader::row).collect::<Vec<_>>(),
            "excluded": self.excluded,
            "inlineBytes": self.inline_bytes,
            "detail": self.detail,
            "bytes": self.bytes,
            "memoryType": self.memory_type,
            "activation": self.activation,
            "modified": self.modified,
            "metrics": self.metrics,
            "badges": self.badges,
        })
    }
}

fn kind_rank(kind: &str) -> usize {
    KIND_ORDER
        .iter()
        .position(|entry| *entry == kind)
        .unwrap_or(KIND_ORDER.len())
}

fn new_row(path: &Path, target: &Path, entry: &Claim, described: Described, reader: Reader) -> Row {
    let name: String = path
        .file_name()
        .map(|value| display_path(Path::new(value)))
        .unwrap_or_default()
        .chars()
        .take(MAX_NAME_CHARS)
        .collect();
    let aliases = if path == target {
        Vec::new()
    } else {
        vec![path.to_path_buf()]
    };
    Row {
        id: stable_id(target),
        name,
        path: path.to_path_buf(),
        realpath: target.to_path_buf(),
        aliases,
        kind: entry.kind,
        scope: entry.scope,
        note: entry.note.chars().take(MAX_NAME_CHARS).collect(),
        readers: vec![reader],
        excluded: entry.excluded,
        inline_bytes: entry.inline,
        detail: described.detail,
        bytes: described.bytes,
        memory_type: described.memory_type,
        activation: if described.paths.is_empty() {
            "always"
        } else {
            "path-scoped"
        },
        modified: described.modified,
        metrics: described.metrics,
        badges: Vec::new(),
    }
}

fn merge(plan: &mut WatchPlan, claims: Vec<Claim>, budget: &mut Budget) -> Vec<Row> {
    let mut rows: Vec<Row> = Vec::new();
    let mut index: HashMap<PathBuf, usize> = HashMap::new();
    for entry in claims {
        let target = realpath_of(&entry.path);
        let reader = Reader {
            agent: entry.agent,
            load_order: entry.load_order,
            scope: entry.scope,
            kind: entry.kind,
            discovery: entry.discovery,
            excluded: entry.excluded,
        };
        match index.get(&target).copied() {
            None => {
                if !budget.take_item() {
                    break;
                }
                let Some(described) = describe(plan, &entry.path) else {
                    continue;
                };
                let mut built = new_row(&entry.path, &target, &entry, described, reader);
                if let Some(inline) = built.inline_bytes {
                    built.detail = String::new();
                    built.bytes = inline;
                    built.memory_type = String::new();
                    built.metrics.insert("bytes".to_string(), json!(inline));
                    for key in ["characters", "words", "tokens"] {
                        built.metrics.insert(key.to_string(), Value::Null);
                    }
                }
                index.insert(target, rows.len());
                rows.push(built);
            }
            Some(position) => {
                let existing = &mut rows[position];
                if entry.path != existing.path && !existing.aliases.contains(&entry.path) {
                    existing.aliases.push(entry.path.clone());
                }
                if !existing
                    .readers
                    .iter()
                    .any(|item| item.agent == reader.agent)
                {
                    existing.readers.push(reader);
                }
                if kind_rank(entry.kind) < kind_rank(existing.kind) {
                    existing.kind = entry.kind;
                }
            }
        }
    }
    for row in &mut rows {
        row.readers.sort_by(|left, right| {
            (left.agent, left.load_order).cmp(&(right.agent, right.load_order))
        });
        if row.readers.len() > 1 {
            row.badges.push("shared");
        }
        let flagged = row.readers.iter().filter(|reader| reader.excluded).count();
        row.excluded = flagged > 0 && flagged == row.readers.len();
        if row.excluded {
            row.badges.push("excluded");
        } else if flagged > 0 {
            row.badges.push("partly-excluded");
        }
        if row.inline_bytes.is_some() {
            row.badges.push("inline");
        }
        if row.path != row.realpath {
            row.badges.push("linked");
        }
        if row.kind == "auto-memory" {
            row.badges.push("agent-written");
        }
    }
    rows
}

pub fn collect_in(plan: &mut WatchPlan, context: &Context, config: &str) -> Map<String, Value> {
    let mut budget = Budget::default();
    let mut claims: Vec<Claim> = Vec::new();
    for adapter in ADAPTERS {
        claims.extend(adapter(plan, context, &mut budget));
    }
    if context.scope != "project" {
        let path = config_path(&context.home, &context.environ, config);
        let roots = extra_roots(plan, &path);
        claims.extend(extra_claims(plan, roots, &mut budget));
    }
    let rows = merge(plan, claims, &mut budget);
    let mut items: Vec<Value> = lane_rows(
        rows.iter().map(Row::value).collect(),
        &context.scope,
        &PROJECT_SCOPES,
        "scope",
    );
    items.sort_by_key(sort_key);
    let mut document = Map::new();
    document.insert("ok".to_string(), json!(true));
    document.insert(
        "project".to_string(),
        json!(path_text(&context.project_root)),
    );
    document.insert("home".to_string(), json!(path_text(&context.home)));
    document.insert("count".to_string(), json!(items.len()));
    document.insert("excludedKinds".to_string(), json!(EXCLUDED_KINDS));
    document.insert("items".to_string(), Value::Array(items));
    document.insert("schemaVersion".to_string(), json!(SCHEMA_VERSION));
    document.insert("truncated".to_string(), json!(budget.truncated()));
    document
}

fn sort_key(row: &Value) -> (usize, String, String) {
    let text = |key: &str| {
        row.get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    (
        kind_rank(&text("kind")),
        text("scope"),
        text("name").to_lowercase(),
    )
}

pub fn collect(
    plan: &mut WatchPlan,
    project: &str,
    home: &str,
    config: &str,
    environ: Environ,
    exact: bool,
    scope: &str,
) -> Map<String, Value> {
    let context = build_context(project, home, environ, exact, scope);
    collect_in(plan, &context, config)
}
