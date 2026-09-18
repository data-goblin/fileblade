use super::value::{Cfg, fingerprint};
use crate::core_modules::canonical::sha256_hex;
use regex::Regex;
use serde_json::{Map, Value, json};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use unicode_general_category::{GeneralCategory, get_general_category};

pub const SCHEMA_VERSION: i64 = 1;
pub const MAX_FIELD_CHARS: usize = 256;
pub const MAX_LABEL_CHARS: usize = 128;
pub const MAX_PATH_PART_CHARS: usize = 128;

pub const CORE_AGENT_IDS: [(&str, &str); 6] = [
    ("claude", "claude-code"),
    ("codex", "codex"),
    ("opencode", "opencode"),
    ("pi", "pi"),
    ("github-copilot-cli", "copilot-cli"),
    ("google-antigravity", "antigravity"),
];

pub fn core_agent_id(agent: &str) -> String {
    CORE_AGENT_IDS
        .iter()
        .find(|(key, _)| *key == agent)
        .map(|(_, value)| (*value).to_string())
        .unwrap_or_else(|| agent.to_string())
}

pub fn digest_bytes(parts: &[&[u8]]) -> String {
    let mut joined: Vec<u8> = Vec::new();
    for (index, part) in parts.iter().enumerate() {
        if index > 0 {
            joined.push(0);
        }
        joined.extend_from_slice(part);
    }
    sha256_hex(&joined)[..24].to_string()
}

pub fn digest(parts: &[&str]) -> String {
    let bytes: Vec<&[u8]> = parts.iter().map(|part| part.as_bytes()).collect();
    digest_bytes(&bytes)
}

fn secret_shapes() -> &'static [Regex; 3] {
    static PATTERNS: OnceLock<[Regex; 3]> = OnceLock::new();
    PATTERNS.get_or_init(|| {
        [
            Regex::new(r"(?i)\bbearer\s+\S+").expect("bearer pattern"),
            Regex::new(
                r"(?i)(?:bearer|token|secret|password|passwd|api[_-]?key|authorization|credential)",
            )
            .expect("secret word pattern"),
            Regex::new(r"\b(?:sk|ghp|github_pat)-?[A-Za-z0-9_]{12,}\b")
                .expect("secret token pattern"),
        ]
    })
}

fn unsafe_category(character: char) -> bool {
    matches!(
        get_general_category(character),
        GeneralCategory::Control
            | GeneralCategory::Format
            | GeneralCategory::Surrogate
            | GeneralCategory::PrivateUse
            | GeneralCategory::Unassigned
    )
}

pub fn safe_unicode(value: &str, maximum: usize) -> bool {
    let length = value.chars().count();
    length > 0
        && length <= maximum
        && !value.chars().any(unsafe_category)
        && !secret_shapes()
            .iter()
            .any(|pattern| pattern.is_match(value))
}

pub fn safe_label(value: &str, identifier: &str) -> String {
    if safe_unicode(value, MAX_LABEL_CHARS) {
        value.to_string()
    } else {
        let short: String = identifier.chars().take(8).collect();
        format!("server-{short}")
    }
}

pub fn safe_path(value: &[u8], identifier: &str) -> (String, bool) {
    if matches!(value, b"<project>" | b"<codex-home>" | b"~") {
        return (String::from_utf8_lossy(value).into_owned(), false);
    }
    let prefixes: [&[u8]; 4] = [b"<project>/", b"<codex-home>/", b"~/", b"/"];
    let mut prefix: &[u8] = b"";
    let mut remainder: &[u8] = value;
    for candidate in prefixes {
        if value.starts_with(candidate) {
            prefix = candidate;
            remainder = &value[candidate.len()..];
            break;
        }
    }
    let mut parts: Vec<String> = Vec::new();
    let mut redacted = false;
    for (index, part) in remainder.split(|byte| *byte == b'/').enumerate() {
        let part_id = digest_bytes(&[
            b"path-part-v1",
            identifier.as_bytes(),
            index.to_string().as_bytes(),
            part,
        ]);
        let text = std::str::from_utf8(part).ok();
        match text {
            Some(text)
                if text != "." && text != ".." && safe_unicode(text, MAX_PATH_PART_CHARS) =>
            {
                parts.push(text.to_string());
            }
            _ => {
                parts.push(format!("segment-{}", &part_id[..8]));
                redacted = true;
            }
        }
    }
    let prefix = String::from_utf8_lossy(prefix).into_owned();
    let result = format!("{prefix}{}", parts.join("/"));
    if result.chars().count() > MAX_FIELD_CHARS {
        let hashed = digest_bytes(&[b"path-value-v1", identifier.as_bytes(), value]);
        return (format!("{prefix}path-{}", &hashed[..8]), true);
    }
    (result, redacted)
}

#[derive(Clone)]
pub struct Definition {
    pub agent: String,
    pub raw_name: String,
    pub scope: String,
    pub source_kind: String,
    pub source_path: Vec<u8>,
    pub support: String,
    pub transport: String,
    pub enabled: Option<bool>,
    pub trusted: Option<bool>,
    pub valid: bool,
    pub priority: i64,
    pub precedence_known: bool,
    pub trust_gates_effective: bool,
    pub trust_role: String,
    pub endpoint_signature: Option<String>,
    pub secret_presence: (bool, bool, bool),
    pub selected: Option<bool>,
    pub shadowed_by: Option<String>,
    pub duplicate_group: Option<String>,
    pub metrics: Map<String, Value>,
    pub raw_config: Option<Cfg>,
    pub absolute_path: Option<PathBuf>,
    pub applied_agents: Vec<String>,
    pub plugin_name: String,
    pub source_id: String,
    pub id: String,
}

fn absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("/"))
            .join(path)
    }
}

impl Definition {
    pub fn finish(mut self) -> Self {
        self.source_id = digest_bytes(&[
            b"source-v1",
            self.agent.as_bytes(),
            self.source_kind.as_bytes(),
            &self.source_path,
        ]);
        let raw = self.raw_config.clone().unwrap_or(Cfg::Null);
        let path_bytes: Vec<u8> = self
            .absolute_path
            .as_ref()
            .map(|path| path.as_os_str().as_bytes().to_vec())
            .unwrap_or_default();
        let raw_fingerprint = fingerprint(&raw);
        self.id = digest_bytes(&[
            b"definition-v2",
            self.agent.as_bytes(),
            self.source_kind.as_bytes(),
            &self.source_path,
            &path_bytes,
            self.raw_name.as_bytes(),
            raw_fingerprint.as_bytes(),
        ]);
        self
    }

    pub fn effective(&self) -> Option<bool> {
        if !self.valid || self.enabled == Some(false) || self.selected == Some(false) {
            return Some(false);
        }
        if self.trust_gates_effective && self.trusted == Some(false) {
            return Some(false);
        }
        if self.enabled.is_none()
            || self.selected.is_none()
            || (self.trust_gates_effective && self.trusted.is_none())
        {
            return None;
        }
        Some(true)
    }

    pub fn state(&self) -> &'static str {
        if !self.valid {
            return "invalid";
        }
        if self.enabled == Some(false) {
            return "disabled";
        }
        if self.trust_gates_effective && self.trusted == Some(false) {
            return "untrusted";
        }
        if self.selected == Some(false) {
            return "shadowed";
        }
        if self.effective().is_none() {
            return "unknown";
        }
        "enabled"
    }

    pub fn public(&self) -> Value {
        let (source_path, source_redacted) = safe_path(&self.source_path, &self.source_id);
        let mut source = Map::new();
        source.insert("id".to_string(), json!(self.source_id));
        source.insert("kind".to_string(), json!(self.source_kind));
        source.insert("path".to_string(), json!(source_path));
        source.insert("redacted".to_string(), json!(source_redacted));
        if !self.plugin_name.is_empty() {
            source.insert(
                "plugin".to_string(),
                json!(safe_label(&self.plugin_name, &self.source_id)),
            );
        }
        if let Some(path) = &self.absolute_path {
            let logical = absolute(path);
            if let Ok(target) = std::fs::canonicalize(path) {
                let (target_path, target_redacted) =
                    safe_path(target.as_os_str().as_bytes(), &self.source_id);
                if target != logical && !target_redacted {
                    source.insert("realpath".to_string(), json!(target_path));
                }
            }
        }
        let optional = |value: Option<bool>| match value {
            Some(flag) => Value::Bool(flag),
            None => Value::Null,
        };
        let mut row = Map::new();
        row.insert("id".to_string(), json!(self.id));
        row.insert("agent".to_string(), json!(self.agent));
        row.insert("agentId".to_string(), json!(core_agent_id(&self.agent)));
        row.insert(
            "name".to_string(),
            json!(safe_label(&self.raw_name, &self.id)),
        );
        row.insert("scope".to_string(), json!(self.scope));
        row.insert("source".to_string(), Value::Object(source));
        row.insert("support".to_string(), json!(self.support));
        row.insert("transport".to_string(), json!(self.transport));
        row.insert("enabled".to_string(), optional(self.enabled));
        row.insert("trusted".to_string(), optional(self.trusted));
        row.insert("trustRole".to_string(), json!(self.trust_role));
        row.insert("selected".to_string(), optional(self.selected));
        row.insert("shadowed".to_string(), json!(self.selected == Some(false)));
        row.insert(
            "shadowedBy".to_string(),
            match &self.shadowed_by {
                Some(value) => json!(value),
                None => Value::Null,
            },
        );
        row.insert(
            "duplicateGroup".to_string(),
            match &self.duplicate_group {
                Some(value) => json!(value),
                None => Value::Null,
            },
        );
        row.insert("effective".to_string(), optional(self.effective()));
        row.insert("state".to_string(), json!(self.state()));
        row.insert("health".to_string(), json!("not-probed"));
        row.insert(
            "secretPresence".to_string(),
            json!({
                "environment": self.secret_presence.0,
                "headers": self.secret_presence.1,
                "authentication": self.secret_presence.2,
            }),
        );
        row.insert(
            "group".to_string(),
            json!(format!("{}, {}", self.agent, self.scope)),
        );
        row.insert("metrics".to_string(), Value::Object(self.metrics.clone()));
        row.insert("appliedAgents".to_string(), json!(self.applied_agents));
        Value::Object(row)
    }
}

impl Default for Definition {
    fn default() -> Self {
        Self {
            agent: String::new(),
            raw_name: String::new(),
            scope: String::new(),
            source_kind: String::new(),
            source_path: Vec::new(),
            support: String::new(),
            transport: String::new(),
            enabled: None,
            trusted: Some(true),
            valid: false,
            priority: 0,
            precedence_known: true,
            trust_gates_effective: true,
            trust_role: "source-approval".to_string(),
            endpoint_signature: None,
            secret_presence: (false, false, false),
            selected: None,
            shadowed_by: None,
            duplicate_group: None,
            metrics: Map::new(),
            raw_config: None,
            absolute_path: None,
            applied_agents: Vec::new(),
            plugin_name: String::new(),
            source_id: String::new(),
            id: String::new(),
        }
    }
}
