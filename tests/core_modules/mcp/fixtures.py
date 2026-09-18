from __future__ import annotations

import json
from pathlib import Path

SENTINEL = "SECRET_SENTINEL_DO_NOT_EMIT"


def write(path: Path, text: str) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")


def json_write(path: Path, value: object) -> None:
    write(path, json.dumps(value))


def build_all(home: Path, project: Path) -> None:
    config = home / ".config"
    project.mkdir(parents=True, exist_ok=True)
    json_write(home / ".claude.json", {
        "mcpServers": {
            "shared": {"command": SENTINEL, "args": [SENTINEL], "env": {SENTINEL: SENTINEL}},
            SENTINEL: {"url": "https://" + SENTINEL},
        },
        "projects": {
            str(project): {
                "mcpServers": {"shared": {"command": "local"}},
                "enabledMcpjsonServers": ["project-only"],
            }
        },
    })
    json_write(project / ".mcp.json", {
        "mcpServers": {
            "shared": {"url": "https://" + SENTINEL, "headers": {SENTINEL: SENTINEL}},
            "project-only": {"url": "https://" + SENTINEL},
            "pi-high": {"command": "project"},
        }
    })

    write(home / ".codex" / "config.toml", f'''
[mcp_servers.codex-user]
command = "{SENTINEL}"
args = ["{SENTINEL}"]
env = {{ {SENTINEL} = "{SENTINEL}" }}

[projects."{project}"]
trust_level = "trusted"

[plugins."codex-plugin@market".mcp_servers.plugin-server]
enabled = false
''')
    codex_plugin = home / ".codex" / "plugins" / "cache" / "market" / "codex-plugin" / "1.0.0"
    json_write(codex_plugin / ".codex-plugin" / "plugin.json", {
        "name": "codex-plugin",
        "mcpServers": ".mcp.json",
    })
    json_write(codex_plugin / ".mcp.json", {
        "mcpServers": {"plugin-server": {"url": "https://" + SENTINEL}}
    })

    write(config / "opencode" / "opencode.jsonc", '''
    { // user v1
      "mcp": {"open-user": {"type": "remote", "url": "https://example.invalid"}},
    }
    ''')
    json_write(project / "opencode.json", {
        "mcp": {"servers": {"open-same": {"type": "local", "command": [SENTINEL]}}}
    })
    json_write(project.parent / ".opencode" / "opencode.json", {
        "mcp": {"servers": {"open-same": {"type": "remote", "url": "https://" + SENTINEL}}}
    })

    json_write(home / ".pi" / "agent" / "settings.json", {
        "packages": ["npm:pi-mcp-adapter@2.0.0"]
    })
    json_write(config / "mcp" / "mcp.json", {
        "mcpServers": {"pi-high": {"command": SENTINEL}}
    })
    json_write(project / ".pi" / "mcp.json", {
        "mcpServers": {"pi-high": {"command": SENTINEL, "disabled": True}}
    })

    json_write(home / ".copilot" / "mcp-config.json", {
        "mcpServers": {"copilot-same": {"command": SENTINEL}}
    })
    json_write(project / ".github" / "mcp.json", {
        "mcpServers": {"copilot-same": {"url": "https://" + SENTINEL}}
    })
    json_write(project / ".mcp.json", {
        "mcpServers": {
            "shared": {"url": "https://" + SENTINEL, "headers": {SENTINEL: SENTINEL}},
            "project-only": {"url": "https://" + SENTINEL},
            "pi-high": {"command": "project"},
            "copilot-same": {"command": SENTINEL},
        }
    })
    write(home / ".copilot" / "settings.json", '''
    {
      // explicit plugin state
      "enabledPlugins": {"tools@market": true},
      "disabledMcpServers": ["disabled-by-setting"],
    }
    ''')
    copilot_plugin = home / ".copilot" / "installed-plugins" / "market" / "tools"
    json_write(copilot_plugin / ".plugin" / "plugin.json", {
        "name": "tools", "mcpServers": ".mcp.json"
    })
    json_write(copilot_plugin / ".mcp.json", {
        "mcpServers": {
            "copilot-same": {"command": SENTINEL},
            "disabled-by-setting": {"command": SENTINEL},
        }
    })

    json_write(home / ".gemini" / "config" / "mcp_config.json", {
        "mcpServers": {"anti-same": {"serverUrl": "https://" + SENTINEL}}
    })
    json_write(project / ".agents" / "mcp_config.json", {
        "mcpServers": {"anti-same": {"serverUrl": "https://" + SENTINEL, "disabled": True}}
    })
