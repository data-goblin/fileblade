pub mod apply;
pub mod bounds;
pub mod discovery;
pub mod registry;
mod usage;

use crate::common::{expanded_os_path, parse_path};
use crate::core_modules::Context;
use crate::core_modules::emit::encoded_items;
use crate::core_modules::watch::{SCOPES, WatchPlan};
use crate::module_helpers::Request;
use crate::{AppError, AppResult};
use discovery::Environment;
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

const MAX_ITEM_STUBS: usize = 1024;

#[derive(Default)]
struct Options {
    project: String,
    exact: bool,
    home: String,
    prefix: String,
    platform: String,
    scope: Option<String>,
    items: Option<String>,
    day: Option<String>,
    id: Option<String>,
    agent: Vec<String>,
    state: Option<String>,
}

fn parsed(arguments: &[String]) -> AppResult<Options> {
    let invalid = || AppError::invalid("unrecognised skills helper arguments");
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
            "--prefix" => options.prefix = value()?,
            "--platform" => options.platform = value()?,
            "--scope" => options.scope = Some(value()?),
            "--items" => options.items = Some(value()?),
            "--day" => options.day = Some(value()?),
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
    if let Some(state) = &options.state
        && state != "on"
        && state != "off"
    {
        return Err(invalid());
    }
    Ok(options)
}

fn environment(options: &Options) -> AppResult<Environment> {
    let home = if options.home.is_empty() {
        expanded_os_path(Path::new("~"))
    } else {
        expanded_os_path(&parse_path(&options.home)?)
    };
    let anchor = if options.project.is_empty() {
        PathBuf::new()
    } else {
        parse_path(&options.project)?
    };
    let prefix = if options.prefix.is_empty() {
        PathBuf::new()
    } else {
        expanded_os_path(&parse_path(&options.prefix)?)
    };
    let platform = if options.platform.is_empty() {
        "linux".to_string()
    } else {
        options.platform.clone()
    };
    Ok(Environment {
        home,
        anchor,
        exact: options.exact,
        platform,
        prefix,
        scope: options.scope.clone().unwrap_or_else(|| "all".to_string()),
    })
}

fn item_stubs(raw: &str) -> Vec<Value> {
    serde_json::from_str::<Value>(raw)
        .ok()
        .and_then(|value| value.as_array().cloned())
        .map(|entries| {
            entries
                .into_iter()
                .filter(Value::is_object)
                .take(MAX_ITEM_STUBS)
                .collect()
        })
        .unwrap_or_default()
}

fn discovered_items(options: &Options) -> AppResult<Vec<Value>> {
    let mut plan = WatchPlan::new();
    let document = discovery::collect(&mut plan, &environment(options)?);
    Ok(match document.get("items") {
        Some(Value::Array(items)) => items.clone(),
        _ => Vec::new(),
    })
}

fn bounded(document: Map<String, Value>) -> AppResult<Value> {
    serde_json::from_slice(&encoded_items(&document))
        .map_err(|_| AppError::command("the skills helper produced no complete document"))
}

fn listing(_: &Request<'_>, arguments: &[String], context: &Context<'_>) -> AppResult<Value> {
    context.check()?;
    let options = parsed(arguments)?;
    let mut plan = WatchPlan::new();
    let mut document = discovery::collect(&mut plan, &environment(&options)?);
    plan.finish(&mut document);
    usage::attach(context, &mut document);
    bounded(document)
}

fn usage_history(_: &Request<'_>, arguments: &[String], context: &Context<'_>) -> AppResult<Value> {
    let options = parsed(arguments)?;
    match &options.items {
        Some(raw) => usage::call(
            context,
            &json!({
                "method": "usage",
                "items": item_stubs(raw),
                "scoped": true,
            }),
        ),
        None => usage::call(
            context,
            &json!({
                "method": "usage",
                "items": discovered_items(&options)?,
            }),
        ),
    }
}

fn usage_counts(_: &Request<'_>, arguments: &[String], context: &Context<'_>) -> AppResult<Value> {
    let options = parsed(arguments)?;
    let items = item_stubs(options.items.as_deref().unwrap_or("[]"));
    usage::call(context, &json!({"method": "counts", "items": items}))
}

fn usage_day(_: &Request<'_>, arguments: &[String], context: &Context<'_>) -> AppResult<Value> {
    let options = parsed(arguments)?;
    let day = options.day.clone().ok_or_else(|| {
        AppError::invalid("the skills usage-day method requires a --day argument")
    })?;
    let items = match &options.items {
        Some(raw) => item_stubs(raw),
        None => discovered_items(&options)?,
    };
    usage::call(
        context,
        &json!({"method": "day", "items": items, "day": day}),
    )
}

fn applying(_: &Request<'_>, arguments: &[String], context: &Context<'_>) -> AppResult<Value> {
    context.check()?;
    let options = parsed(arguments)?;
    let id = options
        .id
        .clone()
        .ok_or_else(|| AppError::invalid("the skills apply method requires an --id argument"))?;
    let state = options
        .state
        .clone()
        .ok_or_else(|| AppError::invalid("the skills apply method requires a --state argument"))?;
    if options.agent.is_empty() {
        return Err(AppError::invalid(
            "the skills apply method requires at least one --agent argument",
        ));
    }
    let document = apply::apply(&environment(&options)?, &id, &options.agent, &state);
    serde_json::from_slice(&crate::core_modules::emit::encoded_results(&document))
        .map_err(|_| AppError::command("the skills helper produced no complete document"))
}

pub fn handler(method: &str) -> Option<crate::core_modules::CoreHandler> {
    match method {
        "list" => Some(listing),
        "usage" => Some(usage_history),
        "usage-counts" => Some(usage_counts),
        "usage-day" => Some(usage_day),
        "apply" => Some(applying),
        _ => None,
    }
}
