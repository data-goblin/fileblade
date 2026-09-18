use super::model::Definition;
use super::parsers::parse_toml;
use super::tomlwrite::{TomlWriteFailure, append_server_block, locate_server_block, split_lines};
use super::value::{Cfg, fingerprint, fingerprint_json};
use crate::core_modules::canonical::sha256_hex;
use serde_json::{Map, Value, json};

pub const MAX_POSITION: i64 = 4096;

#[derive(Debug, Clone)]
pub struct RecordError(pub String);

impl std::fmt::Display for RecordError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

fn refuse(message: &str) -> RecordError {
    RecordError(message.to_string())
}

impl From<TomlWriteFailure> for RecordError {
    fn from(error: TomlWriteFailure) -> Self {
        RecordError(error.0)
    }
}

pub fn text_digest(text: &str) -> String {
    sha256_hex(text.as_bytes())
}

pub fn container_path(definition: &Definition, document: &Cfg) -> Vec<String> {
    let named = |parts: &[&str]| parts.iter().map(|part| (*part).to_string()).collect();
    if definition.agent == "opencode" {
        return if definition.source_kind.starts_with("opencode-v2-") {
            named(&["mcp", "servers"])
        } else {
            named(&["mcp"])
        };
    }
    if definition.agent == "pi" || definition.source_kind == "copilot-project" {
        return if document
            .get("mcpServers")
            .is_some_and(|value| value.is_table())
        {
            named(&["mcpServers"])
        } else {
            Vec::new()
        };
    }
    named(&["mcpServers"])
}

fn descend<'a>(document: &'a Cfg, path: &[String]) -> Result<&'a Cfg, RecordError> {
    let mut current = document;
    for key in path {
        let next = current
            .get(key)
            .ok_or_else(|| refuse("the source container changed; refresh and retry"))?;
        if !next.is_table() {
            return Err(refuse("the source container changed; refresh and retry"));
        }
        current = next;
    }
    Ok(current)
}

fn descend_mut<'a>(
    document: &'a mut Cfg,
    path: &[String],
    create: bool,
) -> Result<&'a mut Cfg, RecordError> {
    let mut current = document;
    for key in path {
        if create && !current.contains(key) {
            current.insert(key, Cfg::table());
        }
        let next = current
            .as_table_mut()
            .and_then(|entries| {
                entries
                    .iter_mut()
                    .find(|(name, _)| name == key)
                    .map(|(_, value)| value)
            })
            .ok_or_else(|| refuse("the source container changed; refresh and retry"))?;
        if !next.is_table() {
            return Err(refuse("the source container changed; refresh and retry"));
        }
        current = next;
    }
    Ok(current)
}

pub fn detach_json(
    definition: &Definition,
    document: &mut Cfg,
) -> Result<Map<String, Value>, RecordError> {
    let path = container_path(definition, document);
    let mapping = descend(document, &path)?;
    let name = definition.raw_name.clone();
    let raw_config = definition.raw_config.clone().unwrap_or(Cfg::Null);
    let existing = mapping.get(&name);
    if existing.is_none_or(|value| fingerprint(value) != fingerprint(&raw_config)) {
        return Err(refuse("the source definition changed; refresh and retry"));
    }
    let position = mapping.position(&name).unwrap_or_default();
    let mapping = descend_mut(document, &path, false)?;
    let raw = mapping
        .remove(&name)
        .ok_or_else(|| refuse("the source definition changed; refresh and retry"))?;
    let mut record = Map::new();
    record.insert("kind".to_string(), json!("json"));
    record.insert("container".to_string(), json!(path));
    record.insert("name".to_string(), json!(name));
    record.insert("raw".to_string(), raw.to_json());
    record.insert("position".to_string(), json!(position));
    record.insert("definition".to_string(), json!(fingerprint(&raw)));
    Ok(record)
}

pub struct JsonRecord {
    pub container: Vec<String>,
    pub name: String,
    pub raw: Value,
    pub position: usize,
}

fn valid_container(path: &[String]) -> bool {
    let parts: Vec<&str> = path.iter().map(String::as_str).collect();
    matches!(
        parts.as_slice(),
        [] | ["mcpServers"] | ["mcp"] | ["mcp", "servers"]
    )
}

pub fn validate_json_record(record: &Value) -> Result<JsonRecord, RecordError> {
    let container: Vec<String> = match record.get("container") {
        Some(Value::Array(items)) if items.iter().all(Value::is_string) => items
            .iter()
            .map(|item| item.as_str().unwrap_or_default().to_string())
            .collect(),
        _ => return Err(refuse("restore record has an invalid source container")),
    };
    let name = record
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if !valid_container(&container) || name.is_empty() {
        return Err(refuse("restore record has an invalid source container"));
    }
    let raw = record.get("raw").cloned().unwrap_or(Value::Null);
    let position = match record.get("position") {
        Some(Value::Number(number)) if !number.is_f64() => number.as_i64().unwrap_or(-1),
        _ => -1,
    };
    if !raw.is_object() || !(0..=MAX_POSITION).contains(&position) {
        return Err(refuse("restore record has an invalid definition"));
    }
    if record.get("definition").and_then(Value::as_str) != Some(fingerprint_json(&raw).as_str()) {
        return Err(refuse("restore definition does not match its fingerprint"));
    }
    Ok(JsonRecord {
        container,
        name: name.to_string(),
        raw,
        position: position as usize,
    })
}

pub fn attach_json(document: &mut Cfg, record: &Value) -> Result<bool, RecordError> {
    let parsed = validate_json_record(record)?;
    let raw = Cfg::from_json(&parsed.raw);
    let mapping = descend_mut(document, &parsed.container, true)?;
    if let Some(existing) = mapping.get(&parsed.name) {
        if fingerprint(existing) == fingerprint(&raw) {
            return Ok(false);
        }
        return Err(refuse(
            "the source already has a different definition; nothing was changed",
        ));
    }
    mapping.insert_at(parsed.position, &parsed.name, raw);
    Ok(true)
}

pub fn normalized_toml(document: &Cfg) -> Cfg {
    let mut result = document.clone();
    if result
        .get("mcp_servers")
        .and_then(Cfg::as_table)
        .is_some_and(Vec::is_empty)
    {
        result.remove("mcp_servers");
    }
    result
}

fn parsed_toml(text: &str) -> Result<Cfg, RecordError> {
    parse_toml(text.as_bytes()).map_err(|error| RecordError(error.0))
}

pub fn detach_toml(
    text: &str,
    name: &str,
    raw: &Cfg,
) -> Result<(String, Map<String, Value>), RecordError> {
    let mut document = parsed_toml(text)?;
    let changed = || refuse("the source definition changed; refresh and retry");
    let mapping = document.get("mcp_servers").ok_or_else(changed)?;
    if !mapping.is_table() {
        return Err(changed());
    }
    let existing = mapping.get(name).ok_or_else(changed)?;
    if fingerprint(existing) != fingerprint(raw) {
        return Err(changed());
    }
    let Some((mut start, end)) = locate_server_block(text, name)? else {
        return Err(RecordError("table-not-found".to_string()));
    };
    let lines = split_lines(text);
    while start > 0 && lines[start - 1].trim().is_empty() {
        start -= 1;
    }
    let prefix: String = lines[..start].concat();
    let fragment: String = lines[start..end].concat();
    let suffix: String = lines[end..].concat();
    let fragment_value = parsed_toml(&fragment)?;
    let expected = Cfg::Table(vec![(
        "mcp_servers".to_string(),
        Cfg::Table(vec![(name.to_string(), raw.clone())]),
    )]);
    if fingerprint(&fragment_value) != fingerprint(&expected) {
        return Err(refuse(
            "the source table cannot be isolated without changing other settings",
        ));
    }
    let updated = format!("{prefix}{suffix}");
    if let Some(servers) = document.as_table_mut().and_then(|entries| {
        entries
            .iter_mut()
            .find(|(key, _)| key == "mcp_servers")
            .map(|(_, value)| value)
    }) {
        servers.remove(name);
    }
    if fingerprint(&normalized_toml(&parsed_toml(&updated)?))
        != fingerprint(&normalized_toml(&document))
    {
        return Err(refuse(
            "removing the source table would change other settings",
        ));
    }
    let mut record = Map::new();
    record.insert("kind".to_string(), json!("toml"));
    record.insert("name".to_string(), json!(name));
    record.insert("text".to_string(), json!(fragment));
    record.insert("offset".to_string(), json!(prefix.chars().count()));
    record.insert("after".to_string(), json!(text_digest(&updated)));
    record.insert("definition".to_string(), json!(fingerprint(raw)));
    Ok((updated, record))
}

pub struct TomlRecord {
    pub name: String,
    pub fragment: String,
    pub offset: usize,
    pub raw: Cfg,
}

pub fn validate_toml_record(record: &Value) -> Result<TomlRecord, RecordError> {
    let name = record
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let fragment = record.get("text").and_then(Value::as_str);
    let offset = match record.get("offset") {
        Some(Value::Number(number)) if !number.is_f64() => number.as_i64().unwrap_or(-1),
        _ => -1,
    };
    let Some(fragment) = fragment else {
        return Err(refuse("restore record has an invalid source table"));
    };
    if name.is_empty() || offset < 0 {
        return Err(refuse("restore record has an invalid source table"));
    }
    let after = record
        .get("after")
        .and_then(Value::as_str)
        .unwrap_or_default();
    if after.len() != 64
        || !after
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(refuse("restore record has an invalid source fingerprint"));
    }
    let saved = parsed_toml(fragment)?;
    let mapping = saved.get("mcp_servers");
    let single = saved.keys() == vec!["mcp_servers".to_string()];
    let names = mapping.map(Cfg::keys).unwrap_or_default();
    if !single || mapping.is_none_or(|value| !value.is_table()) || names != vec![name.to_string()] {
        return Err(refuse("restore record contains more than its source table"));
    }
    let raw = mapping
        .and_then(|value| value.get(name))
        .cloned()
        .unwrap_or(Cfg::Null);
    if !raw.is_table()
        || record.get("definition").and_then(Value::as_str) != Some(fingerprint(&raw).as_str())
    {
        return Err(refuse("restore definition does not match its fingerprint"));
    }
    Ok(TomlRecord {
        name: name.to_string(),
        fragment: fragment.to_string(),
        offset: offset as usize,
        raw,
    })
}

pub fn attach_toml(text: &str, record: &Value) -> Result<Option<String>, RecordError> {
    let parsed = validate_toml_record(record)?;
    let mut document = parsed_toml(text)?;
    if !document.contains("mcp_servers") {
        document.insert("mcp_servers", Cfg::table());
    }
    let servers = document.get("mcp_servers").cloned().unwrap_or(Cfg::Null);
    if !servers.is_table() {
        return Err(refuse("the source container changed; refresh and retry"));
    }
    if let Some(existing) = servers.get(&parsed.name) {
        if fingerprint(existing) == fingerprint(&parsed.raw) {
            return Ok(None);
        }
        return Err(refuse(
            "the source already has a different definition; nothing was changed",
        ));
    }
    let updated = if text_digest(text)
        == record
            .get("after")
            .and_then(Value::as_str)
            .unwrap_or_default()
    {
        let boundary = text
            .char_indices()
            .nth(parsed.offset)
            .map(|(index, _)| index)
            .unwrap_or(text.len());
        if parsed.offset > text.chars().count() {
            return Err(refuse("restore record has an invalid source position"));
        }
        format!(
            "{}{}{}",
            &text[..boundary],
            parsed.fragment,
            &text[boundary..]
        )
    } else {
        append_server_block(text, &parsed.fragment)
    };
    if let Some(slot) = document.as_table_mut().and_then(|entries| {
        entries
            .iter_mut()
            .find(|(key, _)| key == "mcp_servers")
            .map(|(_, value)| value)
    }) {
        slot.insert(&parsed.name, parsed.raw.clone());
    }
    if fingerprint(&parsed_toml(&updated)?) != fingerprint(&document) {
        return Err(refuse("restoring the table would change other settings"));
    }
    Ok(Some(updated))
}
