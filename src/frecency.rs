use crate::common::{display_path, parse_path, path_error, path_text};
use crate::filesystem::entry_for_path;
use crate::index;
use crate::secure::{read_private_bounded, write_private_atomic};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{SystemTime, UNIX_EPOCH};

pub const FRECENCY_CAP: usize = 2000;
const FRECENCY_BYTES: usize = 1024 * 1024;
const HALF_LIFE_SECONDS: f64 = 7.0 * 86_400.0;

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Entry {
    pub path: String,
    pub score: f64,
    pub last: i64,
}

impl Entry {
    fn rank(&self, now: i64) -> f64 {
        let age = (now - self.last).max(0) as f64;
        self.score * 2_f64.powf(-age / HALF_LIFE_SECONDS)
    }
}

pub fn path() -> PathBuf {
    crate::paths::state_dir().join("frecency.json")
}

fn now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|value| value.as_secs() as i64)
        .unwrap_or(0)
}

fn load() -> Vec<Entry> {
    read_private_bounded(&path(), FRECENCY_BYTES)
        .ok()
        .flatten()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn save(mut entries: Vec<Entry>) -> std::io::Result<()> {
    let now = now();
    entries.sort_by(|left, right| right.rank(now).total_cmp(&left.rank(now)));
    entries.truncate(FRECENCY_CAP);
    write_private_atomic(&path(), &serde_json::to_vec(&entries)?)
}

pub fn visit(paths: &[String]) -> Value {
    let mut entries = load();
    let now = now();
    for raw in paths {
        let path = match parse_path(raw) {
            Ok(path) => path_text(&path),
            Err(error) => return path_error(raw, &error),
        };
        match entries.iter_mut().find(|entry| entry.path == path) {
            Some(entry) => {
                entry.score = entry.rank(now) + 1.0;
                entry.last = now;
            }
            None => entries.push(Entry {
                path,
                score: 1.0,
                last: now,
            }),
        }
    }
    match save(entries) {
        Ok(()) => json!({"ok": true, "visited": paths.len()}),
        Err(error) => json!({"ok": false, "error": error.to_string()}),
    }
}

pub fn remap(mappings: &[(String, String)]) {
    if mappings.is_empty() {
        return;
    }
    let mut entries = load();
    for entry in &mut entries {
        let Ok(mut path) = parse_path(&entry.path) else {
            continue;
        };
        for (source, destination) in mappings {
            let (Ok(source), Ok(destination)) = (parse_path(source), parse_path(destination))
            else {
                continue;
            };
            if let Ok(rest) = path.strip_prefix(&source) {
                path = if rest.as_os_str().is_empty() {
                    destination
                } else {
                    destination.join(rest)
                };
            }
        }
        entry.path = path_text(&path);
    }
    let _ = save(entries);
}

pub fn remap_from(result: &Value) {
    let mappings = result["mappings"]
        .as_array()
        .map(|values| {
            values
                .iter()
                .filter_map(|value| {
                    Some((
                        value["source"].as_str()?.to_string(),
                        value["destination"].as_str()?.to_string(),
                    ))
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    remap(&mappings);
}

pub fn scores() -> HashMap<String, f64> {
    let now = now();
    load()
        .into_iter()
        .map(|entry| (entry.path.clone(), entry.rank(now)))
        .collect()
}

pub fn list(limit: usize, query: &str) -> Value {
    list_with_hidden(limit, query, false)
}

pub fn list_with_hidden(limit: usize, query: &str, show_hidden: bool) -> Value {
    let now = now();
    let mut entries = load();
    for imported in imported_recent(&entries) {
        entries.push(imported);
    }
    let mut visibility = crate::visibility::Visibility::new(std::path::Path::new("/"), show_hidden);
    entries.retain(|entry| {
        parse_path(&entry.path)
            .is_ok_and(|path| visibility.path(&path) && std::fs::symlink_metadata(path).is_ok())
    });
    let pattern = (!query.trim().is_empty())
        .then(|| index::parse_pattern(query.trim(), index::case_matching(false)));
    let mut matcher = nucleo::Matcher::new(index::matcher_config());
    let mut ranked = entries
        .into_iter()
        .filter_map(|entry| {
            let (score, indices) = match &pattern {
                Some(pattern) => index::score_text(
                    pattern,
                    &display_path(&parse_path(&entry.path).ok()?),
                    &mut matcher,
                )?,
                None => (0, Vec::new()),
            };
            Some((entry, score, indices))
        })
        .collect::<Vec<_>>();
    ranked.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then_with(|| right.0.rank(now).total_cmp(&left.0.rank(now)))
    });
    let rows = ranked
        .into_iter()
        .take(limit.clamp(1, FRECENCY_CAP))
        .filter_map(|(entry, _, indices)| row(&entry, now, &indices))
        .collect::<Vec<_>>();
    json!({"ok": true, "query": query.trim(), "entries": rows})
}

fn row(entry: &Entry, now: i64, indices: &[u32]) -> Option<Value> {
    let path = parse_path(&entry.path).ok()?;
    let mut item = entry_for_path(&path, false).ok()?;
    let display = display_path(&path);
    let name_offset = display
        .rfind('/')
        .map_or(0, |slash| display[..=slash].chars().count());
    item["relative"] = json!(display);
    item["frecency"] = json!(entry.rank(now));
    item["last_visit"] = json!(entry.last);
    item["relative_spans"] = json!(index::spans_text(indices, 0));
    item["name_spans"] = json!(index::spans_text(indices, name_offset));
    Some(item)
}

fn imported_recent(known: &[Entry]) -> Vec<Entry> {
    let file = crate::paths::xdg_home("XDG_DATA_HOME", "~/.local/share").join("recently-used.xbel");
    let Ok(Some(bytes)) = read_private_bounded(&file, FRECENCY_BYTES)
        .or_else(|_| crate::secure::read_bounded_nofollow(&file, FRECENCY_BYTES))
    else {
        return Vec::new();
    };
    let text = String::from_utf8_lossy(&bytes);
    let bookmark = regex::Regex::new(r#"<bookmark href="([^"]+)"[^>]*visited="([^"]+)""#)
        .expect("static regex");
    bookmark
        .captures_iter(&text)
        .filter_map(|found| {
            let path = crate::desktop::local_path_from_file_uri(&found[1])?;
            let path = path_text(&path);
            if known.iter().any(|entry| entry.path == path) {
                return None;
            }
            let visited = chrono::DateTime::parse_from_rfc3339(&found[2])
                .ok()?
                .timestamp();
            Some(Entry {
                path,
                score: 1.0,
                last: visited,
            })
        })
        .collect()
}
