use super::common::MAX_RULE_DEPTH;
use super::common::{
    Budget, MAX_ENV_DIRS, MAX_FALLBACK_NAMES, MAX_INSTRUCTION_ENTRIES, MAX_SOURCES, ancestors_of,
    bounded_directories, env_path, env_path_value, expanded, expanded_os, is_remote, listed_files,
    load_json_document, load_toml_document, matches_any, safe_basename, walked_files,
};
use super::discovery::Context;
use crate::core_modules::glob::bounded_glob;
use crate::core_modules::watch::WatchPlan;
use serde_json::Value;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};

pub const AGENT_IDS: [&str; 6] = [
    "claude-code",
    "codex",
    "opencode",
    "pi",
    "copilot-cli",
    "antigravity",
];

const MANAGED_CLAUDE_MD: [&str; 2] = [
    "/etc/claude-code/CLAUDE.md",
    "/Library/Application Support/ClaudeCode/CLAUDE.md",
];

const MARKDOWN: [&str; 1] = [".md"];

#[derive(Clone)]
pub struct Claim {
    pub path: PathBuf,
    pub agent: &'static str,
    pub kind: &'static str,
    pub scope: &'static str,
    pub load_order: i64,
    pub discovery: &'static str,
    pub note: String,
    pub excluded: bool,
    pub inline: Option<u64>,
}

pub fn claim(
    path: PathBuf,
    agent: &'static str,
    kind: &'static str,
    scope: &'static str,
    order: i64,
    note: &str,
) -> Claim {
    Claim {
        path,
        agent,
        kind,
        scope,
        load_order: order,
        discovery: "documented",
        note: note.to_string(),
        excluded: false,
        inline: None,
    }
}

fn user_lane(context: &Context) -> bool {
    context.scope != "project"
}

pub struct ClaudeSettings {
    pub excludes: Vec<String>,
    pub auto_memory_directory: String,
    pub inline: Vec<(PathBuf, u64)>,
}

fn strings_from(value: Option<&Value>) -> Vec<String> {
    let Some(Value::Array(entries)) = value else {
        return Vec::new();
    };
    entries
        .iter()
        .take(MAX_INSTRUCTION_ENTRIES)
        .filter_map(Value::as_str)
        .map(str::to_string)
        .collect()
}

fn claude_settings_documents(
    plan: &mut WatchPlan,
    context: &Context,
) -> Vec<(PathBuf, Value, &'static str)> {
    let mut candidates: Vec<(PathBuf, &'static str)> = vec![
        (
            PathBuf::from("/etc/claude-code/managed-settings.json"),
            "managed",
        ),
        (context.home.join(".claude").join("settings.json"), "user"),
    ];
    if !context.project_root.as_os_str().is_empty() {
        let base = context.project_root.clone();
        candidates.push((base.join(".claude").join("settings.json"), "project"));
        candidates.push((base.join(".claude").join("settings.local.json"), "local"));
    }
    let mut found = Vec::new();
    for (path, scope) in candidates {
        if let Some(document) = load_json_document(plan, &path)
            && document
                .as_object()
                .is_some_and(|entries| !entries.is_empty())
        {
            found.push((path, document, scope));
        }
    }
    found
}

pub fn claude_settings(plan: &mut WatchPlan, context: &Context) -> ClaudeSettings {
    let mut settings = ClaudeSettings {
        excludes: Vec::new(),
        auto_memory_directory: String::new(),
        inline: Vec::new(),
    };
    for (path, document, scope) in claude_settings_documents(plan, context) {
        settings
            .excludes
            .extend(strings_from(document.get("claudeMdExcludes")));
        if let Some(value) = document.get("autoMemoryDirectory").and_then(Value::as_str)
            && (value.starts_with('/') || value.starts_with("~/"))
        {
            settings.auto_memory_directory = value.to_string();
        }
        if let Some(managed) = document.get("claudeMd").and_then(Value::as_str)
            && !managed.trim().is_empty()
            && scope == "managed"
        {
            settings.inline.push((path, managed.len() as u64));
        }
    }
    settings
}

fn named_in(plan: &mut WatchPlan, directory: &Path, names: &[&str]) -> Vec<PathBuf> {
    plan.watch_path(directory, true);
    let mut found = Vec::new();
    for name in names {
        let candidate = directory.join(name);
        if std::fs::metadata(&candidate).is_ok_and(|data| data.is_file()) {
            found.push(candidate);
        }
    }
    found
}

fn chain_from(
    plan: &mut WatchPlan,
    root: &Path,
    cwd: &Path,
    names: &[&str],
) -> Vec<(PathBuf, i64)> {
    if root.as_os_str().is_empty() {
        return Vec::new();
    }
    let mut chain = ancestors_of(cwd, Some(root));
    chain.reverse();
    let mut results = Vec::new();
    for (order, directory) in chain.into_iter().enumerate() {
        for candidate in named_in(plan, &directory, names) {
            results.push((candidate, order as i64));
        }
    }
    results
}

pub fn claude_code(plan: &mut WatchPlan, context: &Context, budget: &mut Budget) -> Vec<Claim> {
    let settings = claude_settings(plan, context);
    let mut claims: Vec<Claim> = Vec::new();
    if user_lane(context) {
        for (path, size) in &settings.inline {
            let mut entry = claim(
                path.clone(),
                "claude-code",
                "instructions",
                "managed",
                1,
                "inline claudeMd",
            );
            entry.inline = Some(*size);
            claims.push(entry);
        }
        for managed in MANAGED_CLAUDE_MD {
            let path = PathBuf::from(managed);
            plan.watch_path(&path, false);
            if std::fs::metadata(&path).is_ok_and(|data| data.is_file()) {
                claims.push(claim(path, "claude-code", "instructions", "managed", 0, ""));
            }
        }
        for candidate in named_in(plan, &context.home.join(".claude"), &["CLAUDE.md"]) {
            claims.push(claim(
                candidate,
                "claude-code",
                "instructions",
                "user",
                10,
                "",
            ));
        }
        for candidate in listed_files(plan, &context.home.join(".claude").join("rules"), &MARKDOWN)
        {
            claims.push(claim(candidate, "claude-code", "rules", "user", 11, ""));
        }
    }
    for (candidate, order) in chain_from(
        plan,
        &context.project_root,
        &context.cwd,
        &["CLAUDE.md", "CLAUDE.local.md"],
    ) {
        let scope = if candidate
            .file_name()
            .is_some_and(|name| name == "CLAUDE.local.md")
        {
            "local"
        } else {
            "project"
        };
        claims.push(claim(
            candidate,
            "claude-code",
            "instructions",
            scope,
            20 + order,
            "",
        ));
    }
    if !context.project_root.as_os_str().is_empty() {
        let base = context.project_root.clone();
        for candidate in named_in(plan, &base.join(".claude"), &["CLAUDE.md"]) {
            claims.push(claim(
                candidate,
                "claude-code",
                "instructions",
                "project",
                20,
                "",
            ));
        }
        for candidate in walked_files(
            plan,
            &base.join(".claude").join("rules"),
            &MARKDOWN,
            MAX_RULE_DEPTH,
        ) {
            claims.push(claim(candidate, "claude-code", "rules", "project", 21, ""));
        }
    }
    if user_lane(context) {
        claims.extend(claude_auto_memory(plan, context, &settings, budget));
    }
    if !settings.excludes.is_empty() {
        for entry in &mut claims {
            if entry.inline.is_none() && matches_any(&entry.path, &settings.excludes) {
                entry.excluded = true;
            }
        }
    }
    claims
}

pub fn auto_memory_roots(
    plan: &mut WatchPlan,
    context: &Context,
    settings: &ClaudeSettings,
) -> Vec<PathBuf> {
    if !settings.auto_memory_directory.is_empty() {
        return vec![expanded(&settings.auto_memory_directory)];
    }
    let projects = context.home.join(".claude").join("projects");
    bounded_directories(plan, &projects, MAX_SOURCES, true)
        .into_iter()
        .map(|entry| entry.join("memory"))
        .collect()
}

fn claude_auto_memory(
    plan: &mut WatchPlan,
    context: &Context,
    settings: &ClaudeSettings,
    budget: &mut Budget,
) -> Vec<Claim> {
    let mut claims = Vec::new();
    for memory in auto_memory_roots(plan, context, settings) {
        if !budget.take_source() {
            break;
        }
        let label = if memory.file_name().is_some_and(|name| name == "memory") {
            memory.parent().and_then(Path::file_name)
        } else {
            memory.file_name()
        }
        .map(|name| crate::common::display_path(Path::new(name)))
        .unwrap_or_default();
        let index = memory.join("MEMORY.md");
        plan.watch_path(&index, false);
        if !std::fs::metadata(&index).is_ok_and(|data| data.is_file()) {
            continue;
        }
        claims.push(claim(
            index,
            "claude-code",
            "auto-memory",
            "user",
            30,
            &label,
        ));
        for candidate in listed_files(plan, &memory, &MARKDOWN) {
            if candidate
                .file_name()
                .is_some_and(|name| name != "MEMORY.md")
            {
                claims.push(claim(
                    candidate,
                    "claude-code",
                    "auto-memory",
                    "user",
                    31,
                    &label,
                ));
            }
        }
    }
    claims
}

pub fn codex_fallback_names(plan: &mut WatchPlan, codex_home: &Path) -> Vec<String> {
    let Some(document) = load_toml_document(plan, &codex_home.join("config.toml")) else {
        return Vec::new();
    };
    let Some(values) = document
        .get("project_doc_fallback_filenames")
        .and_then(toml::Value::as_array)
    else {
        return Vec::new();
    };
    let mut names: Vec<String> = Vec::new();
    for value in values.iter().take(MAX_FALLBACK_NAMES) {
        let name = value.as_str().map(safe_basename).unwrap_or_default();
        if !name.is_empty() && !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

pub fn codex(plan: &mut WatchPlan, context: &Context, _: &mut Budget) -> Vec<Claim> {
    let codex_home =
        env_path("CODEX_HOME", &context.environ).unwrap_or_else(|| context.home.join(".codex"));
    let mut claims = Vec::new();
    if user_lane(context) {
        for (name, order) in [("AGENTS.override.md", 0), ("AGENTS.md", 1)] {
            if let Some(candidate) = named_in(plan, &codex_home, &[name]).into_iter().next() {
                claims.push(claim(
                    candidate,
                    "codex",
                    "instructions",
                    "user",
                    10 + order,
                    "",
                ));
            }
        }
    }
    let documented = ["AGENTS.override.md", "AGENTS.md"];
    for (candidate, order) in chain_from(plan, &context.project_root, &context.cwd, &documented) {
        claims.push(claim(
            candidate,
            "codex",
            "instructions",
            "project",
            20 + order,
            "",
        ));
    }
    let fallbacks: Vec<String> = codex_fallback_names(plan, &codex_home)
        .into_iter()
        .filter(|name| !documented.contains(&name.as_str()))
        .collect();
    if !fallbacks.is_empty() {
        let names: Vec<&str> = fallbacks.iter().map(String::as_str).collect();
        for (candidate, order) in chain_from(plan, &context.project_root, &context.cwd, &names) {
            claims.push(claim(
                candidate,
                "codex",
                "instructions",
                "project",
                40 + order,
                "config fallback name",
            ));
        }
    }
    claims
}

pub fn opencode(plan: &mut WatchPlan, context: &Context, budget: &mut Budget) -> Vec<Claim> {
    let config_home = env_path("XDG_CONFIG_HOME", &context.environ)
        .unwrap_or_else(|| context.home.join(".config"));
    let mut claims = Vec::new();
    if user_lane(context) {
        for candidate in named_in(plan, &config_home.join("opencode"), &["AGENTS.md"]) {
            claims.push(claim(candidate, "opencode", "instructions", "user", 10, ""));
        }
        for candidate in named_in(plan, &context.home.join(".claude"), &["CLAUDE.md"]) {
            claims.push(claim(
                candidate,
                "opencode",
                "instructions",
                "user",
                11,
                "claude-code fallback",
            ));
        }
    }
    for (candidate, order) in chain_from(
        plan,
        &context.project_root,
        &context.cwd,
        &["AGENTS.md", "CLAUDE.md"],
    ) {
        claims.push(claim(
            candidate,
            "opencode",
            "instructions",
            "project",
            20 + order,
            "",
        ));
    }
    claims.extend(opencode_instructions(plan, context, &config_home, budget));
    claims
}

fn opencode_instructions(
    plan: &mut WatchPlan,
    context: &Context,
    config_home: &Path,
    budget: &mut Budget,
) -> Vec<Claim> {
    let mut sources: Vec<(PathBuf, &'static str)> = Vec::new();
    if user_lane(context) {
        sources.push((config_home.join("opencode").join("opencode.json"), "user"));
    }
    if !context.project_root.as_os_str().is_empty() {
        sources.push((context.project_root.join("opencode.json"), "project"));
    }
    let mut claims = Vec::new();
    for (path, scope) in sources {
        let Some(document) = load_json_document(plan, &path) else {
            continue;
        };
        let Some(Value::Array(entries)) = document.get("instructions") else {
            continue;
        };
        let entries: Vec<Value> = entries
            .iter()
            .take(MAX_INSTRUCTION_ENTRIES)
            .cloned()
            .collect();
        if !budget.take_source() {
            continue;
        }
        let mut remote = 0usize;
        let base = path.parent().map(Path::to_path_buf).unwrap_or_default();
        for entry in entries {
            let Some(entry) = entry.as_str() else {
                continue;
            };
            if is_remote(entry) {
                remote += 1;
                continue;
            }
            for candidate in bounded_glob(plan, &base, entry) {
                claims.push(claim(
                    candidate,
                    "opencode",
                    "instructions",
                    scope,
                    30,
                    "instructions entry",
                ));
            }
        }
        if remote > 0 {
            let mut marker = claim(
                path,
                "opencode",
                "instructions",
                scope,
                31,
                &format!("{remote} remote entries not fetched"),
            );
            marker.inline = Some(0);
            claims.push(marker);
        }
    }
    claims
}

pub fn pi(plan: &mut WatchPlan, context: &Context, _: &mut Budget) -> Vec<Claim> {
    let agent_home = context.home.join(".pi").join("agent");
    let mut claims = Vec::new();
    if user_lane(context) {
        for (name, kind) in [
            ("AGENTS.md", "instructions"),
            ("CLAUDE.md", "instructions"),
            ("SYSTEM.md", "system-prompt"),
            ("APPEND_SYSTEM.md", "system-prompt"),
        ] {
            for candidate in named_in(plan, &agent_home, &[name]) {
                claims.push(claim(candidate, "pi", kind, "user", 10, ""));
            }
        }
    }
    for (candidate, order) in chain_from(
        plan,
        &context.project_root,
        &context.cwd,
        &["AGENTS.override.md", "AGENTS.md", "CLAUDE.md"],
    ) {
        claims.push(claim(
            candidate,
            "pi",
            "instructions",
            "project",
            20 + order,
            "",
        ));
    }
    if !context.project_root.as_os_str().is_empty() {
        let base = context.project_root.join(".pi");
        for name in ["SYSTEM.md", "APPEND_SYSTEM.md"] {
            for candidate in named_in(plan, &base, &[name]) {
                claims.push(claim(candidate, "pi", "system-prompt", "project", 25, ""));
            }
        }
    }
    claims
}

fn copilot_extra_dirs(plan: &mut WatchPlan, context: &Context) -> Vec<PathBuf> {
    let raw = env_path_value("COPILOT_CUSTOM_INSTRUCTIONS_DIRS", &context.environ);
    let mut found = Vec::new();
    for piece in raw
        .as_bytes()
        .split(|byte| *byte == b',')
        .take(MAX_ENV_DIRS)
    {
        if piece.is_empty() {
            continue;
        }
        let candidate = expanded_os(OsStr::from_bytes(piece));
        plan.watch_path(&candidate, true);
        if std::fs::metadata(&candidate).is_ok_and(|data| data.is_dir()) {
            found.push(candidate);
        }
    }
    found
}

fn instruction_files(plan: &mut WatchPlan, directory: &Path) -> Vec<PathBuf> {
    walked_files(plan, directory, &MARKDOWN, MAX_RULE_DEPTH)
        .into_iter()
        .filter(|candidate| {
            candidate
                .file_name()
                .is_some_and(|name| name.as_bytes().ends_with(b".instructions.md"))
        })
        .collect()
}

pub fn copilot_cli(plan: &mut WatchPlan, context: &Context, budget: &mut Budget) -> Vec<Claim> {
    let copilot_home =
        env_path("COPILOT_HOME", &context.environ).unwrap_or_else(|| context.home.join(".copilot"));
    let mut claims = Vec::new();
    if user_lane(context) {
        for candidate in named_in(plan, &copilot_home, &["copilot-instructions.md"]) {
            claims.push(claim(
                candidate,
                "copilot-cli",
                "instructions",
                "user",
                10,
                "",
            ));
        }
        for candidate in instruction_files(plan, &copilot_home.join("instructions")) {
            claims.push(claim(
                candidate,
                "copilot-cli",
                "instructions",
                "user",
                11,
                "",
            ));
        }
        for directory in copilot_extra_dirs(plan, context) {
            if !budget.take_source() {
                break;
            }
            for candidate in named_in(plan, &directory, &["AGENTS.md"]) {
                claims.push(claim(
                    candidate,
                    "copilot-cli",
                    "instructions",
                    "user",
                    12,
                    "custom instructions dir",
                ));
            }
            for candidate in
                instruction_files(plan, &directory.join(".github").join("instructions"))
            {
                claims.push(claim(
                    candidate,
                    "copilot-cli",
                    "instructions",
                    "user",
                    12,
                    "custom instructions dir",
                ));
            }
        }
    }
    if !context.project_root.as_os_str().is_empty() {
        let base = context.project_root.clone();
        for candidate in named_in(plan, &base.join(".github"), &["copilot-instructions.md"]) {
            claims.push(claim(
                candidate,
                "copilot-cli",
                "instructions",
                "project",
                20,
                "",
            ));
        }
        for candidate in instruction_files(plan, &base.join(".github").join("instructions")) {
            claims.push(claim(
                candidate,
                "copilot-cli",
                "instructions",
                "project",
                21,
                "",
            ));
        }
    }
    for (candidate, order) in chain_from(
        plan,
        &context.project_root,
        &context.cwd,
        &["AGENTS.md", "CLAUDE.md", "GEMINI.md"],
    ) {
        claims.push(claim(
            candidate,
            "copilot-cli",
            "instructions",
            "project",
            22 + order,
            "",
        ));
    }
    if !context.project_root.as_os_str().is_empty() {
        for candidate in named_in(plan, &context.project_root.join(".claude"), &["CLAUDE.md"]) {
            claims.push(claim(
                candidate,
                "copilot-cli",
                "instructions",
                "project",
                23,
                "",
            ));
        }
    }
    claims
}

pub fn antigravity(plan: &mut WatchPlan, context: &Context, budget: &mut Budget) -> Vec<Claim> {
    let mut claims = Vec::new();
    if user_lane(context) {
        for candidate in named_in(plan, &context.home.join(".gemini"), &["GEMINI.md"]) {
            claims.push(claim(
                candidate,
                "antigravity",
                "instructions",
                "user",
                10,
                "",
            ));
        }
    }
    if !context.project_root.as_os_str().is_empty() {
        let base = context.project_root.clone();
        for folder in [".agents/rules", ".agent/rules"] {
            for candidate in walked_files(plan, &base.join(folder), &MARKDOWN, MAX_RULE_DEPTH) {
                claims.push(claim(candidate, "antigravity", "rules", "project", 20, ""));
            }
        }
    }
    let plugins = context
        .home
        .join(".gemini")
        .join("antigravity-cli")
        .join("plugins");
    let directories = if user_lane(context) {
        bounded_directories(plan, &plugins, MAX_SOURCES, true)
    } else {
        Vec::new()
    };
    for directory in directories {
        if !budget.take_source() {
            break;
        }
        let label = directory
            .file_name()
            .map(|name| crate::common::display_path(Path::new(name)))
            .unwrap_or_default();
        for candidate in walked_files(plan, &directory.join("rules"), &MARKDOWN, MAX_RULE_DEPTH) {
            claims.push(claim(
                candidate,
                "antigravity",
                "rules",
                "plugin",
                30,
                &label,
            ));
        }
    }
    claims
}

type Adapter = fn(&mut WatchPlan, &Context, &mut Budget) -> Vec<Claim>;

pub const ADAPTERS: [Adapter; 6] = [claude_code, codex, opencode, pi, copilot_cli, antigravity];
