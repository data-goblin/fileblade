use super::*;
use crate::extension_template::HOST_ID;
use regex::Regex;
use std::sync::OnceLock;

const STATUS_DEADLINE: Duration = Duration::from_secs(2);
const STATUS_STDOUT_LIMIT: usize = 128 * 1024;
const STATUS_STDERR_LIMIT: usize = 4096;
const ROW_LIMIT: usize = 512;
const MAX_ID: usize = 128;
const MAX_NAME: usize = 120;

#[derive(Clone, Debug, Args)]
pub struct HostStatusArgs {
    /// Companion plugin id waiting on FileBlade; repeat for more than one.
    #[arg(long = "companion", required = true)]
    pub companions: Vec<String>,
}

fn identifier() -> &'static Regex {
    static PATTERN: OnceLock<Regex> = OnceLock::new();
    PATTERN.get_or_init(|| Regex::new(r"^[A-Za-z0-9][A-Za-z0-9._-]*$").unwrap())
}

fn valid_id(value: &str) -> bool {
    value.len() <= MAX_ID && !value.contains("..") && identifier().is_match(value)
}

fn unknown() -> Value {
    json!({"schemaVersion": 1, "state": "unknown", "plugins": []})
}

fn command_json(program: &str, arguments: &[&str]) -> Option<Value> {
    let program = which(program)?;
    let output = CommandSpec::new(program)
        .args(arguments)
        .env("LC_ALL", "C")
        .timeout(STATUS_DEADLINE)
        .limits(STATUS_STDOUT_LIMIT, STATUS_STDERR_LIMIT)
        .run()
        .ok()?;
    if !output.status.success() || output.stdout_truncated {
        return None;
    }
    serde_json::from_slice(&output.stdout).ok()
}

fn snapshot(companions: &[String]) -> Value {
    let Some(Value::Array(rows)) = command_json("omarchy", &["plugin", "list", "--json"]) else {
        return unknown();
    };
    if rows.len() > ROW_LIMIT {
        return unknown();
    }
    let mut seen: HashSet<&str> = HashSet::new();
    let mut plugins = Vec::new();
    let mut host = None;
    for row in &rows {
        let Some(row) = row.as_object() else {
            return unknown();
        };
        let id = row.get("id").and_then(Value::as_str).unwrap_or_default();
        let enabled = row.get("enabled").and_then(Value::as_bool);
        if !valid_id(id) || !seen.insert(id) || enabled.is_none() {
            return unknown();
        }
        let enabled = enabled.unwrap_or(false);
        if id == HOST_ID {
            host = Some(enabled);
        }
        if companions.iter().any(|companion| companion == id) {
            let name = row
                .get("name")
                .and_then(Value::as_str)
                .map_or_else(|| id.to_string(), bounded_name);
            plugins.push(json!({"id": id, "name": name, "enabled": enabled}));
        }
    }
    let mut result = json!({"schemaVersion": 1, "state": "missing", "plugins": plugins});
    let Some(enabled) = host else {
        return result;
    };
    if !enabled {
        result["state"] = json!("disabled");
        return result;
    }
    result["state"] = json!("starting");
    if let Some(status) = command_json("omarchy-shell", &[HOST_ID, "status"])
        && status.get("open").and_then(Value::as_bool).is_some()
        && status.get("rootPath").and_then(Value::as_str).is_some()
    {
        result["state"] = json!("ready");
    }
    result
}

fn bounded_name(name: &str) -> String {
    name.chars().take(MAX_NAME).collect()
}

pub(super) fn host_status(options: HostStatusArgs) -> AppResult<PublicResult> {
    for companion in &options.companions {
        if !valid_id(companion) {
            return Err(AppError::invalid(format!("{companion} is not a plugin id")));
        }
    }
    let document = snapshot(&options.companions);
    let mut lines = vec![value_text(&document, "state")];
    for plugin in document["plugins"].as_array().into_iter().flatten() {
        lines.push(format!(
            "  {} {}",
            value_text(plugin, "id"),
            if plugin["enabled"] == true {
                "enabled"
            } else {
                "disabled"
            }
        ));
    }
    Ok(PublicResult::lines(lines, document))
}
