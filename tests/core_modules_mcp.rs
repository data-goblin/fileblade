#[path = "common/mcp_case.rs"]
mod mcp_case;

use fileblade::core_modules::mcp::inventory::{
    self, Inventory, MAX_ENVIRONMENT_PATH_CHARS, MAX_OUTPUT_BYTES, Settings, bounded_json,
    read_environment_path,
};
use fileblade::core_modules::mcp::model::{safe_label, safe_path};
use fileblade::core_modules::mcp::parsers::{parse_json, parse_jsonc, parse_toml};
use fileblade::core_modules::mcp::safeio::{self, bounded_read};
use fileblade::core_modules::mcp::value::Cfg;
use fileblade::core_modules::watch::WatchPlan;
use mcp_case::{Case, SENTINEL, field, json_write, named, text, write};
use serde_json::{Value, json};
use std::ffi::OsString;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

fn populate(case: &Case) {
    json_write(
        &case.home.join(".claude.json"),
        &json!({
            "mcpServers": {
                "shared": {"command": SENTINEL, "args": [SENTINEL], "env": {SENTINEL: SENTINEL}},
                SENTINEL: {"url": format!("https://{SENTINEL}")},
            },
            "projects": {
                case.project.to_string_lossy(): {
                    "mcpServers": {"shared": {"command": "local"}},
                    "enabledMcpjsonServers": ["project-only"],
                }
            },
        }),
    );
    write(
        &case.home.join(".codex").join("config.toml"),
        &format!(
            r#"
[mcp_servers.codex-user]
command = "{SENTINEL}"

[projects."{}"]
trust_level = "trusted"
"#,
            case.project.to_string_lossy()
        ),
    );
    json_write(
        &case.project.join(".mcp.json"),
        &json!({
            "mcpServers": {
                "shared": {"url": format!("https://{SENTINEL}"), "headers": {SENTINEL: SENTINEL}},
                "project-only": {"url": format!("https://{SENTINEL}")},
                "pi-high": {"command": "project"},
            }
        }),
    );
    json_write(
        &case.home.join(".pi").join("agent").join("settings.json"),
        &json!({"packages": ["npm:pi-mcp-adapter@2.0.0"]}),
    );
    json_write(
        &case.config.join("mcp").join("mcp.json"),
        &json!({"mcpServers": {"pi-high": {"command": SENTINEL}}}),
    );
    json_write(
        &case.project.join(".pi").join("mcp.json"),
        &json!({"mcpServers": {"pi-high": {"command": SENTINEL, "disabled": true}}}),
    );
}

#[test]
fn the_schema_hides_every_secret_and_keeps_the_declared_definition_fields() {
    let case = Case::new();
    populate(&case);
    let first = case.scan();
    let second = case.scan();
    assert_eq!(first, second);
    let encoded = bounded_json(&first);
    assert!(!encoded.contains(SENTINEL));
    assert!(!encoded.contains("https://"));
    assert!(!encoded.contains("\"command\""));
    assert!(encoded.len() <= MAX_OUTPUT_BYTES);
    assert_eq!(first.get("schemaVersion"), Some(&json!(1)));
    assert_eq!(first.get("healthBasis"), Some(&json!("configuration-only")));
    let definitions = first
        .get("definitions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap();
    assert!(!definitions.is_empty());
    for definition in &definitions {
        for key in [
            "id",
            "agent",
            "name",
            "scope",
            "source",
            "support",
            "transport",
            "enabled",
            "trusted",
            "trustRole",
            "effective",
            "selected",
            "shadowed",
            "shadowedBy",
            "duplicateGroup",
            "state",
            "health",
            "secretPresence",
            "metrics",
            "agentId",
            "appliedAgents",
        ] {
            assert!(definition.get(key).is_some(), "{key} missing");
        }
        assert_eq!(text(definition, "id").len(), 24);
        assert_eq!(text(definition, "health"), "not-probed");
        for banned in [
            "url", "command", "args", "env", "headers", "token", "bearer",
        ] {
            assert!(definition.get(banned).is_none());
        }
        let applied = field(definition, "appliedAgents")
            .as_array()
            .cloned()
            .unwrap();
        assert!(applied.contains(&json!(text(definition, "agentId"))));
    }
    let claude = named(&definitions, "claude", "shared");
    let selected: Vec<&str> = claude
        .iter()
        .filter(|row| field(row, "selected") == &json!(true))
        .map(|row| text(row, "scope"))
        .collect();
    assert_eq!(selected, ["local"]);
    assert!(
        claude
            .iter()
            .any(|row| field(row, "secretPresence").get("headers") == Some(&json!(true)))
    );
    let pi = named(&definitions, "pi", "pi-high");
    let chosen: Vec<&Value> = pi
        .into_iter()
        .filter(|row| field(row, "selected") == &json!(true))
        .collect();
    assert_eq!(chosen.len(), 1);
    assert_eq!(text(chosen[0], "scope"), "project");
}

#[test]
fn the_claude_managed_file_is_exclusive() {
    let case = Case::new();
    json_write(
        &case.home.join(".claude.json"),
        &json!({"mcpServers": {"user": {"command": "safe"}}}),
    );
    json_write(
        &case.etc.join("claude-code").join("managed-mcp.json"),
        &json!({"mcpServers": {"managed": {"command": "safe"}}}),
    );
    let definitions = case.definitions();
    let user = named(&definitions, "claude", "user");
    let managed = named(&definitions, "claude", "managed");
    assert_eq!(field(user[0], "selected"), &json!(false));
    assert_eq!(field(managed[0], "selected"), &json!(true));
}

#[test]
fn pi_core_is_explicitly_unsupported() {
    let case = Case::new();
    json_write(
        &case.home.join(".pi").join("agent").join("settings.json"),
        &json!({"packages": []}),
    );
    let document = case.scan();
    let agents = document
        .get("agents")
        .and_then(Value::as_array)
        .cloned()
        .unwrap();
    let pi = agents
        .iter()
        .find(|row| text(row, "id") == "pi")
        .cloned()
        .unwrap();
    assert_eq!(
        pi,
        json!({"id": "pi", "support": "unsupported", "reason": "pi-core-has-no-native-mcp"})
    );
    assert!(
        !case
            .definitions()
            .iter()
            .any(|row| text(row, "agent") == "pi")
    );
}

#[test]
fn codex_profiles_are_bounded_unicode_safe_and_never_active() {
    let case = Case::new();
    let profile_name = "配置-日本語-한국어-Ελληνικά-Кириллица-😀-cafe\u{301}.config.toml";
    let server_name = "中文-日本語-한국어-Ελληνικά-Кириллица-😀-cafe\u{301}";
    let codex_home = case.home.join(".codex");
    write(
        &codex_home.join("config.toml"),
        "\n[mcp_servers.collision]\ncommand = \"user\"\n",
    );
    write(
        &codex_home.join(profile_name),
        &format!(
            "\n[mcp_servers.collision]\ncommand = \"profile\"\n\n[mcp_servers.\"{server_name}\"]\ncommand = \"profile\"\n"
        ),
    );
    let outside = case.base.join("outside-profile.config.toml");
    write(
        &outside,
        "[mcp_servers.symlink-server]\ncommand = \"profile\"\n",
    );
    std::os::unix::fs::symlink(&outside, codex_home.join("linked.config.toml")).unwrap();

    let definitions: Vec<Value> = case
        .definitions()
        .into_iter()
        .filter(|row| text(row, "agent") == "codex")
        .collect();
    let profile = definitions
        .iter()
        .find(|row| text(row, "name") == server_name)
        .unwrap();
    assert_eq!(text(profile, "scope"), "profile");
    assert_eq!(
        field(profile, "source").get("path"),
        Some(&json!(format!("~/.codex/{profile_name}")))
    );
    assert_eq!(
        field(profile, "source").get("redacted"),
        Some(&json!(false))
    );
    assert_eq!(field(profile, "enabled"), &Value::Null);
    assert_eq!(field(profile, "selected"), &Value::Null);
    assert_eq!(field(profile, "effective"), &Value::Null);
    let collision: Vec<&Value> = definitions
        .iter()
        .filter(|row| text(row, "name") == "collision")
        .collect();
    let chosen: Vec<&str> = collision
        .iter()
        .filter(|row| field(row, "selected") == &json!(true))
        .map(|row| text(row, "scope"))
        .collect();
    assert_eq!(chosen, ["user"]);
    let unknown: Vec<&str> = collision
        .iter()
        .filter(|row| field(row, "selected") == &Value::Null)
        .map(|row| text(row, "scope"))
        .collect();
    assert_eq!(unknown, ["profile"]);
    assert!(field(collision[0], "duplicateGroup").is_string());
    assert!(
        !definitions
            .iter()
            .any(|row| text(row, "name") == "symlink-server")
    );
}

#[test]
fn the_codex_home_environment_is_aliased_not_exposed() {
    let mut case = Case::new();
    let profile_name = "日本語-😀.config.toml";
    let codex_home = case.base.join(format!("external-{SENTINEL}")).join("codex");
    write(
        &codex_home.join(profile_name),
        "[mcp_servers.profile-server]\ncommand = \"profile\"\n",
    );
    case.environment.insert(
        OsString::from("CODEX_HOME"),
        OsString::from(codex_home.as_os_str()),
    );
    case.codex_home = Some(codex_home.clone());
    let document = case.scan();
    let definitions = document
        .get("definitions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap();
    let profile = definitions
        .iter()
        .find(|row| text(row, "name") == "profile-server")
        .unwrap();
    assert_eq!(
        field(profile, "source").get("path"),
        Some(&json!(format!("<codex-home>/{profile_name}")))
    );
    assert_eq!(
        field(profile, "source").get("redacted"),
        Some(&json!(false))
    );
    assert!(!bounded_json(&document).contains(SENTINEL));
}

#[test]
fn environment_paths_preserve_unicode_and_reject_invalid_values() {
    let segment = "  路  径-日本語-한국어-Ελληνικά-Кириллица-😀-cafe\u{301}  ";
    let exact_limit = format!("  {}  ", "界".repeat(MAX_ENVIRONMENT_PATH_CHARS - 4));
    assert_eq!(exact_limit.chars().count(), MAX_ENVIRONMENT_PATH_CHARS);
    let mut environment = inventory::Environ::new();
    environment.insert(OsString::from("PATH"), OsString::from(&exact_limit));
    assert_eq!(
        read_environment_path(&environment, "PATH").as_deref(),
        Some(exact_limit.as_str())
    );
    for invalid in [
        "",
        "valid-prefix\0invalid-suffix",
        &"界".repeat(MAX_ENVIRONMENT_PATH_CHARS + 1),
    ] {
        let mut environment = inventory::Environ::new();
        environment.insert(OsString::from("CODEX_HOME"), OsString::from(invalid));
        assert!(read_environment_path(&environment, "CODEX_HOME").is_none());
    }

    let mut case = Case::new();
    let codex_home = case.base.join(segment).join("  codex  ");
    write(
        &codex_home.join("  profile 日本語  .config.toml"),
        "\n[mcp_servers.whitespace-codex]\ncommand = \"example\"\n",
    );
    case.codex_home = Some(codex_home);
    assert!(
        case.definitions()
            .iter()
            .any(|row| text(row, "name") == "whitespace-codex")
    );
}

#[test]
fn opencode_system_configuration_is_ignored() {
    let case = Case::new();
    json_write(
        &case.etc.join("opencode").join("opencode.json"),
        &json!({"mcp": {"system-opencode": {"type": "local", "command": [SENTINEL]}}}),
    );
    assert!(
        !case
            .definitions()
            .iter()
            .any(|row| text(row, "name") == "system-opencode")
    );
}

#[test]
fn antigravity_plugins_preserve_unicode_and_redact_secret_paths() {
    let case = Case::new();
    let plugin_name = "插件-日本語-한국어-Ελληνικά-Кириллица-😀-cafe\u{301}";
    let server_name = "中文-日本語-한국어-Ελληνικά-Кириллица-😀-cafe\u{301}";
    let plugin_root = case
        .home
        .join(".gemini")
        .join("antigravity-cli")
        .join("plugins");
    json_write(
        &plugin_root.join(plugin_name).join("mcp_config.json"),
        &json!({"mcpServers": {server_name: {"command": "example"}, "collision": {"command": "plugin"}}}),
    );
    json_write(
        &plugin_root
            .join(format!("plugin-{SENTINEL}"))
            .join("mcp_config.json"),
        &json!({"mcpServers": {"ordinary-server": {"command": "example"}}}),
    );
    json_write(
        &case
            .home
            .join(".gemini")
            .join("config")
            .join("mcp_config.json"),
        &json!({"mcpServers": {"collision": {"command": "global"}}}),
    );
    let outside = case.base.join("outside-antigravity-plugin");
    json_write(
        &outside.join("mcp_config.json"),
        &json!({"mcpServers": {"symlink-server": {"command": "example"}}}),
    );
    std::os::unix::fs::symlink(&outside, plugin_root.join("linked-plugin")).unwrap();

    let document = case.scan();
    let definitions: Vec<Value> = document
        .get("definitions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap()
        .into_iter()
        .filter(|row| text(row, "agent") == "google-antigravity")
        .collect();
    let unicode = definitions
        .iter()
        .find(|row| text(row, "name") == server_name)
        .unwrap();
    assert_eq!(
        field(unicode, "source").get("path"),
        Some(&json!(format!(
            "~/.gemini/antigravity-cli/plugins/{plugin_name}/mcp_config.json"
        )))
    );
    assert_eq!(
        field(unicode, "source").get("redacted"),
        Some(&json!(false))
    );
    assert_eq!(field(unicode, "enabled"), &Value::Null);
    assert_eq!(field(unicode, "trusted"), &Value::Null);
    assert_eq!(field(unicode, "effective"), &Value::Null);
    let collisions: Vec<&Value> = definitions
        .iter()
        .filter(|row| text(row, "name") == "collision")
        .map(|row| field(row, "selected"))
        .collect();
    assert_eq!(collisions, [&Value::Null, &Value::Null]);
    let secret = definitions
        .iter()
        .find(|row| text(row, "name") == "ordinary-server")
        .map(|row| field(row, "source"))
        .unwrap();
    assert_eq!(secret.get("redacted"), Some(&json!(true)));
    let path = secret.get("path").and_then(Value::as_str).unwrap();
    assert!(
        regex::Regex::new(r"/segment-[0-9a-f]{8}/mcp_config\.json$")
            .unwrap()
            .is_match(path),
        "{path}"
    );
    let serialized = bounded_json(&document);
    assert!(!serialized.contains(SENTINEL));
    assert!(
        !definitions
            .iter()
            .any(|row| text(row, "name") == "symlink-server")
    );
}

#[test]
fn the_unicode_policy_rejects_unsafe_categories_dots_and_overlong_values() {
    let safe = "中文-日本語-한국어-Ελληνικά-Кириллица-😀-cafe\u{301}";
    assert_eq!(safe_label(safe, "1234567890abcdef"), safe);
    assert_eq!(
        safe_path(format!("~/{safe}").as_bytes(), "source-id"),
        (format!("~/{safe}"), false)
    );
    let unsafe_values = [
        "control-\n",
        "format-\u{200d}",
        "private-\u{e000}",
        "unassigned-\u{fdd0}",
        ".",
        "..",
        &"x".repeat(129),
        SENTINEL,
    ];
    let pattern = regex::Regex::new(r"^server-[0-9a-f]{8}$").unwrap();
    for value in unsafe_values {
        let (sanitized, redacted) = safe_path(format!("~/{value}").as_bytes(), "source-id");
        assert!(redacted, "{value:?}");
        assert_ne!(sanitized, format!("~/{value}"));
        if value != "." && value != ".." {
            assert!(
                pattern.is_match(&safe_label(value, "1234567890abcdef")),
                "{value:?}"
            );
        }
    }
    let surrogate: Vec<u8> = [b"~/".as_slice(), b"surrogate-", &[0xedu8, 0xa0, 0x80]].concat();
    let (sanitized, redacted) = safe_path(&surrogate, "source-id");
    assert!(redacted);
    assert!(sanitized.starts_with("~/segment-"));
}

#[test]
fn source_metrics_preserve_unicode_counts_and_dates() {
    let case = Case::new();
    let source = case.project.join(".mcp.json");
    let text_body =
        r#"{"mcpServers":{"設定":{"command":"中文 日本語 한국어 Ελληνικά Кириллица 😀"}}}"#;
    write(&source, text_body);
    let document = case.scan();
    let definitions = document
        .get("definitions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap();
    let definition = definitions
        .iter()
        .find(|row| text(row, "name") == "設定")
        .unwrap();
    let metrics = field(definition, "metrics");
    let direct = safeio::artifact_metrics(&source, text_body.as_bytes());
    assert_eq!(metrics.get("bytes"), Some(&json!(text_body.len())));
    assert_eq!(
        metrics.get("characters"),
        Some(&json!(text_body.chars().count()))
    );
    assert_eq!(metrics.get("words"), direct.get("words"));
    assert_eq!(
        metrics.get("tokens"),
        Some(&json!(text_body.len().div_ceil(4)))
    );
    let updated = metrics.get("updated").and_then(Value::as_str).unwrap();
    assert!(
        regex::Regex::new(r"^\d{4}-\d{2}-\d{2} \d{2}:\d{2}$")
            .unwrap()
            .is_match(updated)
    );
}

#[test]
fn symlinked_regular_files_resolve_with_output_bounds() {
    let case = Case::new();
    let mut plan = WatchPlan::new();
    let target = case.base.join("target.json");
    let link = case.base.join("link.json");
    write(&target, "{}");
    std::os::unix::fs::symlink(&target, &link).unwrap();
    assert_eq!(
        bounded_read(&mut plan, &link, 100, &safeio::Options::default()).data,
        Some(b"{}".to_vec())
    );
    let real = case.base.join("real");
    fs::create_dir_all(&real).unwrap();
    write(&real.join("inside.json"), "{}");
    let directory_link = case.base.join("linked-directory");
    std::os::unix::fs::symlink(&real, &directory_link).unwrap();
    assert_eq!(
        bounded_read(
            &mut plan,
            &directory_link.join("inside.json"),
            100,
            &safeio::Options::default()
        )
        .data,
        Some(b"{}".to_vec())
    );
    write(&case.base.join("large.json"), &"x".repeat(101));
    assert_eq!(
        bounded_read(
            &mut plan,
            &case.base.join("large.json"),
            100,
            &safeio::Options::default()
        )
        .error,
        Some("oversized")
    );
    let owner = rustix::process::getuid().as_raw();
    assert_eq!(
        bounded_read(
            &mut plan,
            &target,
            100,
            &safeio::Options {
                required_owner_uid: Some(owner + 1),
                reject_group_or_world_writable: false,
            }
        )
        .error,
        Some("insecure-owner")
    );
    for mode in [0o664, 0o666] {
        fs::set_permissions(&target, fs::Permissions::from_mode(mode)).unwrap();
        assert_eq!(
            bounded_read(&mut plan, &target, 100, &safeio::Options::managed(owner)).error,
            Some("insecure-mode")
        );
    }

    let mut document = serde_json::Map::new();
    document.insert("schemaVersion".to_string(), json!(1));
    document.insert("healthBasis".to_string(), json!("configuration-only"));
    document.insert(
        "definitions".to_string(),
        json!(
            (0..100)
                .map(|_| json!({"value": "x".repeat(10000)}))
                .collect::<Vec<Value>>()
        ),
    );
    assert!(bounded_json(&document).len() <= MAX_OUTPUT_BYTES);
}

#[test]
fn the_parsers_are_bounded_and_jsonc_aware() {
    let parsed =
        parse_jsonc(br#"{"url":"https://example.invalid//kept",/*drop*/"x":[1,],}"#).unwrap();
    assert_eq!(
        parsed.get("x").and_then(Cfg::as_array).map(Vec::len),
        Some(1)
    );
    assert_eq!(
        parsed.get("url").and_then(Cfg::as_str),
        Some("https://example.invalid//kept")
    );
    let toml = parse_toml(b"[mcp_servers.a]\ncommand=\"x\"\n").unwrap();
    assert_eq!(
        toml.get("mcp_servers")
            .and_then(|value| value.get("a"))
            .and_then(|value| value.get("command"))
            .and_then(Cfg::as_str),
        Some("x")
    );
    assert_eq!(
        parse_json(br#"{"a":1,"a":2}"#).unwrap_err().code(),
        "duplicate-json-key"
    );
    assert_eq!(
        parse_json(br#"{"a":NaN}"#).unwrap_err().code(),
        "nonfinite-json-number"
    );
    assert_eq!(parse_json(b"[]").unwrap_err().code(), "root-not-object");
    let deep = [
        b"{\"value\":".to_vec(),
        b"[".repeat(5000),
        b"0".to_vec(),
        b"]".repeat(5000),
        b"}".to_vec(),
    ]
    .concat();
    assert!(parse_json(&deep).is_err());
    let deep_toml = [
        b"value = ".to_vec(),
        b"[".repeat(5000),
        b"0".to_vec(),
        b"]".repeat(5000),
    ]
    .concat();
    assert!(parse_toml(&deep_toml).is_err());
}

#[test]
fn an_expired_deadline_truncates_and_warns() {
    let case = Case::new();
    let mut store = Inventory::new(case.settings());
    store.deadline = safeio::Deadline::new(0);
    std::thread::sleep(std::time::Duration::from_millis(2));
    let document = store.scan();
    assert_eq!(document.get("truncated"), Some(&json!(true)));
    let warnings = document
        .get("warnings")
        .and_then(Value::as_array)
        .cloned()
        .unwrap();
    assert!(warnings.contains(&json!({"code": "deadline", "sourceId": "inventory"})));
}

#[test]
fn the_scope_lanes_split_user_and_project_rows() {
    let mut case = Case::new();
    populate(&case);
    let both = case.definitions();
    case.scope = "user".to_string();
    let user = case.definitions();
    case.scope = "project".to_string();
    let project = case.definitions();
    let user_scopes: Vec<&str> = user.iter().map(|row| text(row, "scope")).collect();
    assert!(!user_scopes.contains(&"project"));
    assert!(!user_scopes.contains(&"local"));
    for row in &project {
        assert!(matches!(text(row, "scope"), "project" | "local"));
    }
    assert_eq!(both.len(), user.len() + project.len());
}

#[test]
fn project_mcpjson_approvals_merge_from_the_settings_files() {
    let case = Case::new();
    populate(&case);
    json_write(
        &case.project.join(".claude").join("settings.json"),
        &json!({"enabledMcpjsonServers": ["pi-high"]}),
    );
    json_write(
        &case.project.join(".claude").join("settings.local.json"),
        &json!({"disabledMcpjsonServers": ["project-only"]}),
    );
    let rows: Vec<Value> = case
        .definitions()
        .into_iter()
        .filter(|row| text(row, "agent") == "claude" && text(row, "scope") == "project")
        .collect();
    let trusted = |name: &str| {
        rows.iter()
            .find(|row| text(row, "name") == name)
            .map(|row| field(row, "trusted").clone())
    };
    assert_eq!(trusted("pi-high"), Some(json!(true)));
    assert_eq!(trusted("project-only"), Some(json!(false)));
}

#[test]
fn watch_paths_cover_missing_sources_symlink_targets_and_nested_plugins() {
    let case = Case::new();
    let mut store = Inventory::new(Settings {
        etc_root: case.base.join("etc"),
        ..case.settings()
    });
    let mut document = store.scan();
    let mut plan = std::mem::take(&mut store.plan);
    plan.finish(&mut document);
    let watched: Vec<String> = document
        .get("watchPaths")
        .and_then(Value::as_array)
        .cloned()
        .unwrap()
        .iter()
        .map(|value| value.as_str().unwrap_or_default().to_string())
        .collect();
    assert!(watched.contains(&case.project.to_string_lossy().into_owned()));
    assert!(watched.contains(&case.home.to_string_lossy().into_owned()));

    let source = case.project.join(".mcp.json");
    let target = case.base.join("vault").join("config.json");
    write(
        &target,
        r#"{"mcpServers":{"tool":{"command":"private-command"}}}"#,
    );
    std::os::unix::fs::symlink(&target, &source).unwrap();
    let mut store = Inventory::new(case.settings());
    let mut document = store.scan();
    let mut plan = std::mem::take(&mut store.plan);
    plan.finish(&mut document);
    let watched = document
        .get("watchPaths")
        .and_then(Value::as_array)
        .cloned()
        .unwrap();
    let holds = |path: &Path| watched.contains(&json!(path.to_string_lossy()));
    assert!(holds(source.parent().unwrap()));
    assert!(holds(target.parent().unwrap()));
    assert!(
        !bounded_json(&document).contains("private-command"),
        "the listing must never echo a command"
    );
}

#[test]
fn bounded_entries_refuse_symlinked_directories() {
    let case = Case::new();
    let mut plan = WatchPlan::new();
    let real = case.base.join("real-root");
    fs::create_dir_all(real.join("child")).unwrap();
    write(&real.join("inside.json"), "{}");
    let link = case.base.join("linked-root");
    std::os::unix::fs::symlink(&real, &link).unwrap();
    let mut deadline = safeio::Deadline::new(2000);
    assert_eq!(
        safeio::bounded_files(&mut plan, &real, 16, &mut deadline).unwrap(),
        vec![real.join("inside.json")]
    );
    assert_eq!(
        safeio::bounded_directories(&mut plan, &real, 16, &mut deadline).unwrap(),
        vec![real.join("child")]
    );
    assert!(
        safeio::bounded_files(&mut plan, &link, 16, &mut deadline)
            .unwrap()
            .is_empty()
    );
    assert!(
        safeio::bounded_directories(&mut plan, &link, 16, &mut deadline)
            .unwrap()
            .is_empty()
    );
    assert!(
        safeio::bounded_files(&mut plan, &link.join("child"), 16, &mut deadline)
            .unwrap()
            .is_empty()
    );
}

#[test]
fn the_undo_record_bound_measures_raw_utf8_bytes() {
    let payload = json!({"definition": "\u{4f60}\u{4f60}\u{4f60}\u{4f60}"});
    assert_eq!(
        fileblade::core_modules::canonical::compact_json(&payload).len(),
        "{\"definition\":\"".len() + 12 + 2
    );
    assert_eq!(
        fileblade::core_modules::canonical::compact_ascii_json(&payload).len(),
        "{\"definition\":\"".len() + 24 + 2
    );
    assert_eq!(
        fileblade::core_modules::canonical::compact_json(&json!({"a": "\u{1f600}"})),
        "{\"a\":\"\u{1f600}\"}"
    );
    assert_eq!(
        fileblade::core_modules::canonical::compact_json(&json!({"a": "x\u{1}\n\""})),
        "{\"a\":\"x\\u0001\\n\\\"\"}"
    );
}
