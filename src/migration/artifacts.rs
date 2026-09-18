use super::prepare::Entry;
use super::storage::Content;
use crate::{AppError, AppResult};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Component, Path, PathBuf};

fn refuse(message: &str) -> AppError {
    AppError::invalid(format!("artifact recovery evidence: {message}"))
}

fn strings(value: &Value) -> bool {
    value
        .as_array()
        .is_some_and(|rows| rows.iter().all(Value::is_string))
}

fn module_name(value: &str) -> bool {
    (1..=32).contains(&value.len())
        && value.as_bytes()[0].is_ascii_lowercase()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
}

fn entry_name(value: &str) -> bool {
    (1..=121).contains(&value.len())
        && value.as_bytes()[0].is_ascii_alphanumeric()
        && !value.contains("..")
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || b"._:-".contains(&byte))
}

fn identifier(value: &str) -> bool {
    value.len() == 32
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(super) fn validate(entries: &[Entry]) -> AppResult<()> {
    let mut payloads = Vec::new();
    for entry in entries.iter().filter(|entry| entry.source == "bin") {
        let parts: Vec<_> = entry.from.iter().collect();
        let module = parts
            .first()
            .and_then(|part| part.to_str())
            .unwrap_or_default();
        if !module_name(module)
            || parts.len() > 1 && !entry_name(parts[1].to_str().unwrap_or_default())
        {
            return Err(refuse("invalid module or entry directory"));
        }
        if parts.len() <= 2 {
            if entry.content != Content::Directory {
                return Err(refuse("module or entry is not a directory"));
            }
            if parts.len() == 2
                && !entries.iter().any(|candidate| {
                    candidate.source == "bin" && candidate.from == entry.from.join("manifest.json")
                })
            {
                return Err(refuse("entry manifest is missing"));
            }
        }
        if parts.len() == 3 && parts[2] == "manifest.json" {
            let Content::File(bytes) = &entry.content else {
                return Err(refuse("manifest is not a regular file"));
            };
            if bytes.len() > 4 * 1024 * 1024 {
                return Err(refuse("manifest exceeds byte bound"));
            }
            let value: Value = serde_json::from_slice(bytes)?;
            let directory = entry.from.parent().unwrap();
            manifest(&value, module, directory, entries, &mut payloads)?;
        }
    }
    validate_payloads(payloads)
}

fn validate_payloads(records: Vec<Value>) -> AppResult<()> {
    if records.is_empty() {
        return Ok(());
    }
    let bytes = serde_json::to_vec(&serde_json::json!({"records": records}))?;
    if bytes.len() > 24 * 1024 * 1024 {
        return Err(refuse("helper payloads exceed byte bound"));
    }
    if records.len() > 4096 {
        return Err(refuse("helper payload does not satisfy the restore parser"));
    }
    for record in &records {
        if validated_payload(record).is_err() {
            return Err(refuse("helper payload does not satisfy the restore parser"));
        }
    }
    Ok(())
}

fn validated_payload(record: &Value) -> Result<(), ()> {
    use crate::core_modules::mcp::{parsers, records as mcp_records};
    let module = record.get("module").and_then(Value::as_str).ok_or(())?;
    let encoded = serde_json::to_vec(record.get("payload").ok_or(())?).map_err(|_| ())?;
    let payload = parsers::parse_json(&encoded).map_err(|_| ())?.to_json();
    let kind = payload
        .get("kind")
        .and_then(Value::as_str)
        .unwrap_or_default();
    match (module, kind) {
        ("hooks", _) => {
            let entries = payload.as_object().ok_or(())?;
            crate::core_modules::hooks::records::validate_record(entries).map_err(|_| ())
        }
        ("mcp", "json") => mcp_records::validate_json_record(&payload)
            .map(|_| ())
            .map_err(|_| ()),
        ("mcp", "toml") => mcp_records::validate_toml_record(&payload)
            .map(|_| ())
            .map_err(|_| ()),
        _ => Err(()),
    }
}

fn manifest(
    value: &Value,
    module: &str,
    directory: &Path,
    entries: &[Entry],
    payloads: &mut Vec<Value>,
) -> AppResult<()> {
    if !value.is_object()
        || value["schemaVersion"] != 1
        || value["module"] != module
        || [
            "module",
            "id",
            "name",
            "kind",
            "scope",
            "detail",
            "path",
            "realpath",
            "deletedAt",
        ]
        .iter()
        .any(|key| !value[*key].is_string())
    {
        return Err(refuse(
            "manifest has missing or unsupported required fields",
        ));
    }
    if value
        .get("deletedAtEpoch")
        .is_some_and(|value| !value.is_null() && value.as_i64().is_none())
    {
        return Err(refuse("invalid manifest timestamp"));
    }
    if value.get("position").is_some_and(|value| {
        !value.is_null()
            && !value
                .as_u64()
                .is_some_and(|position| position <= u32::MAX as u64)
    }) || value.get("groups").is_some_and(|value| !strings(value))
        || value.get("metrics").is_some_and(|value| !value.is_object())
        || value
            .get("helperRestored")
            .is_some_and(|value| !value.is_boolean())
        || value
            .get("restoreCompleted")
            .is_some_and(|value| !strings(value))
    {
        return Err(refuse("invalid optional manifest field"));
    }
    let route = match value.get("restoreHelper").filter(|value| !value.is_null()) {
        Some(route) => Some(
            serde_json::from_value::<crate::module_helpers::Route>(route.clone())
                .map_err(|_| refuse("invalid helper route"))?,
        ),
        None => None,
    };
    if value["helperRestored"] == true && route.is_none() {
        return Err(refuse("restored helper lacks its route"));
    }
    let id = value
        .get("helperRecordId")
        .filter(|value| !value.is_null())
        .map(|id| {
            id.as_str()
                .filter(|id| identifier(id))
                .ok_or_else(|| refuse("invalid helper record identifier"))
        })
        .transpose()?;
    if let Some(core) = route
        .as_ref()
        .and_then(|route| crate::module_helpers::canonical_route(&route.provider, &route.helper))
    {
        if core.module() != module || !["hooks", "mcp"].contains(&module) {
            return Err(refuse("core helper module mismatch"));
        }
        if value["helperRestored"] != true {
            let directory = PathBuf::from(format!("{module}-recovery"));
            let record = entries
                .iter()
                .find(|entry| {
                    if entry.source != "recovery"
                        || entry.from.parent() != Some(directory.as_path())
                    {
                        return false;
                    }
                    if let Some(id) = id {
                        return entry
                            .from
                            .file_name()
                            .is_some_and(|name| name == format!("{id}.json").as_str());
                    }
                    let Content::File(bytes) = &entry.content else {
                        return false;
                    };
                    serde_json::from_slice::<Value>(bytes)
                        .is_ok_and(|record| record["payload"] == value["payload"])
                })
                .ok_or_else(|| refuse("missing paired helper record"))?;
            let Content::File(bytes) = &record.content else {
                return Err(refuse("helper record is not a file"));
            };
            let record_id = record
                .from
                .file_stem()
                .and_then(|name| name.to_str())
                .filter(|id| identifier(id))
                .ok_or_else(|| refuse("invalid paired record filename"))?;
            if bytes.len() > 1024 * 1024 + 8192 {
                return Err(refuse("helper record exceeds its byte bound"));
            }
            let record: Value = serde_json::from_slice(bytes)?;
            if record["formatVersion"] != 1
                || !record["createdAt"].as_u64().is_some_and(|time| time > 0)
                || !record["context"].is_object()
                || !record["payload"].is_object()
                || record["payload"] != value["payload"]
                || record["definitionId"] != value["id"]
                || record
                    .get("transactionId")
                    .is_some_and(|transaction| transaction != record_id)
            {
                return Err(refuse("unsupported or mismatched helper record"));
            }
            let payload = &record["payload"];
            let source = if module == "hooks" { "source" } else { "path" };
            if payload["format"] != 2
                || !payload["agent"].is_string()
                || !payload[source].as_str().is_some_and(|value| {
                    crate::common::parse_path(value).is_ok_and(|path| path.is_absolute())
                })
                || !payload["target"].as_str().is_some_and(|value| {
                    crate::common::parse_path(value).is_ok_and(|path| path.is_absolute())
                })
                || module == "hooks"
                    && (!payload["event"].is_string() || !payload["entry"].is_object())
                || module == "mcp"
                    && ![Some("json"), Some("toml")].contains(&payload["kind"].as_str())
            {
                return Err(refuse("incomplete core helper payload"));
            }
            payloads.push(serde_json::json!({"module":module,"payload":payload}));
        }
    }
    let items = value["items"]
        .as_array()
        .filter(|items| items.len() <= 2000)
        .ok_or_else(|| refuse("invalid item array"))?;
    let mut sources = BTreeSet::new();
    let mut stored = BTreeMap::new();
    let mut bytes = 0u64;
    for item in items {
        let source = item["from"]
            .as_str()
            .ok_or_else(|| refuse("missing item source"))?;
        let kind = item["type"]
            .as_str()
            .ok_or_else(|| refuse("missing item kind"))?;
        let name = item["stored"]
            .as_str()
            .ok_or_else(|| refuse("missing stored path"))?;
        let size = item["size"]
            .as_u64()
            .ok_or_else(|| refuse("missing item size"))?;
        if !["file", "dir", "symlink"].contains(&kind)
            || size > 16 * 1024 * 1024
            || kind != "file" && size != 0
            || !item["mode"]
                .as_u64()
                .is_some_and(|mode| mode <= u32::MAX as u64)
            || !(source.starts_with('/') || source.starts_with("file://"))
            || source.contains('\0')
            || crate::common::parse_path(source).is_err()
            || Path::new(source)
                .components()
                .any(|part| part == Component::ParentDir)
            || !sources.insert(source)
            || stored.insert(name, kind).is_some()
        {
            return Err(refuse("invalid or duplicated stored item"));
        }
        for field in ["dev", "ino"] {
            if item
                .get(field)
                .is_some_and(|value| !value.is_null() && value.as_u64().is_none())
            {
                return Err(refuse("invalid stored identity"));
            }
        }
        for field in ["atime_ns", "mtime_ns"] {
            if item
                .get(field)
                .is_some_and(|value| !value.is_null() && value.as_i64().is_none())
            {
                return Err(refuse("invalid stored timestamp"));
            }
        }
        let target = match item.get("target") {
            None => "",
            Some(value) => value
                .as_str()
                .ok_or_else(|| refuse("invalid link target"))?,
        };
        let target_bytes = match item.get("target_bytes").filter(|value| !value.is_null()) {
            Some(value) => serde_json::from_value::<Vec<u8>>(value.clone())
                .map_err(|_| refuse("invalid link bytes"))?,
            None => target.as_bytes().to_vec(),
        };
        if target.contains('\0')
            || target_bytes.contains(&0)
            || name.contains('\0')
            || name
                .split('/')
                .any(|segment| segment.is_empty() || matches!(segment, "." | ".."))
            || !Path::new(name).starts_with("items")
            || Path::new(name).components().count() < 2
        {
            return Err(refuse("unsafe stored path or target"));
        }
        let object = entries
            .iter()
            .find(|entry| entry.source == "bin" && entry.from == directory.join(name))
            .ok_or_else(|| refuse("stored object missing"))?;
        match (&object.content, kind) {
            (Content::File(content), "file") if content.len() as u64 == size => {}
            (Content::Directory, "dir") => {}
            (Content::Link(content), "symlink") if content == &target_bytes => {}
            _ => return Err(refuse("stored object kind, size or link target mismatch")),
        }
        bytes = bytes.saturating_add(size);
    }
    if bytes > 64 * 1024 * 1024 {
        return Err(refuse("stored tree exceeds byte bound"));
    }
    for name in stored.keys() {
        let path = Path::new(name);
        if path.components().count() > 2
            && stored.get(path.parent().unwrap().to_str().unwrap()) != Some(&"dir")
        {
            return Err(refuse("stored child lacks directory evidence"));
        }
    }
    for object in entries
        .iter()
        .filter(|entry| entry.source == "bin" && entry.from.starts_with(directory))
    {
        let path = object.from.strip_prefix(directory).unwrap();
        if path.as_os_str().is_empty()
            || path == Path::new("manifest.json")
            || path == Path::new("items") && object.content == Content::Directory
        {
            continue;
        }
        if !stored.contains_key(path.to_str().unwrap_or_default()) {
            return Err(refuse("unrecorded object in artifact entry"));
        }
    }
    if let Some(completed) = value["restoreCompleted"].as_array() {
        for name in completed {
            let name = name.as_str().unwrap();
            if Path::new(name).components().count() != 2 || !stored.contains_key(name) {
                return Err(refuse("invalid completed restore root"));
            }
        }
    }
    Ok(())
}
