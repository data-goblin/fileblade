pub mod apply;
pub mod inventory;
pub mod model;
pub mod parsers;
pub mod records;
pub mod safeio;
pub mod tomlwrite;
pub mod value;

use crate::core_modules::Context as CoreContext;
use crate::core_modules::usage;
use crate::core_modules::watch::{SCOPES, lane_rows};
use crate::module_helpers::Request;
use crate::{AppError, AppResult};
use apply::{Applier, Removal};
use inventory::{Environ, Inventory, PROJECT_SCOPES, Settings};
use serde_json::{Value, json};
use std::path::{Path, PathBuf};

pub const DEFAULT_ETC_ROOT: &str = "/etc";

const LIST_FAILURE: &str = concat!(
    r#"{"ok":false,"error":"Inventory failed, not probed","definitions":[],"#,
    r#""healthBasis":"configuration-only","schemaVersion":1,"truncated":true,"#,
    r#""warnings":[{"code":"inventory-failed","sourceId":"inventory"}]}"#
);

#[derive(Default)]
struct Options {
    project: String,
    scope: String,
    watch: bool,
    no_usage: bool,
    id: Option<String>,
    agent: Vec<String>,
    state: Option<String>,
    transaction_id: String,
    record_id: Option<String>,
    payload_stdin: bool,
    before: Option<String>,
}

fn parsed(arguments: &[String]) -> AppResult<Options> {
    let invalid = || AppError::invalid("unrecognised mcp helper arguments");
    let mut options = Options {
        scope: "all".to_string(),
        ..Options::default()
    };
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
            "--watch" => options.watch = true,
            "--no-usage" => options.no_usage = true,
            "--payload-stdin" => options.payload_stdin = true,
            "--project" => options.project = value()?,
            "--scope" => options.scope = value()?,
            "--id" => options.id = Some(value()?),
            "--agent" => options.agent.push(value()?),
            "--state" => options.state = Some(value()?),
            "--transaction-id" => options.transaction_id = value()?,
            "--record-id" => options.record_id = Some(value()?),
            "--before" => options.before = Some(value()?),
            _ => return Err(invalid()),
        }
        index += 1;
    }
    if !SCOPES.contains(&options.scope.as_str()) {
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

pub fn settings(project: &str, scope: &str, environment: Environ) -> Settings {
    let home = crate::common::expanded_os_path(Path::new("~"));
    let project = if project.is_empty() {
        PathBuf::new()
    } else {
        inventory::expanded(project)
    };
    let codex_home = inventory::read_environment_path(&environment, "CODEX_HOME")
        .map(|value| safeio::absolute(&inventory::expanded(&value)))
        .unwrap_or_else(|| home.join(".codex"));
    Settings {
        project: safeio::absolute(&project),
        config_home: home.join(".config"),
        etc_root: PathBuf::from(DEFAULT_ETC_ROOT),
        codex_home,
        home: safeio::absolute(&home),
        system_owner_uid: 0,
        scope: scope.to_string(),
        environment,
    }
}

fn list_failure() -> Value {
    serde_json::from_str(LIST_FAILURE).unwrap_or_else(|_| json!({"ok": false}))
}

fn listing(_: &Request<'_>, arguments: &[String], context: &CoreContext<'_>) -> AppResult<Value> {
    if context.check().is_err() {
        return Ok(list_failure());
    }
    let Ok(options) = parsed(arguments) else {
        return Ok(list_failure());
    };
    let mut store = Inventory::new(settings(
        &options.project,
        &options.scope,
        inventory::environ(),
    ));
    let mut document = store.scan();
    if options.watch {
        let mut plan = std::mem::take(&mut store.plan);
        plan.finish(&mut document);
    }
    let environment = usage::environment();
    let mut definitions: Vec<Value> = document
        .get("definitions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if !options.no_usage {
        let extra = usage::attach_mcp(&environment, &mut definitions);
        for (key, value) in extra {
            document.insert(key, value);
        }
    }
    document.insert(
        "usageWatchPaths".to_string(),
        Value::Array(usage::watch_paths(&environment)),
    );
    let rows = lane_rows(definitions, &options.scope, &PROJECT_SCOPES, "scope");
    let ambiguous = rows
        .iter()
        .filter(|row| row.get("usageAmbiguous") == Some(&Value::Bool(true)))
        .count();
    document.insert("definitions".to_string(), Value::Array(rows));
    document.insert("usageAmbiguous".to_string(), json!(ambiguous));
    let encoded = inventory::bounded_json(&document);
    Ok(serde_json::from_str(&encoded).unwrap_or_else(|_| list_failure()))
}

fn usage_counts(
    _: &Request<'_>,
    arguments: &[String],
    context: &CoreContext<'_>,
) -> AppResult<Value> {
    context.check()?;
    let options = parsed(arguments)?;
    let mut store = Inventory::new(settings(&options.project, "all", inventory::environ()));
    let mut document = store.scan();
    let mut definitions = document
        .remove("definitions")
        .and_then(|value| value.as_array().cloned())
        .unwrap_or_default();
    let environment = usage::environment();
    let extra = usage::attach_mcp(&environment, &mut definitions);
    let counts: serde_json::Map<String, Value> = definitions
        .into_iter()
        .filter_map(|row| {
            let id = row.get("id")?.as_str()?.to_string();
            let values = [
                "uses",
                "usesAgent",
                "usesUser",
                "usesScheduled",
                "failed",
                "observed",
                "usageAmbiguous",
            ]
            .into_iter()
            .map(|key| {
                (
                    key.to_string(),
                    row.get(key).cloned().unwrap_or_else(|| {
                        if key == "usageAmbiguous" {
                            Value::Bool(false)
                        } else {
                            Value::Null
                        }
                    }),
                )
            })
            .collect::<serde_json::Map<String, Value>>();
            Some((id, Value::Object(values)))
        })
        .collect();
    let mut response = json!({
        "ok": !extra.contains_key("usageError"),
        "schemaVersion": 1,
        "counts": counts,
        "usageWatchPaths": usage::watch_paths(&environment),
    });
    response.as_object_mut().unwrap().extend(extra);
    Ok(response)
}

fn applier(options: &Options) -> Applier {
    Applier::new(Inventory::new(settings(
        &options.project,
        "all",
        inventory::environ(),
    )))
}

fn recovery_listing(
    _: &Request<'_>,
    arguments: &[String],
    context: &CoreContext<'_>,
) -> AppResult<Value> {
    context.check()?;
    let options = parsed(arguments)?;
    Ok(applier(&options).recovery.inventory())
}

fn applying(_: &Request<'_>, arguments: &[String], context: &CoreContext<'_>) -> AppResult<Value> {
    context.check()?;
    let options = parsed(arguments)?;
    let id = options
        .id
        .clone()
        .ok_or_else(|| AppError::invalid("the mcp apply method requires an --id argument"))?;
    let state = options
        .state
        .clone()
        .ok_or_else(|| AppError::invalid("the mcp apply method requires a --state argument"))?;
    Ok(applier(&options).apply(&id, &options.agent, &state))
}

fn stdin_text<'a>(request: &'a Request<'a>) -> Option<&'a str> {
    let raw = request.input?;
    let trimmed = raw.strip_suffix('\n').unwrap_or(raw);
    (!trimmed.is_empty()).then_some(trimmed)
}

fn removing(
    request: &Request<'_>,
    arguments: &[String],
    context: &CoreContext<'_>,
) -> AppResult<Value> {
    context.check()?;
    let options = parsed(arguments)?;
    let id = options
        .id
        .clone()
        .ok_or_else(|| AppError::invalid("the mcp removal methods require an --id argument"))?;
    let prepare = request.method == "prepare-remove";
    let mut expected: Option<Value> = None;
    if request.method == "remove-prepared" {
        let Some(text) = stdin_text(request) else {
            return Ok(apply::failure(
                "prepared recovery payload is missing or invalid",
            ));
        };
        if text.len() > apply::MAX_RESTORE_PAYLOAD_BYTES {
            return Ok(apply::failure("restore payload exceeds its byte limit"));
        }
        match serde_json::from_str::<Value>(text) {
            Ok(value) if value.is_object() => expected = Some(value),
            _ => {
                return Ok(apply::failure(
                    "prepared recovery payload is missing or invalid",
                ));
            }
        }
    }
    Ok(applier(&options).remove(
        &id,
        &Removal {
            prepare,
            expected_payload: expected.as_ref(),
            transaction_id: &options.transaction_id,
        },
    ))
}

fn restoring(
    request: &Request<'_>,
    arguments: &[String],
    context: &CoreContext<'_>,
) -> AppResult<Value> {
    context.check()?;
    let options = parsed(arguments)?;
    let record_id = options.record_id.clone().unwrap_or_default();
    let mut store = applier(&options);
    if !options.payload_stdin {
        if request.method == "discard" {
            return Ok(store.recovery.discard_payload(&record_id, None));
        }
        return Ok(store.restore(&record_id, None));
    }
    let Some(text) = stdin_text(request) else {
        return Ok(apply::failure("restore payload is missing"));
    };
    if text.len() > apply::MAX_RESTORE_PAYLOAD_BYTES {
        return Ok(apply::failure("restore payload exceeds its byte limit"));
    }
    if request.method == "discard" {
        let payload: Value = match serde_json::from_str(text) {
            Ok(value) => value,
            Err(_) => {
                return Ok(json!({
                    "ok": false,
                    "schemaVersion": 1,
                    "message": "recovery payload must be an object",
                }));
            }
        };
        return Ok(store.recovery.discard_payload(&record_id, Some(&payload)));
    }
    Ok(store.restore(&record_id, Some(text)))
}

pub fn handler(method: &str) -> Option<crate::core_modules::CoreHandler> {
    match method {
        "list" => Some(listing),
        "usage-counts" => Some(usage_counts),
        "recovery-list" => Some(recovery_listing),
        "apply" => Some(applying),
        "prepare-remove" | "remove-prepared" => Some(removing),
        "restore" | "discard" => Some(restoring),
        _ => usage::handler(method),
    }
}
