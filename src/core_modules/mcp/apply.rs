use super::inventory::{Inventory, MAX_CLAUDE_BYTES, MAX_CONFIG_BYTES, Settings};
use super::model::{Definition, SCHEMA_VERSION, core_agent_id, safe_label};
use super::parsers::{parse_json, parse_toml};
use super::records;
use super::tomlwrite::{append_server_block, remove_server_block, render_server_table};
use super::value::{Cfg, fingerprint, unrepresentable};
use crate::common::{parse_path, path_text};
use crate::core_modules::recovery_store::{RecoveryError, RecoveryStore};
use crate::core_modules::snapshot::Snapshot;
use serde_json::{Map, Value, json};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

pub const WRITE_AGENTS: [&str; 6] = [
    "claude-code",
    "codex",
    "opencode",
    "pi",
    "copilot-cli",
    "antigravity",
];

pub const MAX_RESTORE_PAYLOAD_BYTES: usize = 1024 * 1024;

fn supported_transports(agent: &str) -> &'static [&'static str] {
    match agent {
        "codex" => &["stdio", "streamable-http"],
        _ => &["stdio", "sse", "streamable-http"],
    }
}

pub struct Refused(pub String);

fn refuse(message: &str) -> Refused {
    Refused(message.to_string())
}

type Outcome<T> = Result<T, Refused>;

#[derive(Clone)]
pub struct ServerSpec {
    pub name: String,
    pub display: String,
    pub transport: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
    pub url: String,
    pub headers: Vec<(String, String)>,
}

pub struct AgentResult {
    pub agent: String,
    pub ok: bool,
    pub changed: bool,
    pub message: String,
    pub touched: Vec<String>,
}

impl AgentResult {
    fn new(agent: &str, ok: bool, changed: bool, message: &str, touched: Vec<String>) -> Self {
        Self {
            agent: agent.to_string(),
            ok,
            changed,
            message: message.to_string(),
            touched,
        }
    }

    pub fn public(&self) -> Value {
        json!({
            "agent": self.agent,
            "ok": self.ok,
            "changed": self.changed,
            "message": self.message,
            "touched": self.touched,
        })
    }
}

fn string_map(value: Option<&Cfg>) -> Vec<(String, String)> {
    let Some(Cfg::Table(entries)) = value else {
        return Vec::new();
    };
    entries
        .iter()
        .filter_map(|(key, item)| item.as_str().map(|text| (key.clone(), text.to_string())))
        .collect()
}

fn string_table(pairs: &[(String, String)]) -> Cfg {
    Cfg::Table(
        pairs
            .iter()
            .map(|(key, value)| (key.clone(), Cfg::Str(value.clone())))
            .collect(),
    )
}

fn string_array(values: &[String]) -> Cfg {
    Cfg::Array(values.iter().map(|value| Cfg::Str(value.clone())).collect())
}

pub fn server_spec(definition: &Definition) -> Outcome<ServerSpec> {
    let Some(config) = definition
        .raw_config
        .as_ref()
        .filter(|value| value.is_table())
    else {
        return Err(refuse("source definition is not an object"));
    };
    let transport = definition.transport.clone();
    let display = safe_label(&definition.raw_name, &definition.id);
    if transport != "stdio" && transport != "sse" && transport != "streamable-http" {
        return Err(Refused(format!("transport {transport} cannot be copied")));
    }
    let environment = config.get("env").or_else(|| config.get("environment"));
    if transport == "stdio" {
        let command = config.get("command");
        if let Some(Cfg::Array(items)) = command {
            if items.is_empty() || !items.iter().all(|item| item.as_str().is_some()) {
                return Err(refuse("source command is not a string list"));
            }
            let parts: Vec<String> = items
                .iter()
                .filter_map(Cfg::as_str)
                .map(str::to_string)
                .collect();
            return Ok(ServerSpec {
                name: definition.raw_name.clone(),
                display,
                transport,
                command: parts[0].clone(),
                args: parts[1..].to_vec(),
                env: string_map(environment),
                url: String::new(),
                headers: Vec::new(),
            });
        }
        let Some(command) = command
            .and_then(Cfg::as_str)
            .filter(|text| !text.is_empty())
        else {
            return Err(refuse("source command is missing"));
        };
        let args = match config.get("args") {
            None => Vec::new(),
            Some(Cfg::Array(items)) if items.iter().all(|item| item.as_str().is_some()) => items
                .iter()
                .filter_map(Cfg::as_str)
                .map(str::to_string)
                .collect(),
            _ => return Err(refuse("source args are not a string list")),
        };
        return Ok(ServerSpec {
            name: definition.raw_name.clone(),
            display,
            transport,
            command: command.to_string(),
            args,
            env: string_map(environment),
            url: String::new(),
            headers: Vec::new(),
        });
    }
    let mut url = String::new();
    for key in ["httpUrl", "serverUrl", "url"] {
        if let Some(text) = config.get(key).and_then(Cfg::as_str) {
            url = text.to_string();
            break;
        }
    }
    if url.is_empty() {
        return Err(refuse("source url is missing"));
    }
    let headers = string_map(config.get("headers").or_else(|| config.get("http_headers")));
    Ok(ServerSpec {
        name: definition.raw_name.clone(),
        display,
        transport,
        command: String::new(),
        args: Vec::new(),
        env: Vec::new(),
        url,
        headers,
    })
}

pub fn render_entry(agent: &str, spec: &ServerSpec) -> Outcome<Cfg> {
    let stdio = spec.transport == "stdio";
    let http = spec.transport == "streamable-http";
    let mut entry: Vec<(String, Cfg)> = Vec::new();
    let mut push = |key: &str, value: Cfg| entry.push((key.to_string(), value));
    match agent {
        "claude-code" => {
            if stdio {
                push("type", Cfg::Str("stdio".to_string()));
                push("command", Cfg::Str(spec.command.clone()));
                push("args", string_array(&spec.args));
                if !spec.env.is_empty() {
                    push("env", string_table(&spec.env));
                }
            } else {
                push(
                    "type",
                    Cfg::Str(if http { "http" } else { "sse" }.to_string()),
                );
                push("url", Cfg::Str(spec.url.clone()));
                if !spec.headers.is_empty() {
                    push("headers", string_table(&spec.headers));
                }
            }
        }
        "codex" | "pi" => {
            if stdio {
                push("command", Cfg::Str(spec.command.clone()));
                if !spec.args.is_empty() {
                    push("args", string_array(&spec.args));
                }
                if !spec.env.is_empty() {
                    push("env", string_table(&spec.env));
                }
            } else {
                push("url", Cfg::Str(spec.url.clone()));
                if !spec.headers.is_empty() {
                    let key = if agent == "codex" {
                        "http_headers"
                    } else {
                        "headers"
                    };
                    push(key, string_table(&spec.headers));
                }
            }
        }
        "opencode" => {
            if stdio {
                push("type", Cfg::Str("local".to_string()));
                let mut command = vec![spec.command.clone()];
                command.extend(spec.args.clone());
                push("command", string_array(&command));
                if !spec.env.is_empty() {
                    push("environment", string_table(&spec.env));
                }
            } else {
                push("type", Cfg::Str("remote".to_string()));
                push("url", Cfg::Str(spec.url.clone()));
                if !spec.headers.is_empty() {
                    push("headers", string_table(&spec.headers));
                }
            }
        }
        "copilot-cli" => {
            if stdio {
                push("type", Cfg::Str("local".to_string()));
                push("command", Cfg::Str(spec.command.clone()));
                push("args", string_array(&spec.args));
                if !spec.env.is_empty() {
                    push("env", string_table(&spec.env));
                }
            } else {
                push(
                    "type",
                    Cfg::Str(if http { "http" } else { "sse" }.to_string()),
                );
                push("url", Cfg::Str(spec.url.clone()));
                if !spec.headers.is_empty() {
                    push("headers", string_table(&spec.headers));
                }
            }
            push("tools", string_array(&["*".to_string()]));
        }
        "antigravity" => {
            if stdio {
                push("command", Cfg::Str(spec.command.clone()));
                if !spec.args.is_empty() {
                    push("args", string_array(&spec.args));
                }
                if !spec.env.is_empty() {
                    push("env", string_table(&spec.env));
                }
            } else {
                push("serverUrl", Cfg::Str(spec.url.clone()));
                if !spec.headers.is_empty() {
                    push("headers", string_table(&spec.headers));
                }
            }
        }
        _ => return Err(refuse("unknown agent")),
    }
    Ok(Cfg::Table(entry))
}

fn conflict_message(spec: &ServerSpec, label: &str) -> String {
    format!(
        "a different server named {} already exists in {label}; nothing was changed",
        spec.display
    )
}

pub struct Applier {
    pub inventory: Inventory,
    snapshots: BTreeMap<PathBuf, Snapshot>,
    pub recovery: RecoveryStore,
}

fn rendered_json(document: &Cfg) -> Outcome<Vec<u8>> {
    if unrepresentable(document) {
        return Err(refuse(
            "the updated configuration cannot be represented as JSON",
        ));
    }
    let mut text = serde_json::to_string_pretty(&document.to_json())
        .map_err(|_| refuse("the updated configuration cannot be represented as JSON"))?;
    text.push('\n');
    Ok(text.into_bytes())
}

impl Applier {
    pub fn new(inventory: Inventory) -> Self {
        let recovery = RecoveryStore::new(inventory.recovery_directory());
        Self {
            inventory,
            snapshots: BTreeMap::new(),
            recovery,
        }
    }

    fn home(&self) -> PathBuf {
        self.inventory.settings.home.clone()
    }

    fn pi_target(&mut self) -> Outcome<PathBuf> {
        let settings_path = self.home().join(".pi").join("agent").join("settings.json");
        let mut plan = std::mem::take(&mut self.inventory.plan);
        let result = super::safeio::bounded_read(
            &mut plan,
            &settings_path,
            MAX_CONFIG_BYTES,
            &super::safeio::Options::default(),
        );
        self.inventory.plan = plan;
        let packages = result
            .ok()
            .and_then(|data| parse_json(&data).ok())
            .map(|document| self.inventory.pi_packages(&document))
            .unwrap_or_default();
        if packages.contains("pi-mcp-adapter") {
            return Ok(self.home().join(".pi").join("agent").join("mcp.json"));
        }
        if packages.contains("pi-codemode-mcp") {
            return Ok(self.home().join(".pi").join("agent").join(".mcp.json"));
        }
        Err(refuse("Pi has no enabled MCP adapter package"))
    }

    fn target_path(&mut self, agent: &str) -> Outcome<PathBuf> {
        match agent {
            "claude-code" => Ok(self.home().join(".claude.json")),
            "codex" => Ok(self.inventory.settings.codex_home.join("config.toml")),
            "opencode" => Ok(self
                .inventory
                .settings
                .config_home
                .join("opencode")
                .join("opencode.json")),
            "copilot-cli" => Ok(self.home().join(".copilot").join("mcp-config.json")),
            "antigravity" => Ok(self
                .home()
                .join(".gemini")
                .join("config")
                .join("mcp_config.json")),
            "pi" => self.pi_target(),
            _ => Err(refuse("unknown agent")),
        }
    }

    fn read_target(&mut self, path: &Path, limit: usize) -> Outcome<Option<Vec<u8>>> {
        let snapshot = Snapshot::read(path, limit).map_err(|error| Refused(error.to_string()))?;
        let data = snapshot.data.clone();
        self.snapshots.insert(path.to_path_buf(), snapshot);
        Ok(data)
    }

    fn read_json_target(&mut self, path: &Path, limit: usize) -> Outcome<Cfg> {
        let Some(data) = self.read_target(path, limit)? else {
            return Ok(Cfg::table());
        };
        parse_json(&data).map_err(|_| refuse("target is not strict JSON"))
    }

    fn atomic_write(&self, path: &Path, data: &[u8]) -> Outcome<()> {
        let result = match self.snapshots.get(path) {
            Some(snapshot) => snapshot.write(data),
            None => Snapshot::read(path, crate::core_modules::snapshot::MAX_CONTENT)
                .map_err(|error| crate::AppError::invalid(error.to_string()))
                .and_then(|snapshot| snapshot.write(data)),
        };
        result.map_err(|error| Refused(format!("target is {error}")))
    }

    fn write_json_target(&self, path: &Path, document: &Cfg) -> Outcome<()> {
        let payload = rendered_json(document)?;
        self.atomic_write(path, &payload)
    }

    fn json_container(
        agent: &str,
        document: &mut Cfg,
        create: bool,
    ) -> (Option<Vec<String>>, String) {
        if agent == "opencode" {
            let mcp = document.get("mcp").cloned();
            if let Some(value) = mcp.as_ref().filter(|value| value.is_table())
                && !value.contains("servers")
                && !value.keys().is_empty()
            {
                return (Some(vec!["mcp".to_string()]), "mcp (v1 map)".to_string());
            }
            if mcp.as_ref().is_none_or(|value| !value.is_table()) {
                if !create {
                    return (None, "mcp.servers".to_string());
                }
                document.insert("mcp", Cfg::table());
            }
            let servers = document
                .get("mcp")
                .and_then(|value| value.get("servers"))
                .cloned();
            if servers.is_none_or(|value| !value.is_table()) {
                if !create {
                    return (None, "mcp.servers".to_string());
                }
                if let Some(mcp) = document.as_table_mut().and_then(|entries| {
                    entries
                        .iter_mut()
                        .find(|(key, _)| key == "mcp")
                        .map(|(_, value)| value)
                }) {
                    mcp.insert("servers", Cfg::table());
                }
            }
            return (
                Some(vec!["mcp".to_string(), "servers".to_string()]),
                "mcp.servers".to_string(),
            );
        }
        let servers = document.get("mcpServers").cloned();
        if servers.is_none_or(|value| !value.is_table()) {
            if !create {
                return (None, "mcpServers".to_string());
            }
            document.insert("mcpServers", Cfg::table());
        }
        (
            Some(vec!["mcpServers".to_string()]),
            "mcpServers".to_string(),
        )
    }

    fn at<'a>(document: &'a mut Cfg, path: &[String]) -> Option<&'a mut Cfg> {
        let mut current = document;
        for key in path {
            current = current.as_table_mut().and_then(|entries| {
                entries
                    .iter_mut()
                    .find(|(name, _)| name == key)
                    .map(|(_, value)| value)
            })?;
        }
        Some(current)
    }

    fn apply_json(
        &mut self,
        agent: &str,
        path: &Path,
        spec: &ServerSpec,
        state: &str,
        limit: usize,
    ) -> Outcome<AgentResult> {
        let logical = String::from_utf8_lossy(&self.inventory.logical_path(path)).into_owned();
        let mut document = self.read_json_target(path, limit)?;
        if state == "off" {
            let (container, label) = Self::json_container(agent, &mut document, false);
            let present = container
                .as_ref()
                .and_then(|path| Self::at(&mut document, path))
                .is_some_and(|value| value.contains(&spec.name));
            if !present {
                return Ok(AgentResult::new(agent, true, false, "not present", vec![]));
            }
            if let Some(container) = container
                .as_ref()
                .and_then(|path| Self::at(&mut document, path))
            {
                container.remove(&spec.name);
            }
            self.write_json_target(path, &document)?;
            return Ok(AgentResult::new(
                agent,
                true,
                true,
                &format!("removed from {label}"),
                vec![logical],
            ));
        }
        let (container, label) = Self::json_container(agent, &mut document, true);
        let entry = render_entry(agent, spec)?;
        let existing = container
            .as_ref()
            .and_then(|path| Self::at(&mut document, path))
            .and_then(|value| value.get(&spec.name))
            .cloned();
        if let Some(existing) = existing {
            if existing == entry {
                return Ok(AgentResult::new(
                    agent,
                    true,
                    false,
                    "already present",
                    vec![],
                ));
            }
            return Ok(AgentResult::new(
                agent,
                false,
                false,
                &conflict_message(spec, &label),
                vec![],
            ));
        }
        if let Some(container) = container
            .as_ref()
            .and_then(|path| Self::at(&mut document, path))
        {
            container.insert(&spec.name, entry);
        }
        self.write_json_target(path, &document)?;
        Ok(AgentResult::new(
            agent,
            true,
            true,
            &format!("written to {label}"),
            vec![logical],
        ))
    }

    fn apply_toml(
        &mut self,
        agent: &str,
        path: &Path,
        spec: &ServerSpec,
        state: &str,
    ) -> Outcome<AgentResult> {
        let logical = String::from_utf8_lossy(&self.inventory.logical_path(path)).into_owned();
        let data = self.read_target(path, MAX_CONFIG_BYTES)?;
        let mut text = String::new();
        let mut document = Cfg::table();
        if let Some(data) = data.as_deref() {
            let parsed = parse_toml(data).map_err(|_| refuse("target is not strict TOML"))?;
            let decoded =
                std::str::from_utf8(data).map_err(|_| refuse("target is not strict TOML"))?;
            document = parsed;
            text = decoded.to_string();
        }
        let existing = document
            .get("mcp_servers")
            .filter(|value| value.is_table())
            .and_then(|servers| servers.get(&spec.name))
            .cloned();
        let entry = render_entry(agent, spec)?;
        let updated = if state == "off" {
            if existing.is_none() {
                return Ok(AgentResult::new(agent, true, false, "not present", vec![]));
            }
            remove_server_block(&text, &spec.name)
                .map_err(|error| Refused(format!("cannot locate table: {error}")))?
        } else {
            if let Some(existing) = existing.as_ref() {
                if *existing == entry {
                    return Ok(AgentResult::new(
                        agent,
                        true,
                        false,
                        "already present",
                        vec![],
                    ));
                }
                return Ok(AgentResult::new(
                    agent,
                    false,
                    false,
                    &conflict_message(spec, "mcp_servers"),
                    vec![],
                ));
            }
            let block = render_server_table(&spec.name, &entry)
                .map_err(|error| Refused(format!("cannot locate table: {error}")))?;
            append_server_block(&text, &block)
        };
        let verified =
            parse_toml(updated.as_bytes()).map_err(|_| refuse("edited TOML would not parse"))?;
        let verified_servers = verified.get("mcp_servers").filter(|value| value.is_table());
        let present = verified_servers.is_some_and(|value| value.contains(&spec.name));
        if state == "off" && present {
            return Err(refuse("table is also defined elsewhere in the file"));
        }
        if state == "on" {
            let matching = verified_servers
                .and_then(|value| value.get(&spec.name))
                .is_some_and(|value| fingerprint(value) == fingerprint(&entry));
            if !present || !matching {
                return Err(refuse("edited TOML does not contain the intended table"));
            }
        }
        let mut expected = document.clone();
        if !expected.contains("mcp_servers") {
            expected.insert("mcp_servers", Cfg::table());
        }
        let Some(expected_servers) = expected.get("mcp_servers").cloned() else {
            return Err(refuse("target has an invalid mcp_servers table"));
        };
        if !expected_servers.is_table() {
            return Err(refuse("target has an invalid mcp_servers table"));
        }
        if let Some(slot) = expected.as_table_mut().and_then(|entries| {
            entries
                .iter_mut()
                .find(|(key, _)| key == "mcp_servers")
                .map(|(_, value)| value)
        }) {
            if state == "off" {
                slot.remove(&spec.name);
            } else {
                slot.insert(&spec.name, entry.clone());
            }
        }
        if fingerprint(&records::normalized_toml(&verified))
            != fingerprint(&records::normalized_toml(&expected))
        {
            return Err(refuse("edited TOML would change other settings"));
        }
        self.atomic_write(path, updated.as_bytes())?;
        Ok(AgentResult::new(
            agent,
            true,
            true,
            if state == "off" {
                "removed table"
            } else {
                "written table"
            },
            vec![logical],
        ))
    }

    fn apply_agent(
        &mut self,
        definition: &Definition,
        spec: &ServerSpec,
        agent: &str,
        state: &str,
    ) -> Outcome<AgentResult> {
        if !WRITE_AGENTS.contains(&agent) {
            return Ok(AgentResult::new(
                agent,
                false,
                false,
                "unknown agent",
                vec![],
            ));
        }
        if !supported_transports(agent).contains(&spec.transport.as_str()) {
            return Ok(AgentResult::new(
                agent,
                false,
                false,
                &format!("transport {} is not supported by {agent}", spec.transport),
                vec![],
            ));
        }
        let path = self.target_path(agent)?;
        if let Some(source) = &definition.absolute_path
            && super::safeio::absolute(&path) == *source
        {
            if state == "off" {
                return Ok(AgentResult::new(
                    agent,
                    false,
                    false,
                    "refusing to remove the definition's own source entry",
                    vec![],
                ));
            }
            return Ok(AgentResult::new(
                agent,
                true,
                false,
                "already present",
                vec![],
            ));
        }
        if agent == "codex" {
            return self.apply_toml(agent, &path, spec, state);
        }
        let limit = if agent == "claude-code" {
            MAX_CLAUDE_BYTES
        } else {
            MAX_CONFIG_BYTES
        };
        self.apply_json(agent, &path, spec, state, limit)
    }

    pub fn apply(&mut self, identifier: &str, agents: &[String], state: &str) -> Value {
        if state != "on" && state != "off" {
            return failure("state must be on or off");
        }
        self.inventory.scan();
        let Some(definition) = self.inventory.definition_by_id(identifier).cloned() else {
            return failure("unknown definition id");
        };
        let spec = match server_spec(&definition) {
            Ok(spec) => spec,
            Err(error) => return failure(&error.0),
        };
        let mut requested: Vec<String> = Vec::new();
        for agent in agents {
            if agent == "all" {
                for item in WRITE_AGENTS {
                    if !requested.iter().any(|value| value == item) {
                        requested.push(item.to_string());
                    }
                }
                continue;
            }
            let core = core_agent_id(agent);
            if !requested.contains(&core) {
                requested.push(core);
            }
        }
        let mut results: Vec<AgentResult> = Vec::new();
        for agent in requested {
            match self.apply_agent(&definition, &spec, &agent, state) {
                Ok(result) => results.push(result),
                Err(error) => {
                    results.push(AgentResult::new(&agent, false, false, &error.0, vec![]))
                }
            }
        }
        let ok = !results.is_empty() && results.iter().all(|result| result.ok);
        let message = if ok {
            String::new()
        } else {
            results
                .iter()
                .filter(|result| !result.ok)
                .map(|result| format!("{}: {}", result.agent, result.message))
                .collect::<Vec<String>>()
                .join("; ")
        };
        json!({
            "ok": ok,
            "schemaVersion": SCHEMA_VERSION,
            "project": "<project>",
            "changed": results.iter().any(|result| result.changed),
            "message": message,
            "results": results.iter().map(AgentResult::public).collect::<Vec<Value>>(),
        })
    }

    pub fn recovery_context(&self) -> Value {
        json!({
            "project": path_text(&self.inventory.settings.project),
            "home": path_text(&self.inventory.settings.home),
            "configHome": path_text(&self.inventory.settings.config_home),
            "codexHome": path_text(&self.inventory.settings.codex_home),
            "etcRoot": path_text(&self.inventory.settings.etc_root),
        })
    }

    fn recorded_inventory(&self, context: &Value) -> Inventory {
        let text = |key: &str, fallback: &Path| -> PathBuf {
            match context.get(key).and_then(Value::as_str) {
                Some(value) if !value.is_empty() => super::safeio::absolute(
                    &parse_path(value).unwrap_or_else(|_| PathBuf::from(value)),
                ),
                _ => fallback.to_path_buf(),
            }
        };
        Inventory::new(Settings {
            project: text("project", &self.inventory.settings.project),
            home: text("home", &self.inventory.settings.home),
            config_home: text("configHome", &self.inventory.settings.config_home),
            etc_root: text("etcRoot", &self.inventory.settings.etc_root),
            codex_home: text("codexHome", &self.inventory.settings.codex_home),
            system_owner_uid: self.inventory.settings.system_owner_uid,
            scope: "all".to_string(),
            environment: self.inventory.settings.environment.clone(),
        })
    }

    fn outcome(&self, result: &AgentResult, payload: Option<&Value>, record_id: &str) -> Value {
        let mut document = Map::new();
        document.insert("ok".to_string(), json!(result.ok));
        document.insert("schemaVersion".to_string(), json!(SCHEMA_VERSION));
        document.insert("project".to_string(), json!("<project>"));
        document.insert("changed".to_string(), json!(result.changed));
        document.insert(
            "message".to_string(),
            json!(if result.ok {
                String::new()
            } else {
                format!("{}: {}", result.agent, result.message)
            }),
        );
        document.insert("results".to_string(), json!([result.public()]));
        if let Some(payload) = payload {
            document.insert("payload".to_string(), payload.clone());
        }
        if !record_id.is_empty() {
            document.insert("recordId".to_string(), json!(record_id));
        }
        Value::Object(document)
    }
}

pub fn failure(message: &str) -> Value {
    json!({
        "ok": false,
        "schemaVersion": SCHEMA_VERSION,
        "project": "<project>",
        "changed": false,
        "message": message,
        "results": [],
    })
}

pub struct Removal<'a> {
    pub prepare: bool,
    pub expected_payload: Option<&'a Value>,
    pub transaction_id: &'a str,
}

impl Applier {
    pub fn remove(&mut self, identifier: &str, removal: &Removal<'_>) -> Value {
        self.inventory.scan();
        let Some(definition) = self.inventory.definition_by_id(identifier).cloned() else {
            return failure("unknown definition id");
        };
        let agent = core_agent_id(&definition.agent);
        if !WRITE_AGENTS.contains(&agent.as_str())
            || definition.absolute_path.is_none()
            || (definition.scope != "user" && definition.scope != "project")
        {
            return failure(
                "only user and project definitions in a writable config can be removed",
            );
        }
        match self.perform_removal(&definition, &agent, identifier, removal) {
            Ok(value) => value,
            Err(error) => failure(&error.0),
        }
    }

    fn perform_removal(
        &mut self,
        definition: &Definition,
        agent: &str,
        identifier: &str,
        removal: &Removal<'_>,
    ) -> Outcome<Value> {
        server_spec(definition)?;
        let path = definition
            .absolute_path
            .clone()
            .ok_or_else(|| refuse("the source definition is missing"))?;
        let target = std::fs::canonicalize(&path).map_err(|error| Refused(error.to_string()))?;
        let limit = if agent == "claude-code" {
            MAX_CLAUDE_BYTES
        } else {
            MAX_CONFIG_BYTES
        };
        let (mut payload, encoded_source) = if agent == "codex" {
            let data = self
                .read_target(&path, limit)?
                .ok_or_else(|| refuse("the source definition is missing"))?;
            let text = std::str::from_utf8(&data).map_err(|_| {
                refuse("the definition cannot be preserved as a Unicode undo record; edit the source directly")
            })?;
            let raw = definition.raw_config.clone().unwrap_or(Cfg::Null);
            let (updated, record) = records::detach_toml(text, &definition.raw_name, &raw)
                .map_err(|error| Refused(error.0))?;
            (record, updated.into_bytes())
        } else {
            let mut document = self.read_json_target(&path, limit)?;
            let record = records::detach_json(definition, &mut document)
                .map_err(|error| Refused(error.0))?;
            let stored = Cfg::from_json(&Value::Object(record.clone()));
            if unrepresentable(&stored) || unrepresentable(&document) {
                return Err(refuse(
                    "the definition cannot be preserved as a Unicode undo record; edit the source directly",
                ));
            }
            (record, rendered_json(&document)?)
        };
        payload.insert("format".to_string(), json!(2));
        payload.insert("agent".to_string(), json!(agent));
        payload.insert("path".to_string(), json!(path_text(&path)));
        payload.insert("target".to_string(), json!(path_text(&target)));
        let payload = Value::Object(payload);
        let encoded = crate::core_modules::canonical::compact_json(&payload);
        if encoded.len() > 1024 * 1024 {
            return Ok(failure(
                "the definition exceeds the undo record size limit; edit the source directly",
            ));
        }
        if let Some(expected) = removal.expected_payload
            && *expected != payload
        {
            return Ok(failure(
                "the source definition changed after recovery was prepared; nothing was changed",
            ));
        }
        let record_id = match self.recovery.write(
            &payload,
            identifier,
            &self.recovery_context(),
            removal.transaction_id,
        ) {
            Ok(record_id) => record_id,
            Err(RecoveryError::Full(message)) => return Ok(failure(&message)),
            Err(error) => return Ok(failure(&error.to_string())),
        };
        if removal.prepare {
            let result = AgentResult::new(agent, true, false, "prepared recovery", vec![]);
            return Ok(self.outcome(&result, Some(&payload), &record_id));
        }
        if std::fs::canonicalize(&path).ok().as_deref() != Some(target.as_path()) {
            return Err(refuse("the source path changed; refresh and retry"));
        }
        self.atomic_write(&path, &encoded_source)?;
        let logical = String::from_utf8_lossy(&self.inventory.logical_path(&path)).into_owned();
        let result = AgentResult::new(
            agent,
            true,
            true,
            "removed source definition",
            vec![logical],
        );
        Ok(self.outcome(&result, Some(&payload), &record_id))
    }

    fn restore_record(&mut self, payload: &Value, context: Option<&Value>) -> Value {
        match self.perform_restore(payload, context) {
            Ok(value) => value,
            Err(error) => failure(&error.0),
        }
    }

    fn perform_restore(&mut self, payload: &Value, context: Option<&Value>) -> Outcome<Value> {
        let agent = payload
            .get("agent")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let raw_path = payload.get("path").and_then(Value::as_str);
        let raw_target = payload.get("target").and_then(Value::as_str);
        let (Some(raw_path), Some(raw_target)) = (raw_path, raw_target) else {
            return Ok(failure("restore payload is incomplete"));
        };
        if !WRITE_AGENTS.contains(&agent) {
            return Ok(failure("restore payload is incomplete"));
        }
        let path = parse_path(raw_path).map_err(|error| Refused(error.to_string()))?;
        let target = parse_path(raw_target).map_err(|error| Refused(error.to_string()))?;
        if !path.is_absolute() || !target.is_absolute() {
            return Err(refuse("restore payload needs absolute source paths"));
        }
        let known = match context {
            Some(context) => {
                let mut recorded = self.recorded_inventory(context);
                recorded.scan();
                recorded
                    .config_paths
                    .get(&super::safeio::absolute(&path))
                    .is_some_and(|agents| agents.contains(agent))
            }
            None => {
                self.inventory.scan();
                self.inventory
                    .config_paths
                    .get(&super::safeio::absolute(&path))
                    .is_some_and(|agents| agents.contains(agent))
            }
        };
        if !known {
            return Err(refuse(
                "the recorded source is not a known configuration file for that agent",
            ));
        }
        if crate::core_modules::path::realpath(&super::safeio::absolute(&path)) != target {
            return Err(refuse("the source path changed; restore was refused"));
        }
        let kind = payload
            .get("kind")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let (changed, encoded) = if kind == "toml" && agent == "codex" {
            let data = self.read_target(&path, MAX_CONFIG_BYTES)?;
            let text = match data.as_deref() {
                Some(data) => std::str::from_utf8(data)
                    .map_err(|_| refuse("target is not strict TOML"))?
                    .to_string(),
                None => String::new(),
            };
            let updated = records::attach_toml(&text, payload).map_err(|error| Refused(error.0))?;
            match updated {
                Some(updated) => (true, updated.into_bytes()),
                None => (false, Vec::new()),
            }
        } else if kind == "json" && agent != "codex" {
            let limit = if agent == "claude-code" {
                MAX_CLAUDE_BYTES
            } else {
                MAX_CONFIG_BYTES
            };
            let mut document = self.read_json_target(&path, limit)?;
            let changed =
                records::attach_json(&mut document, payload).map_err(|error| Refused(error.0))?;
            if changed {
                (true, rendered_json(&document)?)
            } else {
                (false, Vec::new())
            }
        } else {
            return Err(refuse("restore payload has an invalid source format"));
        };
        if changed {
            if crate::core_modules::path::realpath(&super::safeio::absolute(&path)) != target {
                return Err(refuse("the source path changed; restore was refused"));
            }
            self.atomic_write(&path, &encoded)?;
        }
        let logical = String::from_utf8_lossy(&self.inventory.logical_path(&path)).into_owned();
        let result = AgentResult::new(
            agent,
            true,
            changed,
            if changed {
                "restored source definition"
            } else {
                "already present"
            },
            if changed { vec![logical] } else { vec![] },
        );
        Ok(self.outcome(&result, None, ""))
    }

    pub fn restore(&mut self, record_id: &str, raw_payload: Option<&str>) -> Value {
        let stored = self.recovery.read(record_id);
        let payload = match raw_payload {
            None => match stored.as_ref() {
                Some(record) => record.payload().clone(),
                None => {
                    return failure(
                        "restore payload is not JSON or its recovery record is unavailable",
                    );
                }
            },
            Some(raw) => match parse_json(raw.as_bytes()) {
                Ok(value) => value.to_json(),
                Err(_) => {
                    return failure(
                        "restore payload is not JSON or its recovery record is unavailable",
                    );
                }
            },
        };
        if !payload.is_object() {
            return failure("restore payload is not a record");
        }
        let mut record = self.recovery.read(record_id);
        if record
            .as_ref()
            .is_some_and(|record| *record.payload() != payload)
        {
            record = None;
        }
        if record.is_none() {
            record = self.recovery.find(&payload);
        }
        let Some(record) = record else {
            return failure("no prepared recovery record matches this payload");
        };
        let prepared = record.payload().clone();
        let format = prepared.get("format");
        if !matches!(format, Some(Value::Number(number)) if !number.is_f64() && number.as_i64() == Some(2))
        {
            return failure("restore payload has an unsupported format");
        }
        let context = record.context().clone();
        let context = context.is_object().then_some(context);
        let document = self.restore_record(&prepared, context.as_ref());
        if document.get("ok") == Some(&Value::Bool(true)) {
            self.recovery.mark_restored(&record.record_id);
        }
        document
    }
}
