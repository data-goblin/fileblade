use std::fs;
use std::path::Path;

const FILES: [(&str, &str); 18] = [
    (
        "home/.claude.json",
        r#"{"mcpServers": {"shared": {"command": "SECRET_SENTINEL_DO_NOT_EMIT", "args": ["SECRET_SENTINEL_DO_NOT_EMIT"], "env": {"SECRET_SENTINEL_DO_NOT_EMIT": "SECRET_SENTINEL_DO_NOT_EMIT"}}, "SECRET_SENTINEL_DO_NOT_EMIT": {"url": "https://SECRET_SENTINEL_DO_NOT_EMIT"}}, "projects": {"{PROJECT}": {"mcpServers": {"shared": {"command": "local"}}, "enabledMcpjsonServers": ["project-only"]}}}"#,
    ),
    (
        "home/.codex/config.toml",
        r#"
[mcp_servers.codex-user]
command = "SECRET_SENTINEL_DO_NOT_EMIT"
args = ["SECRET_SENTINEL_DO_NOT_EMIT"]
env = { SECRET_SENTINEL_DO_NOT_EMIT = "SECRET_SENTINEL_DO_NOT_EMIT" }

[projects."{PROJECT}"]
trust_level = "trusted"

[plugins."codex-plugin@market".mcp_servers.plugin-server]
enabled = false
"#,
    ),
    (
        "home/.codex/plugins/cache/market/codex-plugin/1.0.0/.codex-plugin/plugin.json",
        r#"{"name": "codex-plugin", "mcpServers": ".mcp.json"}"#,
    ),
    (
        "home/.codex/plugins/cache/market/codex-plugin/1.0.0/.mcp.json",
        r#"{"mcpServers": {"plugin-server": {"url": "https://SECRET_SENTINEL_DO_NOT_EMIT"}}}"#,
    ),
    (
        "home/.config/mcp/mcp.json",
        r#"{"mcpServers": {"pi-high": {"command": "SECRET_SENTINEL_DO_NOT_EMIT"}}}"#,
    ),
    (
        "home/.config/opencode/opencode.jsonc",
        r#"
        { // user v1
          "mcp": {"open-user": {"type": "remote", "url": "https://example.invalid"}},
        }
        "#,
    ),
    (
        "home/.copilot/installed-plugins/market/tools/.mcp.json",
        r#"{"mcpServers": {"copilot-same": {"command": "SECRET_SENTINEL_DO_NOT_EMIT"}, "disabled-by-setting": {"command": "SECRET_SENTINEL_DO_NOT_EMIT"}}}"#,
    ),
    (
        "home/.copilot/installed-plugins/market/tools/.plugin/plugin.json",
        r#"{"name": "tools", "mcpServers": ".mcp.json"}"#,
    ),
    (
        "home/.copilot/mcp-config.json",
        r#"{"mcpServers": {"copilot-same": {"command": "SECRET_SENTINEL_DO_NOT_EMIT"}}}"#,
    ),
    (
        "home/.copilot/settings.json",
        r#"
        {
          // explicit plugin state
          "enabledPlugins": {"tools@market": true},
          "disabledMcpServers": ["disabled-by-setting"],
        }
        "#,
    ),
    (
        "home/.gemini/config/mcp_config.json",
        r#"{"mcpServers": {"anti-same": {"serverUrl": "https://SECRET_SENTINEL_DO_NOT_EMIT"}}}"#,
    ),
    (
        "home/.pi/agent/settings.json",
        r#"{"packages": ["npm:pi-mcp-adapter@2.0.0"]}"#,
    ),
    (
        "work/.opencode/opencode.json",
        r#"{"mcp": {"servers": {"open-same": {"type": "remote", "url": "https://SECRET_SENTINEL_DO_NOT_EMIT"}}}}"#,
    ),
    (
        "work/project/.agents/mcp_config.json",
        r#"{"mcpServers": {"anti-same": {"serverUrl": "https://SECRET_SENTINEL_DO_NOT_EMIT", "disabled": true}}}"#,
    ),
    (
        "work/project/.github/mcp.json",
        r#"{"mcpServers": {"copilot-same": {"url": "https://SECRET_SENTINEL_DO_NOT_EMIT"}}}"#,
    ),
    (
        "work/project/.mcp.json",
        r#"{"mcpServers": {"shared": {"url": "https://SECRET_SENTINEL_DO_NOT_EMIT", "headers": {"SECRET_SENTINEL_DO_NOT_EMIT": "SECRET_SENTINEL_DO_NOT_EMIT"}}, "project-only": {"url": "https://SECRET_SENTINEL_DO_NOT_EMIT"}, "pi-high": {"command": "project"}, "copilot-same": {"command": "SECRET_SENTINEL_DO_NOT_EMIT"}}}"#,
    ),
    (
        "work/project/.pi/mcp.json",
        r#"{"mcpServers": {"pi-high": {"command": "SECRET_SENTINEL_DO_NOT_EMIT", "disabled": true}}}"#,
    ),
    (
        "work/project/opencode.json",
        r#"{"mcp": {"servers": {"open-same": {"type": "local", "command": ["SECRET_SENTINEL_DO_NOT_EMIT"]}}}}"#,
    ),
];

pub fn build_all(root: &Path, project: &Path) {
    let project_text = project.to_string_lossy().into_owned();
    for (name, body) in FILES {
        let path = root.join(name);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).unwrap();
        }
        fs::write(&path, body.replace("{PROJECT}", &project_text)).unwrap();
    }
}
