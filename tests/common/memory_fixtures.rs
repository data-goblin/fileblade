#![allow(dead_code)]

use fileblade::core_modules::memory::common::Environ;
use fileblade::core_modules::memory::discovery;
use fileblade::core_modules::watch::WatchPlan;
use serde_json::{Map, Value};
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

pub const AUDIT_CANARY: &str = "CANARY-CONFIG-CONTENT";

pub fn write(path: &Path, text: &str) -> PathBuf {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
    path.to_path_buf()
}

pub fn write_default(path: &Path) -> PathBuf {
    write(path, "# heading\n\nbody\n")
}

pub fn environ(home: &Path) -> Environ {
    vec![(OsString::from("HOME"), OsString::from(home))]
}

pub fn environ_with(home: &Path, extra: &[(&str, &str)]) -> Environ {
    let mut variables = environ(home);
    for (key, value) in extra {
        variables.push((OsString::from(*key), OsString::from(*value)));
    }
    variables
}

pub fn collect(
    home: &Path,
    project: &Path,
    config: &str,
    variables: Environ,
) -> Map<String, Value> {
    let mut plan = WatchPlan::new();
    discovery::collect(
        &mut plan,
        &project.to_string_lossy(),
        &home.to_string_lossy(),
        config,
        variables,
        false,
        "all",
    )
}

pub fn collect_scoped(
    home: &Path,
    project: &Path,
    scope: &str,
) -> (Map<String, Value>, Vec<PathBuf>) {
    let mut plan = WatchPlan::new();
    let mut document = discovery::collect(
        &mut plan,
        &project.to_string_lossy(),
        &home.to_string_lossy(),
        "",
        environ(home),
        false,
        scope,
    );
    let paths = plan.paths();
    plan.finish(&mut document);
    (document, paths)
}

pub fn payload(home: &Path, project: &Path) -> Map<String, Value> {
    collect(home, project, "", environ(home))
}

pub fn rows(document: &Map<String, Value>) -> Vec<&Value> {
    document
        .get("items")
        .and_then(Value::as_array)
        .map(|items| items.iter().collect())
        .unwrap_or_default()
}

pub fn by_name<'a>(document: &'a Map<String, Value>, name: &str) -> Option<&'a Value> {
    rows(document)
        .into_iter()
        .find(|row| row.get("name").and_then(Value::as_str) == Some(name))
}

pub fn field<'a>(row: &'a Value, key: &str) -> &'a str {
    row.get(key).and_then(Value::as_str).unwrap_or_default()
}

pub fn readers_of(row: &Value) -> Vec<String> {
    let mut agents: Vec<String> = row
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
    agents.sort();
    agents
}

pub fn agents_for(document: &Map<String, Value>, name: &str) -> Vec<String> {
    by_name(document, name).map(readers_of).unwrap_or_default()
}

pub fn badges_of(row: &Value) -> Vec<String> {
    row.get("badges")
        .and_then(Value::as_array)
        .map(|entries| {
            entries
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

pub fn build_home(home: &Path) {
    write(&home.join(".claude/CLAUDE.md"), "# user claude\n");
    write(
        &home.join(".claude/rules/style.md"),
        "---\npaths:\n  - \"src/**\"\n---\n# style\n",
    );
    write(
        &home.join(".claude/projects/repo/memory/MEMORY.md"),
        "# index\n",
    );
    write(
        &home.join(".claude/projects/repo/memory/feedback_tests.md"),
        "---\ntype: feedback\nmodified: 2026-08-31T00:00:00Z\n---\nprefers pytest\n",
    );
    write(&home.join(".claude/projects/repo/session.jsonl"), "{}\n");
    write(&home.join(".codex/AGENTS.md"), "# codex user\n");
    write(
        &home.join(".config/opencode/AGENTS.md"),
        "# opencode global\n",
    );
    write(&home.join(".pi/agent/SYSTEM.md"), "# pi system\n");
    write(
        &home.join(".copilot/copilot-instructions.md"),
        "# copilot user\n",
    );
    write(
        &home.join(".copilot/instructions/team.instructions.md"),
        "# copilot modular\n",
    );
    write(&home.join(".gemini/GEMINI.md"), "# antigravity global\n");
    write(
        &home.join(".gemini/antigravity-cli/plugins/demo/rules/one.md"),
        "# plugin rule\n",
    );
    fs::create_dir_all(home.join(".codex")).unwrap();
    fs::write(
        home.join(".codex/memories_1.sqlite"),
        b"SQLite format 3\x00",
    )
    .unwrap();
}

pub fn build_project(root: &Path) {
    fs::create_dir_all(root.join(".git")).unwrap();
    write(&root.join("AGENTS.md"), "# shared agents\n");
    write(&root.join("CLAUDE.md"), "@AGENTS.md\n\n# claude project\n");
    write(&root.join("CLAUDE.local.md"), "# local only\n");
    write(&root.join(".claude/rules/api.md"), "# api rules\n");
    write(
        &root.join(".agents/rules/workspace.md"),
        "# antigravity workspace\n",
    );
    write(
        &root.join(".github/copilot-instructions.md"),
        "# repo copilot\n",
    );
    write(&root.join("GEMINI.md"), "# gemini project\n");
}

pub fn build_audit_fixtures(home: &Path, project: &Path) {
    write(
        &home.join(".codex/config.toml"),
        &[
            "model = \"gpt-5\"",
            "project_doc_max_bytes = 65536",
            "project_doc_fallback_filenames = [\"CONTRIBUTING.md\", \"../escape.md\", \"ok.md\", \".hidden\"]",
            &format!("instructions = \"{AUDIT_CANARY}\""),
            "",
        ]
        .join("\n"),
    );
    write(
        &project.join("CONTRIBUTING.md"),
        "# contributing fallback\n",
    );
    write(&project.join("ok.md"), "# second fallback\n");

    write(
        &home.join(".claude/settings.json"),
        &serde_json::json!({
            "claudeMdExcludes": [
                project.join("CLAUDE.md").to_string_lossy(),
                project.join(".claude/rules/api.md").to_string_lossy(),
            ],
            "apiKeyHelper": AUDIT_CANARY,
        })
        .to_string(),
    );
    write(
        &project.join("CLAUDE.md"),
        "@AGENTS.md\n\n# claude project\n",
    );

    write(
        &home.join(".config/opencode/opencode.json"),
        &serde_json::json!({
            "instructions": [
                "local-guide.md",
                "docs/*.md",
                "https://example.invalid/remote.md",
                "/etc/passwd",
                "../outside.md",
            ],
            "apiKey": AUDIT_CANARY,
        })
        .to_string(),
    );
    write(
        &home.join(".config/opencode/local-guide.md"),
        "# opencode local guide\n",
    );
    write(
        &home.join(".config/opencode/docs/one.md"),
        "# globbed one\n",
    );
    write(
        &home.join(".config/opencode/docs/two.md"),
        "# globbed two\n",
    );
    write(&home.join("outside.md"), "# must not be reached\n");

    write(
        &home.join("copilot-extra/AGENTS.md"),
        "# copilot extra dir\n",
    );
    write(
        &home.join("copilot-extra/.github/instructions/nested/deep.instructions.md"),
        "# copilot nested\n",
    );
}

pub fn build_all(home: &Path, project: &Path) {
    build_home(home);
    build_project(project);
    build_audit_fixtures(home, project);
}
