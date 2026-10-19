use crate::common::{parse_path, path_text};
use crate::secure;
use serde_json::{Value, json};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

const INTENT_BYTES: usize = 16 * 1024;

pub fn inflight_dir() -> PathBuf {
    crate::paths::state_dir().join("inflight")
}

pub fn sweep() -> Value {
    let mut restored = Vec::new();
    let mut removed = Vec::new();
    let mut conflicts = Vec::new();
    let mut deferred = Vec::new();
    let mut errors = Vec::new();
    let directory = inflight_dir();
    let entries = match fs::read_dir(&directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return summary(restored, removed, conflicts, deferred, errors);
        }
        Err(error) => {
            errors.push(json!({"path": path_text(&directory), "error": error.to_string()}));
            return summary(restored, removed, conflicts, deferred, errors);
        }
    };
    let mut intents: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension()
                .is_some_and(|extension| extension == "json")
        })
        .collect();
    intents.sort();
    for intent in intents {
        match recover_intent(&intent) {
            Ok(Outcome::Restored(path)) => restored.push(path),
            Ok(Outcome::Removed(path)) => removed.push(path),
            Ok(Outcome::Conflict(value)) => conflicts.push(value),
            Ok(Outcome::Deferred(folder)) => {
                deferred.push(json!({"path": path_text(&intent), "folder": folder}))
            }
            Ok(Outcome::Done) => {}
            Err(error) => {
                errors.push(json!({"path": path_text(&intent), "error": error.to_string()}))
            }
        }
    }
    summary(restored, removed, conflicts, deferred, errors)
}

enum Outcome {
    Restored(String),
    Removed(String),
    Conflict(Value),
    Deferred(String),
    Done,
}

fn recover_intent(intent: &Path) -> std::io::Result<Outcome> {
    let Some(lease) = secure::try_lock_private(intent)? else {
        return Ok(Outcome::Done);
    };
    let mut bytes = Vec::new();
    lease
        .file()
        .take((INTENT_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > INTENT_BYTES {
        return Err(std::io::Error::other(
            "recovery intent exceeds its size limit",
        ));
    }
    let record: Value = match serde_json::from_slice(&bytes) {
        Ok(value) => value,
        Err(_) => {
            secure::remove_nondirectory(intent)?;
            return Ok(Outcome::Done);
        }
    };
    let kind = record["kind"].as_str().unwrap_or_default();
    let outcome = match kind {
        "stage" => recover_stage(&record)?,
        "partial" => recover_partial(&record)?,
        _ => Outcome::Done,
    };
    if !matches!(outcome, Outcome::Conflict(_) | Outcome::Deferred(_)) {
        secure::remove_nondirectory(intent)?;
    }
    Ok(outcome)
}

fn recover_stage(record: &Value) -> std::io::Result<Outcome> {
    let stage = record_path(record, "stage")?;
    let item = stage.join(record["item"].as_str().unwrap_or_default());
    let original = record_path(record, "original")?;
    match reachable(&stage)? {
        None => return Ok(Outcome::Deferred(parent_text(&stage))),
        Some(false) => return Ok(Outcome::Done),
        Some(true) => {}
    }
    if !secure::entry_exists(&item)? {
        let _ = secure::remove_empty_directory(&stage);
        return Ok(Outcome::Done);
    }
    if secure::entry_exists(&original)? {
        return Ok(Outcome::Conflict(json!({
            "original": crate::common::path_text(&original),
            "staged": crate::common::path_text(&item),
        })));
    }
    secure::rename_noreplace(&item, &original)?;
    let _ = secure::remove_empty_directory(&stage);
    Ok(Outcome::Restored(crate::common::path_text(&original)))
}

fn recover_partial(record: &Value) -> std::io::Result<Outcome> {
    let partial = record_path(record, "partial")?;
    match reachable(&partial)? {
        None => return Ok(Outcome::Deferred(parent_text(&partial))),
        Some(false) => return Ok(Outcome::Done),
        Some(true) => {}
    }
    let name = partial
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or_default();
    if !name.starts_with(".fileblade-partial-") {
        return Ok(Outcome::Done);
    }
    match (record["dev"].as_u64(), record["ino"].as_u64()) {
        (Some(dev), Some(ino)) => secure::remove_path_matching(
            &partial,
            secure::EntryIdentity {
                dev,
                ino,
                kind: secure::EntryKind::Directory,
            },
        )?,
        (None, None) => secure::remove_path(&partial)?,
        _ => return Err(std::io::Error::other("Incomplete staging identity")),
    }
    Ok(Outcome::Removed(crate::common::path_text(&partial)))
}

fn reachable(path: &Path) -> std::io::Result<Option<bool>> {
    match secure::entry_exists(path) {
        Ok(found) => Ok(Some(found)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(error),
    }
}

fn parent_text(path: &Path) -> String {
    path_text(path.parent().unwrap_or(path))
}

fn record_path(record: &Value, field: &str) -> std::io::Result<PathBuf> {
    let raw = record[field].as_str().unwrap_or_default();
    if !(raw.starts_with('/') || raw.starts_with("file://")) {
        return Err(std::io::Error::other("Invalid recovery path"));
    }
    parse_path(raw)
}

fn summary(
    restored: Vec<String>,
    removed: Vec<String>,
    conflicts: Vec<Value>,
    deferred: Vec<Value>,
    errors: Vec<Value>,
) -> Value {
    json!({
        "ok": errors.is_empty(),
        "operation": "recover",
        "restored": restored,
        "removed": removed,
        "conflicts": conflicts,
        "deferred": deferred,
        "errors": errors,
    })
}
