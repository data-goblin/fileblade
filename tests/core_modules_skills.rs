#[path = "common/skill_fixtures.rs"]
mod skill_fixtures;

use fileblade::core_modules::skills::bounds::{
    MAX_DESCRIPTOR_BYTES, MAX_ENV_PATH, MAX_ITEMS, MAX_PROJECT_WALK, artifact_metrics,
    bounded_names, env_path_value, read_descriptor, read_secure_document,
};
use fileblade::core_modules::skills::discovery;
use fileblade::core_modules::skills::registry;
use fileblade::core_modules::text::clean;
use fileblade::core_modules::watch::WatchPlan;
use serde_json::{Value, json};
use skill_fixtures::{
    collect, collect_watched, environment, field, link_skill, names, native_name, row_for,
    write_skill, write_skill_raw,
};
use std::collections::BTreeSet;
use std::ffi::{OsStr, OsString};
use std::fs;
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use tempfile::TempDir;

fn sandbox() -> TempDir {
    tempfile::Builder::new()
        .prefix("fileblade-skills-")
        .tempdir()
        .unwrap()
}

fn agent_set(row: &Value) -> BTreeSet<String> {
    row.get("agents")
        .and_then(Value::as_array)
        .map(|agents| {
            agents
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

fn expected(values: &[&str]) -> BTreeSet<String> {
    values.iter().map(|value| value.to_string()).collect()
}

#[test]
fn only_uppercase_descriptors_are_discovered() {
    let base = sandbox();
    let home = base.path().join("home");
    let root = home.join(".claude/skills");
    write_skill(&root, "good", "A fixture skill");
    fs::create_dir_all(root.join("lower")).unwrap();
    fs::write(root.join("lower/skill.md"), "---\nname: lower\n---\n").unwrap();
    let found = names(&collect(&environment(&home)));
    assert!(found.contains(&"good".to_string()));
    assert!(!found.contains(&"lower".to_string()));
}

#[test]
fn only_documented_roots_are_scanned() {
    let base = sandbox();
    let home = base.path().join("home");
    for (relative, name) in [
        (".claude/skills", "claude-user"),
        (".agents/skills", "agents-user"),
        (".pi/agent/skills", "pi-user"),
        (".copilot/skills", "copilot-user"),
        (".config/opencode/skills", "opencode-user"),
        (".gemini/antigravity-cli/skills", "antigravity-cli"),
        (".gemini/antigravity/skills", "antigravity-undocumented"),
        (".agent/skills", "antigravity-legacy"),
        (".codex/skills", "codex-undocumented"),
        (".invented/skills", "invented"),
        (".claude/skill", "singular"),
    ] {
        write_skill(&home.join(relative), name, "A fixture skill");
    }
    let found = names(&collect(&environment(&home)));
    for wanted in [
        "claude-user",
        "agents-user",
        "pi-user",
        "copilot-user",
        "opencode-user",
        "antigravity-cli",
    ] {
        assert!(found.contains(&wanted.to_string()), "{wanted} missing");
    }
    for refused in [
        "antigravity-undocumented",
        "antigravity-legacy",
        "codex-undocumented",
        "invented",
        "singular",
    ] {
        assert!(!found.contains(&refused.to_string()), "{refused} found");
    }
}

#[test]
fn shared_roots_attribute_every_reader() {
    let base = sandbox();
    let home = base.path().join("home");
    write_skill(&home.join(".agents/skills"), "shared", "A fixture skill");
    write_skill(
        &home.join(".claude/skills"),
        "claude-only",
        "A fixture skill",
    );
    let document = collect(&environment(&home));
    assert_eq!(
        agent_set(row_for(&document, "shared").unwrap()),
        expected(&["codex", "opencode", "pi", "copilot-cli"])
    );
    assert_eq!(
        agent_set(row_for(&document, "claude-only").unwrap()),
        expected(&["claude-code", "opencode"])
    );
}

#[test]
fn symlinked_copies_collapse_into_one_row() {
    let base = sandbox();
    let home = base.path().join("home");
    let descriptor = write_skill(&base.path().join("vault"), "linked", "A fixture skill");
    let target = descriptor.parent().unwrap();
    link_skill(&home.join(".claude/skills"), "linked", target);
    link_skill(&home.join(".agents/skills"), "linked", target);
    let document = collect(&environment(&home));
    let rows: Vec<&Value> = skill_fixtures::rows(&document)
        .into_iter()
        .filter(|row| field(row, "name") == "linked")
        .collect();
    assert_eq!(rows.len(), 1);
    assert_eq!(agent_set(rows[0]).len(), 5);
    assert!(field(rows[0], "path").starts_with(&home.join(".claude").display().to_string()));
    assert_eq!(
        field(rows[0], "link_target"),
        descriptor.display().to_string()
    );
}

#[test]
fn user_and_project_copies_stay_separate_rows() {
    let base = sandbox();
    let home = base.path().join("home");
    let project = base.path().join("project");
    fs::create_dir_all(project.join(".git")).unwrap();
    let descriptor = write_skill(&base.path().join("vault2"), "both", "A fixture skill");
    let target = descriptor.parent().unwrap();
    link_skill(&home.join(".claude/skills"), "both", target);
    link_skill(&project.join(".claude/skills"), "both", target);
    let document = collect(&environment(&home).anchored(&project));
    let mut scopes: Vec<&str> = skill_fixtures::rows(&document)
        .into_iter()
        .filter(|row| field(row, "name") == "both")
        .map(|row| field(row, "scope"))
        .collect();
    scopes.sort_unstable();
    assert_eq!(scopes, ["project", "user"]);
}

#[test]
fn the_scope_lanes_partition_the_full_listing() {
    let base = sandbox();
    let home = base.path().join("home");
    let project = base.path().join("project");
    fs::create_dir_all(project.join(".git")).unwrap();
    write_skill(&home.join(".claude/skills"), "user-only", "A fixture skill");
    write_skill(
        &project.join(".claude/skills"),
        "project-only",
        "A fixture skill",
    );
    let base_environment = environment(&home).anchored(&project);
    let both = collect(&base_environment);
    let user = collect_watched(&base_environment.clone().scoped("user"));
    let project_lane = collect_watched(&base_environment.clone().scoped("project"));
    assert_eq!(names(&user.0), vec!["user-only".to_string()]);
    assert_eq!(names(&project_lane.0), vec!["project-only".to_string()]);
    assert_eq!(
        skill_fixtures::rows(&both).len(),
        skill_fixtures::rows(&user.0).len() + skill_fixtures::rows(&project_lane.0).len()
    );
    assert!(
        !user.1.iter().any(|path| path.starts_with(&project)),
        "the user lane must not watch the project"
    );
}

#[test]
fn the_exact_flag_pins_the_project_to_its_anchor() {
    let base = sandbox();
    let home = base.path().join("home");
    let repo = base.path().join("repo");
    let nested = repo.join("packages/web");
    fs::create_dir_all(&nested).unwrap();
    fs::create_dir_all(repo.join(".git")).unwrap();
    write_skill(
        &nested.join(".claude/skills"),
        "nested-only",
        "A fixture skill",
    );
    let walked = environment(&home).anchored(&nested);
    let exact = walked.clone().exactly();
    let document = collect(&exact);
    assert_eq!(
        document.get("project").and_then(Value::as_str),
        Some(nested.display().to_string().as_str())
    );
    assert!(names(&document).contains(&"nested-only".to_string()));
    assert_eq!(
        collect(&walked).get("project").and_then(Value::as_str),
        Some(repo.display().to_string().as_str())
    );
}

#[test]
fn the_project_walk_stops_at_the_repository_root() {
    let base = sandbox();
    let home = base.path().join("home");
    let outside = base.path().join("outside");
    let repo = outside.join("repo");
    let nested = repo.join("packages/service");
    fs::create_dir_all(&nested).unwrap();
    fs::create_dir_all(repo.join(".git")).unwrap();
    write_skill(
        &repo.join(".agents/skills"),
        "root-skill",
        "A fixture skill",
    );
    write_skill(
        &nested.join(".agents/skills"),
        "nested-skill",
        "A fixture skill",
    );
    write_skill(
        &outside.join(".agents/skills"),
        "outside-decoy",
        "A fixture skill",
    );
    let found = names(&collect(&environment(&home).anchored(&nested)));
    assert!(found.contains(&"root-skill".to_string()));
    assert!(found.contains(&"nested-skill".to_string()));
    assert!(!found.contains(&"outside-decoy".to_string()));
    let (root, chain) = discovery::project_chain(&nested);
    assert_eq!(root, repo);
    assert_eq!(chain.last(), Some(&repo));
    assert!(!chain.contains(&outside));
    assert!(chain.len() <= MAX_PROJECT_WALK);
}

#[test]
fn every_documented_project_root_is_read_at_the_workspace() {
    let base = sandbox();
    let home = base.path().join("home");
    let parent = base.path().join("parent");
    let workspace = parent.join("workspace");
    fs::create_dir_all(workspace.join(".git")).unwrap();
    for (relative, name) in [
        (".agents/skills", "shared-skill"),
        (".opencode/skills", "opencode-skill"),
        (".pi/skills", "pi-skill"),
        (".claude/skills", "claude-skill"),
        (".github/skills", "github-skill"),
    ] {
        write_skill(&workspace.join(relative), name, "A fixture skill");
    }
    write_skill(
        &parent.join(".agents/skills"),
        "parent-decoy",
        "A fixture skill",
    );
    let found = names(&collect(&environment(&home).anchored(&workspace)));
    for name in [
        "shared-skill",
        "opencode-skill",
        "pi-skill",
        "claude-skill",
        "github-skill",
    ] {
        assert!(found.contains(&name.to_string()), "{name} missing");
    }
    assert!(!found.contains(&"parent-decoy".to_string()));
    let (root, chain) = discovery::project_chain(&workspace);
    assert_eq!(root, workspace);
    assert_eq!(chain, vec![workspace]);
    assert_eq!(
        discovery::project_chain(Path::new("")),
        (std::path::PathBuf::new(), Vec::new())
    );
}

#[test]
fn managed_and_system_roots_follow_the_platform_and_prefix() {
    let base = sandbox();
    let home = base.path().join("home");
    let prefix = base.path().join("prefix");
    write_skill(
        &prefix.join("etc/claude-code/.claude/skills"),
        "managed-skill",
        "A fixture skill",
    );
    write_skill(
        &prefix.join("etc/codex/skills"),
        "codex-system",
        "A fixture skill",
    );
    let document = collect(&environment(&home).prefixed(&prefix));
    let managed = row_for(&document, "managed-skill").unwrap();
    assert_eq!(field(managed, "scope"), "managed");
    assert_eq!(managed.get("precedence"), Some(&json!(0)));
    assert_eq!(
        field(row_for(&document, "codex-system").unwrap(), "scope"),
        "system"
    );
    let darwin = collect(&environment(&home).prefixed(&prefix).on_platform("darwin"));
    assert!(row_for(&darwin, "managed-skill").is_none());
}

#[test]
fn the_reserved_synced_directory_is_recursed_only_under_claude_roots() {
    let base = sandbox();
    let home = base.path().join("home");
    write_skill(
        &home.join(".claude/skills/synced"),
        "from-claude-ai",
        "A fixture skill",
    );
    write_skill(
        &home.join(".agents/skills/synced"),
        "not-reserved",
        "A fixture skill",
    );
    let document = collect(&environment(&home));
    let synced = row_for(&document, "from-claude-ai").unwrap();
    assert_eq!(field(synced, "source"), "synced");
    assert!(row_for(&document, "not-reserved").is_none());
}

#[test]
fn installed_claude_plugins_carry_their_enable_state() {
    let base = sandbox();
    let home = base.path().join("home");
    let cache = home.join(".claude/plugins/cache/market/demo");
    write_skill(
        &cache.join("1.0.0/skills"),
        "installed-skill",
        "A fixture skill",
    );
    write_skill(
        &cache.join("0.9.0/skills"),
        "stale-skill",
        "A fixture skill",
    );
    let single = home.join(".claude/plugins/cache/market/solo/1.0.0");
    fs::create_dir_all(&single).unwrap();
    fs::write(
        single.join("SKILL.md"),
        "---\nname: solo-root\ndescription: one\n---\n",
    )
    .unwrap();
    let plugins = home.join(".claude/plugins");
    fs::write(
        plugins.join("installed_plugins.json"),
        json!({
            "version": 2,
            "plugins": {
                "demo@market": [{"installPath": cache.join("1.0.0"), "version": "1.0.0"}],
                "solo@market": [{"installPath": single, "version": "1.0.0"}],
            },
        })
        .to_string(),
    )
    .unwrap();
    fs::write(
        home.join(".claude/settings.json"),
        json!({"enabledPlugins": {"demo@market": true, "solo@market": false}}).to_string(),
    )
    .unwrap();
    let document = collect(&environment(&home));
    let installed = row_for(&document, "installed-skill").unwrap();
    let solo = row_for(&document, "solo-root").unwrap();
    assert!(row_for(&document, "stale-skill").is_none());
    assert_eq!(field(installed, "source"), "plugin:demo@market");
    assert_eq!(installed.get("enabled"), Some(&json!(true)));
    assert_eq!(solo.get("enabled"), Some(&json!(false)));
    assert!(
        solo.get("badges")
            .and_then(Value::as_array)
            .unwrap()
            .contains(&json!("disabled"))
    );
}

#[test]
fn the_codex_config_supplies_enable_switches_and_survives_broken_toml() {
    let base = sandbox();
    let home = base.path().join("home");
    let descriptor = write_skill(&home.join(".agents/skills"), "switched", "A fixture skill");
    fs::create_dir_all(home.join(".codex")).unwrap();
    fs::write(
        home.join(".codex/config.toml"),
        format!(
            "[[skills.config]]\npath = \"{}\"\nenabled = false\n",
            descriptor.display()
        ),
    )
    .unwrap();
    let document = collect(&environment(&home));
    assert_eq!(
        row_for(&document, "switched").unwrap().get("enabled"),
        Some(&json!(false))
    );
    let broken = base.path().join("broken");
    fs::create_dir_all(broken.join(".codex")).unwrap();
    fs::write(broken.join(".codex/config.toml"), "not = [valid toml\n").unwrap();
    let mut plan = WatchPlan::new();
    assert!(discovery::codex_switches(&mut plan, &environment(&broken)).is_empty());
}

#[test]
fn the_listing_is_capped_by_items_and_by_detail_length() {
    let base = sandbox();
    let home = base.path().join("home");
    let root = home.join(".claude/skills");
    write_skill(&root, "huge", &"D".repeat(4000));
    write_skill(&root, "overcap", &"E".repeat(32 * 1024 + 4096));
    let document = collect(&environment(&home));
    assert_eq!(
        field(row_for(&document, "huge").unwrap(), "detail")
            .chars()
            .count(),
        512
    );
    assert_eq!(field(row_for(&document, "overcap").unwrap(), "detail"), "");
    for index in 0..MAX_ITEMS + 20 {
        write_skill(&root, &format!("bulk-{index:04}"), "A fixture skill");
    }
    let capped = collect(&environment(&home));
    assert_eq!(capped.get("count"), Some(&json!(MAX_ITEMS)));
    assert_eq!(capped.get("truncated"), Some(&json!(true)));
}

#[test]
fn row_ids_are_stable_and_unique() {
    let base = sandbox();
    let home = base.path().join("home");
    write_skill(&home.join(".claude/skills"), "stable", "A fixture skill");
    let first = collect(&environment(&home));
    let second = collect(&environment(&home));
    assert_eq!(
        field(row_for(&first, "stable").unwrap(), "id"),
        field(row_for(&second, "stable").unwrap(), "id")
    );
    let ids: BTreeSet<&str> = skill_fixtures::rows(&first)
        .into_iter()
        .map(|row| field(row, "id"))
        .collect();
    assert_eq!(ids.len(), skill_fixtures::rows(&first).len());
}

#[test]
fn hostile_descriptors_are_cleaned_or_skipped() {
    let base = sandbox();
    let home = base.path().join("home");
    let root = home.join(".claude/skills");
    fs::create_dir_all(root.join("escape")).unwrap();
    fs::write(
        root.join("escape/SKILL.md"),
        "---\nname: bad\u{7}name\ndescription: <b>markup</b> and \u{0} nul\n---\n",
    )
    .unwrap();
    std::os::unix::fs::symlink(base.path().join("nowhere"), root.join("dangling")).unwrap();
    fs::create_dir_all(root.join("fifo")).unwrap();
    rustix::fs::mknodat(
        rustix::fs::CWD,
        root.join("fifo/SKILL.md"),
        rustix::fs::FileType::Fifo,
        rustix::fs::Mode::from_raw_mode(0o600),
        0,
    )
    .unwrap();
    let document = collect(&environment(&home));
    let row = row_for(&document, "badname").expect("control characters are stripped");
    assert!(!field(row, "detail").contains('\u{0}'));
    let found = names(&document);
    assert!(!found.contains(&"dangling".to_string()));
    assert!(!found.contains(&"fifo".to_string()));
}

#[test]
fn an_empty_home_lists_nothing() {
    let base = sandbox();
    let document = collect(&environment(&base.path().join("absent-home")));
    assert_eq!(document.get("ok"), Some(&json!(true)));
    assert_eq!(document.get("count"), Some(&json!(0)));
    assert_eq!(document.get("schemaVersion"), Some(&json!(1)));
}

#[test]
fn the_registry_declares_six_agents_and_known_kinds() {
    assert_eq!(registry::AGENT_LABELS.len(), 6);
    assert!(
        !registry::ROOTS
            .iter()
            .any(|root| root.path.starts_with(".codex/"))
    );
    assert!(
        !registry::ROOTS
            .iter()
            .any(|root| root.path.contains("global_skills"))
    );
    for root in registry::ROOTS.iter() {
        assert!(
            matches!(
                root.kind,
                "managed" | "user" | "project" | "system" | "extension"
            ),
            "{}",
            root.kind
        );
    }
}

#[test]
fn unicode_and_native_byte_names_stay_distinct() {
    let base = sandbox();
    let home = base.path().join("home");
    write_skill(
        &home.join(".claude/skills"),
        "日本語スキル",
        "A fixture skill",
    );
    write_skill(&home.join(".claude/skills"), "café", "A fixture skill");
    write_skill(
        &home.join(".agents/skills"),
        "cafe\u{301}",
        "A fixture skill",
    );
    write_skill(
        &home.join(".claude/skills"),
        "emoji-\u{1f600}",
        "A fixture skill",
    );
    write_skill_raw(
        &home.join(".claude/skills"),
        &native_name(0xfe),
        "native-byte-name",
        "A fixture skill",
    );
    let document = collect(&environment(&home));
    let found = names(&document);
    for name in [
        "日本語スキル",
        "café",
        "cafe\u{301}",
        "emoji-\u{1f600}",
        "native-byte-name",
    ] {
        assert!(found.contains(&name.to_string()), "{name} missing");
    }
    for row in skill_fixtures::rows(&document) {
        assert!(!field(row, "name").contains('\u{0}'));
        assert!(!field(row, "name").contains('\n'));
    }
}

#[test]
fn environment_path_values_keep_their_edges_and_refuse_hostile_input() {
    let entry = |value: &str| vec![(OsString::from("X"), OsString::from(value))];
    assert_eq!(env_path_value(&entry("  spaced  "), "X"), "  spaced  ");
    assert_eq!(env_path_value(&entry("a   b"), "X"), "a   b");
    assert_eq!(env_path_value(&entry("каталог/файл"), "X"), "каталог/файл");
    assert_eq!(env_path_value(&[], "X"), "");
    assert_eq!(env_path_value(&entry(""), "X"), "");
    assert_eq!(env_path_value(&entry("a\u{0}b"), "X"), "");
    assert_eq!(
        env_path_value(&entry(&"x".repeat(MAX_ENV_PATH + 1)), "X"),
        ""
    );
}

#[test]
fn descriptor_reads_refuse_oversize_irregular_and_symlinked_files() {
    let base = sandbox();
    let mut plan = WatchPlan::new();
    let big = base.path().join("big.md");
    fs::write(&big, "x".repeat(MAX_DESCRIPTOR_BYTES + 4096)).unwrap();
    assert!(read_descriptor(&mut plan, &big, MAX_DESCRIPTOR_BYTES, false).is_none());
    let small = base.path().join("small.md");
    fs::write(&small, "hello").unwrap();
    assert_eq!(
        read_descriptor(&mut plan, &small, MAX_DESCRIPTOR_BYTES, false),
        Some(b"hello".to_vec())
    );
    let fifo = base.path().join("fifo.md");
    rustix::fs::mknodat(
        rustix::fs::CWD,
        &fifo,
        rustix::fs::FileType::Fifo,
        rustix::fs::Mode::from_raw_mode(0o600),
        0,
    )
    .unwrap();
    assert!(read_descriptor(&mut plan, &fifo, MAX_DESCRIPTOR_BYTES, false).is_none());
    let link = base.path().join("link.md");
    std::os::unix::fs::symlink(&small, &link).unwrap();
    assert!(read_descriptor(&mut plan, &link, MAX_DESCRIPTOR_BYTES, false).is_none());
    assert_eq!(clean("a\u{0}b\u{7}c", 64), "abc");
    assert_eq!(clean(&"y".repeat(900), 512).chars().count(), 512);
}

#[test]
fn the_secure_read_checks_the_open_descriptor() {
    let base = sandbox();
    let mut plan = WatchPlan::new();
    let path = base.path().join("settings.json");
    fs::write(&path, "private").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o666)).unwrap();
    assert_eq!(read_secure_document(&mut plan, &path, true), "");
    assert_eq!(read_secure_document(&mut plan, &path, false), "private");
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    assert_eq!(
        read_secure_document(&mut plan, &path, true),
        if rustix::process::getuid().is_root() {
            "private"
        } else {
            ""
        }
    );
}

#[test]
fn directory_listings_are_bounded_before_they_are_sorted() {
    let base = sandbox();
    let mut plan = WatchPlan::new();
    let root = base.path().join("many");
    fs::create_dir_all(&root).unwrap();
    for index in 0..40 {
        fs::write(root.join(format!("{index:03}")), "x").unwrap();
    }
    let (names, truncated) = bounded_names(&mut plan, &root, 4);
    assert_eq!(names.len(), 4);
    assert!(truncated);
    assert!(names.windows(2).all(|pair| pair[0] <= pair[1]));
    let (all, complete) = bounded_names(&mut plan, &root, 512);
    assert_eq!(all.len(), 40);
    assert!(!complete);
}

#[test]
fn many_sibling_files_do_not_hide_the_descriptor() {
    let base = sandbox();
    let mut plan = WatchPlan::new();
    let root = base.path().join("crowded");
    fs::create_dir_all(&root).unwrap();
    for index in 0..100 {
        fs::write(root.join(index.to_string()), "x").unwrap();
    }
    let descriptor = root.join("SKILL.md");
    fs::write(&descriptor, "skill").unwrap();
    assert_eq!(discovery::descriptor(&mut plan, &root), Some(descriptor));
}

#[test]
fn new_roots_descriptors_and_link_targets_are_watched() {
    let base = sandbox();
    let home = base.path().join("home");
    let project = base.path().join("project");
    let vault = base.path().join("vault");
    for directory in [&home, &project, &vault] {
        fs::create_dir_all(directory).unwrap();
    }
    let watched = environment(&home)
        .anchored(&project)
        .exactly()
        .prefixed(&base.path().join("prefix"));
    let (_, paths, _) = collect_watched(&watched);
    assert!(paths.contains(&fs::canonicalize(&project).unwrap()));
    assert!(paths.contains(&fs::canonicalize(&home).unwrap()));
    let skill = project.join(".claude/skills/example");
    fs::create_dir_all(skill.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&vault, &skill).unwrap();
    let (document, paths, _) = collect_watched(&watched);
    assert!(skill_fixtures::rows(&document).is_empty());
    assert!(paths.contains(&fs::canonicalize(&vault).unwrap()));
    assert!(paths.contains(&fs::canonicalize(skill.parent().unwrap()).unwrap()));
    fs::write(
        vault.join("SKILL.md"),
        "---\nname: example\ndescription: watch me\n---\n",
    )
    .unwrap();
    let (document, _, _) = collect_watched(&watched);
    assert_eq!(names(&document), vec!["example".to_string()]);
    let listed = document
        .get("watchPaths")
        .and_then(Value::as_array)
        .unwrap();
    assert!(listed.contains(&json!(
        fs::canonicalize(&vault).unwrap().display().to_string()
    )));
    assert!(listed.len() <= 512);
}

#[test]
fn artifact_metrics_count_unicode_characters_words_and_tokens() {
    let base = sandbox();
    let text = "汉字 日本語 한국어 Кириллица Ελληνικά café 😀";
    let descriptor = base.path().join("技能-навык-δεξιότητα.md");
    fs::write(&descriptor, text).unwrap();
    let description = "汉字 日本語";
    let metrics = artifact_metrics(&descriptor, text, description);
    assert!(!metrics["updated"].as_str().unwrap().is_empty());
    assert!(metrics["created"].is_string());
    assert_eq!(metrics["bytes"], json!(text.len()));
    assert_eq!(metrics["characters"], json!(text.chars().count()));
    assert_eq!(metrics["words"], json!(6));
    assert_eq!(metrics["tokens"], json!(description.len().div_ceil(4)));
    assert_eq!(metrics["fileTokens"], json!(text.len().div_ceil(4)));
    assert_eq!(artifact_metrics(&descriptor, text, "")["tokens"], json!(0));
    assert!(artifact_metrics(&base.path().join("absent.md"), text, "").is_empty());
}

#[test]
fn native_byte_paths_keep_distinct_names_ids_and_uris() {
    let base = sandbox();
    let home = base.path().join("home");
    fs::create_dir_all(&home).unwrap();
    let project = base.path().join(OsStr::from_bytes(b"repo-\xff"));
    let skills = project.join(".claude/skills");
    fs::create_dir_all(&skills).unwrap();
    let raw: [&[u8]; 4] = [b"\xff", b"\xfe", "\u{fffd}".as_bytes(), b"\\xFF"];
    for name in raw {
        let directory = skills.join(OsStr::from_bytes(name));
        fs::create_dir_all(&directory).unwrap();
        fs::write(directory.join("SKILL.md"), "# original\n").unwrap();
    }
    let document = collect(
        &environment(&home)
            .anchored(&project)
            .exactly()
            .prefixed(&base.path().join("etc")),
    );
    let rows = skill_fixtures::rows(&document);
    assert_eq!(rows.len(), 4);
    let ids: BTreeSet<&str> = rows.iter().map(|row| field(row, "id")).collect();
    assert_eq!(ids.len(), 4);
    let names: BTreeSet<String> = rows
        .iter()
        .map(|row| field(row, "name").to_string())
        .collect();
    let expected_names: BTreeSet<String> = raw
        .iter()
        .map(|name| fileblade::common::display_path(Path::new(OsStr::from_bytes(name))))
        .collect();
    assert_eq!(names, expected_names);
    let paths: BTreeSet<std::path::PathBuf> = rows
        .iter()
        .map(|row| fileblade::common::parse_path(field(row, "path")).unwrap())
        .collect();
    let expected_paths: BTreeSet<std::path::PathBuf> = raw
        .iter()
        .map(|name| skills.join(OsStr::from_bytes(name)).join("SKILL.md"))
        .collect();
    assert_eq!(paths, expected_paths);
}
