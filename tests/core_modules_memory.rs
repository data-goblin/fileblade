#[path = "common/memory_fixtures.rs"]
mod memory_fixtures;

use fileblade::core_modules::emit::{MAX_OUTPUT_BYTES, encoded_items};
use fileblade::core_modules::glob::bounded_glob;
use fileblade::core_modules::memory::adapters::{
    ClaudeSettings, auto_memory_roots, codex_fallback_names,
};
use fileblade::core_modules::memory::common::{
    MAX_BASENAME_CHARS, MAX_DESCRIPTOR_BYTES, MAX_ENV_PATH_CHARS, MAX_RULE_DEPTH,
    bounded_directories, env_path_value, safe_basename, stable_id, walked_files,
};
use fileblade::core_modules::memory::discovery::{self, build_context};
use fileblade::core_modules::watch::WatchPlan;
use memory_fixtures::{
    agents_for, badges_of, build_all, by_name, collect, collect_scoped, environ, environ_with,
    field, payload, rows, write, write_default,
};
use serde_json::{Map, Value, json};
use std::collections::BTreeSet;
use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};

struct Sandbox {
    _base: tempfile::TempDir,
    root: PathBuf,
    home: PathBuf,
    project: PathBuf,
}

fn sandbox() -> Sandbox {
    let base = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(base.path()).unwrap();
    let home = root.join("home");
    let project = root.join("work/repo");
    fs::create_dir_all(&project).unwrap();
    build_all(&home, &project);
    Sandbox {
        _base: base,
        root,
        home,
        project,
    }
}

fn paths_of(document: &Map<String, Value>) -> Vec<String> {
    rows(document)
        .into_iter()
        .map(|row| field(row, "path").to_string())
        .collect()
}

#[test]
fn the_document_carries_the_schema_every_kind_and_every_scope() {
    let fixture = sandbox();
    let document = payload(&fixture.home, &fixture.project);
    assert_eq!(document.get("schemaVersion"), Some(&json!(1)));
    assert_eq!(document.get("ok"), Some(&json!(true)));
    assert_eq!(
        document.get("project").and_then(Value::as_str),
        Some(fixture.project.to_str().unwrap())
    );
    let kinds: BTreeSet<&str> = rows(&document)
        .into_iter()
        .map(|row| field(row, "kind"))
        .collect();
    for kind in ["instructions", "rules", "auto-memory", "system-prompt"] {
        assert!(kinds.contains(kind), "{kind} missing from {kinds:?}");
    }
    let scopes: BTreeSet<&str> = rows(&document)
        .into_iter()
        .map(|row| field(row, "scope"))
        .collect();
    for scope in ["user", "project", "local", "plugin"] {
        assert!(scopes.contains(scope), "{scope} missing from {scopes:?}");
    }
}

#[test]
fn the_scope_lanes_split_user_and_project_without_leaking_watch_paths() {
    let fixture = sandbox();
    let (user, user_paths) = collect_scoped(&fixture.home, &fixture.project, "user");
    let (project, project_paths) = collect_scoped(&fixture.home, &fixture.project, "project");
    let both = payload(&fixture.home, &fixture.project);
    let user_scopes: BTreeSet<&str> = rows(&user)
        .into_iter()
        .map(|row| field(row, "scope"))
        .collect();
    assert!(!user_scopes.contains("project") && !user_scopes.contains("local"));
    let project_scopes: BTreeSet<&str> = rows(&project)
        .into_iter()
        .map(|row| field(row, "scope"))
        .collect();
    assert_eq!(
        project_scopes,
        BTreeSet::from(["project", "local"]),
        "the project lane carries only project rows"
    );
    assert_eq!(user.get("project"), Some(&json!("")));
    assert_eq!(
        project.get("project").and_then(Value::as_str),
        Some(fixture.project.to_str().unwrap())
    );
    assert_eq!(rows(&both).len(), rows(&user).len() + rows(&project).len());
    assert!(
        !user_paths
            .iter()
            .any(|path| path.starts_with(&fixture.project)),
        "the user lane never watches inside the project"
    );
    let user_roots = [
        ".copilot",
        ".pi",
        ".config/opencode",
        ".gemini/antigravity-cli",
        ".claude/projects",
        ".claude/rules",
    ]
    .map(|part| fixture.home.join(part));
    assert!(
        !project_paths
            .iter()
            .any(|path| user_roots.iter().any(|root| path.starts_with(root))),
        "the project lane never watches the user roots"
    );
}

#[test]
fn shared_files_record_every_reader_and_no_filename_cross_products() {
    let fixture = sandbox();
    write(&fixture.home.join(".claude/AGENTS.md"), "# not claude\n");
    write(&fixture.home.join(".codex/CLAUDE.md"), "# not codex\n");
    let document = payload(&fixture.home, &fixture.project);
    assert_eq!(
        agents_for(&document, "AGENTS.md"),
        ["codex", "copilot-cli", "opencode", "pi"]
    );
    let claude = agents_for(&document, "CLAUDE.md");
    assert!(
        claude.contains(&"claude-code".to_string()) && claude.contains(&"opencode".to_string())
    );
    for path in paths_of(&document) {
        assert!(!path.ends_with(".claude/AGENTS.md"), "{path}");
        assert!(!path.ends_with(".codex/CLAUDE.md"), "{path}");
    }
    let row = by_name(&document, "AGENTS.md").unwrap();
    let readers = row.get("readers").and_then(Value::as_array).unwrap();
    assert!(
        readers.iter().all(|reader| {
            reader.get("discovery").and_then(Value::as_str) == Some("documented")
        })
    );
    let codex = readers
        .iter()
        .find(|reader| reader.get("agent").and_then(Value::as_str) == Some("codex"))
        .unwrap();
    assert!(codex.get("loadOrder").and_then(Value::as_i64).unwrap() >= 20);
}

#[test]
fn auto_memory_is_its_own_kind_and_private_stores_never_appear() {
    let fixture = sandbox();
    let document = payload(&fixture.home, &fixture.project);
    let index = by_name(&document, "MEMORY.md").unwrap();
    assert_eq!(field(index, "kind"), "auto-memory");
    assert!(badges_of(index).contains(&"agent-written".to_string()));
    let topic = by_name(&document, "feedback_tests.md").unwrap();
    assert_eq!(field(topic, "memoryType"), "feedback");
    assert_eq!(field(topic, "modified"), "2026-08-31T00:00:00Z");
    assert!(by_name(&document, "memories_1.sqlite").is_none());
    assert!(by_name(&document, "session.jsonl").is_none());
    assert_eq!(
        document.get("excludedKinds"),
        Some(&json!([
            "session-transcript",
            "private-database",
            "configuration"
        ]))
    );
}

#[test]
fn a_linked_file_is_one_row_with_its_alias_and_a_stable_identity() {
    let fixture = sandbox();
    let target = write(&fixture.home.join("shared/rules.md"), "# shared rule\n");
    let link = fixture.home.join(".claude/rules/linked.md");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    let first = payload(&fixture.home, &fixture.project);
    let matching: Vec<&Value> = rows(&first)
        .into_iter()
        .filter(|row| field(row, "realpath") == target.to_str().unwrap())
        .collect();
    assert_eq!(matching.len(), 1);
    assert_eq!(matching[0].get("alias"), Some(&json!(true)));
    assert!(badges_of(matching[0]).contains(&"linked".to_string()));
    let second = payload(&fixture.home, &fixture.project);
    let identities = |document: &Map<String, Value>| -> Vec<String> {
        rows(document)
            .into_iter()
            .map(|row| field(row, "id").to_string())
            .collect()
    };
    assert_eq!(identities(&first), identities(&second));
    let row = by_name(&first, "CLAUDE.md").unwrap();
    assert_eq!(
        field(row, "id"),
        stable_id(Path::new(field(row, "realpath")))
    );
}

#[test]
fn extra_roots_come_from_the_config_and_its_legacy_location() {
    let fixture = sandbox();
    let note = write(&fixture.home.join("vaultish/note.md"), "# vault note\n");
    let document = json!({"extraRoots": [{"path": note.parent().unwrap().to_string_lossy(), "label": "vault"}]})
        .to_string();
    let explicit = fixture.home.join("config.json");
    write(&explicit, &document);
    let configured = collect(
        &fixture.home,
        &fixture.project,
        explicit.to_str().unwrap(),
        environ(&fixture.home),
    );
    let row = by_name(&configured, "note.md").unwrap();
    assert_eq!(field(row, "kind"), "extra");
    assert_eq!(field(row, "scope"), "extra");
    assert_eq!(
        row.get("readers").and_then(Value::as_array).unwrap()[0]
            .get("discovery")
            .and_then(Value::as_str),
        Some("user-configured")
    );
    assert!(by_name(&payload(&fixture.home, &fixture.project), "note.md").is_none());

    let config_home = fixture.root.join("xdg");
    write(
        &config_home.join("kurt.agent-memory/config.json"),
        &document,
    );
    let variables = environ_with(
        &fixture.home,
        &[("XDG_CONFIG_HOME", config_home.to_str().unwrap())],
    );
    let legacy = collect(&fixture.home, &fixture.project, "", variables.clone());
    assert!(
        by_name(&legacy, "note.md").is_some(),
        "the legacy kurt.agent-memory config is still read"
    );
    write(
        &config_home.join("data-goblin.fileblade-memory/config.json"),
        &json!({"extraRoots": []}).to_string(),
    );
    let current = collect(&fixture.home, &fixture.project, "", variables);
    assert!(
        by_name(&current, "note.md").is_none(),
        "the current config wins once it exists"
    );
}

#[test]
fn traversal_stops_at_the_rule_depth_and_metrics_stay_bounded() {
    let fixture = sandbox();
    let mut deep = fixture.project.join(".claude/rules");
    for level in 0..MAX_RULE_DEPTH + 3 {
        deep = deep.join(format!("level{level}"));
    }
    write(&deep.join("deep.md"), "# too deep\n");
    assert!(by_name(&payload(&fixture.home, &fixture.project), "deep.md").is_none());

    let text = "# 記憶\n\n漢字 日本語 한국어 кириллица ελληνικά café\n";
    let path = write(
        &fixture.project.join(".claude/rules/測試-память-μνήμη.md"),
        text,
    );
    let document = payload(&fixture.home, &fixture.project);
    let row = by_name(&document, path.file_name().unwrap().to_str().unwrap()).unwrap();
    let metrics = row.get("metrics").unwrap();
    assert_eq!(
        metrics.get("bytes").and_then(Value::as_u64),
        Some(text.len() as u64)
    );
    assert_eq!(
        metrics.get("characters").and_then(Value::as_u64),
        Some(text.chars().count() as u64)
    );
    assert_eq!(metrics.get("words").and_then(Value::as_u64), Some(7));
    assert_eq!(
        metrics.get("tokens").and_then(Value::as_u64),
        Some((text.len() as u64).div_ceil(4))
    );

    let oversized_text = format!("# large\n{}", "界".repeat(MAX_DESCRIPTOR_BYTES));
    let oversized = write(
        &fixture.project.join(".claude/rules/大きい-файл.md"),
        &oversized_text,
    );
    let document = payload(&fixture.home, &fixture.project);
    let row = by_name(&document, oversized.file_name().unwrap().to_str().unwrap()).unwrap();
    let metrics = row.get("metrics").unwrap();
    assert_eq!(
        metrics.get("bytes").and_then(Value::as_u64),
        Some(oversized_text.len() as u64)
    );
    for key in ["characters", "words", "tokens"] {
        assert_eq!(metrics.get(key), Some(&Value::Null), "{key}");
    }
}

#[test]
fn irregular_files_and_a_missing_home_produce_nothing() {
    let fixture = sandbox();
    let fifo = fixture.project.join(".claude/rules/pipe.md");
    fs::create_dir_all(fifo.parent().unwrap()).unwrap();
    rustix::fs::mknodat(
        rustix::fs::CWD,
        &fifo,
        rustix::fs::FileType::Fifo,
        rustix::fs::Mode::from_bits_truncate(0o600),
        0,
    )
    .unwrap();
    assert!(by_name(&payload(&fixture.home, &fixture.project), "pipe.md").is_none());

    let empty = fixture.root.join("nohome");
    let mut plan = WatchPlan::new();
    let document = discovery::collect(
        &mut plan,
        "",
        empty.to_str().unwrap(),
        "",
        Vec::new(),
        false,
        "all",
    );
    assert_eq!(document.get("ok"), Some(&json!(true)));
    assert_eq!(document.get("count"), Some(&json!(0)));
}

#[test]
fn codex_reads_its_configured_fallback_names_and_refuses_the_hazardous_ones() {
    let fixture = sandbox();
    let document = payload(&fixture.home, &fixture.project);
    let contributing = by_name(&document, "CONTRIBUTING.md").unwrap();
    assert!(by_name(&document, "ok.md").is_some());
    assert_eq!(readers(contributing), vec!["codex".to_string()]);
    assert_eq!(field(contributing, "note"), "config fallback name");
    let mut plan = WatchPlan::new();
    assert_eq!(
        codex_fallback_names(&mut plan, &fixture.home.join(".codex")),
        vec![
            "CONTRIBUTING.md".to_string(),
            "ok.md".to_string(),
            ".hidden".to_string()
        ]
    );
}

fn readers(row: &Value) -> Vec<String> {
    memory_fixtures::readers_of(row)
}

#[test]
fn the_configured_auto_memory_directory_replaces_the_project_stores() {
    let fixture = sandbox();
    let alternate = fixture.root.join("alternate-home");
    copy_tree(&fixture.home, &alternate);
    write(
        &alternate.join(".claude/settings.json"),
        &json!({"autoMemoryDirectory": alternate.join("custom-memory").to_string_lossy()})
            .to_string(),
    );
    write(
        &alternate.join("custom-memory/MEMORY.md"),
        "# relocated index\n",
    );
    write(
        &alternate.join("custom-memory/user_role.md"),
        "---\ntype: user\n---\nprefers rust\n",
    );
    let document = collect(&alternate, &fixture.project, "", environ(&alternate));
    let index = by_name(&document, "MEMORY.md").unwrap();
    assert!(field(index, "path").starts_with(alternate.join("custom-memory").to_str().unwrap()));
    assert_eq!(field(index, "kind"), "auto-memory");
    assert_eq!(
        field(by_name(&document, "user_role.md").unwrap(), "memoryType"),
        "user"
    );
    assert!(by_name(&document, "feedback_tests.md").is_none());
}

fn copy_tree(source: &Path, destination: &Path) {
    fs::create_dir_all(destination).unwrap();
    for entry in fs::read_dir(source).unwrap() {
        let entry = entry.unwrap();
        let target = destination.join(entry.file_name());
        let metadata = fs::symlink_metadata(entry.path()).unwrap();
        if metadata.is_dir() {
            copy_tree(&entry.path(), &target);
        } else if metadata.is_symlink() {
            std::os::unix::fs::symlink(fs::read_link(entry.path()).unwrap(), &target).unwrap();
        } else {
            fs::copy(entry.path(), &target).unwrap();
        }
    }
}

#[test]
fn claude_md_excludes_mark_rows_whole_and_partly() {
    let fixture = sandbox();
    let document = payload(&fixture.home, &fixture.project);
    let rule = by_name(&document, "api.md").unwrap();
    assert_eq!(rule.get("excluded"), Some(&json!(true)));
    assert!(badges_of(rule).contains(&"excluded".to_string()));
    let shared = by_name(&document, "CLAUDE.md").unwrap();
    assert_eq!(shared.get("excluded"), Some(&json!(false)));
    assert!(badges_of(shared).contains(&"partly-excluded".to_string()));
    let flags: Vec<(String, bool)> = shared
        .get("readers")
        .and_then(Value::as_array)
        .unwrap()
        .iter()
        .map(|reader| {
            (
                reader
                    .get("agent")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
                reader
                    .get("excluded")
                    .and_then(Value::as_bool)
                    .unwrap_or_default(),
            )
        })
        .collect();
    assert!(flags.contains(&("claude-code".to_string(), true)));
    assert!(flags.contains(&("opencode".to_string(), false)));
    let other = by_name(&document, "CLAUDE.local.md").unwrap();
    assert_eq!(other.get("excluded"), Some(&json!(false)));
    assert!(!badges_of(other).contains(&"excluded".to_string()));
}

#[test]
fn opencode_instruction_entries_stay_local_and_never_fetch() {
    let fixture = sandbox();
    let document = payload(&fixture.home, &fixture.project);
    let guide = by_name(&document, "local-guide.md").unwrap();
    assert_eq!(readers(guide), vec!["opencode".to_string()]);
    assert!(by_name(&document, "one.md").is_some());
    assert!(by_name(&document, "outside.md").is_none());
    assert!(by_name(&document, "passwd").is_none());
    let marker = by_name(&document, "opencode.json").unwrap();
    assert!(field(marker, "note").starts_with("1 remote entries"));
    assert_eq!(field(marker, "detail"), "");
    let blob = serde_json::to_string(&Value::Object(document)).unwrap();
    assert!(!blob.contains(memory_fixtures::AUDIT_CANARY));
    assert!(!blob.contains("example.invalid"));
    assert!(!blob.contains("https://"));
}

#[test]
fn copilot_custom_instruction_directories_are_opt_in() {
    let fixture = sandbox();
    let plain = payload(&fixture.home, &fixture.project);
    let extra_root = fixture.home.join("copilot-extra");
    assert!(
        !paths_of(&plain)
            .iter()
            .any(|path| path.starts_with(extra_root.to_str().unwrap()))
    );
    let variables = environ_with(
        &fixture.home,
        &[(
            "COPILOT_CUSTOM_INSTRUCTIONS_DIRS",
            &format!(
                "{}, , {}",
                extra_root.display(),
                fixture.home.join("missing").display()
            ),
        )],
    );
    let document = collect(&fixture.home, &fixture.project, "", variables);
    let paths = paths_of(&document);
    let agents = extra_root.join("AGENTS.md");
    let row = rows(&document)
        .into_iter()
        .find(|row| field(row, "path") == agents.to_str().unwrap())
        .unwrap();
    assert_eq!(readers(row), vec!["copilot-cli".to_string()]);
    assert_eq!(field(row, "note"), "custom instructions dir");
    assert!(
        paths
            .iter()
            .any(|path| path.ends_with("deep.instructions.md"))
    );
}

#[test]
fn the_antigravity_global_context_is_one_row() {
    let fixture = sandbox();
    let document = payload(&fixture.home, &fixture.project);
    let global = fixture.home.join(".gemini/GEMINI.md");
    let matching: Vec<&Value> = rows(&document)
        .into_iter()
        .filter(|row| field(row, "path") == global.to_str().unwrap())
        .collect();
    assert_eq!(matching.len(), 1);
    assert_eq!(readers(matching[0]), vec!["antigravity".to_string()]);
}

#[test]
fn safe_basename_accepts_atypical_names_and_refuses_hazards() {
    for name in [
        "GEMINI.md",
        ".context.md",
        "notes..md",
        "..leading.md",
        "trailing..",
        "КОНТЕКСТ.md",
        "ΣΥΜΦΡΑΖΌΜΕΝΑ.md",
        "上下文.md",
        "한국어.md",
        "café.md",
        "cafe\u{301}.md",
        "emoji-\u{1f600}.md",
        "e\u{301}\u{328}.md",
        "name with spaces.md",
        "dash-and_underscore.md",
    ] {
        assert_eq!(safe_basename(name), name, "{name}");
    }
    let longest = "x".repeat(MAX_BASENAME_CHARS);
    assert_eq!(safe_basename(&longest), longest);
    for name in [
        "",
        "   ",
        ".",
        "..",
        "a/b.md",
        "a\\b.md",
        "sub/dir.md",
        "../escape.md",
        "nul\0.md",
        "bell\u{7}.md",
        "del\u{7f}.md",
        "c1\u{9f}.md",
        "bom\u{feff}.md",
        "lrm\u{200e}.md",
        "zwsp\u{200b}.md",
    ] {
        assert_eq!(safe_basename(name), "", "{name:?}");
    }
    assert_eq!(safe_basename(&"x".repeat(MAX_BASENAME_CHARS + 1)), "");
}

#[test]
fn symlinked_rule_directories_are_never_traversed() {
    let fixture = sandbox();
    let outside = fixture.root.join("outside-rules");
    write(&outside.join("secret.md"), "# outside rule\n");
    std::os::unix::fs::symlink(&outside, fixture.project.join(".claude/rules/linked")).unwrap();
    let document = payload(&fixture.home, &fixture.project);
    for path in paths_of(&document) {
        assert!(!path.contains("linked"), "{path}");
        assert!(!path.starts_with(outside.to_str().unwrap()), "{path}");
    }
    let linked_root = fixture.root.join("linked-rule-root");
    std::os::unix::fs::symlink(&outside, &linked_root).unwrap();
    let mut plan = WatchPlan::new();
    assert_eq!(
        walked_files(&mut plan, &linked_root, &[".md"], MAX_RULE_DEPTH),
        Vec::<PathBuf>::new()
    );
}

#[test]
fn environment_path_values_keep_spaces_and_refuse_hazards() {
    let named = |value: &str| vec![(OsString::from("X"), OsString::from(value))];
    assert_eq!(env_path_value("X", &named("  spaced  ")), "  spaced  ");
    assert_eq!(env_path_value("X", &named("   ")), "   ");
    assert_eq!(
        env_path_value("X", &named("директория/файл")),
        "директория/файл"
    );
    assert_eq!(env_path_value("X", &Vec::new()), "");
    assert_eq!(env_path_value("X", &named("")), "");
    assert_eq!(env_path_value("X", &named("bad\0path")), "");
    assert_eq!(
        env_path_value("X", &named(&"x".repeat(MAX_ENV_PATH_CHARS + 1))),
        ""
    );

    let fixture = sandbox();
    let codex_home = fixture.root.join("  codex  дом  ");
    write(&codex_home.join("AGENTS.md"), "# spaced codex home\n");
    let document = collect(
        &fixture.home,
        &fixture.project,
        "",
        environ_with(
            &fixture.home,
            &[("CODEX_HOME", codex_home.to_str().unwrap())],
        ),
    );
    assert!(
        paths_of(&document).contains(&codex_home.join("AGENTS.md").to_string_lossy().to_string())
    );

    let copilot = fixture.root.join(" copilot  инструкции ");
    write(
        &copilot.join(".github/instructions/team.instructions.md"),
        "# spaced copilot dir\n",
    );
    let document = collect(
        &fixture.home,
        &fixture.project,
        "",
        environ_with(
            &fixture.home,
            &[(
                "COPILOT_CUSTOM_INSTRUCTIONS_DIRS",
                copilot.to_str().unwrap(),
            )],
        ),
    );
    assert!(
        paths_of(&document).contains(
            &copilot
                .join(".github/instructions/team.instructions.md")
                .to_string_lossy()
                .to_string()
        )
    );
}

#[test]
fn environment_path_values_keep_native_bytes_that_are_not_utf8() {
    use std::os::unix::ffi::OsStringExt;

    let raw = OsString::from_vec(b"/tmp/\xffdir".to_vec());
    let named = vec![(OsString::from("X"), raw.clone())];
    assert_eq!(env_path_value("X", &named), raw);
    let mut long = b"/".to_vec();
    long.extend(std::iter::repeat_n(b'\xff', MAX_ENV_PATH_CHARS));
    assert_eq!(
        env_path_value("X", &vec![(OsString::from("X"), OsString::from_vec(long))]),
        OsString::new()
    );

    let fixture = sandbox();
    let codex_home = fixture.root.join(PathBuf::from(OsString::from_vec(
        b"codex-\xffhome".to_vec(),
    )));
    write(&codex_home.join("AGENTS.md"), "# native byte codex home\n");
    let mut variables = environ(&fixture.home);
    variables.push((
        OsString::from("CODEX_HOME"),
        codex_home.as_os_str().to_os_string(),
    ));
    let document = collect(&fixture.home, &fixture.project, "", variables);
    assert!(
        paths_of(&document).contains(&fileblade::common::path_text(&codex_home.join("AGENTS.md")))
    );
}

#[test]
fn the_listing_is_bounded_to_the_output_limit() {
    let row = json!({
        "id": "x",
        "name": "entry",
        "path": "p".repeat(4096),
        "detail": "d".repeat(1024),
    });
    let mut payload = Map::new();
    payload.insert("ok".to_string(), json!(true));
    payload.insert("schemaVersion".to_string(), json!(1));
    payload.insert("truncated".to_string(), json!(false));
    payload.insert("items".to_string(), json!(vec![row; 1000]));
    let raw = encoded_items(&payload);
    assert!(raw.len() <= MAX_OUTPUT_BYTES);
    let document: Value = serde_json::from_slice(&raw).unwrap();
    assert_eq!(document.get("truncated"), Some(&json!(true)));
    assert!(document.get("count").and_then(Value::as_u64).unwrap() < 1000);
}

#[test]
fn linked_auto_memory_directories_stay_discoverable() {
    let base = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(base.path()).unwrap();
    let (home, external) = (root.join("home"), root.join("external"));
    let projects = home.join(".claude/projects");
    fs::create_dir_all(&projects).unwrap();
    fs::create_dir_all(external.join("memory")).unwrap();
    std::os::unix::fs::symlink(&external, projects.join("linked")).unwrap();
    let context = build_context("", home.to_str().unwrap(), environ(&home), false, "all");
    let settings = ClaudeSettings {
        excludes: Vec::new(),
        auto_memory_directory: String::new(),
        inline: Vec::new(),
    };
    let mut plan = WatchPlan::new();
    assert_eq!(
        auto_memory_roots(&mut plan, &context, &settings),
        vec![projects.join("linked/memory")]
    );
    assert_eq!(
        bounded_directories(&mut plan, &projects, 10, false),
        Vec::<PathBuf>::new()
    );
}

#[test]
fn an_empty_project_and_a_later_nested_rule_are_both_watched() {
    let base = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(base.path()).unwrap();
    let (home, project) = (root.join("home"), root.join("project"));
    fs::create_dir_all(&home).unwrap();
    fs::create_dir_all(&project).unwrap();
    let scan = |home: &Path, project: &Path| {
        let mut plan = WatchPlan::new();
        let document = discovery::collect(
            &mut plan,
            project.to_str().unwrap(),
            home.to_str().unwrap(),
            "",
            environ(home),
            true,
            "all",
        );
        (document, plan.paths())
    };
    let (_, initial) = scan(&home, &project);
    assert!(initial.contains(&project));
    let rule = project.join(".claude/rules/nested/new.md");
    write(&rule, "new rule");
    let (document, paths) = scan(&home, &project);
    assert!(paths.contains(&rule.parent().unwrap().to_path_buf()));
    assert!(
        rows(&document)
            .into_iter()
            .any(|row| field(row, "path") == rule.to_str().unwrap())
    );
}

#[test]
fn the_glob_watches_empty_matching_directories_and_stays_contained() {
    let base = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(base.path()).unwrap();
    let (docs, outside) = (root.join("docs"), root.join("docs-sibling"));
    let nested = docs.join("nested");
    fs::create_dir_all(&nested).unwrap();
    fs::create_dir_all(&outside).unwrap();
    write(&outside.join("private.md"), "outside");
    std::os::unix::fs::symlink(&outside, docs.join("alias")).unwrap();
    std::os::unix::fs::symlink(&docs, docs.join("loop")).unwrap();
    let mut plan = WatchPlan::new();
    assert_eq!(
        bounded_glob(&mut plan, &docs, "**/*.md"),
        Vec::<PathBuf>::new()
    );
    let paths = plan.paths();
    assert!(paths.contains(&nested));
    assert!(!paths.contains(&outside));
    write(&nested.join("new.md"), "new");
    let mut plan = WatchPlan::new();
    assert_eq!(
        bounded_glob(&mut plan, &docs, "**/*.md"),
        vec![nested.join("new.md")]
    );
}

#[test]
fn listing_writes_nothing_to_disk() {
    let fixture = sandbox();
    let before = snapshot(&fixture.root);
    for _ in 0..2 {
        payload(&fixture.home, &fixture.project);
    }
    assert_eq!(snapshot(&fixture.root), before);
}

fn snapshot(base: &Path) -> Vec<(PathBuf, u32, i64)> {
    let mut rows: Vec<(PathBuf, u32, i64)> = Vec::new();
    let mut pending = vec![base.to_path_buf()];
    while let Some(directory) = pending.pop() {
        let Ok(entries) = fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten() {
            use std::os::unix::fs::MetadataExt;
            let metadata = fs::symlink_metadata(entry.path()).unwrap();
            rows.push((entry.path(), metadata.mode(), metadata.mtime_nsec()));
            if metadata.is_dir() {
                pending.push(entry.path());
            }
        }
    }
    rows.sort();
    rows
}

#[test]
fn a_plain_write_default_file_is_described_from_its_first_line() {
    let base = tempfile::tempdir().unwrap();
    let root = fs::canonicalize(base.path()).unwrap();
    let (home, project) = (root.join("home"), root.join("work/repo"));
    fs::create_dir_all(project.join(".git")).unwrap();
    fs::create_dir_all(&home).unwrap();
    write_default(&project.join("AGENTS.md"));
    let document = payload(&home, &project);
    let row = by_name(&document, "AGENTS.md").unwrap();
    assert_eq!(field(row, "detail"), "heading");
    assert_eq!(field(row, "activation"), "always");
}
