#![allow(dead_code)]

use fileblade::core_modules::hooks::discovery;
use fileblade::core_modules::hooks::labels::Environ;
use fileblade::core_modules::hooks::safeio::Budget;
use serde_json::{Map, Value, json};
use std::ffi::{OsStr, OsString};
use std::fs;
use std::path::{Path, PathBuf};

pub const CANARY_TOKEN: &str = "sk-CANARY-0000-SECRET";
pub const CANARY_ENV_KEY: &str = "CANARY_API_KEY";
pub const CANARY_ENV_VALUE: &str = "CANARY-ENV-VALUE";
pub const CANARY_URL: &str = "https://canary.example.invalid/hook?token=CANARY-QUERY";
pub const CANARY_HEADER: &str = "CANARY-HEADER-VALUE";
pub const CANARY_PATH: &str = "/canary/secret/operand.txt";

pub fn canary_command() -> String {
    format!("curl -H 'Authorization: {CANARY_TOKEN}' {CANARY_URL} > {CANARY_PATH}")
}

pub fn write_json(path: &Path, payload: &Value) -> PathBuf {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, serde_json::to_string_pretty(payload).unwrap()).unwrap();
    path.to_path_buf()
}

pub fn write_text(path: &Path, text: &str) -> PathBuf {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
    path.to_path_buf()
}

fn claude_fixtures(home: &Path, project: &Path) {
    let command = canary_command();
    write_json(
        &home.join(".claude/settings.json"),
        &json!({
            "hooks": {
                "PreToolUse": [{
                    "matcher": "Bash",
                    "hooks": [
                        {"type": "command", "command": command, "timeout": 30},
                        {"type": "command", "command": format!("echo {CANARY_TOKEN}"), "if": "true"},
                    ],
                }],
                "SessionStart": [{"hooks": [{"type": "command", "command": format!("cat {CANARY_PATH}")}]}],
                "NotARealEvent": [{"hooks": [{"type": "command", "command": "echo hi"}]}],
            },
        }),
    );
    write_json(
        &project.join(".claude/settings.json"),
        &json!({"hooks": {"Stop": [{"hooks": [{"type": "command", "command": command}]}]}}),
    );
    write_json(
        &project.join(".claude/settings.local.json"),
        &json!({"hooks": {"PostToolUse": [{"hooks": [{"type": "command", "command": "local-only"}]}]}}),
    );
    write_json(
        &home.join(".claude/plugins/cache/market/demo/1.0.0/hooks/hooks.json"),
        &json!({"hooks": {"PostToolUse": [{"hooks": [{"type": "command", "command": format!("plugin {CANARY_PATH}")}]}]}}),
    );
}

fn codex_fixtures(home: &Path, project: &Path) {
    let command = canary_command();
    write_json(
        &home.join(".codex/hooks.json"),
        &json!({"Stop": [{"hooks": [{"type": "command", "command": command, "timeout": 5}]}]}),
    );
    write_text(
        &home.join(".codex/config.toml"),
        "model = \"gpt-5\"\n\n[features]\nhooks = true\n\n[hooks]\nPreCompact = [{ hooks = [{ type = \"command\", command = \"inline-compact\" }] }]\n",
    );
    write_text(
        &home.join(".codex/t\u{fb}rk\u{e7}e-pr\u{f6}fil.config.toml"),
        "[hooks]\nSubagentStart = [{ hooks = [{ type = \"command\", command = \"profile-unicode\" }] }]\n",
    );
    write_text(
        &home.join(".codex/plain.config.toml"),
        "[hooks]\nStop = [{ hooks = [{ type = \"command\", command = \"profile-plain\" }] }]\n",
    );
    write_json(
        &project.join(".codex/hooks.json"),
        &json!({"hooks": {"SubagentStop": [{"hooks": [{"type": "command", "command": format!("proj {CANARY_PATH}")}]}]}}),
    );
}

pub fn copilot_policy_fixtures(etc: &Path) {
    write_json(
        &etc.join("github-copilot/policy.d/20-second.json"),
        &json!({"version": 1, "hooks": {"preToolUse": [{"hooks": [{"type": "command", "command": "policy-second"}]}]}}),
    );
    write_json(
        &etc.join("github-copilot/policy.d/10-first.json"),
        &json!({"version": 1, "hooks": {"sessionStart": [{"hooks": [{"type": "command", "command": "policy-first"}]}]}}),
    );
    write_json(
        &etc.join("github-copilot/policy.d/30-badversion.json"),
        &json!({"version": 7, "hooks": {"notification": [{"hooks": [{"type": "command", "command": "POLICY-BAD-VERSION"}]}]}}),
    );
    let loose = write_json(
        &etc.join("github-copilot/policy.d/40-loose.json"),
        &json!({"version": 1, "hooks": {"agentStop": [{"hooks": [{"type": "command", "command": "POLICY-GROUP-WRITABLE"}]}]}}),
    );
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&loose, fs::Permissions::from_mode(0o664)).unwrap();
}

fn copilot_plugin_fixtures(home: &Path) {
    let market = home.join(".copilot/installed-plugins/acme/guard");
    write_json(
        &market.join(".plugin/plugin.json"),
        &json!({"name": "guard-plugin"}),
    );
    write_json(
        &market.join("hooks.json"),
        &json!({"hooks": {"preToolUse": [{"hooks": [{"type": "command", "command": format!("plugin {CANARY_PATH}")}]}]}}),
    );
    let direct = home.join(".copilot/installed-plugins/_direct/local-source");
    write_json(
        &direct.join(".claude-plugin/plugin.json"),
        &json!({"name": "direct-plugin"}),
    );
    write_json(
        &direct.join("hooks/hooks.json"),
        &json!({"hooks": {"Notification": [{"hooks": [{"type": "command", "command": "direct-nested"}]}]}}),
    );
}

fn copilot_settings_fixtures(home: &Path, project: &Path) {
    write_json(
        &home.join(".copilot/settings.json"),
        &json!({"hooks": {"userPromptSubmitted": [{"hooks": [{"type": "command", "command": "user-inline"}]}]}}),
    );
    write_json(
        &project.join(".github/copilot/settings.local.json"),
        &json!({"hooks": {"PreToolUse": [{"hooks": [{"type": "command", "command": "alias-inline"}]}]}}),
    );
}

fn copilot_fixtures(home: &Path, project: &Path) {
    let command = canary_command();
    write_json(
        &home.join(".copilot/hooks/guard.json"),
        &json!({
            "version": 1,
            "hooks": {
                "preToolUse": [{
                    "matcher": "shell",
                    "hooks": [{
                        "type": "command",
                        "bash": command,
                        "powershell": "Write-Host secret",
                        "cwd": CANARY_PATH,
                        "env": {CANARY_ENV_KEY: CANARY_ENV_VALUE},
                        "allowedEnvVars": [CANARY_ENV_KEY],
                        "timeoutSec": 15,
                    }],
                }],
                "sessionStart": [{"hooks": [{"type": "http", "url": CANARY_URL,
                                             "headers": {"Authorization": CANARY_HEADER}}]}],
            },
        }),
    );
    write_json(
        &home.join(".copilot/hooks/off.json"),
        &json!({
            "version": 1,
            "disableAllHooks": true,
            "hooks": {"agentStop": [{"hooks": [{"type": "command", "command": "never runs"}]}]},
        }),
    );
    write_json(
        &project.join(".github/hooks/repo.json"),
        &json!({"version": 1, "hooks": {"postToolUse": [{"hooks": [{"type": "prompt", "prompt": format!("summarize {CANARY_PATH}")}]}]}}),
    );
    write_json(
        &project.join(".github/hooks/badversion.json"),
        &json!({"version": 99, "hooks": {"notification": [{"hooks": [{"type": "command", "command": "REJECTED-BAD-VERSION"}]}]}}),
    );
    write_json(
        &project.join(".github/copilot/settings.json"),
        &json!({"hooks": {"subagentStart": [{"hooks": [{"type": "command", "command": "inline-settings"}]}]}}),
    );
}

fn antigravity_fixtures(home: &Path, project: &Path) {
    let command = canary_command();
    write_json(
        &home.join(".gemini/config/hooks.json"),
        &json!({
            "guard": {
                "enabled": true,
                "PreToolUse": [{"type": "command", "command": command, "timeout": 20}],
                "Stop": [{"type": "command", "command": "cleanup"}],
            },
            "disabled-hook": {
                "enabled": false,
                "PostToolUse": [{"type": "command", "command": "audit"}],
            },
        }),
    );
    write_json(
        &project.join(".agents/hooks.json"),
        &json!({"workspace": {"PreInvocation": [{"type": "command", "command": format!("inject {CANARY_TOKEN}")}]}}),
    );
}

fn opencode_fixtures(home: &Path, project: &Path) {
    let command = canary_command();
    write_text(
        &home.join(".config/opencode/plugins/alpha.ts"),
        &format!("export const plugin = () => ({{ command: '{command}' }})\n"),
    );
    write_text(
        &project.join(".opencode/plugins/beta.js"),
        &format!("module.exports = () => require('child_process').exec('{command}')\n"),
    );
    write_json(
        &home.join(".config/opencode/opencode.json"),
        &json!({"plugin": ["opencode-example", "@scope/other"]}),
    );
}

fn pi_fixtures(home: &Path, project: &Path) {
    let command = canary_command();
    write_text(
        &home.join(".pi/agent/extensions/hooky.ts"),
        &format!("export default (pi) => pi.on('tool_call', () => '{command}')\n"),
    );
    write_json(
        &home.join(".pi/agent/settings.json"),
        &json!({"extensions": ["./local.ts"], "packages": ["pi-plugin-a", "pi-plugin-b"]}),
    );
    write_text(
        &project.join(".pi/extensions/proj.ts"),
        "export default () => {}\n",
    );
}

fn unsupported_noise(home: &Path, project: &Path) {
    let _ = project;
    write_json(
        &home.join(".cursor/hooks.json"),
        &json!({"hooks": {"PreToolUse": [{"hooks": [{"command": "nope"}]}]}}),
    );
    write_json(
        &home.join(".gemini/antigravity-cli/plugins/demo/hooks.json"),
        &json!({"plugin-hook": {"PostInvocation": [{"type": "command", "command": "UNDOCUMENTED-PLUGIN-PATH"}]}}),
    );
    write_json(
        &home.join(".claude/credentials.json"),
        &json!({"token": CANARY_TOKEN}),
    );
    write_text(
        &home.join(".claude/.credentials.json"),
        &serde_json::to_string(&json!({"token": CANARY_TOKEN})).unwrap(),
    );
    write_text(
        &home.join(".netrc"),
        &format!("machine example.invalid password {CANARY_TOKEN}\n"),
    );
}

pub fn build_all(home: &Path, project: &Path, etc: Option<&Path>) {
    fs::create_dir_all(project.join(".git")).unwrap();
    if let Some(etc) = etc {
        copilot_policy_fixtures(etc);
    }
    copilot_plugin_fixtures(home);
    copilot_settings_fixtures(home, project);
    claude_fixtures(home, project);
    codex_fixtures(home, project);
    copilot_fixtures(home, project);
    antigravity_fixtures(home, project);
    opencode_fixtures(home, project);
    pi_fixtures(home, project);
    unsupported_noise(home, project);
}

pub fn environ(home: &Path) -> Environ {
    vec![(OsString::from("HOME"), OsString::from(home))]
}

pub fn environ_with(home: &Path, extra: &[(&str, &OsStr)]) -> Environ {
    let mut variables = environ(home);
    for (key, value) in extra {
        variables.push((OsString::from(*key), (*value).to_os_string()));
    }
    variables
}

pub fn collect(home: &Path, project: &Path, etc: &Path, uid: u32) -> Map<String, Value> {
    let mut budget = Budget::default();
    discovery::collect(
        &mut budget,
        &discovery::Query {
            project: &project.to_string_lossy(),
            home: &home.to_string_lossy(),
            etc_root: &etc.to_string_lossy(),
            policy_owner_uid: uid,
            exact: false,
            scope: "all",
        },
        environ(home),
    )
}

pub fn collect_with(
    home: &Path,
    project: &Path,
    variables: Environ,
    etc: &Path,
    uid: u32,
    exact: bool,
    scope: &str,
) -> (Map<String, Value>, Budget) {
    let mut budget = Budget::default();
    let document = discovery::collect(
        &mut budget,
        &discovery::Query {
            project: &project.to_string_lossy(),
            home: &home.to_string_lossy(),
            etc_root: &etc.to_string_lossy(),
            policy_owner_uid: uid,
            exact,
            scope,
        },
        variables,
    );
    (document, budget)
}

pub fn rows(document: &Map<String, Value>) -> Vec<Map<String, Value>> {
    document
        .get("items")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| item.as_object().cloned())
                .collect()
        })
        .unwrap_or_default()
}

pub fn rows_for(document: &Map<String, Value>, agent: &str) -> Vec<Map<String, Value>> {
    rows(document)
        .into_iter()
        .filter(|row| row.get("agent").and_then(Value::as_str) == Some(agent))
        .collect()
}

pub fn text(row: &Map<String, Value>, key: &str) -> String {
    row.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

pub fn summary(row: &Map<String, Value>, key: &str) -> Value {
    row.get("summary")
        .and_then(Value::as_object)
        .and_then(|summary| summary.get(key))
        .cloned()
        .unwrap_or(Value::Null)
}

pub fn badges(row: &Map<String, Value>) -> Vec<String> {
    row.get("badges")
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_string)
                .collect()
        })
        .unwrap_or_default()
}

pub fn source_path(row: &Map<String, Value>) -> String {
    row.get("source")
        .and_then(Value::as_object)
        .and_then(|source| source.get("path"))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}
