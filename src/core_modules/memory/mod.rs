pub mod adapters;
pub mod apply;
pub mod common;
pub mod discovery;

use crate::core_modules::Context as CoreContext;
use crate::core_modules::emit::{encoded_items, encoded_results};
use crate::core_modules::watch::{SCOPES, WatchPlan};
use crate::module_helpers::Request;
use crate::{AppError, AppResult};
use common::Environ;
use serde_json::{Map, Value, json};

#[derive(Default)]
struct Options {
    project: String,
    exact: bool,
    home: String,
    config: String,
    scope: Option<String>,
    id: Option<String>,
    agent: Vec<String>,
    state: Option<String>,
}

fn parsed(arguments: &[String]) -> AppResult<Options> {
    let invalid = || AppError::invalid("unrecognised memory helper arguments");
    let mut options = Options::default();
    let mut index = 0;
    while index < arguments.len() {
        let (flag, inline) = match arguments[index].split_once('=') {
            Some((flag, value)) if flag.starts_with("--") => (flag, Some(value.to_string())),
            _ => (arguments[index].as_str(), None),
        };
        let mut value = || match inline.clone() {
            Some(value) => Ok(value),
            None => {
                index += 1;
                arguments.get(index).cloned().ok_or_else(invalid)
            }
        };
        match flag {
            "--json" => {}
            "--exact" => options.exact = true,
            "--project" => options.project = value()?,
            "--home" => options.home = value()?,
            "--config" => options.config = value()?,
            "--prefix" => {
                value()?;
            }
            "--scope" => options.scope = Some(value()?),
            "--id" => options.id = Some(value()?),
            "--agent" => options.agent.push(value()?),
            "--state" => options.state = Some(value()?),
            _ => return Err(invalid()),
        }
        index += 1;
    }
    if let Some(scope) = &options.scope
        && !SCOPES.contains(&scope.as_str())
    {
        return Err(invalid());
    }
    Ok(options)
}

fn environ() -> Environ {
    std::env::vars_os().collect()
}

fn fallback() -> Value {
    json!({"ok": false, "schemaVersion": 1, "truncated": true, "items": []})
}

fn bounded(document: &Map<String, Value>) -> Value {
    serde_json::from_slice(&encoded_items(document)).unwrap_or_else(|_| fallback())
}

fn listing(_: &Request<'_>, arguments: &[String], context: &CoreContext<'_>) -> AppResult<Value> {
    if context.check().is_err() {
        return Ok(fallback());
    }
    let Ok(options) = parsed(arguments) else {
        return Ok(fallback());
    };
    let mut plan = WatchPlan::new();
    let mut document = discovery::collect(
        &mut plan,
        &options.project,
        &options.home,
        &options.config,
        environ(),
        options.exact,
        options.scope.as_deref().unwrap_or("all"),
    );
    plan.finish(&mut document);
    Ok(bounded(&document))
}

fn applying(_: &Request<'_>, arguments: &[String], context: &CoreContext<'_>) -> AppResult<Value> {
    context.check()?;
    let options = parsed(arguments)?;
    let id = options
        .id
        .clone()
        .ok_or_else(|| AppError::invalid("the memory apply method requires an --id argument"))?;
    let state = options
        .state
        .clone()
        .ok_or_else(|| AppError::invalid("the memory apply method requires a --state argument"))?;
    let memory_context = discovery::build_context(
        &options.project,
        &options.home,
        environ(),
        options.exact,
        "all",
    );
    let document = apply::apply_in(
        &memory_context,
        &options.config,
        &id,
        &options.agent,
        &state,
    );
    serde_json::from_slice(&encoded_results(&document))
        .map_err(|_| AppError::command("the memory helper produced no complete document"))
}

pub fn handler(method: &str) -> Option<crate::core_modules::CoreHandler> {
    match method {
        "list" => Some(listing),
        "apply" => Some(applying),
        _ => None,
    }
}
