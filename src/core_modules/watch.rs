use crate::common::path_text;
use serde_json::{Map, Value, json};
use std::collections::{BTreeSet, HashMap, HashSet};
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

pub const MAX_WATCH_PATHS: usize = 512;
pub const MAX_WATCH_CANDIDATES: usize = 2048;
pub const MAX_WATCH_BYTES: usize = 64 * 1024;
pub const SCOPES: [&str; 3] = ["all", "user", "project"];
const MAX_WALK_UP: usize = 64;

#[derive(Default)]
pub struct WatchPlan {
    entries: HashMap<PathBuf, Option<BTreeSet<String>>>,
    candidates: HashSet<(PathBuf, bool)>,
    truncated: bool,
}

impl WatchPlan {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn truncated(&self) -> bool {
        self.truncated
    }

    pub fn paths(&self) -> Vec<PathBuf> {
        let mut paths: Vec<PathBuf> = self.entries.keys().cloned().collect();
        paths.sort_by(|left, right| {
            left.as_os_str()
                .as_bytes()
                .cmp(right.as_os_str().as_bytes())
        });
        paths
    }

    pub fn names(&self, path: &Path) -> Option<&BTreeSet<String>> {
        self.entries.get(path).and_then(Option::as_ref)
    }

    fn insert(&mut self, directory: PathBuf, child: Option<&OsStr>) {
        let wanted = child.map(|name| name.to_str().map(str::to_string));
        let entry = self
            .entries
            .entry(directory)
            .or_insert_with(|| Some(BTreeSet::new()));
        match (entry.as_mut(), wanted) {
            (Some(names), Some(Some(name))) => {
                names.insert(name);
            }
            (Some(_), _) => *entry = None,
            (None, _) => {}
        }
    }

    fn admit(&mut self, path: &Path, whole: bool) -> bool {
        if !path.is_absolute() || self.candidates.contains(&(path.to_path_buf(), whole)) {
            return false;
        }
        if self.entries.len() >= MAX_WATCH_PATHS || self.candidates.len() >= MAX_WATCH_CANDIDATES {
            self.truncated = true;
            return false;
        }
        self.candidates.insert((path.to_path_buf(), whole));
        true
    }

    fn nearest(&mut self, path: &Path, child: Option<&OsStr>) {
        let mut current = path.to_path_buf();
        let mut wanted = child.map(OsStr::to_os_string);
        for _ in 0..MAX_WALK_UP {
            if current.is_dir() {
                let resolved = std::fs::canonicalize(&current).unwrap_or(current);
                self.insert(resolved, wanted.as_deref());
                return;
            }
            let Some(parent) = current.parent() else {
                return;
            };
            if parent == current {
                return;
            }
            wanted = current.file_name().map(OsStr::to_os_string);
            current = parent.to_path_buf();
        }
    }

    pub fn directory(&mut self, path: &Path) {
        if self.admit(path, true) {
            self.nearest(path, None);
        }
    }

    fn child(&mut self, path: &Path) {
        let (Some(parent), Some(name)) = (path.parent(), path.file_name()) else {
            return;
        };
        if self.admit(path, false) {
            self.nearest(parent, Some(name));
        }
    }

    pub fn watch_path(&mut self, path: &Path, directory: bool) {
        self.child(path);
        if directory {
            self.directory(path);
            return;
        }
        if std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_symlink())
            && let Ok(resolved) = std::fs::canonicalize(path)
        {
            self.child(&resolved);
        }
    }

    pub fn finish(&mut self, document: &mut Map<String, Value>) {
        let mut encoded = Vec::new();
        let mut names = Map::new();
        let mut size = 2;
        for path in self.paths() {
            let text = path_text(&path);
            let mut wire = String::new();
            super::canonical::escape_ascii(&text, &mut wire);
            let children = self.names(&path).cloned().unwrap_or_default();
            let cost = wire.len() + 1 + children.iter().map(|name| name.len() + 3).sum::<usize>();
            if size + cost > MAX_WATCH_BYTES {
                self.truncated = true;
                break;
            }
            size += cost;
            if !children.is_empty() {
                names.insert(text.clone(), json!(children));
            }
            encoded.push(Value::from(text));
        }
        document.insert("watchPaths".to_string(), Value::Array(encoded));
        if !names.is_empty() {
            document.insert("watchNames".to_string(), Value::Object(names));
        }
        document.insert("watchTruncated".to_string(), json!(self.truncated));
    }
}

pub fn lane_rows(rows: Vec<Value>, scope: &str, project_scopes: &[&str], key: &str) -> Vec<Value> {
    let selects = |row: &Value| {
        row.get(key)
            .and_then(Value::as_str)
            .is_some_and(|value| project_scopes.contains(&value))
    };
    match scope {
        "project" => rows.into_iter().filter(selects).collect(),
        "user" => rows.into_iter().filter(|row| !selects(row)).collect(),
        _ => rows,
    }
}
