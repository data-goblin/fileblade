use crate::actions::{
    plugin_root, read_manifest, resolve_plugin_program, valid_id, valid_provider_id,
};
use crate::command::CommandSpec;
use crate::{AppError, AppResult};
use serde_json::Value;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

pub const INPUT_LIMIT: usize = 64 * 1024;
const OUTPUT_LIMIT: usize = 2 * 1024 * 1024;
const ARGUMENT_BYTES: usize = 64 * 1024;
const EXTENSION: &str = "data-goblin.fileblade/helper";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CoreRoute {
    Skills,
    Memory,
    Hooks,
    Mcp,
}

impl CoreRoute {
    pub fn module(self) -> &'static str {
        match self {
            Self::Skills => "skills",
            Self::Memory => "memory",
            Self::Hooks => "hooks",
            Self::Mcp => "mcp",
        }
    }

    pub fn provider(self) -> &'static str {
        match self {
            Self::Skills => "fileblade.core.skills",
            Self::Memory => "fileblade.core.memory",
            Self::Hooks => "fileblade.core.hooks",
            Self::Mcp => "fileblade.core.mcp",
        }
    }

    fn permits(self, method: &str, write: bool) -> bool {
        match (self, write) {
            (_, false) => {
                method == "list"
                    || matches!(self, Self::Hooks | Self::Mcp) && method == "recovery-list"
                    || matches!(self, Self::Skills | Self::Mcp) && method == "usage"
                    || self == Self::Skills && matches!(method, "usage-counts" | "usage-day")
            }
            (Self::Skills | Self::Memory, true) => method == "apply",
            (Self::Hooks | Self::Mcp, true) => {
                matches!(
                    method,
                    "apply" | "prepare-remove" | "remove-prepared" | "restore" | "discard"
                ) || self == Self::Hooks && method == "label"
                    || self == Self::Mcp && method == "usage-forget"
            }
        }
    }
}

pub fn canonical_route(provider_id: &str, helper_id: &str) -> Option<CoreRoute> {
    if helper_id != "inventory" {
        return None;
    }
    match provider_id {
        "fileblade.core.skills" | "data-goblin.fileblade-skills" | "kurt.agent-skills" => {
            Some(CoreRoute::Skills)
        }
        "fileblade.core.memory" | "data-goblin.fileblade-memory" | "kurt.agent-memory" => {
            Some(CoreRoute::Memory)
        }
        "fileblade.core.hooks" | "data-goblin.fileblade-hooks" | "kurt.agent-hooks" => {
            Some(CoreRoute::Hooks)
        }
        "fileblade.core.mcp" | "data-goblin.fileblade-mcp" | "kurt.agent-mcp" => {
            Some(CoreRoute::Mcp)
        }
        _ => None,
    }
}

pub struct Request<'a> {
    pub provider: &'a str,
    pub directory: &'a str,
    pub helper: &'a str,
    pub method: &'a str,
    pub arguments: &'a str,
    pub input: Option<&'a str>,
    pub write: bool,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Route {
    pub provider: String,
    pub directory: String,
    pub helper: String,
}

impl Route {
    pub fn request<'a>(
        &'a self,
        method: &'a str,
        arguments: &'a str,
        input: Option<&'a str>,
        write: bool,
    ) -> Request<'a> {
        Request {
            provider: &self.provider,
            directory: &self.directory,
            helper: &self.helper,
            method,
            arguments,
            input,
            write,
        }
    }

    pub fn validate(&self, method: &str, write: bool) -> AppResult<()> {
        declaration(&self.request(method, "[]", None, write)).map(|_| ())
    }
}

struct Declaration {
    root: PathBuf,
    program: PathBuf,
    timeout: Duration,
    core: Option<CoreRoute>,
}

fn declaration(request: &Request<'_>) -> AppResult<Declaration> {
    if !valid_provider_id(request.provider)
        || !valid_id(request.helper)
        || !valid_id(request.method)
    {
        return Err(AppError::invalid(
            "invalid provider, helper or method identifier",
        ));
    }
    if let Some(core) = canonical_route(request.provider, request.helper) {
        if request.provider == core.provider() && !request.directory.is_empty() {
            return Err(AppError::invalid(
                "core helpers do not accept a caller-supplied directory",
            ));
        }
        if !core.permits(request.method, request.write) {
            return Err(AppError::invalid(
                "core helper method is not declared for this request kind",
            ));
        }
        let root = crate::paths::app_root()?;
        let entry = format!("python/bin/agent-{}ctl", core.module());
        let program = resolve_plugin_program(&entry, &root).map_err(AppError::invalid)?;
        return Ok(Declaration {
            root,
            program,
            timeout: Duration::from_secs(8),
            core: Some(core),
        });
    }
    if request.provider.starts_with("fileblade.core.")
        || canonical_route(request.provider, "inventory").is_some()
    {
        return Err(AppError::invalid("unknown core helper route"));
    }
    let root = plugin_root(request.directory).map_err(AppError::invalid)?;
    let manifest = read_manifest(&root).map_err(AppError::invalid)?;
    if manifest.get("id").and_then(Value::as_str) != Some(request.provider) {
        return Err(AppError::invalid(
            "helper provider does not match its manifest",
        ));
    }
    let entries = manifest
        .get("extensions")
        .and_then(|value| value.get(EXTENSION))
        .and_then(Value::as_array)
        .filter(|entries| entries.len() <= 16)
        .ok_or_else(|| AppError::invalid("provider has no bounded helper declaration"))?;
    let mut matched = entries
        .iter()
        .filter(|entry| entry.get("id").and_then(Value::as_str) == Some(request.helper));
    let helper = matched
        .next()
        .ok_or_else(|| AppError::invalid("helper is not declared by its provider"))?;
    if matched.next().is_some() {
        return Err(AppError::invalid(
            "provider declares the helper more than once",
        ));
    }
    let reads = methods(helper, "read")?;
    let writes = methods(helper, "write")?;
    if reads.iter().any(|name| writes.contains(name)) {
        return Err(AppError::invalid("helper read and write methods overlap"));
    }
    if !(if request.write { &writes } else { &reads }).contains(&request.method) {
        return Err(AppError::invalid(
            "helper method is not declared for this request kind",
        ));
    }
    let entry = helper
        .get("entry")
        .and_then(Value::as_str)
        .ok_or_else(|| AppError::invalid("helper entry is missing"))?;
    let program = resolve_plugin_program(entry, &root).map_err(AppError::invalid)?;
    let timeout = helper
        .get("timeoutMs")
        .map(|value| {
            value
                .as_u64()
                .filter(|value| (100..=30_000).contains(value))
                .ok_or_else(|| AppError::invalid("helper timeoutMs must be 100..30000"))
        })
        .transpose()?
        .unwrap_or(8000);
    Ok(Declaration {
        root,
        program,
        timeout: Duration::from_millis(timeout),
        core: None,
    })
}

fn methods<'a>(helper: &'a Value, kind: &str) -> AppResult<Vec<&'a str>> {
    let Some(value) = helper.get(kind) else {
        return Ok(Vec::new());
    };
    let entries = value
        .as_array()
        .filter(|entries| entries.len() <= 16)
        .ok_or_else(|| AppError::invalid("helper method list exceeds its bounds"))?;
    let mut result = Vec::new();
    for entry in entries {
        let method = entry
            .as_str()
            .filter(|name| valid_id(name))
            .ok_or_else(|| AppError::invalid("invalid helper method declaration"))?;
        if result.contains(&method) {
            return Err(AppError::invalid("duplicate helper method"));
        }
        result.push(method);
    }
    Ok(result)
}

pub fn validate_arguments(raw: &str) -> AppResult<Vec<String>> {
    if raw.len() > ARGUMENT_BYTES {
        return Err(AppError::invalid("helper arguments exceed 64 KiB"));
    }
    let arguments: Vec<String> = serde_json::from_str(raw)
        .map_err(|_| AppError::invalid("helper arguments must be a JSON string array"))?;
    if arguments.len() > 128 || arguments.iter().any(|value| value.contains('\0')) {
        return Err(AppError::invalid(
            "helper arguments exceed their bounds or contain NUL",
        ));
    }
    Ok(arguments)
}

pub fn run(request: &Request<'_>, cancelled: &AtomicBool) -> AppResult<Value> {
    if cancelled.load(std::sync::atomic::Ordering::Relaxed) {
        return Err(AppError::Cancelled);
    }
    let arguments = validate_arguments(request.arguments)?;
    if request.input.is_some_and(|input| input.len() > INPUT_LIMIT) {
        return Err(AppError::invalid("helper input exceeds 64 KiB"));
    }
    let declared = declaration(request)?;
    if declared.core.is_none() {
        crate::plugin_catalog::require_enabled(request.provider, &declared.root)?;
    }
    if request.write && matches!(declared.core, Some(CoreRoute::Skills | CoreRoute::Memory)) {
        crate::preferences::require_agent_management()?
    }
    if let Some(core) = declared.core
        && let Some(document) = crate::core_modules::dispatch(
            core,
            request,
            &arguments,
            &crate::core_modules::Context::new(declared.timeout, cancelled),
        )
    {
        return document;
    }
    let mut command = CommandSpec::new(declared.program)
        .args([request.method])
        .args(&arguments)
        .cwd(declared.root)
        .timeout(declared.timeout)
        .limits(OUTPUT_LIMIT, 4096);
    if let Some(input) = request.input {
        command = command.stdin(input.as_bytes());
    }
    let output = command.run_cancellable(cancelled)?;
    if output.stdout_truncated {
        return Err(AppError::command("helper output exceeds 2 MiB"));
    }
    let document: Value = serde_json::from_slice(&output.stdout)
        .map_err(|_| AppError::command("helper returned no complete JSON document"))?;
    if !document.is_object() {
        return Err(AppError::command("helper response must be a JSON object"));
    }
    if !output.status.success() && document.get("ok") != Some(&Value::Bool(false)) {
        return Err(AppError::command(
            "helper failed after producing its response",
        ));
    }
    Ok(document)
}
