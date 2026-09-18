use super::safeio::digest;
use serde_json::{Map, Value, json};
use unicode_general_category::{GeneralCategory, get_general_category};

pub const LABEL_FIELDS: [&str; 3] = ["statusMessage", "name", "description"];
pub const MAX_LABEL_CHARS: usize = 80;
pub const MAX_BASENAME_CHARS: usize = 64;
pub const PAYLOAD_FIELDS: [&str; 5] = ["command", "bash", "powershell", "url", "prompt"];
pub const BANNED_SUMMARY_FIELDS: [&str; 8] = [
    "command",
    "bash",
    "powershell",
    "url",
    "headers",
    "env",
    "prompt",
    "cwd",
];

fn printable(character: char) -> bool {
    if character == ' ' {
        return true;
    }
    !matches!(
        get_general_category(character),
        GeneralCategory::Control
            | GeneralCategory::Format
            | GeneralCategory::Surrogate
            | GeneralCategory::PrivateUse
            | GeneralCategory::Unassigned
            | GeneralCategory::LineSeparator
            | GeneralCategory::ParagraphSeparator
            | GeneralCategory::SpaceSeparator
    )
}

fn text_field<'a>(entry: &'a Map<String, Value>, key: &str) -> Option<&'a str> {
    entry.get(key).and_then(Value::as_str)
}

pub fn field_inventory(entry: &Map<String, Value>) -> Vec<String> {
    let mut keys: Vec<String> = entry.keys().cloned().collect();
    keys.sort();
    keys.truncate(24);
    keys
}

pub fn safe_type(entry: &Map<String, Value>) -> String {
    let Some(value) = text_field(entry, "type") else {
        let has_command = ["command", "bash", "powershell"]
            .iter()
            .any(|key| entry.contains_key(*key));
        return if has_command { "command" } else { "unknown" }.to_string();
    };
    let cleaned = value.trim().to_lowercase();
    if matches!(
        cleaned.as_str(),
        "command" | "http" | "prompt" | "script" | "mcp_tool" | "agent"
    ) {
        cleaned
    } else {
        "unknown".to_string()
    }
}

pub fn safe_timeout(entry: &Map<String, Value>) -> Option<i64> {
    for field in ["timeout", "timeoutSec"] {
        let Some(Value::Number(number)) = entry.get(field) else {
            continue;
        };
        let value = number.as_f64().unwrap_or_default();
        if value > 0.0 && value < 86400.0 {
            return Some(value.trunc() as i64);
        }
    }
    None
}

pub fn payload_material(entry: &Map<String, Value>) -> String {
    let mut parts: Vec<String> = Vec::new();
    for field in PAYLOAD_FIELDS {
        if let Some(value) = text_field(entry, field) {
            parts.push(format!("{field}:{value}"));
        }
    }
    parts.join(" ")
}

pub fn primary_payload(entry: &Map<String, Value>) -> String {
    for field in PAYLOAD_FIELDS {
        if let Some(value) = text_field(entry, field)
            && !value.is_empty()
        {
            return value.to_string();
        }
    }
    String::new()
}

pub fn payload_digest(entry: &Map<String, Value>) -> String {
    let primary = primary_payload(entry);
    if primary.is_empty() {
        String::new()
    } else {
        digest(&primary)
    }
}

pub fn env_count(entry: &Map<String, Value>) -> usize {
    let mut total = 0;
    if let Some(Value::Object(values)) = entry.get("env") {
        total += values.len();
    }
    if let Some(Value::Array(values)) = entry.get("allowedEnvVars") {
        total += values.len();
    }
    total
}

pub fn matcher_shape(entry: &Map<String, Value>) -> String {
    match entry.get("matcher") {
        None | Some(Value::Null) => "none".to_string(),
        Some(Value::String(value)) => {
            if value.trim().is_empty() {
                "empty".to_string()
            } else if value
                .chars()
                .any(|character| ".*+?[]()|\\^$".contains(character))
            {
                "pattern".to_string()
            } else {
                "literal".to_string()
            }
        }
        Some(_) => "invalid".to_string(),
    }
}

pub fn enabled_state(entry: &Map<String, Value>, fallback: Option<bool>) -> Option<bool> {
    match entry.get("enabled") {
        Some(Value::Bool(flag)) => Some(*flag),
        Some(_) => None,
        None => fallback,
    }
}

pub fn clean_label(value: Option<&Value>) -> String {
    let Some(Value::String(text)) = value else {
        return String::new();
    };
    let collapsed = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let filtered: String = collapsed
        .chars()
        .filter(|character| printable(*character))
        .collect();
    filtered
        .chars()
        .take(MAX_LABEL_CHARS)
        .collect::<String>()
        .trim()
        .to_string()
}

pub fn command_basename(entry: &Map<String, Value>) -> String {
    for field in ["command", "bash", "powershell"] {
        let Some(value) = text_field(entry, field) else {
            continue;
        };
        if value.trim().is_empty() {
            continue;
        }
        let Some(first) = value.split_whitespace().next() else {
            return String::new();
        };
        let token = first.trim_matches(|character| character == '"' || character == '\'');
        if !token.contains('/') {
            return String::new();
        }
        let base = token.rsplit('/').next().unwrap_or_default();
        if base.is_empty()
            || base == "."
            || base == ".."
            || base.chars().count() > MAX_BASENAME_CHARS
        {
            return String::new();
        }
        if base
            .chars()
            .all(|character| printable(character) && !character.is_whitespace())
        {
            return base.to_string();
        }
        return String::new();
    }
    String::new()
}

pub fn safe_label(entry: &Map<String, Value>) -> (String, String) {
    for field in LABEL_FIELDS {
        let text = clean_label(entry.get(field));
        if !text.is_empty() {
            return (text, field.to_string());
        }
    }
    let base = command_basename(entry);
    if base.is_empty() {
        (String::new(), String::new())
    } else {
        (base, "command".to_string())
    }
}

pub fn safe_summary(entry: &Map<String, Value>) -> Map<String, Value> {
    let material = payload_material(entry);
    let (label, label_source) = safe_label(entry);
    let mut summary = Map::new();
    summary.insert("label".to_string(), json!(label));
    summary.insert("labelSource".to_string(), json!(label_source));
    summary.insert("type".to_string(), json!(safe_type(entry)));
    summary.insert("fields".to_string(), json!(field_inventory(entry)));
    summary.insert("digest".to_string(), json!(payload_digest(entry)));
    summary.insert("payloadBytes".to_string(), json!(material.len()));
    summary.insert("envCount".to_string(), json!(env_count(entry)));
    summary.insert("matcher".to_string(), json!(matcher_shape(entry)));
    summary.insert(
        "timeoutSeconds".to_string(),
        match safe_timeout(entry) {
            Some(seconds) => json!(seconds),
            None => Value::Null,
        },
    );
    summary.insert("hasCondition".to_string(), json!(entry.contains_key("if")));
    summary
}

pub fn assert_safe(row: &mut Map<String, Value>) {
    if let Some(Value::Object(summary)) = row.get_mut("summary") {
        for banned in BANNED_SUMMARY_FIELDS {
            summary.remove(banned);
        }
    }
}
