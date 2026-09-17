use crate::command::{CommandSpec, which};
use crate::core_modules::Context;
use crate::{AppError, AppResult};
use serde_json::{Map, Value};

const BRIDGE: &str = r#"
import json, sys
import agent_usage
request = json.load(sys.stdin)
items = request.get("items") or []
method = request["method"]
if method == "attach":
    extra = agent_usage.attach_skills(items)
    document = {"items": items, "extra": extra, "watchPaths": agent_usage.watch_paths()}
elif method == "usage":
    scoped = {"scoped": True} if request.get("scoped") else {}
    document = agent_usage.skill_usage(items, **scoped)
elif method == "counts":
    document = agent_usage.skill_counts(items)
else:
    document = agent_usage.skill_day(items, request.get("day") or "")
sys.stdout.write(json.dumps(document))
"#;

pub fn call(context: &Context<'_>, request: &Value) -> AppResult<Value> {
    context.check()?;
    let root = crate::paths::app_root()?;
    let interpreter =
        which("python3").ok_or_else(|| AppError::command("python3 is not available"))?;
    let output = CommandSpec::new(interpreter)
        .args(["-B", "-c", BRIDGE])
        .env("PYTHONPATH", root.join("python"))
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .cwd(root)
        .timeout(context.remaining())
        .limits(8 * 1024 * 1024, 4096)
        .stdin(serde_json::to_vec(request).map_err(|error| AppError::command(error.to_string()))?)
        .run_cancellable(context.cancelled())?;
    if !output.status.success() || output.stdout_truncated {
        return Err(AppError::command("the usage store could not be queried"));
    }
    serde_json::from_slice(&output.stdout)
        .map_err(|_| AppError::command("the usage store returned no complete JSON document"))
}

pub const UNAVAILABLE: &str = "usage store unavailable";

pub fn attach(context: &Context<'_>, document: &mut Map<String, Value>) {
    let items = document
        .get("items")
        .cloned()
        .unwrap_or(Value::Array(Vec::new()));
    let Ok(answer) = call(
        context,
        &serde_json::json!({"method": "attach", "items": items}),
    ) else {
        document.insert("usageError".to_string(), Value::from(UNAVAILABLE));
        document.insert("usageWatchPaths".to_string(), Value::Array(Vec::new()));
        return;
    };
    if let Some(items) = answer.get("items") {
        document.insert("items".to_string(), items.clone());
    }
    if let Some(extra) = answer.get("extra").and_then(Value::as_object) {
        for (key, value) in extra {
            document.insert(key.clone(), value.clone());
        }
    }
    let watched: Vec<Value> = answer
        .get("watchPaths")
        .and_then(Value::as_array)
        .map(|paths| {
            paths
                .iter()
                .filter_map(Value::as_str)
                .map(|path| Value::from(crate::common::path_text(std::path::Path::new(path))))
                .collect()
        })
        .unwrap_or_default();
    document.insert("usageWatchPaths".to_string(), Value::Array(watched));
}
