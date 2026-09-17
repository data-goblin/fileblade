use super::bounds::{MAX_ENTRIES_PER_ROOT, bounded_names, realpath};
use super::discovery::{self, Candidate, Collected, Environment};
use super::registry::{self, Root};
use crate::common::path_text;
use crate::core_modules::watch::WatchPlan;
use serde_json::{Map, Value, json};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

const APPLICABLE_SCOPES: [&str; 2] = ["user", "project"];
const ALL_AGENTS: &str = "all";

pub struct Outcome {
    pub agent: String,
    pub ok: bool,
    pub changed: bool,
    pub message: String,
    pub touched: Vec<PathBuf>,
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

fn refusal(agent: &str, message: String) -> Outcome {
    Outcome {
        agent: agent.to_string(),
        ok: false,
        changed: false,
        message,
        touched: Vec::new(),
    }
}

fn settled(agent: &str, changed: bool, message: String, touched: Vec<PathBuf>) -> Outcome {
    Outcome {
        agent: agent.to_string(),
        ok: true,
        changed,
        message,
        touched,
    }
}

pub fn requested_agents(requested: &[String]) -> Vec<String> {
    if requested.iter().any(|agent| agent == ALL_AGENTS) {
        return registry::agents().into_iter().map(str::to_string).collect();
    }
    let mut ordered: Vec<String> = Vec::new();
    for agent in requested {
        if !ordered.contains(agent) {
            ordered.push(agent.clone());
        }
    }
    ordered
}

fn find_candidate(collected: &Collected, row_id: &str) -> Option<Candidate> {
    collected
        .values()
        .find(|candidate| discovery::stable_id(&candidate.scope, &candidate.target) == row_id)
        .cloned()
}

pub fn valid_name(name: &Path) -> bool {
    let bytes = name.as_os_str().as_bytes();
    !bytes.is_empty()
        && bytes != b"."
        && bytes != b".."
        && !bytes.contains(&b'/')
        && !bytes.contains(&0)
}

fn inside(root: &Path, path: &Path) -> bool {
    path.parent()
        .map(realpath)
        .is_some_and(|parent| parent == realpath(root))
}

fn scoped_roots(environment: &Environment, scope: &str) -> Vec<(&'static Root, PathBuf)> {
    discovery::resolve_roots(environment)
        .into_iter()
        .filter(|(entry, _)| entry.kind == scope)
        .collect()
}

fn agent_roots(environment: &Environment, agent: &str, scope: &str) -> Vec<PathBuf> {
    let mut seen: Vec<PathBuf> = Vec::new();
    for (entry, path) in scoped_roots(environment, scope) {
        if entry.agent == agent && !seen.contains(&path) {
            seen.push(path);
        }
    }
    seen
}

fn resolve_root(environment: &Environment, project_root: &Path, entry: &Root) -> PathBuf {
    if entry.anchor == "home" {
        environment.home.join(entry.path)
    } else {
        project_root.join(entry.path)
    }
}

fn primary_root(
    environment: &Environment,
    project_root: &Path,
    agent: &str,
    scope: &str,
) -> PathBuf {
    registry::ROOTS
        .iter()
        .find(|entry| entry.agent == agent && entry.kind == scope)
        .map(|entry| resolve_root(environment, project_root, entry))
        .unwrap_or_default()
}

fn readers(environment: &Environment, scope: &str, roots: &[PathBuf]) -> Vec<&'static str> {
    let found: Vec<&str> = scoped_roots(environment, scope)
        .into_iter()
        .filter(|(_, path)| roots.contains(path))
        .map(|(entry, _)| entry.agent)
        .collect();
    registry::agents()
        .into_iter()
        .filter(|agent| found.contains(agent))
        .collect()
}

fn shared_note(environment: &Environment, agent: &str, scope: &str, roots: &[PathBuf]) -> String {
    let others: Vec<&str> = readers(environment, scope, roots)
        .into_iter()
        .filter(|reader| *reader != agent)
        .collect();
    if others.is_empty() {
        String::new()
    } else {
        format!("; also applies to {}", others.join(", "))
    }
}

fn entries_for(root: &Path, target: &Path) -> Result<Vec<PathBuf>, String> {
    let mut plan = WatchPlan::new();
    let (names, truncated) = bounded_names(&mut plan, root, MAX_ENTRIES_PER_ROOT as isize);
    if truncated {
        return Err("skill root exceeds the entry limit; nothing was changed".to_string());
    }
    Ok(names
        .into_iter()
        .map(|name| root.join(name))
        .filter(|child| realpath(child) == target)
        .collect())
}

fn text(path: &Path) -> String {
    crate::common::display_path(path)
}

fn strerror(error: &std::io::Error) -> String {
    let rendered = error.to_string();
    match rendered.split_once(" (os error ") {
        Some((message, _)) => message.to_string(),
        None => rendered,
    }
}

fn link_on(
    environment: &Environment,
    agent: &str,
    scope: &str,
    root: &Path,
    name: &Path,
    target: &Path,
) -> Outcome {
    let link = root.join(name);
    let note = shared_note(
        environment,
        agent,
        scope,
        std::slice::from_ref(&root.to_path_buf()),
    );
    if let Err(error) = std::fs::create_dir_all(root) {
        return refusal(
            agent,
            format!("cannot create {}: {}", text(root), strerror(&error)),
        );
    }
    if !inside(root, &link) {
        return refusal(
            agent,
            format!("{} would escape {}", text(&link), text(root)),
        );
    }
    if std::fs::symlink_metadata(&link).is_ok() {
        if !std::fs::symlink_metadata(&link).is_ok_and(|metadata| metadata.is_symlink()) {
            return refusal(
                agent,
                format!(
                    "{} exists and is not a symlink; nothing was changed",
                    text(&link)
                ),
            );
        }
        if realpath(&link) == target {
            return settled(
                agent,
                false,
                format!("{} already points at {}{note}", text(&link), text(target)),
                Vec::new(),
            );
        }
        return refusal(
            agent,
            format!(
                "{} is a symlink to {}, not to this skill; nothing was changed",
                text(&link),
                text(&realpath(&link))
            ),
        );
    }
    if let Err(error) = create_link(&link, target) {
        return refusal(agent, format!("cannot link {}: {error}", text(&link)));
    }
    settled(
        agent,
        true,
        format!("linked {} -> {}{note}", text(&link), text(target)),
        vec![link],
    )
}

fn create_link(link: &Path, target: &Path) -> Result<(), String> {
    let parent = link
        .parent()
        .map(realpath)
        .ok_or_else(|| "path has no parent".to_string())?;
    let resolved = std::fs::canonicalize(target).map_err(|error| error.to_string())?;
    crate::companion_mutations::link(link, &parent, &resolved).map_err(|error| error.to_string())
}

fn remove_link(entry: &Path, target: &Path) -> Result<(), String> {
    let parent = std::fs::canonicalize(
        entry
            .parent()
            .ok_or_else(|| "path has no parent".to_string())?,
    )
    .map_err(|error| error.to_string())?;
    let current = std::fs::symlink_metadata(entry).map_err(|error| error.to_string())?;
    if !current.is_symlink() {
        return Err("selected link changed; nothing was removed".to_string());
    }
    let destination = std::fs::read_link(entry).map_err(|error| error.to_string())?;
    let pointed = if destination.is_absolute() {
        realpath(&destination)
    } else {
        realpath(&parent.join(destination))
    };
    if pointed != realpath(target) {
        return Err("selected link points elsewhere; nothing was removed".to_string());
    }
    use std::os::unix::fs::MetadataExt;
    crate::companion_mutations::unlink(entry, &parent, current.dev(), current.ino())
        .map_err(|error| error.to_string())
}

fn unlink_off(
    environment: &Environment,
    agent: &str,
    scope: &str,
    roots: &[PathBuf],
    target: &Path,
) -> Outcome {
    let mut found: Vec<(PathBuf, PathBuf)> = Vec::new();
    for root in roots {
        match entries_for(root, target) {
            Ok(entries) => {
                for entry in entries {
                    found.push((root.clone(), entry));
                }
            }
            Err(message) => return refusal(agent, message),
        }
    }
    if found.is_empty() {
        return settled(
            agent,
            false,
            format!("no entry for {agent} in its {scope} roots"),
            Vec::new(),
        );
    }
    if let Some((_, entry)) = found
        .iter()
        .find(|(_, entry)| !std::fs::symlink_metadata(entry).is_ok_and(|data| data.is_symlink()))
    {
        return refusal(
            agent,
            format!(
                "{} is a real directory, the only copy of this skill; refusing to delete it",
                text(entry)
            ),
        );
    }
    let mut owners: Vec<PathBuf> = found.iter().map(|(root, _)| root.clone()).collect();
    owners.sort();
    owners.dedup();
    let note = shared_note(environment, agent, scope, &owners);
    let mut removed: Vec<PathBuf> = Vec::new();
    for (_, entry) in &found {
        if let Err(error) = remove_link(entry, target) {
            return Outcome {
                agent: agent.to_string(),
                ok: false,
                changed: !removed.is_empty(),
                message: format!("cannot unlink {}: {error}", text(entry)),
                touched: removed,
            };
        }
        removed.push(entry.clone());
    }
    let listed: Vec<String> = removed.iter().map(|entry| text(entry)).collect();
    settled(
        agent,
        true,
        format!("unlinked {}{note}", listed.join(", ")),
        removed,
    )
}

fn outcome(
    environment: &Environment,
    project_root: &Path,
    candidate: Option<&Candidate>,
    agent: &str,
    state: &str,
) -> Outcome {
    if registry::label(agent).is_none() {
        return refusal(agent, format!("unknown agent id {agent}"));
    }
    let Some(candidate) = candidate else {
        return refusal(agent, "unknown row id".to_string());
    };
    let scope = candidate.scope.as_str();
    if !APPLICABLE_SCOPES.contains(&scope) {
        return refusal(
            agent,
            format!("{scope} rows cannot be applied; only user and project rows can"),
        );
    }
    if scope == "project" && project_root.as_os_str().is_empty() {
        return refusal(agent, "no project directory resolved".to_string());
    }
    let Some(name) = candidate.path.parent().and_then(Path::file_name) else {
        return refusal(
            agent,
            "skill directory name '' is not a single path component".to_string(),
        );
    };
    if !valid_name(Path::new(name)) {
        return refusal(
            agent,
            format!(
                "skill directory name '{}' is not a single path component",
                text(Path::new(name))
            ),
        );
    }
    let target = candidate
        .path
        .parent()
        .map(realpath)
        .unwrap_or_else(|| PathBuf::from("/"));
    let roots = agent_roots(environment, agent, scope);
    if roots.is_empty() {
        return refusal(agent, format!("{agent} documents no {scope} skill root"));
    }
    if state == "on" {
        if candidate.agents.contains(agent) {
            return settled(
                agent,
                false,
                format!("already applied to {agent}"),
                Vec::new(),
            );
        }
        let root = primary_root(environment, project_root, agent, scope);
        return link_on(environment, agent, scope, &root, Path::new(name), &target);
    }
    if !candidate.agents.contains(agent) {
        return settled(agent, false, format!("not applied to {agent}"), Vec::new());
    }
    unlink_off(environment, agent, scope, &roots, &target)
}

pub fn apply(
    environment: &Environment,
    row_id: &str,
    requested: &[String],
    state: &str,
) -> Map<String, Value> {
    let (project_root, _) = discovery::chain_for(environment);
    let mut plan = WatchPlan::new();
    let mut budget = discovery::Budget::default();
    let mut candidate = find_candidate(
        &discovery::candidates(&mut plan, environment, &mut budget),
        row_id,
    );
    let mut results: Vec<Outcome> = Vec::new();
    for agent in requested_agents(requested) {
        if results.last().is_some_and(|last| last.changed) {
            let mut plan = WatchPlan::new();
            let mut budget = discovery::Budget::default();
            candidate = find_candidate(
                &discovery::candidates(&mut plan, environment, &mut budget),
                row_id,
            );
        }
        results.push(outcome(
            environment,
            &project_root,
            candidate.as_ref(),
            &agent,
            state,
        ));
    }
    let mut document = Map::new();
    document.insert(
        "ok".to_string(),
        json!(results.iter().all(|result| result.ok)),
    );
    document.insert("schemaVersion".to_string(), json!(1));
    document.insert("project".to_string(), json!(path_text(&project_root)));
    document.insert("id".to_string(), json!(row_id));
    document.insert("state".to_string(), json!(state));
    document.insert(
        "results".to_string(),
        Value::Array(results.iter().map(Outcome::row).collect()),
    );
    document
}
