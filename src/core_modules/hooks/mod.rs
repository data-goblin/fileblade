pub mod adapters;
pub mod apply;
pub mod discovery;
pub mod events;
pub mod labels;
pub mod records;
pub mod redaction;
pub mod safeio;

use crate::core_modules::Context as CoreContext;
use crate::core_modules::emit::encoded_items;
use crate::core_modules::watch::SCOPES;
use crate::module_helpers::Request;
use crate::{AppError, AppResult};
use labels::Environ;
use safeio::Budget;
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

pub const DEFAULT_ETC_ROOT: &str = "/etc";

#[derive(Default)]
struct Options {
    project: String,
    exact: bool,
    home: String,
    watch: bool,
    scope: String,
    id: Option<String>,
    agent: Vec<String>,
    state: Option<String>,
    transaction_id: String,
    record_id: Option<String>,
    payload_stdin: bool,
    digest: Option<String>,
    text: Option<String>,
}

fn parsed(arguments: &[String]) -> AppResult<Options> {
    let invalid = || AppError::invalid("unrecognised hooks helper arguments");
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
            "--exact" => options.exact = true,
            "--watch" => options.watch = true,
            "--payload-stdin" => options.payload_stdin = true,
            "--project" => options.project = value()?,
            "--home" => options.home = value()?,
            "--scope" => options.scope = value()?,
            "--id" => options.id = Some(value()?),
            "--agent" => options.agent.push(value()?),
            "--state" => options.state = Some(value()?),
            "--transaction-id" => options.transaction_id = value()?,
            "--record-id" => options.record_id = Some(value()?),
            "--digest" => options.digest = Some(value()?),
            "--text" => options.text = Some(value()?),
            _ => return Err(invalid()),
        }
        index += 1;
    }
    if !SCOPES.contains(&options.scope.as_str()) {
        return Err(invalid());
    }
    Ok(options)
}

fn environ() -> Environ {
    std::env::vars_os().collect()
}

fn home_path(home: &str) -> PathBuf {
    if home.is_empty() {
        crate::common::expanded_os_path(Path::new("~"))
    } else {
        safeio::expanded(home)
    }
}

fn locator<'a>(options: &'a Options, variables: Environ) -> apply::Locator<'a> {
    apply::Locator {
        project: &options.project,
        home: &options.home,
        environ: variables,
        etc_root: DEFAULT_ETC_ROOT,
        policy_owner_uid: 0,
        exact: options.exact,
    }
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
    let mut budget = Budget::default();
    let mut document = discovery::collect(
        &mut budget,
        &discovery::Query {
            project: &options.project,
            home: &options.home,
            etc_root: DEFAULT_ETC_ROOT,
            policy_owner_uid: 0,
            exact: options.exact,
            scope: &options.scope,
        },
        environ(),
    );
    if options.watch {
        let mut plan = std::mem::take(&mut budget.plan);
        plan.finish(&mut document);
    }
    Ok(bounded(&document))
}

fn recovery_listing(
    _: &Request<'_>,
    arguments: &[String],
    context: &CoreContext<'_>,
) -> AppResult<Value> {
    context.check()?;
    let options = parsed(arguments)?;
    let variables = environ();
    Ok(apply::recovery_store(&home_path(&options.home), &variables).inventory())
}

fn applying(_: &Request<'_>, arguments: &[String], context: &CoreContext<'_>) -> AppResult<Value> {
    context.check()?;
    let options = parsed(arguments)?;
    let id = options
        .id
        .clone()
        .ok_or_else(|| AppError::invalid("the hooks apply method requires an --id argument"))?;
    let state = options
        .state
        .clone()
        .ok_or_else(|| AppError::invalid("the hooks apply method requires a --state argument"))?;
    let variables = environ();
    let locator = locator(&options, variables);
    Ok(Value::Object(apply::apply(
        &locator,
        &id,
        &options.agent,
        &state,
    )))
}

fn stdin_payload(request: &Request<'_>) -> Result<Option<Value>, Map<String, Value>> {
    let Some(raw) = request.input else {
        return Ok(None);
    };
    let trimmed = raw.strip_suffix('\n').unwrap_or(raw);
    if trimmed.is_empty() {
        return Ok(None);
    }
    if trimmed.len() > apply::MAX_RESTORE_PAYLOAD_BYTES {
        return Err(apply::failure("", "restore payload exceeds its byte limit"));
    }
    match serde_json::from_str::<Value>(trimmed) {
        Ok(value) => Ok(Some(value)),
        Err(_) => Err(apply::failure("", "prepared recovery payload is invalid")),
    }
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
        .ok_or_else(|| AppError::invalid("the hooks removal methods require an --id argument"))?;
    let prepare = request.method == "prepare-remove";
    let expected = if request.method == "remove-prepared" {
        match stdin_payload(request) {
            Ok(Some(value)) if value.is_object() => Some(value),
            Ok(_) => {
                return Ok(Value::Object(apply::failure(
                    "",
                    "prepared recovery payload is missing",
                )));
            }
            Err(document) => return Ok(Value::Object(document)),
        }
    } else {
        None
    };
    let variables = environ();
    let locator = locator(&options, variables);
    let removal = apply::Removal {
        prepare,
        expected_payload: expected.as_ref(),
        transaction_id: &options.transaction_id,
    };
    Ok(Value::Object(apply::remove(&locator, &id, &removal)))
}

fn restoring(
    request: &Request<'_>,
    arguments: &[String],
    context: &CoreContext<'_>,
) -> AppResult<Value> {
    context.check()?;
    let options = parsed(arguments)?;
    let record_id = options.record_id.clone().unwrap_or_default();
    let variables = environ();
    let store_home = home_path(&options.home);
    if !options.payload_stdin {
        if request.method == "discard" {
            return Ok(
                apply::recovery_store(&store_home, &variables).discard_payload(&record_id, None)
            );
        }
        return Ok(Value::Object(apply::restore(
            &record_id,
            None,
            &options.home,
            &variables,
            DEFAULT_ETC_ROOT,
            0,
        )));
    }
    let raw = request.input.unwrap_or_default();
    let trimmed = raw.strip_suffix('\n').unwrap_or(raw);
    if trimmed.is_empty() {
        return Ok(Value::Object(apply::failure(
            "",
            "restore payload is missing",
        )));
    }
    if trimmed.len() > apply::MAX_RESTORE_PAYLOAD_BYTES {
        return Ok(Value::Object(apply::failure(
            "",
            "restore payload exceeds its byte limit",
        )));
    }
    if request.method == "discard" {
        let payload: Value = match serde_json::from_str(trimmed) {
            Ok(value) => value,
            Err(_) => {
                return Ok(json!({
                    "ok": false,
                    "schemaVersion": 1,
                    "message": "recovery payload must be an object",
                }));
            }
        };
        return Ok(apply::recovery_store(&store_home, &variables)
            .discard_payload(&record_id, Some(&payload)));
    }
    Ok(Value::Object(apply::restore(
        &record_id,
        Some(trimmed),
        &options.home,
        &variables,
        DEFAULT_ETC_ROOT,
        0,
    )))
}

fn labelling(_: &Request<'_>, arguments: &[String], context: &CoreContext<'_>) -> AppResult<Value> {
    context.check()?;
    let options = parsed(arguments)?;
    let digest = options
        .digest
        .clone()
        .ok_or_else(|| AppError::invalid("the hooks label method requires a --digest argument"))?;
    let text = options
        .text
        .clone()
        .ok_or_else(|| AppError::invalid("the hooks label method requires a --text argument"))?;
    let variables = environ();
    Ok(labels::set_label(
        &home_path(&options.home),
        &variables,
        &digest,
        &text,
    ))
}

pub fn handler(method: &str) -> Option<crate::core_modules::CoreHandler> {
    match method {
        "list" => Some(listing),
        "recovery-list" => Some(recovery_listing),
        "apply" => Some(applying),
        "prepare-remove" | "remove-prepared" => Some(removing),
        "restore" | "discard" => Some(restoring),
        "label" => Some(labelling),
        _ => None,
    }
}
