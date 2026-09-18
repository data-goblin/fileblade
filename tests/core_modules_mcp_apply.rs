#[path = "common/mcp_case.rs"]
mod mcp_case;

use fileblade::core_modules::mcp::apply::{Applier, Removal, WRITE_AGENTS};
use fileblade::core_modules::mcp::inventory::bounded_json;
use fileblade::core_modules::mcp::tomlwrite::{
    locate_server_block, remove_server_block, render_server_table,
};
use fileblade::core_modules::mcp::value::Cfg;
use mcp_case::{Case, SENTINEL, field, json_write, serial, text, write};
use serde_json::{Value, json};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

const HEADER_SENTINEL: &str = "HEADER_SENTINEL_DO_NOT_EMIT";

fn prepared() -> Case {
    let case = Case::new();
    json_write(
        &case.home.join(".claude.json"),
        &json!({
            "numStartups": 3,
            "mcpServers": {
                "local-tool": {"command": "npx", "args": ["-y", "local-tool"], "env": {"TOKEN": SENTINEL}},
                "remote-tool": {"type": "http", "url": "https://mcp.example.invalid/mcp", "headers": {"Authorization": HEADER_SENTINEL}},
                "events": {"type": "sse", "url": "https://mcp.example.invalid/sse"},
                "socket-tool": {"socket": "/run/mcp.sock"},
            },
        }),
    );
    json_write(
        &case.home.join(".pi").join("agent").join("settings.json"),
        &json!({"packages": ["npm:pi-mcp-adapter@2.0.0"]}),
    );
    case
}

fn identifier(case: &Case, name: &str, agent: &str) -> String {
    case.definitions()
        .into_iter()
        .find(|row| text(row, "agentId") == agent && text(row, "name") == name)
        .map(|row| text(&row, "id").to_string())
        .unwrap_or_else(|| panic!("no definition for {agent}/{name}"))
}

fn applied(case: &Case, name: &str, agents: &[&str], state: &str) -> Value {
    let id = identifier(case, name, "claude-code");
    let agents: Vec<String> = agents.iter().map(|value| (*value).to_string()).collect();
    Applier::new(case.inventory()).apply(&id, &agents, state)
}

fn read_json(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

fn results(document: &Value) -> Vec<Value> {
    document
        .get("results")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default()
}

#[test]
fn a_source_definition_is_removed_and_restored() {
    let _guard = serial();
    let case = prepared();
    let before = read_json(&case.home.join(".claude.json"));
    let id = identifier(&case, "local-tool", "claude-code");
    let removed = Applier::new(case.inventory()).remove(
        &id,
        &Removal {
            prepare: false,
            expected_payload: None,
            transaction_id: "",
        },
    );
    assert_eq!(removed.get("ok"), Some(&json!(true)), "{removed:?}");
    assert_eq!(removed.get("changed"), Some(&json!(true)));
    let without_payload: Value = Value::Object(
        removed
            .as_object()
            .unwrap()
            .iter()
            .filter(|(key, _)| key.as_str() != "payload")
            .map(|(key, value)| (key.clone(), value.clone()))
            .collect(),
    );
    assert!(!without_payload.to_string().contains(SENTINEL));
    let payload = removed.get("payload").cloned().unwrap();
    assert_eq!(payload.get("agent"), Some(&json!("claude-code")));
    assert_eq!(payload.get("format"), Some(&json!(2)));
    assert_eq!(payload.get("name"), Some(&json!("local-tool")));
    assert_eq!(
        payload.get("raw").and_then(|raw| raw.get("env")),
        Some(&json!({"TOKEN": SENTINEL}))
    );
    let after = read_json(&case.home.join(".claude.json"));
    assert!(after["mcpServers"].get("local-tool").is_none());
    assert_eq!(after["numStartups"], json!(3));

    let missing = Applier::new(case.inventory()).remove(
        &id,
        &Removal {
            prepare: false,
            expected_payload: None,
            transaction_id: "",
        },
    );
    assert_eq!(missing.get("ok"), Some(&json!(false)));

    let restored = Applier::new(case.inventory()).restore("", Some(&payload.to_string()));
    assert_eq!(restored.get("ok"), Some(&json!(true)), "{restored:?}");
    assert!(restored.get("payload").is_none());
    assert!(!restored.to_string().contains(SENTINEL));
    assert_eq!(
        read_json(&case.home.join(".claude.json"))["mcpServers"]["local-tool"],
        before["mcpServers"]["local-tool"]
    );
    let twice = Applier::new(case.inventory()).restore("", Some(&payload.to_string()));
    assert_eq!(twice.get("ok"), Some(&json!(true)));
    assert_eq!(twice.get("changed"), Some(&json!(false)));
    assert_eq!(
        Applier::new(case.inventory())
            .restore("", Some("nope"))
            .get("ok"),
        Some(&json!(false))
    );
    assert_eq!(
        Applier::new(case.inventory())
            .restore("", Some(r#"{"agent":"x","path":"/tmp/x","spec":{}}"#))
            .get("ok"),
        Some(&json!(false))
    );
}

#[test]
fn a_symlinked_source_is_preserved_through_removal_and_restore() {
    let _guard = serial();
    let case = prepared();
    let source = case.home.join(".claude.json");
    let target = case.base.join("vault").join("claude.json");
    fs::create_dir_all(target.parent().unwrap()).unwrap();
    fs::rename(&source, &target).unwrap();
    std::os::unix::fs::symlink(&target, &source).unwrap();
    let definition = case
        .definitions()
        .into_iter()
        .find(|row| text(row, "agentId") == "claude-code" && text(row, "name") == "local-tool")
        .unwrap();
    assert_eq!(
        field(&definition, "source").get("realpath"),
        Some(&json!(target.to_string_lossy()))
    );
    let before = read_json(&target);
    let removed = Applier::new(case.inventory()).remove(
        text(&definition, "id"),
        &Removal {
            prepare: false,
            expected_payload: None,
            transaction_id: "",
        },
    );
    assert_eq!(removed.get("ok"), Some(&json!(true)), "{removed:?}");
    assert!(fs::symlink_metadata(&source).unwrap().is_symlink());
    assert_ne!(read_json(&target), before);
    let payload = removed.get("payload").cloned().unwrap();
    let restored = Applier::new(case.inventory()).restore("", Some(&payload.to_string()));
    assert_eq!(restored.get("ok"), Some(&json!(true)), "{restored:?}");
    assert!(fs::symlink_metadata(&source).unwrap().is_symlink());
    assert_eq!(read_json(&target), before);
}

#[test]
fn on_then_off_round_trips_for_every_target() {
    let _guard = serial();
    let case = prepared();
    for name in ["local-tool", "remote-tool"] {
        let result = applied(&case, name, &["all"], "on");
        let encoded = result.to_string();
        assert!(!encoded.contains(SENTINEL));
        assert!(!encoded.contains(HEADER_SENTINEL));
        assert!(!encoded.contains("https://"));
        assert_eq!(result.get("ok"), Some(&json!(true)), "{result:?}");
        let agents: Vec<String> = results(&result)
            .iter()
            .map(|row| text(row, "agent").to_string())
            .collect();
        assert_eq!(agents, WRITE_AGENTS);
        for row in results(&result) {
            assert_eq!(field(&row, "ok"), &json!(true));
            assert_eq!(
                field(&row, "changed"),
                &json!(text(&row, "agent") != "claude-code")
            );
            for touched in field(&row, "touched").as_array().cloned().unwrap() {
                assert!(touched.as_str().unwrap().starts_with("~/"));
            }
        }
    }

    let claude = read_json(&case.home.join(".claude.json"));
    assert_eq!(claude["numStartups"], json!(3));
    assert!(claude["mcpServers"]["local-tool"].get("type").is_none());

    let codex_text = fs::read_to_string(case.home.join(".codex").join("config.toml")).unwrap();
    let codex: toml::Table = codex_text.parse().unwrap();
    let local = &codex["mcp_servers"]["local-tool"];
    assert_eq!(local["command"].as_str(), Some("npx"));
    assert_eq!(local["args"][0].as_str(), Some("-y"));
    assert_eq!(local["args"][1].as_str(), Some("local-tool"));
    assert_eq!(local["env"]["TOKEN"].as_str(), Some(SENTINEL));
    assert_eq!(
        codex["mcp_servers"]["remote-tool"]["url"].as_str(),
        Some("https://mcp.example.invalid/mcp")
    );
    assert_eq!(
        codex["mcp_servers"]["remote-tool"]["http_headers"]["Authorization"]
            .as_str()
            .unwrap(),
        HEADER_SENTINEL
    );

    let opencode = read_json(&case.config.join("opencode").join("opencode.json"));
    assert_eq!(
        opencode["mcp"]["servers"]["local-tool"],
        json!({"type": "local", "command": ["npx", "-y", "local-tool"], "environment": {"TOKEN": SENTINEL}})
    );
    assert_eq!(
        opencode["mcp"]["servers"]["remote-tool"],
        json!({"type": "remote", "url": "https://mcp.example.invalid/mcp", "headers": {"Authorization": HEADER_SENTINEL}})
    );

    let pi = read_json(&case.home.join(".pi").join("agent").join("mcp.json"));
    assert_eq!(
        pi["mcpServers"]["local-tool"],
        json!({"command": "npx", "args": ["-y", "local-tool"], "env": {"TOKEN": SENTINEL}})
    );
    assert_eq!(
        pi["mcpServers"]["remote-tool"],
        json!({"url": "https://mcp.example.invalid/mcp", "headers": {"Authorization": HEADER_SENTINEL}})
    );

    let copilot = read_json(&case.home.join(".copilot").join("mcp-config.json"));
    assert_eq!(
        copilot["mcpServers"]["local-tool"],
        json!({"type": "local", "command": "npx", "args": ["-y", "local-tool"], "env": {"TOKEN": SENTINEL}, "tools": ["*"]})
    );
    assert_eq!(
        copilot["mcpServers"]["remote-tool"],
        json!({"type": "http", "url": "https://mcp.example.invalid/mcp", "headers": {"Authorization": HEADER_SENTINEL}, "tools": ["*"]})
    );

    let antigravity = read_json(
        &case
            .home
            .join(".gemini")
            .join("config")
            .join("mcp_config.json"),
    );
    assert_eq!(
        antigravity["mcpServers"]["local-tool"],
        json!({"command": "npx", "args": ["-y", "local-tool"], "env": {"TOKEN": SENTINEL}})
    );
    assert_eq!(
        antigravity["mcpServers"]["remote-tool"],
        json!({"serverUrl": "https://mcp.example.invalid/mcp", "headers": {"Authorization": HEADER_SENTINEL}})
    );

    for path in [
        case.home
            .join(".gemini")
            .join("config")
            .join("mcp_config.json"),
        case.home.join(".codex").join("config.toml"),
    ] {
        assert_eq!(
            fs::metadata(&path).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert!(fs::read_to_string(&path).unwrap().ends_with('\n'));
    }

    let listed = case.scan();
    assert!(!bounded_json(&listed).contains(SENTINEL));
    let rows = listed
        .get("definitions")
        .and_then(Value::as_array)
        .cloned()
        .unwrap();
    let applied_for = |agent: &str, name: &str| {
        rows.iter()
            .find(|row| text(row, "agentId") == agent && text(row, "name") == name)
            .map(|row| field(row, "appliedAgents").clone())
    };
    let mut sorted: Vec<&str> = WRITE_AGENTS.to_vec();
    sorted.sort();
    assert_eq!(
        applied_for("claude-code", "local-tool"),
        Some(json!(sorted))
    );
    assert_eq!(applied_for("codex", "remote-tool"), Some(json!(sorted)));
    assert_eq!(
        applied_for("claude-code", "events"),
        Some(json!(["claude-code"]))
    );

    let second = applied(&case, "local-tool", &["all"], "on");
    assert_eq!(second.get("ok"), Some(&json!(true)));
    assert_eq!(second.get("changed"), Some(&json!(false)));
    assert!(
        results(&second)
            .iter()
            .all(|row| text(row, "message") == "already present")
    );

    let others: Vec<&str> = WRITE_AGENTS
        .iter()
        .copied()
        .filter(|agent| *agent != "claude-code")
        .collect();
    for name in ["local-tool", "remote-tool"] {
        let result = applied(&case, name, &others, "off");
        assert_eq!(result.get("ok"), Some(&json!(true)), "{result:?}");
        assert!(
            results(&result)
                .iter()
                .all(|row| field(&row.clone(), "changed") == &json!(true))
        );
        assert!(!result.to_string().contains(SENTINEL));
    }
    assert_eq!(
        fs::read_to_string(case.home.join(".codex").join("config.toml")).unwrap(),
        ""
    );
    assert_eq!(
        read_json(&case.config.join("opencode").join("opencode.json")),
        json!({"mcp": {"servers": {}}})
    );
    assert_eq!(
        read_json(
            &case
                .home
                .join(".gemini")
                .join("config")
                .join("mcp_config.json")
        ),
        json!({"mcpServers": {}})
    );
    assert_eq!(
        read_json(&case.home.join(".claude.json"))["mcpServers"]["local-tool"]["env"]["TOKEN"],
        json!(SENTINEL)
    );
    let again = applied(&case, "local-tool", &others, "off");
    assert_eq!(again.get("ok"), Some(&json!(true)));
    assert_eq!(again.get("changed"), Some(&json!(false)));
}

#[test]
fn a_header_secret_never_reaches_the_apply_or_restore_output() {
    let _guard = serial();
    let case = prepared();
    let id = identifier(&case, "remote-tool", "claude-code");
    let removed = Applier::new(case.inventory()).remove(
        &id,
        &Removal {
            prepare: false,
            expected_payload: None,
            transaction_id: "",
        },
    );
    assert_eq!(removed.get("ok"), Some(&json!(true)), "{removed:?}");
    let payload = removed.get("payload").cloned().unwrap();
    assert_eq!(
        payload["raw"]["headers"],
        json!({"Authorization": HEADER_SENTINEL})
    );
    let restored = Applier::new(case.inventory()).restore("", Some(&payload.to_string()));
    assert_eq!(restored.get("ok"), Some(&json!(true)), "{restored:?}");
    assert!(!restored.to_string().contains(HEADER_SENTINEL));
    assert_eq!(
        read_json(&case.home.join(".claude.json"))["mcpServers"]["remote-tool"]["headers"],
        json!({"Authorization": HEADER_SENTINEL})
    );
}

#[test]
fn the_source_entry_is_never_rewritten_or_removed() {
    let _guard = serial();
    let case = prepared();
    let source = case.home.join(".claude.json");
    let before = fs::read(&source).unwrap();
    let on = applied(&case, "local-tool", &["claude-code"], "on");
    assert_eq!(text(&results(&on)[0], "message"), "already present");
    assert_eq!(on.get("changed"), Some(&json!(false)));
    let off = applied(&case, "local-tool", &["claude-code"], "off");
    assert_eq!(off.get("ok"), Some(&json!(false)));
    assert!(text(&results(&off)[0], "message").contains("own source entry"));
    assert_eq!(fs::read(&source).unwrap(), before);
}

#[test]
fn the_toml_edit_preserves_unrelated_content_and_quotes_names() {
    let _guard = serial();
    let case = prepared();
    let config = case.home.join(".codex").join("config.toml");
    let original = concat!(
        "model = \"gpt-5\"\n",
        "# keep this comment\n",
        "[mcp_servers.other]\n",
        "command = \"other\"\n",
        "\n",
        "[projects.\"/tmp/demo\"]\n",
        "trust_level = \"trusted\"\n"
    );
    write(&config, original);
    json_write(
        &case.home.join(".claude.json"),
        &json!({"mcpServers": {"dotted.name": {"command": "run", "args": ["a\"b"], "env": {"K": "v\\w"}}}}),
    );
    let result = applied(&case, "dotted.name", &["codex"], "on");
    assert_eq!(result.get("ok"), Some(&json!(true)), "{result:?}");
    let text_body = fs::read_to_string(&config).unwrap();
    assert!(text_body.starts_with(original));
    assert!(text_body.contains("[mcp_servers.\"dotted.name\"]"));
    let parsed: toml::Table = text_body.parse().unwrap();
    assert_eq!(
        parsed["mcp_servers"]["dotted.name"]["args"][0].as_str(),
        Some("a\"b")
    );
    assert_eq!(
        parsed["mcp_servers"]["dotted.name"]["env"]["K"].as_str(),
        Some("v\\w")
    );
    assert_eq!(
        parsed["mcp_servers"]["other"]["command"].as_str(),
        Some("other")
    );
    assert_eq!(
        parsed["projects"]["/tmp/demo"]["trust_level"].as_str(),
        Some("trusted")
    );

    let written = fs::read(&config).unwrap();
    json_write(
        &case.home.join(".claude.json"),
        &json!({"mcpServers": {"dotted.name": {"command": "run", "args": ["changed"]}}}),
    );
    let conflict = applied(&case, "dotted.name", &["codex"], "on");
    assert_eq!(conflict.get("ok"), Some(&json!(false)));
    assert_eq!(
        results(&conflict)[0],
        json!({
            "agent": "codex",
            "ok": false,
            "changed": false,
            "message": "a different server named dotted.name already exists in mcp_servers; nothing was changed",
            "touched": [],
        })
    );
    assert_eq!(fs::read(&config).unwrap(), written);

    let removed = applied(&case, "dotted.name", &["codex"], "off");
    assert_eq!(removed.get("ok"), Some(&json!(true)));
    assert_eq!(fs::read_to_string(&config).unwrap(), original);
}

#[test]
fn on_refuses_a_different_entry_of_the_same_name_without_naming_it() {
    let _guard = serial();
    let case = prepared();
    let gemini = case
        .home
        .join(".gemini")
        .join("config")
        .join("mcp_config.json");
    json_write(
        &gemini,
        &json!({"theme": "dark", "mcpServers": {"local-tool": {"command": "mine", "args": []}}}),
    );
    let before = fs::read(&gemini).unwrap();
    let result = applied(&case, "local-tool", &["antigravity"], "on");
    assert_eq!(result.get("ok"), Some(&json!(false)));
    assert_eq!(result.get("changed"), Some(&json!(false)));
    assert_eq!(
        text(&results(&result)[0], "message"),
        "a different server named local-tool already exists in mcpServers; nothing was changed"
    );
    assert_eq!(fs::read(&gemini).unwrap(), before);
    assert!(!result.to_string().contains(SENTINEL));

    json_write(
        &case.home.join(".claude.json"),
        &json!({"mcpServers": {SENTINEL: {"command": "run"}}}),
    );
    json_write(
        &gemini,
        &json!({"mcpServers": {SENTINEL: {"command": "other"}}}),
    );
    let mut store = case.inventory();
    store.scan();
    let hidden = store
        .definitions
        .iter()
        .find(|item| item.raw_name == SENTINEL && item.agent == "claude")
        .map(|item| item.id.clone())
        .unwrap();
    let refused = Applier::new(case.inventory()).apply(&hidden, &["antigravity".to_string()], "on");
    assert_eq!(refused.get("ok"), Some(&json!(false)));
    assert!(!refused.to_string().contains(SENTINEL));
    assert!(
        regex::Regex::new(r"named server-[0-9a-f]{8} already exists")
            .unwrap()
            .is_match(text(&results(&refused)[0], "message"))
    );
    let off = Applier::new(case.inventory()).apply(&hidden, &["antigravity".to_string()], "off");
    assert_eq!(off.get("ok"), Some(&json!(true)));
    assert_eq!(off.get("changed"), Some(&json!(true)));
    assert_eq!(read_json(&gemini), json!({"mcpServers": {}}));
}

#[test]
fn ambiguous_or_unlocatable_tables_are_refused() {
    let _guard = serial();
    let case = prepared();
    let config = case.home.join(".codex").join("config.toml");
    json_write(
        &case.home.join(".claude.json"),
        &json!({"mcpServers": {"tool": {"command": "run"}}}),
    );
    write(&config, "mcp_servers = { tool = { command = \"run\" } }\n");
    let result = applied(&case, "tool", &["codex"], "off");
    assert_eq!(result.get("ok"), Some(&json!(false)));
    assert!(text(&results(&result)[0], "message").contains("table-not-found"));
    assert_eq!(
        fs::read_to_string(&config).unwrap(),
        "mcp_servers = { tool = { command = \"run\" } }\n"
    );

    assert_eq!(
        locate_server_block("[mcp_servers.tool]\ncommand = \"one\"\n", "tool").unwrap(),
        Some((0, 2))
    );
    assert!(
        locate_server_block(
            "[mcp_servers.tool]\ncommand = \"one\"\n[mcp_servers.tool]\n",
            "tool"
        )
        .is_err()
    );
    assert!(remove_server_block("", "tool").is_err());

    write(&config, "not = toml = at all\n");
    let broken = applied(&case, "tool", &["codex"], "on");
    assert_eq!(broken.get("ok"), Some(&json!(false)));
    assert_eq!(
        text(&results(&broken)[0], "message"),
        "target is not strict TOML"
    );
}

#[test]
fn the_toml_emitter_round_trips_supported_scalars() {
    let _guard = serial();
    let entry = Cfg::from_json(&json!({
        "command": "c",
        "args": ["x", "y"],
        "enabled": true,
        "startup_timeout_sec": 5,
        "env": {"A": "1"},
    }));
    let block = render_server_table("we ird", &entry).unwrap();
    let parsed: toml::Table = block.parse().unwrap();
    assert_eq!(
        parsed["mcp_servers"]["we ird"]["startup_timeout_sec"].as_integer(),
        Some(5)
    );
    assert_eq!(
        parsed["mcp_servers"]["we ird"]["env"]["A"].as_str(),
        Some("1")
    );
    let extended = Cfg::from_json(&json!({
        "weight": 1.5,
        "empty": {},
        "nested": {"items": [1, true, {"key": "value"}]},
    }));
    let parsed: toml::Table = render_server_table("x", &extended)
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(parsed["mcp_servers"]["x"]["weight"].as_float(), Some(1.5));
    assert!(
        parsed["mcp_servers"]["x"]["empty"]
            .as_table()
            .is_some_and(toml::map::Map::is_empty)
    );
    assert_eq!(
        parsed["mcp_servers"]["x"]["nested"]["items"][2]["key"].as_str(),
        Some("value")
    );
    assert!(render_server_table("x", &Cfg::from_json(&json!({"bad": null}))).is_err());
}

#[test]
fn jsonc_targets_are_refused_and_symlinked_targets_are_updated() {
    let _guard = serial();
    let case = prepared();
    let copilot = case.home.join(".copilot").join("mcp-config.json");
    write(&copilot, "{\n  // comment\n  \"mcpServers\": {}\n}\n");
    let before = fs::read(&copilot).unwrap();
    let result = applied(&case, "local-tool", &["copilot-cli"], "on");
    assert_eq!(result.get("ok"), Some(&json!(false)));
    assert_eq!(
        text(&results(&result)[0], "message"),
        "target is not strict JSON"
    );
    assert_eq!(fs::read(&copilot).unwrap(), before);

    let target = case.base.join("elsewhere.json");
    json_write(&target, &json!({"mcpServers": {}}));
    let gemini = case
        .home
        .join(".gemini")
        .join("config")
        .join("mcp_config.json");
    fs::create_dir_all(gemini.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(&target, &gemini).unwrap();
    let linked = applied(&case, "local-tool", &["antigravity"], "on");
    assert_eq!(linked.get("ok"), Some(&json!(true)), "{linked:?}");
    assert!(fs::symlink_metadata(&gemini).unwrap().is_symlink());
    assert!(read_json(&target)["mcpServers"].get("local-tool").is_some());

    write(
        &case.config.join("opencode").join("opencode.jsonc"),
        "{\"mcp\": {}} // v1\n",
    );
    assert_eq!(
        applied(&case, "local-tool", &["opencode"], "on").get("ok"),
        Some(&json!(true))
    );
    assert!(case.config.join("opencode").join("opencode.json").exists());
}

#[test]
fn unsupported_transports_are_refused() {
    let _guard = serial();
    let case = prepared();
    let result = applied(&case, "events", &["codex", "antigravity"], "on");
    assert_eq!(result.get("ok"), Some(&json!(false)));
    let rows = results(&result);
    let codex = rows
        .iter()
        .find(|row| text(row, "agent") == "codex")
        .unwrap();
    assert_eq!(field(codex, "ok"), &json!(false));
    assert_eq!(
        text(codex, "message"),
        "transport sse is not supported by codex"
    );
    let antigravity = rows
        .iter()
        .find(|row| text(row, "agent") == "antigravity")
        .unwrap();
    assert_eq!(field(antigravity, "ok"), &json!(true));
    assert_eq!(
        read_json(
            &case
                .home
                .join(".gemini")
                .join("config")
                .join("mcp_config.json")
        )["mcpServers"]["events"]["serverUrl"],
        json!("https://mcp.example.invalid/sse")
    );
    assert!(!case.home.join(".codex").join("config.toml").exists());

    let socket = applied(&case, "socket-tool", &["antigravity"], "on");
    assert_eq!(socket.get("ok"), Some(&json!(false)));
    assert_eq!(socket.get("results"), Some(&json!([])));
    assert_eq!(
        socket.get("message"),
        Some(&json!("transport unix cannot be copied"))
    );
}

#[test]
fn an_unknown_id_or_agent_is_reported_without_writing() {
    let _guard = serial();
    let case = prepared();
    let unknown =
        Applier::new(case.inventory()).apply(&"0".repeat(24), &["codex".to_string()], "on");
    assert_eq!(
        unknown,
        json!({
            "ok": false,
            "schemaVersion": 1,
            "project": "<project>",
            "changed": false,
            "message": "unknown definition id",
            "results": [],
        })
    );
    let result = applied(&case, "local-tool", &["cursor"], "on");
    assert_eq!(result.get("ok"), Some(&json!(false)));
    assert_eq!(
        results(&result)[0],
        json!({"agent": "cursor", "ok": false, "changed": false, "message": "unknown agent", "touched": []})
    );
    let aliased = applied(
        &case,
        "local-tool",
        &["google-antigravity", "antigravity"],
        "on",
    );
    let agents: Vec<String> = results(&aliased)
        .iter()
        .map(|row| text(row, "agent").to_string())
        .collect();
    assert_eq!(agents, ["antigravity"]);
}

#[test]
fn the_pi_target_follows_the_enabled_adapter() {
    let _guard = serial();
    let case = prepared();
    json_write(
        &case.home.join(".pi").join("agent").join("settings.json"),
        &json!({"packages": ["pi-codemode-mcp"]}),
    );
    let result = applied(&case, "local-tool", &["pi"], "on");
    assert_eq!(result.get("ok"), Some(&json!(true)), "{result:?}");
    assert_eq!(
        field(&results(&result)[0], "touched"),
        &json!(["~/.pi/agent/.mcp.json"])
    );
    json_write(
        &case.home.join(".pi").join("agent").join("settings.json"),
        &json!({"packages": []}),
    );
    let refused = applied(&case, "local-tool", &["pi"], "on");
    assert_eq!(refused.get("ok"), Some(&json!(false)));
    assert_eq!(
        text(&results(&refused)[0], "message"),
        "Pi has no enabled MCP adapter package"
    );
}

#[test]
fn an_opencode_v1_map_stays_in_its_v1_shape() {
    let _guard = serial();
    let case = prepared();
    let opencode = case.config.join("opencode").join("opencode.json");
    json_write(
        &opencode,
        &json!({"mcp": {"existing": {"type": "remote", "url": "https://old.invalid"}}}),
    );
    let result = applied(&case, "local-tool", &["opencode"], "on");
    assert_eq!(
        text(&results(&result)[0], "message"),
        "written to mcp (v1 map)"
    );
    let document = read_json(&opencode);
    let keys: Vec<&String> = document["mcp"].as_object().unwrap().keys().collect();
    assert_eq!(keys.len(), 2);
    assert!(document["mcp"].get("servers").is_none());
    assert_eq!(
        applied(&case, "local-tool", &["opencode"], "off").get("changed"),
        Some(&json!(true))
    );
    assert_eq!(read_json(&opencode)["mcp"].as_object().unwrap().len(), 1);
}

#[test]
fn listing_never_writes_to_a_configuration_file() {
    let _guard = serial();
    let case = prepared();
    applied(&case, "local-tool", &["all"], "on");
    let snapshot = |root: &Path| -> std::collections::BTreeMap<String, Vec<u8>> {
        let mut found = std::collections::BTreeMap::new();
        let mut stack = vec![root.to_path_buf()];
        while let Some(directory) = stack.pop() {
            let Ok(entries) = fs::read_dir(&directory) else {
                continue;
            };
            for entry in entries.flatten() {
                let path = entry.path();
                let Ok(metadata) = fs::symlink_metadata(&path) else {
                    continue;
                };
                if metadata.is_dir() {
                    stack.push(path);
                } else if metadata.is_file() {
                    found.insert(
                        path.to_string_lossy().into_owned(),
                        fs::read(&path).unwrap(),
                    );
                }
            }
        }
        found
    };
    let before = snapshot(&case.base);
    let document = case.scan();
    assert!(!bounded_json(&document).contains(SENTINEL));
    let after = snapshot(&case.base);
    assert_eq!(after, before);
}
