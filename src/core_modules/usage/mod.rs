pub mod query;
pub mod records;
pub mod sql;
pub mod store;

use crate::core_modules::Context;
use crate::module_helpers::Request;
use crate::{AppError, AppResult};
use serde_json::{Map, Value, json};
use store::Environment;

pub use query::UNAVAILABLE;

pub fn environment() -> Environment {
    Environment::current()
}

pub fn watch_paths(environment: &Environment) -> Vec<Value> {
    store::watch_paths(environment)
        .iter()
        .map(|path| Value::from(crate::common::path_text(path)))
        .collect()
}

pub fn attach_skills(environment: &Environment, document: &mut Map<String, Value>) {
    let mut items = match document.get("items") {
        Some(Value::Array(items)) => items.clone(),
        _ => Vec::new(),
    };
    let extra = query::attach_skills(environment, &mut items);
    document.insert("items".to_string(), Value::Array(items));
    for (key, value) in extra {
        document.insert(key, value);
    }
    document.insert(
        "usageWatchPaths".to_string(),
        Value::Array(watch_paths(environment)),
    );
}

pub fn attach_mcp(environment: &Environment, definitions: &mut [Value]) -> Map<String, Value> {
    query::attach_mcp(environment, definitions)
}

fn before_argument(arguments: &[String]) -> AppResult<Option<String>> {
    let invalid = || AppError::invalid("unrecognised mcp usage arguments");
    let mut before = None;
    let mut index = 0;
    while index < arguments.len() {
        let (flag, inline) = match arguments[index].split_once('=') {
            Some((flag, value)) if flag.starts_with("--") => (flag, Some(value.to_string())),
            _ => (arguments[index].as_str(), None),
        };
        match flag {
            "--json" => {}
            "--before" => {
                before = Some(match inline {
                    Some(value) => value,
                    None => {
                        index += 1;
                        arguments.get(index).cloned().ok_or_else(invalid)?
                    }
                });
            }
            _ => return Err(invalid()),
        }
        index += 1;
    }
    if let Some(day) = &before
        && chrono::NaiveDate::parse_from_str(day, "%Y-%m-%d")
            .ok()
            .filter(|parsed| parsed.format("%Y-%m-%d").to_string() == *day)
            .is_none()
    {
        return Err(invalid());
    }
    Ok(before)
}

fn mcp_history(_: &Request<'_>, arguments: &[String], context: &Context<'_>) -> AppResult<Value> {
    context.check()?;
    if arguments.iter().any(|argument| argument != "--json") {
        return Err(AppError::invalid("unrecognised mcp usage arguments"));
    }
    Ok(query::mcp_usage(&environment()))
}

fn mcp_forget(_: &Request<'_>, arguments: &[String], context: &Context<'_>) -> AppResult<Value> {
    context.check()?;
    let before = before_argument(arguments)?;
    Ok(query::forget(&environment(), before.as_deref()))
}

pub fn handler(method: &str) -> Option<crate::core_modules::CoreHandler> {
    match method {
        "usage" => Some(mcp_history),
        "usage-forget" => Some(mcp_forget),
        _ => None,
    }
}

pub fn unavailable_document(kind: &str) -> Value {
    json!({
        "ok": false,
        "schemaVersion": query::SCHEMA_VERSION,
        "kind": kind,
        "error": UNAVAILABLE,
    })
}
