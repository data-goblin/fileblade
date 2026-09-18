use super::redaction::clean_label;
use super::safeio::{Budget, expanded_os, load_json};
use crate::common::path_text;
use serde_json::{Map, Value, json};
use std::ffi::OsString;
use std::path::{Path, PathBuf};

pub const MAX_LABELS: usize = 512;

pub type Environ = Vec<(OsString, OsString)>;

pub fn environ_value(environ: &Environ, name: &str) -> OsString {
    environ
        .iter()
        .find(|(key, _)| key == name)
        .map(|(_, value)| value.clone())
        .unwrap_or_default()
}

pub fn labels_path(home: &Path, environ: &Environ) -> PathBuf {
    let configured = environ_value(environ, "XDG_CONFIG_HOME");
    let base = if configured.is_empty() {
        home.join(".config")
    } else {
        expanded_os(&configured)
    };
    base.join("omarchy")
        .join("fileblade")
        .join("hooks")
        .join("labels.json")
}

fn is_digest(value: &str) -> bool {
    value.len() == 8
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub fn load_labels(budget: &mut Budget, path: &Path) -> Map<String, Value> {
    let mut labels = Map::new();
    let Some(document) = load_json(budget, path, false, None) else {
        return labels;
    };
    let Some(Value::Object(raw)) = document.get("labels") else {
        return labels;
    };
    for (digest, text) in raw.iter().take(MAX_LABELS) {
        if !is_digest(digest) || !text.is_string() {
            continue;
        }
        let cleaned = clean_label(Some(text));
        if !cleaned.is_empty() {
            labels.insert(digest.clone(), json!(cleaned));
        }
    }
    labels
}

pub fn attach_labels(rows: &mut [Value], labels: &Map<String, Value>) {
    for row in rows {
        let Some(summary) = row.get_mut("summary").and_then(Value::as_object_mut) else {
            continue;
        };
        let digest = summary
            .get("digest")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string();
        let Some(text) = labels.get(&digest).and_then(Value::as_str) else {
            continue;
        };
        if text.is_empty() {
            continue;
        }
        summary.insert("label".to_string(), json!(text));
        summary.insert("labelSource".to_string(), json!("user"));
    }
}

fn refusal(digest: &str, label: &str, message: &str) -> Value {
    json!({
        "ok": false,
        "schemaVersion": 1,
        "digest": digest,
        "label": label,
        "changed": false,
        "message": message,
        "touched": [],
    })
}

pub fn set_label(home: &Path, environ: &Environ, digest: &str, text: &str) -> Value {
    if !is_digest(digest) {
        return refusal("", "", "digest must be eight hex characters");
    }
    let cleaned = clean_label(Some(&Value::from(text)));
    let path = labels_path(home, environ);
    let mut budget = Budget::default();
    let mut labels = load_labels(&mut budget, &path);
    if labels.len() >= MAX_LABELS && !cleaned.is_empty() && !labels.contains_key(digest) {
        return refusal(digest, "", "the label store is full");
    }
    let current = labels
        .get(digest)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();
    let changed = current != cleaned;
    if changed {
        if cleaned.is_empty() {
            labels.shift_remove(digest);
        } else {
            labels.insert(digest.to_string(), json!(cleaned));
        }
        let mut document = Map::new();
        document.insert("version".to_string(), json!(1));
        document.insert("labels".to_string(), Value::Object(labels));
        if let Err(message) = super::apply::write_path(&path, &document, &[]) {
            return refusal(digest, &cleaned, &message);
        }
    }
    json!({
        "ok": true,
        "schemaVersion": 1,
        "digest": digest,
        "label": cleaned,
        "changed": changed,
        "message": "",
        "touched": if changed { vec![Value::from(path_text(&path))] } else { Vec::new() },
    })
}
