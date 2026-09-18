use super::safeio::MAX_ITEMS_PER_SOURCE;
use crate::core_modules::canonical::fingerprint as canonical_fingerprint;
use serde_json::{Map, Value, json};
use std::os::unix::ffi::OsStrExt;
use std::path::Path;

pub fn fingerprint(value: &Value) -> String {
    canonical_fingerprint(value)
}

pub fn group_fields(group: &Map<String, Value>) -> Map<String, Value> {
    group
        .iter()
        .filter(|(key, _)| key.as_str() != "hooks")
        .map(|(key, value)| (key.clone(), value.clone()))
        .collect()
}

pub fn identity(
    agent: &str,
    path: &Path,
    event: &str,
    index: usize,
    group: &Map<String, Value>,
    entry: &Map<String, Value>,
    namespace: &str,
) -> String {
    let mut content = Map::new();
    content.insert("group".to_string(), Value::Object(group_fields(group)));
    content.insert("entry".to_string(), Value::Object(entry.clone()));
    content.insert(
        "grouped".to_string(),
        json!(matches!(group.get("hooks"), Some(Value::Array(_)))),
    );
    let digest = fingerprint(&Value::Object(content));
    let index_text = index.to_string();
    super::safeio::stable_id(&[
        agent.as_bytes(),
        path.as_os_str().as_bytes(),
        event.as_bytes(),
        index_text.as_bytes(),
        namespace.as_bytes(),
        digest.as_bytes(),
    ])
}

pub fn row_group(row: &Map<String, Value>) -> String {
    row.get("group")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

pub fn container_key(row: &Map<String, Value>) -> String {
    if row.get("agent").and_then(Value::as_str) == Some("antigravity") {
        row_group(row)
    } else {
        "hooks".to_string()
    }
}

pub fn mapping<'a>(
    row: &Map<String, Value>,
    document: &'a Map<String, Value>,
) -> Option<&'a Map<String, Value>> {
    document.get(&container_key(row)).and_then(Value::as_object)
}

pub fn mapping_mut<'a>(
    row: &Map<String, Value>,
    document: &'a mut Map<String, Value>,
) -> Option<&'a mut Map<String, Value>> {
    document
        .get_mut(&container_key(row))
        .and_then(Value::as_object_mut)
}

pub struct Located {
    pub group_index: usize,
    pub entry_index: usize,
    pub group: Map<String, Value>,
    pub entry: Map<String, Value>,
}

fn row_index(row: &Map<String, Value>) -> Option<usize> {
    let value = row.get("index")?;
    let number = value.as_u64()?;
    if value.is_f64() {
        return None;
    }
    usize::try_from(number).ok()
}

pub fn locate(row: &Map<String, Value>, document: &Map<String, Value>) -> Option<Located> {
    let container = mapping(row, document)?;
    let event = row.get("event").and_then(Value::as_str)?;
    let Some(Value::Array(definitions)) = container.get(event) else {
        return None;
    };
    let index = row_index(row)?;
    let group_index = index / MAX_ITEMS_PER_SOURCE;
    let entry_index = index % MAX_ITEMS_PER_SOURCE;
    let group = definitions.get(group_index)?.as_object()?;
    let candidates: Vec<&Value> = match group.get("hooks") {
        Some(Value::Array(entries)) => entries.iter().collect(),
        _ => vec![definitions.get(group_index)?],
    };
    let entry = candidates.get(entry_index)?.as_object()?;
    Some(Located {
        group_index,
        entry_index,
        group: group.clone(),
        entry: entry.clone(),
    })
}

pub fn matches(row: &Map<String, Value>, document: &Map<String, Value>) -> bool {
    let Some(found) = locate(row, document) else {
        return false;
    };
    let Some(agent) = row.get("agent").and_then(Value::as_str) else {
        return false;
    };
    let Some(event) = row.get("event").and_then(Value::as_str) else {
        return false;
    };
    let Some(index) = row_index(row) else {
        return false;
    };
    let Some(source) = row
        .get("source")
        .and_then(Value::as_object)
        .and_then(|source| source.get("path"))
        .and_then(Value::as_str)
    else {
        return false;
    };
    let path =
        crate::common::parse_path(source).unwrap_or_else(|_| Path::new(source).to_path_buf());
    let expected = identity(
        agent,
        &path,
        event,
        index,
        &found.group,
        &found.entry,
        &row_group(row),
    );
    row.get("id").and_then(Value::as_str) == Some(expected.as_str())
}

pub fn detach(
    row: &Map<String, Value>,
    document: &mut Map<String, Value>,
) -> Option<Map<String, Value>> {
    let found = locate(row, document)?;
    let event = row.get("event").and_then(Value::as_str)?.to_string();
    let grouped = matches!(found.group.get("hooks"), Some(Value::Array(_)));
    let entry_count = match found.group.get("hooks") {
        Some(Value::Array(entries)) => entries.len(),
        _ => 0,
    };
    let removed_group = !grouped || entry_count == 1;
    let container = mapping_mut(row, document)?;
    let Some(Value::Array(definitions)) = container.get_mut(&event) else {
        return None;
    };
    let before = fingerprint(&Value::Array(definitions.clone()));
    if removed_group {
        definitions.remove(found.group_index);
    } else if let Some(Value::Array(entries)) = definitions
        .get_mut(found.group_index)
        .and_then(Value::as_object_mut)
        .and_then(|group| group.get_mut("hooks"))
    {
        entries.remove(found.entry_index);
    }
    let remaining = definitions.clone();
    if remaining.is_empty() {
        container.shift_remove(&event);
    }
    let after = if remaining.is_empty() {
        fingerprint(&Value::Null)
    } else {
        fingerprint(&Value::Array(remaining))
    };
    let mut record = Map::new();
    record.insert("format".to_string(), json!(2));
    record.insert(
        "agent".to_string(),
        json!(row.get("agent").and_then(Value::as_str).unwrap_or_default()),
    );
    record.insert("event".to_string(), json!(event));
    record.insert("group".to_string(), json!(row_group(row)));
    record.insert(
        "index".to_string(),
        json!(row_index(row).unwrap_or_default()),
    );
    record.insert("grouped".to_string(), json!(grouped));
    record.insert("removedGroup".to_string(), json!(removed_group));
    record.insert(
        "fields".to_string(),
        Value::Object(if grouped {
            group_fields(&found.group)
        } else {
            Map::new()
        }),
    );
    record.insert("entry".to_string(), Value::Object(found.entry.clone()));
    record.insert("before".to_string(), json!(before));
    record.insert("after".to_string(), json!(after));
    Some(record)
}

fn hex_digest(record: &Map<String, Value>, key: &str) -> bool {
    record
        .get(key)
        .and_then(Value::as_str)
        .is_some_and(|value| {
            value.len() == 64
                && value
                    .bytes()
                    .all(|byte| b"0123456789abcdef".contains(&byte))
        })
}

pub fn validate_record(record: &Map<String, Value>) -> Result<(), String> {
    let incomplete = || "restore record is incomplete".to_string();
    if record.get("format") != Some(&json!(2)) {
        return Err(incomplete());
    }
    let index = match record.get("index") {
        Some(value) if value.is_u64() && !value.is_f64() => value.as_u64().unwrap_or_default(),
        _ => return Err(incomplete()),
    };
    if index >= (MAX_ITEMS_PER_SOURCE * MAX_ITEMS_PER_SOURCE) as u64 {
        return Err(incomplete());
    }
    let grouped = match record.get("grouped") {
        Some(Value::Bool(flag)) => *flag,
        _ => return Err(incomplete()),
    };
    let removed_group = match record.get("removedGroup") {
        Some(Value::Bool(flag)) => *flag,
        _ => return Err(incomplete()),
    };
    let Some(Value::Object(fields)) = record.get("fields") else {
        return Err(incomplete());
    };
    if fields.contains_key("hooks") {
        return Err(incomplete());
    }
    if !record.get("entry").is_some_and(Value::is_object)
        || !record.get("group").is_some_and(Value::is_string)
        || !hex_digest(record, "before")
        || !hex_digest(record, "after")
    {
        return Err(incomplete());
    }
    let position = "restore record has an invalid group position".to_string();
    if !grouped && !removed_group {
        return Err(position);
    }
    if grouped && removed_group && !(index as usize).is_multiple_of(MAX_ITEMS_PER_SOURCE) {
        return Err(position);
    }
    Ok(())
}

pub fn attach(
    record: &Map<String, Value>,
    document: &mut Map<String, Value>,
) -> Result<bool, String> {
    validate_record(record)?;
    let event = record
        .get("event")
        .and_then(Value::as_str)
        .ok_or_else(|| "restore record is incomplete".to_string())?
        .to_string();
    if mapping(record, document).is_none() {
        return Err("the source hook container changed; restore it manually".to_string());
    }
    let current = mapping(record, document)
        .and_then(|container| container.get(&event))
        .cloned();
    let current_value = current.clone().unwrap_or(Value::Null);
    let current_id = fingerprint(&current_value);
    if Some(current_id.as_str()) == record.get("before").and_then(Value::as_str) {
        return Ok(false);
    }
    let changed = "the source event changed since removal; restore it manually".to_string();
    if Some(current_id.as_str()) != record.get("after").and_then(Value::as_str) {
        return Err(changed);
    }
    if current.as_ref().is_some_and(|value| !value.is_array()) {
        return Err(changed);
    }
    let mut definitions: Vec<Value> = match current {
        Some(Value::Array(items)) => items,
        _ => Vec::new(),
    };
    let index = record
        .get("index")
        .and_then(Value::as_u64)
        .unwrap_or_default() as usize;
    let group_index = index / MAX_ITEMS_PER_SOURCE;
    let entry_index = index % MAX_ITEMS_PER_SOURCE;
    let grouped = record.get("grouped") == Some(&json!(true));
    let removed_group = record.get("removedGroup") == Some(&json!(true));
    let entry = record.get("entry").cloned().unwrap_or(Value::Null);
    let position = "restore record has an invalid group position".to_string();
    if removed_group {
        if group_index > definitions.len() || (grouped && entry_index != 0) {
            return Err(position);
        }
        let group = if grouped {
            let mut fields = record
                .get("fields")
                .and_then(Value::as_object)
                .cloned()
                .unwrap_or_default();
            fields.insert("hooks".to_string(), Value::Array(vec![entry]));
            Value::Object(fields)
        } else {
            entry
        };
        definitions.insert(group_index, group);
    } else {
        let entries = definitions
            .get_mut(group_index)
            .and_then(Value::as_object_mut)
            .and_then(|group| group.get_mut("hooks"))
            .and_then(Value::as_array_mut);
        let invalid = "restore record has an invalid entry position".to_string();
        let Some(entries) = entries else {
            return Err(invalid);
        };
        if !grouped || entry_index > entries.len() {
            return Err(invalid);
        }
        entries.insert(entry_index, entry);
    }
    if fingerprint(&Value::Array(definitions.clone()))
        != record
            .get("before")
            .and_then(Value::as_str)
            .unwrap_or_default()
    {
        return Err("restore record does not match the original event".to_string());
    }
    let container = mapping_mut(record, document)
        .ok_or_else(|| "the source hook container changed; restore it manually".to_string())?;
    container.insert(event, Value::Array(definitions));
    Ok(true)
}
