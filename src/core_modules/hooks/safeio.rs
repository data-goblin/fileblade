use crate::core_modules::canonical::sha256_hex;
use crate::core_modules::metrics::creation_time;
use crate::core_modules::watch::WatchPlan;
use regex::Regex;
use rustix::fs::{Mode, OFlags};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, BTreeSet};
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

pub const MAX_FILE_BYTES: usize = 256 * 1024;
pub const MAX_DIR_ENTRIES: usize = 256;
pub const MAX_TRAVERSAL_DEPTH: usize = 5;
pub const MAX_JSON_DEPTH: usize = 12;
pub const MAX_ITEMS_PER_SOURCE: usize = 200;
pub const MAX_SOURCES: i64 = 128;
pub const MAX_ROWS: i64 = 600;
pub const MAX_STDOUT_BYTES: usize = 1024 * 1024;

pub struct Budget {
    pub sources: i64,
    pub rows: i64,
    pub truncated: bool,
    pub hook_sources: BTreeMap<String, BTreeSet<PathBuf>>,
    pub plan: WatchPlan,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            sources: MAX_SOURCES,
            rows: MAX_ROWS,
            truncated: false,
            hook_sources: BTreeMap::new(),
            plan: WatchPlan::new(),
        }
    }
}

impl Budget {
    pub fn note_source(&mut self, agent: &str, path: &Path) {
        self.hook_sources
            .entry(agent.to_string())
            .or_default()
            .insert(path.to_path_buf());
    }

    pub fn knows(&self, agent: &str, path: &Path) -> bool {
        self.hook_sources
            .get(agent)
            .is_some_and(|paths| paths.contains(path))
    }

    pub fn take_source(&mut self) -> bool {
        if self.sources <= 0 {
            self.truncated = true;
            return false;
        }
        self.sources -= 1;
        true
    }

    pub fn take_row(&mut self) -> bool {
        if self.rows <= 0 {
            self.truncated = true;
            return false;
        }
        self.rows -= 1;
        true
    }
}

pub fn expanded(path: &str) -> PathBuf {
    match crate::common::parse_path(path) {
        Ok(parsed) => crate::common::expanded_os_path(&parsed),
        Err(_) => crate::common::expanded_os_path(Path::new(path)),
    }
}

pub fn expanded_os(value: &OsStr) -> PathBuf {
    match value.to_str() {
        Some(text) => expanded(text),
        None => crate::common::expanded_os_path(Path::new(value)),
    }
}

pub fn absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("/"))
            .join(path)
    }
}

pub fn stable_id(parts: &[&[u8]]) -> String {
    crate::core_modules::canonical::stable_id_bytes(parts)
}

pub fn digest(value: &str) -> String {
    sha256_hex(value.as_bytes())[..8].to_string()
}

pub fn suffix_bytes(path: &Path) -> &[u8] {
    let Some(name) = path.file_name() else {
        return b"";
    };
    let bytes = name.as_bytes();
    match bytes.iter().rposition(|byte| *byte == b'.') {
        Some(index) if index > 0 && index + 1 < bytes.len() => &bytes[index..],
        _ => b"",
    }
}

pub fn suffix_of(path: &Path) -> String {
    String::from_utf8_lossy(suffix_bytes(path)).into_owned()
}

pub fn has_suffix(path: &Path, suffix: &str) -> bool {
    suffix_bytes(path) == suffix.as_bytes()
}

pub fn resolved_file(budget: &mut Budget, path: &Path) -> Option<PathBuf> {
    budget.plan.watch_path(path, false);
    let resolved = std::fs::canonicalize(path).ok()?;
    let metadata = std::fs::symlink_metadata(&resolved).ok()?;
    metadata.is_file().then_some(resolved)
}

pub fn existing_file(budget: &mut Budget, path: &Path) -> bool {
    resolved_file(budget, path).is_some()
}

pub fn is_plain_dir(budget: &mut Budget, path: &Path) -> bool {
    budget.plan.watch_path(path, true);
    std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_dir())
}

fn secure_metadata(stat: &rustix::fs::Stat, owner_uid: u32) -> bool {
    stat.st_uid == owner_uid && stat.st_mode & (libc::S_IWGRP | libc::S_IWOTH) == 0
}

pub fn read_bytes(budget: &mut Budget, path: &Path, owner_uid: Option<u32>) -> Option<Vec<u8>> {
    let target = resolved_file(budget, path)?;
    let descriptor = rustix::fs::open(
        &target,
        OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK,
        Mode::empty(),
    )
    .ok()?;
    let metadata = rustix::fs::fstat(&descriptor).ok()?;
    if metadata.st_mode & libc::S_IFMT != libc::S_IFREG
        || metadata.st_size as usize > MAX_FILE_BYTES
    {
        return None;
    }
    if let Some(owner) = owner_uid
        && !secure_metadata(&metadata, owner)
    {
        return None;
    }
    let mut buffer = vec![0u8; MAX_FILE_BYTES];
    let count = rustix::io::read(&descriptor, &mut buffer).ok()?;
    buffer.truncate(count);
    Some(buffer)
}

pub fn strip_comments(text: &str) -> String {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    let pattern = PATTERN.get_or_init(|| {
        Regex::new(r#"(?s)("(?:\\.|[^"\\])*")|(//[^\n]*|/\*.*?\*/)"#).expect("comment pattern")
    });
    pattern
        .replace_all(text, |captures: &regex::Captures<'_>| {
            captures
                .get(1)
                .map(|quoted| quoted.as_str().to_string())
                .unwrap_or_else(|| " ".to_string())
        })
        .into_owned()
}

pub fn bounded_depth(value: &Value) -> bool {
    fn walk(value: &Value, level: usize) -> bool {
        if level > MAX_JSON_DEPTH {
            return false;
        }
        match value {
            Value::Object(entries) => entries.values().all(|item| walk(item, level + 1)),
            Value::Array(items) => items.iter().all(|item| walk(item, level + 1)),
            _ => true,
        }
    }
    walk(value, 0)
}

pub fn load_json(
    budget: &mut Budget,
    path: &Path,
    allow_comments: bool,
    owner_uid: Option<u32>,
) -> Option<Map<String, Value>> {
    let raw = read_bytes(budget, path, owner_uid)?;
    let text = String::from_utf8_lossy(&raw).into_owned();
    let text = if allow_comments {
        strip_comments(&text)
    } else {
        text
    };
    let parsed: Value = serde_json::from_str(&text).ok()?;
    let object = parsed.as_object()?;
    bounded_depth(&parsed).then(|| object.clone())
}

pub fn toml_value(value: &toml::Value) -> Value {
    match value {
        toml::Value::String(text) => Value::from(text.clone()),
        toml::Value::Integer(number) => Value::from(*number),
        toml::Value::Float(number) => Value::from(*number),
        toml::Value::Boolean(flag) => Value::from(*flag),
        toml::Value::Datetime(stamp) => Value::from(stamp.to_string()),
        toml::Value::Array(items) => Value::Array(items.iter().map(toml_value).collect()),
        toml::Value::Table(entries) => Value::Object(
            entries
                .iter()
                .map(|(key, item)| (key.clone(), toml_value(item)))
                .collect(),
        ),
    }
}

pub fn load_toml(budget: &mut Budget, path: &Path) -> Option<Map<String, Value>> {
    let raw = read_bytes(budget, path, None)?;
    let text = String::from_utf8_lossy(&raw).into_owned();
    let table: toml::Table = text.parse().ok()?;
    let parsed = Value::Object(
        table
            .iter()
            .map(|(key, item)| (key.clone(), toml_value(item)))
            .collect(),
    );
    bounded_depth(&parsed)
        .then(|| parsed.as_object().cloned())
        .flatten()
}

fn component_key(path: &Path) -> Vec<Vec<u8>> {
    path.components()
        .map(|component| component.as_os_str().as_bytes().to_vec())
        .collect()
}

fn sorted_paths(mut paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths.sort_by_key(|path| component_key(path));
    paths
}

pub fn scan_dir(budget: &mut Budget, directory: &Path, suffix: &str, depth: usize) -> Vec<PathBuf> {
    let depth = depth.min(MAX_TRAVERSAL_DEPTH);
    if depth == 0 || !is_plain_dir(budget, directory) {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = Vec::new();
    for (count, entry) in entries.enumerate() {
        if count >= MAX_DIR_ENTRIES {
            break;
        }
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        let Ok(metadata) = std::fs::symlink_metadata(&path) else {
            continue;
        };
        if metadata.is_symlink() {
            continue;
        }
        if metadata.is_dir() && depth > 1 {
            found.extend(scan_dir(budget, &path, suffix, depth - 1));
        } else if metadata.is_file() && has_suffix(&path, suffix) {
            found.push(path);
        }
    }
    let mut sorted = sorted_paths(found);
    sorted.truncate(MAX_DIR_ENTRIES);
    sorted
}

pub fn scan_dir_directories(budget: &mut Budget, root: &Path) -> Vec<PathBuf> {
    if !is_plain_dir(budget, root) {
        return Vec::new();
    }
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = Vec::new();
    for (count, entry) in entries.enumerate() {
        if count >= MAX_ITEMS_PER_SOURCE {
            break;
        }
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if std::fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_dir()) {
            found.push(path);
        }
    }
    sorted_paths(found)
}

pub fn artifact_metrics(budget: &mut Budget, path: &Path, text: &str) -> Map<String, Value> {
    let Some(target) = resolved_file(budget, path) else {
        return Map::new();
    };
    let Ok(metadata) = std::fs::symlink_metadata(&target) else {
        return Map::new();
    };
    let countable = text.len() <= MAX_FILE_BYTES;
    let counted = |value: Value| if countable { value } else { Value::Null };
    let updated = metadata
        .modified()
        .ok()
        .map(
            |moment| match moment.duration_since(std::time::UNIX_EPOCH) {
                Ok(elapsed) => crate::core_modules::metrics::minute_stamp(
                    elapsed.as_secs() as i64,
                    elapsed.subsec_nanos(),
                ),
                Err(error) => crate::core_modules::metrics::minute_stamp(
                    -(error.duration().as_secs() as i64),
                    0,
                ),
            },
        )
        .unwrap_or_default();
    let mut metrics = Map::new();
    metrics.insert("updated".to_string(), json!(updated));
    metrics.insert("created".to_string(), json!(creation_time(&target)));
    metrics.insert("bytes".to_string(), json!(text.len()));
    metrics.insert(
        "characters".to_string(),
        counted(json!(text.chars().count())),
    );
    metrics.insert(
        "words".to_string(),
        counted(json!(crate::core_modules::text::word_count(text))),
    );
    metrics.insert("tokens".to_string(), counted(json!(text.len().div_ceil(4))));
    metrics
}

pub fn document_kind(paths: &[&Path]) -> &'static str {
    if paths
        .iter()
        .any(|path| suffix_of(path).eq_ignore_ascii_case(".toml"))
    {
        "toml"
    } else {
        "json"
    }
}

fn parse_document(data: &[u8], kind: &str) -> Result<Value, String> {
    let text = std::str::from_utf8(data).map_err(|error| error.to_string())?;
    let parsed = if kind == "toml" {
        let table: toml::Table = text
            .parse()
            .map_err(|error: toml::de::Error| error.message().to_string())?;
        toml_value(&toml::Value::Table(table))
    } else {
        serde_json::from_str::<Value>(text).map_err(|error| error.to_string())?
    };
    if parsed.is_object() {
        Ok(parsed)
    } else {
        Err("the document is not a table of named keys".to_string())
    }
}

fn container_kind(value: &Value) -> &'static str {
    match value {
        Value::Object(_) => "table",
        Value::Array(_) => "array",
        _ => "value",
    }
}

fn listed(mut names: Vec<String>) -> String {
    names.sort();
    names.truncate(4);
    names.join(", ")
}

pub fn refuse_update(
    before: Option<&[u8]>,
    candidate: &[u8],
    kind: &str,
    removals: &[String],
) -> String {
    let label = kind.to_uppercase();
    let updated = match parse_document(candidate, kind) {
        Ok(value) => value,
        Err(error) => {
            return format!("the update does not parse as {label} ({error}); nothing was written");
        }
    };
    if !bounded_depth(&updated) {
        return format!("the update exceeds the {label} nesting limit; nothing was written");
    }
    let Some(before) = before else {
        return String::new();
    };
    let original = match parse_document(before, kind) {
        Ok(value) => value,
        Err(error) => {
            return format!("the file on disk is not valid {label} ({error}); nothing was written");
        }
    };
    let original = original.as_object().expect("parsed table");
    let updated = updated.as_object().expect("parsed table");
    let missing: Vec<String> = original
        .keys()
        .filter(|key| !updated.contains_key(*key) && !removals.contains(key))
        .cloned()
        .collect();
    if !missing.is_empty() {
        return format!(
            "the update would drop these top-level keys: {}; nothing was written",
            listed(missing)
        );
    }
    let reshaped: Vec<String> = original
        .iter()
        .filter(|(key, value)| {
            updated
                .get(*key)
                .is_some_and(|other| container_kind(value) != container_kind(other))
        })
        .map(|(key, _)| key.clone())
        .collect();
    if !reshaped.is_empty() {
        return format!(
            "the update would change the type of these top-level keys: {}; nothing was written",
            listed(reshaped)
        );
    }
    String::new()
}
