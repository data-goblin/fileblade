use crate::AppResult;
use crate::secure;
use chrono::{SecondsFormat, Utc};
use serde_json::{Value, json};
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::time::Instant;

pub const AUDIT_FILE_CAP: u64 = 32 * 1024 * 1024;
pub const AUDIT_READ_CAP: usize = 8 * 1024 * 1024;
const AUDIT_EXCERPT_BYTES: usize = 1024;
const AUDITED: &[&str] = &[
    "copy",
    "move",
    "rename",
    "create",
    "trash",
    "trash-restore",
    "trash-delete",
    "trash-empty",
    "trash-prune",
    "undo",
    "redo",
    "bin-put",
    "bin-remove",
    "bin-restore",
    "bin-purge",
    "archive-extract",
    "plugin-add",
    "set-default",
    "drop-run",
    "action-run",
    "helper-write",
    "companion-mutate",
    "recover",
];

pub struct Event<'a> {
    pub via: &'a str,
    pub actor: &'a str,
    pub command: &'a str,
    pub arguments: &'a [String],
    pub outcome: &'a AppResult<Value>,
    pub started: Instant,
}

pub fn is_audited(command: &str) -> bool {
    AUDITED.contains(&command)
}

pub fn path() -> PathBuf {
    crate::paths::state_dir().join("audit.jsonl")
}

pub fn changed_nothing(command: &str, value: &Value) -> bool {
    if value.get("ok").and_then(Value::as_bool) != Some(true) {
        return false;
    }
    let empty = |key: &str| value[key].as_array().is_none_or(Vec::is_empty);
    let zero = |key: &str| value[key].as_u64() == Some(0);
    match command {
        "trash-prune" => zero("eligible") && zero("completed") && zero("failed"),
        "recover" => empty("restored") && empty("removed") && empty("conflicts") && empty("errors"),
        _ => false,
    }
}

pub fn record(event: &Event<'_>) -> io::Result<()> {
    if !is_audited(event.command) {
        return Ok(());
    }
    if let Ok(value) = event.outcome
        && changed_nothing(event.command, value)
    {
        return Ok(());
    }
    let (ok, error, excerpt) = match event.outcome {
        Ok(value) => (
            value.get("ok").and_then(Value::as_bool).unwrap_or(true),
            audit_error(
                event.command,
                value.get("error").and_then(Value::as_str).unwrap_or(""),
            ),
            audit_excerpt(event.command, value),
        ),
        Err(error) => (
            false,
            audit_error(event.command, &error.to_string()),
            Value::Null,
        ),
    };
    let arguments = audit_arguments(event.command, event.arguments);
    let entry = json!({
        "ts": Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true),
        "via": event.via,
        "actor": event.actor,
        "command": event.command,
        "arguments": arguments,
        "ok": ok,
        "error": error,
        "elapsed_ms": event.started.elapsed().as_millis() as u64,
        "result": excerpt,
        "pid": std::process::id(),
        "display": std::env::var("WAYLAND_DISPLAY").unwrap_or_default(),
        "home": std::env::var("HOME").unwrap_or_default(),
    });
    crate::lease::durable::append(&path(), &entry.to_string(), AUDIT_FILE_CAP)
}

fn audit_arguments(command: &str, arguments: &[String]) -> Vec<String> {
    if command == "helper-write" {
        let mut result = Vec::new();
        for (index, argument) in arguments.iter().enumerate() {
            for flag in ["--provider", "--helper", "--method"] {
                if argument == flag {
                    result.push(flag.to_string());
                    result.push(arguments.get(index + 1).cloned().unwrap_or_default());
                } else if argument.starts_with(&format!("{flag}=")) {
                    result.push(argument.clone());
                }
            }
        }
        return result;
    }
    if command != "bin-put" && command != "bin-remove" {
        return arguments.to_vec();
    }
    let mut redacted = Vec::with_capacity(arguments.len());
    let mut hide_next = false;
    for argument in arguments {
        if hide_next {
            redacted.push("<redacted>".to_string());
            hide_next = false;
        } else if matches!(
            argument.as_str(),
            "--item" | "--arguments" | "--helper-route"
        ) {
            redacted.push(argument.clone());
            hide_next = true;
        } else if let Some((flag, _)) = argument.split_once('=')
            && matches!(flag, "--item" | "--arguments" | "--helper-route")
        {
            redacted.push(format!("{flag}=<redacted>"));
        } else {
            redacted.push(argument.clone());
        }
    }
    redacted
}

fn audit_error(command: &str, error: &str) -> String {
    if matches!(command, "helper-write" | "companion-mutate") && !error.is_empty() {
        "helper operation failed".to_string()
    } else if command.starts_with("bin-") && !error.is_empty() {
        "bin operation failed".to_string()
    } else {
        error.to_string()
    }
}

fn audit_excerpt(command: &str, value: &Value) -> Value {
    if matches!(command, "helper-write" | "companion-mutate") {
        return json!({"ok": value.get("ok").and_then(Value::as_bool).unwrap_or(false)});
    }
    if command == "action-run" {
        return json!({
            "ok": value.get("ok").and_then(Value::as_bool).unwrap_or(false),
            "exit_code": value.get("exit_code").and_then(Value::as_i64),
            "timed_out": value.get("timed_out").and_then(Value::as_bool).unwrap_or(false),
            "detached": value.get("detached").and_then(Value::as_bool).unwrap_or(false),
            "elapsed_ms": value.get("elapsed_ms").and_then(Value::as_u64).unwrap_or(0),
            "stdout_truncated": value.get("stdout_truncated").and_then(Value::as_bool).unwrap_or(false),
            "stderr_truncated": value.get("stderr_truncated").and_then(Value::as_bool).unwrap_or(false),
            "targets": value.get("targets").and_then(Value::as_u64).unwrap_or(0),
            "error": crate::actions::plain(
                value.get("error").and_then(Value::as_str).unwrap_or(""),
                512,
            ),
        });
    }
    if !command.starts_with("bin-") {
        return excerpt(value);
    }
    let results = value["results"]
        .as_array()
        .map(Vec::as_slice)
        .unwrap_or(&[]);
    json!({
        "ok": value.get("ok").and_then(Value::as_bool).unwrap_or(false),
        "schemaVersion": value.get("schemaVersion").and_then(Value::as_u64),
        "entry": value.get("entry").and_then(Value::as_str).unwrap_or(""),
        "pending_commit": value.get("pending_commit").and_then(Value::as_bool).unwrap_or(false),
        "results": {
            "count": results.len(),
            "changed": results.iter().filter(|item| item["changed"].as_bool().unwrap_or(false)).count(),
            "failed": results.iter().filter(|item| !item["ok"].as_bool().unwrap_or(false)).count(),
        },
        "payload": "<redacted>",
    })
}

pub fn read(limit: usize, since: &str, command: &str) -> Value {
    match tail(&path()) {
        Ok((lines, truncated)) => {
            let malformed = lines
                .iter()
                .filter(|line| serde_json::from_str::<Value>(line).is_err())
                .count();
            let entries = lines
                .iter()
                .filter_map(|line| serde_json::from_str::<Value>(line).ok())
                .filter(|entry| since.is_empty() || entry["ts"].as_str().unwrap_or("") >= since)
                .filter(|entry| command.is_empty() || entry["command"] == command)
                .collect::<Vec<_>>();
            let start = entries.len().saturating_sub(limit);
            json!({
                "ok": true,
                "path": crate::common::path_text(&path()),
                "truncated": truncated,
                "malformed_records": malformed,
                "entries": entries[start..]
            })
        }
        Err(error) => json!({
            "ok": false,
            "path": crate::common::path_text(&path()),
            "truncated": false,
            "error": error.to_string(),
            "entries": []
        }),
    }
}

fn tail(path: &Path) -> io::Result<(Vec<String>, bool)> {
    if !secure::entry_exists(path)? {
        return Ok((Vec::new(), false));
    }
    let _lock = secure::open_record_lock(&path.with_extension("lock"))?;
    let Some(mut file) = secure::open_private_read(path)? else {
        return Ok((Vec::new(), false));
    };
    let size = file.metadata()?.len();
    let truncated = size > AUDIT_READ_CAP as u64;
    if truncated {
        file.seek(SeekFrom::Start(size - AUDIT_READ_CAP as u64))?;
    }
    let mut bytes = Vec::new();
    file.take(AUDIT_READ_CAP as u64).read_to_end(&mut bytes)?;
    let text = String::from_utf8_lossy(&bytes);
    let mut lines = text.lines().map(ToOwned::to_owned).collect::<Vec<_>>();
    if truncated && !lines.is_empty() {
        lines.remove(0);
    }
    Ok((lines, truncated))
}

fn excerpt(value: &Value) -> Value {
    let text = value.to_string();
    if text.len() <= AUDIT_EXCERPT_BYTES {
        return value.clone();
    }
    let cut = text.floor_char_boundary(AUDIT_EXCERPT_BYTES);
    json!({"excerpt": &text[..cut], "bytes": text.len()})
}
