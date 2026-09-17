use super::adapters::AGENT_IDS;
use super::common::{SCHEMA_VERSION, ancestors_of, env_path, realpath_of, safe_basename_os};
use super::discovery::{self, Context};
use crate::common::{display_path, path_text};
use crate::core_modules::watch::WatchPlan;
use serde_json::{Map, Value, json};
use std::ffi::OsStr;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

pub const STATES: [&str; 2] = ["on", "off"];
const LINKABLE_KIND: &str = "instructions";
const USER_SCOPES: [&str; 1] = ["user"];
const PROJECT_SCOPES: [&str; 2] = ["project", "local"];
const ANTIGRAVITY_RULES: &str = ".agents/rules";

fn project_name(agent: &str) -> &'static str {
    match agent {
        "claude-code" => "CLAUDE.md",
        "codex" | "opencode" | "pi" | "copilot-cli" => "AGENTS.md",
        _ => "",
    }
}

pub struct Outcome {
    agent: String,
    ok: bool,
    changed: bool,
    message: String,
    touched: Vec<PathBuf>,
}

impl Outcome {
    fn row(&self) -> Value {
        json!({
            "agent": self.agent,
            "ok": self.ok,
            "changed": self.changed,
            "message": self.message,
            "touched": self.touched.iter().map(|path| Value::from(path_text(path))).collect::<Vec<_>>(),
        })
    }
}

fn result(agent: &str, ok: bool, changed: bool, message: String, touched: Vec<PathBuf>) -> Outcome {
    Outcome {
        agent: agent.to_string(),
        ok,
        changed,
        message,
        touched,
    }
}

fn refused(agent: &str, message: String) -> Outcome {
    result(agent, false, false, message, Vec::new())
}

fn unchanged(agent: &str, message: String) -> Outcome {
    result(agent, true, false, message, Vec::new())
}

fn text(path: &Path) -> String {
    display_path(path)
}

fn without_code(rendered: String) -> String {
    match rendered.split_once(" (os error ") {
        Some((message, _)) => message.to_string(),
        None => rendered,
    }
}

fn strerror(error: &std::io::Error) -> String {
    without_code(error.to_string())
}

fn user_directory(agent: &str, context: &Context) -> Option<PathBuf> {
    match agent {
        "claude-code" => Some(context.home.join(".claude")),
        "codex" => Some(
            env_path("CODEX_HOME", &context.environ).unwrap_or_else(|| context.home.join(".codex")),
        ),
        "opencode" => Some(
            env_path("XDG_CONFIG_HOME", &context.environ)
                .unwrap_or_else(|| context.home.join(".config"))
                .join("opencode"),
        ),
        "pi" => Some(context.home.join(".pi").join("agent")),
        "copilot-cli" => Some(
            env_path("COPILOT_HOME", &context.environ)
                .unwrap_or_else(|| context.home.join(".copilot")),
        ),
        "antigravity" => Some(context.home.join(".gemini")),
        _ => None,
    }
}

fn user_name(agent: &str) -> &'static str {
    match agent {
        "copilot-cli" => "copilot-instructions.md",
        "antigravity" => "GEMINI.md",
        _ => project_name(agent),
    }
}

fn project_base(context: &Context) -> PathBuf {
    if context.project_root.as_os_str().is_empty() {
        context.anchor.clone()
    } else {
        context.project_root.clone()
    }
}

fn chain_directories(context: &Context) -> Vec<PathBuf> {
    ancestors_of(&context.anchor, Some(&project_base(context)))
}

fn same_place(left: &Path, right: &Path) -> bool {
    realpath_of(left) == realpath_of(right)
}

fn contained(path: &Path, base: &Path) -> bool {
    let inner = realpath_of(path);
    let outer = realpath_of(base);
    inner == outer || inner.starts_with(&outer)
}

fn user_target(agent: &str, context: &Context) -> Result<PathBuf, Outcome> {
    let Some(directory) = user_directory(agent, context) else {
        return Err(refused(agent, format!("unknown agent id '{agent}'")));
    };
    let name = user_name(agent);
    if name.is_empty() {
        return Err(refused(
            agent,
            "Gemini CLI context.fileName is configured empty, so there is no file name to link"
                .to_string(),
        ));
    }
    Ok(directory.join(name))
}

fn project_target(
    agent: &str,
    context: &Context,
    row_path: &Path,
    source: &Path,
) -> Result<PathBuf, Outcome> {
    let base = project_base(context);
    if agent == "antigravity" {
        let markdown = source
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|value| value == "md");
        if !markdown {
            return Err(refused(
                agent,
                format!("Antigravity reads only .md files under {ANTIGRAVITY_RULES}"),
            ));
        }
        let Some(name) = source.file_name() else {
            return Err(refused(
                agent,
                format!("Antigravity reads only .md files under {ANTIGRAVITY_RULES}"),
            ));
        };
        return Ok(base.join(ANTIGRAVITY_RULES).join(name));
    }
    let name = project_name(agent);
    if name.is_empty() {
        return Err(refused(agent, format!("unknown agent id '{agent}'")));
    }
    let Some(directory) = row_path.parent() else {
        return Err(refused(agent, format!("unknown agent id '{agent}'")));
    };
    if !chain_directories(context)
        .iter()
        .any(|candidate| same_place(directory, candidate))
    {
        return Err(refused(
            agent,
            format!(
                "{agent} reads {name} only between {} and {}, not in {}",
                text(&base),
                text(&context.anchor),
                text(directory)
            ),
        ));
    }
    Ok(directory.join(name))
}

fn resolve_target(agent: &str, context: &Context, row: &Value) -> Result<PathBuf, Outcome> {
    if !AGENT_IDS.contains(&agent) {
        return Err(refused(agent, format!("unknown agent id '{agent}'")));
    }
    let field = |key: &str| {
        row.get(key)
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string()
    };
    let kind = field("kind");
    let scope = field("scope");
    if kind != LINKABLE_KIND {
        return Err(refused(
            agent,
            format!(
                "{} is a {kind} file; {agent} has no documented equivalent for it",
                field("name")
            ),
        ));
    }
    let (target, base) = if USER_SCOPES.contains(&scope.as_str()) {
        (user_target(agent, context)?, user_directory(agent, context))
    } else if PROJECT_SCOPES.contains(&scope.as_str()) {
        let row_path = row_path_of(row, "path");
        let source = row_path_of(row, "realpath");
        (
            project_target(agent, context, &row_path, &source)?,
            Some(project_base(context)),
        )
    } else {
        return Err(refused(
            agent,
            format!("{scope} scope has no documented location for {agent}"),
        ));
    };
    let named = target.file_name().unwrap_or_else(|| OsStr::new(""));
    let escapes = base.is_none()
        || safe_basename_os(named).as_os_str() != named
        || !target
            .parent()
            .is_some_and(|parent| contained(parent, base.as_deref().unwrap_or(Path::new("/"))));
    if escapes {
        return Err(refused(
            agent,
            format!(
                "{} would escape {}",
                text(&target),
                base.as_deref()
                    .map(text)
                    .unwrap_or_else(|| "None".to_string())
            ),
        ));
    }
    Ok(target)
}

fn row_path_of(row: &Value, key: &str) -> PathBuf {
    row.get(key)
        .and_then(Value::as_str)
        .map(|value| crate::common::parse_path(value).unwrap_or_else(|_| PathBuf::from(value)))
        .unwrap_or_default()
}

fn link_to(target: &Path, source: &Path) -> Result<(), String> {
    let absolute = crate::common::normalize_path(target);
    let parent = absolute
        .parent()
        .map(realpath_of)
        .ok_or_else(|| "path has no parent".to_string())?;
    let resolved = std::fs::canonicalize(source).map_err(|error| strerror(&error))?;
    crate::companion_mutations::link(&absolute, &parent, &resolved)
        .map_err(|error| without_code(error.to_string()))
}

fn unlink_from(target: &Path, source: &Path, dev: u64, ino: u64) -> Result<(), String> {
    let absolute = crate::common::normalize_path(target);
    let parent = std::fs::canonicalize(
        absolute
            .parent()
            .ok_or_else(|| "path has no parent".to_string())?,
    )
    .map_err(|error| strerror(&error))?;
    let current = std::fs::symlink_metadata(&absolute).map_err(|error| strerror(&error))?;
    if !current.is_symlink() || (current.dev(), current.ino()) != (dev, ino) {
        return Err("selected link changed; nothing was removed".to_string());
    }
    let destination = std::fs::read_link(&absolute).map_err(|error| strerror(&error))?;
    let pointed = if destination.is_absolute() {
        realpath_of(&destination)
    } else {
        realpath_of(&parent.join(destination))
    };
    if pointed != realpath_of(source) {
        return Err("selected link points elsewhere; nothing was removed".to_string());
    }
    crate::companion_mutations::unlink(&absolute, &parent, dev, ino)
        .map_err(|error| without_code(error.to_string()))
}

fn turn_on(agent: &str, target: &Path, source: &Path) -> Outcome {
    match std::fs::symlink_metadata(target) {
        Ok(current) => {
            if current.is_symlink() {
                if same_place(target, source) {
                    return unchanged(
                        agent,
                        format!("{} already links to this file", text(target)),
                    );
                }
                let destination = std::fs::read_link(target).unwrap_or_default();
                return refused(
                    agent,
                    format!(
                        "{} already links to a different file ({})",
                        text(target),
                        text(&destination)
                    ),
                );
            }
            if same_place(target, source) {
                return unchanged(agent, format!("{agent} already reads {}", text(target)));
            }
            refused(
                agent,
                format!("{} exists as a real file; not replacing it", text(target)),
            )
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            match link_to(target, source) {
                Ok(()) => result(
                    agent,
                    true,
                    true,
                    format!("linked {}", text(target)),
                    vec![target.to_path_buf()],
                ),
                Err(message) => refused(agent, format!("cannot link {}: {message}", text(target))),
            }
        }
        Err(error) => refused(
            agent,
            format!("cannot inspect {}: {}", text(target), strerror(&error)),
        ),
    }
}

fn turn_off(agent: &str, target: &Path, source: &Path, reads_directly: bool) -> Outcome {
    let current = match std::fs::symlink_metadata(target) {
        Ok(current) => current,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            if reads_directly {
                return unchanged(
                    agent,
                    format!("{agent} reads this file without a link; nothing to remove"),
                );
            }
            return unchanged(agent, format!("nothing linked at {}", text(target)));
        }
        Err(error) => {
            return refused(
                agent,
                format!("cannot inspect {}: {}", text(target), strerror(&error)),
            );
        }
    };
    if !current.is_symlink() {
        if same_place(target, source) {
            return unchanged(
                agent,
                format!(
                    "{} is the memory file itself; a real file is never deleted",
                    text(target)
                ),
            );
        }
        return unchanged(
            agent,
            format!("{} is a separate real file; left alone", text(target)),
        );
    }
    if !same_place(target, source) {
        return unchanged(
            agent,
            format!("{} links to a different file; left alone", text(target)),
        );
    }
    match unlink_from(target, source, current.dev(), current.ino()) {
        Ok(()) => result(
            agent,
            true,
            true,
            format!("unlinked {}", text(target)),
            vec![target.to_path_buf()],
        ),
        Err(message) => refused(agent, format!("cannot unlink {}: {message}", text(target))),
    }
}

fn find_row(document: &Map<String, Value>, row_id: &str) -> Option<Value> {
    document
        .get("items")
        .and_then(Value::as_array)
        .and_then(|items| {
            items
                .iter()
                .find(|row| row.get("id").and_then(Value::as_str) == Some(row_id))
                .cloned()
        })
}

fn document(context: &Context, results: &[Outcome]) -> Map<String, Value> {
    let failures: Vec<String> = results
        .iter()
        .filter(|entry| !entry.ok)
        .map(|entry| format!("{}: {}", entry.agent, entry.message))
        .collect();
    let mut payload = Map::new();
    payload.insert("ok".to_string(), json!(failures.is_empty()));
    payload.insert("schemaVersion".to_string(), json!(SCHEMA_VERSION));
    payload.insert(
        "project".to_string(),
        json!(path_text(&context.project_root)),
    );
    payload.insert("message".to_string(), json!(failures.join("; ")));
    payload.insert(
        "results".to_string(),
        Value::Array(results.iter().map(Outcome::row).collect()),
    );
    payload
}

pub fn apply_in(
    context: &Context,
    config: &str,
    row_id: &str,
    agents: &[String],
    state: &str,
) -> Map<String, Value> {
    if !STATES.contains(&state) {
        let results: Vec<Outcome> = agents
            .iter()
            .map(|agent| refused(agent, format!("state must be one of {}", STATES.join(", "))))
            .collect();
        return document(context, &results);
    }
    if agents.is_empty() {
        return document(
            context,
            &[refused("", "at least one --agent is required".to_string())],
        );
    }
    let mut plan = WatchPlan::new();
    let listing = discovery::collect_in(&mut plan, context, config);
    let Some(row) = find_row(&listing, row_id) else {
        let where_at = if context.project_root.as_os_str().is_empty() {
            text(&context.anchor)
        } else {
            text(&context.project_root)
        };
        let results: Vec<Outcome> = agents
            .iter()
            .map(|agent| {
                refused(
                    agent,
                    format!("no memory row with id '{row_id}' for {where_at}"),
                )
            })
            .collect();
        return document(context, &results);
    };
    let source = row_path_of(&row, "realpath");
    if !std::fs::metadata(&source).is_ok_and(|data| data.is_file()) {
        let results: Vec<Outcome> = agents
            .iter()
            .map(|agent| refused(agent, format!("{} is not a regular file", text(&source))))
            .collect();
        return document(context, &results);
    }
    let readers: Vec<String> = row
        .get("readers")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(|entry| entry.get("agent").and_then(Value::as_str))
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default();
    let mut results: Vec<Outcome> = Vec::new();
    for agent in agents {
        match resolve_target(agent, context, &row) {
            Err(outcome) => results.push(outcome),
            Ok(target) if state == "on" => results.push(turn_on(agent, &target, &source)),
            Ok(target) => results.push(turn_off(
                agent,
                &target,
                &source,
                readers.iter().any(|reader| reader == agent),
            )),
        }
    }
    document(context, &results)
}
