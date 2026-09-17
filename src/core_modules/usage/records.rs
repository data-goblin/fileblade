use serde_json::{Map, Value};
use std::sync::OnceLock;

pub const MAX_URI_BYTES: usize = 512;
const SKILL_FILE: &str = "SKILL.md";
const COPILOT_SKIPPED_TRIGGERS: [&str; 1] = ["context-load"];
const FINISHED: [&str; 2] = ["completed", "error"];
const CODEX_SKILL_KIND: &str = "skills.selected_skill_instructions";

const OPENCODE_BUILTIN_TOOLS: [&str; 27] = [
    "bash",
    "shell",
    "read",
    "write",
    "edit",
    "multiedit",
    "patch",
    "glob",
    "grep",
    "list",
    "ls",
    "webfetch",
    "websearch",
    "todowrite",
    "todoread",
    "task",
    "question",
    "skill",
    "lsp",
    "codesearch",
    "apply_patch",
    "web_fetch",
    "web_search",
    "todo_write",
    "todo_read",
    "batch",
    "invalid",
];

#[derive(Clone, Debug)]
pub struct Event {
    pub agent: String,
    pub call: String,
    pub at: i64,
    pub kind: String,
    pub origin: String,
    pub server: String,
    pub name: String,
    pub subagent: i64,
    pub project: Option<String>,
    pub failed: i64,
}

#[derive(Default)]
pub struct Batch {
    pub offset: i64,
    pub project: Option<String>,
    pub source: String,
    pub pending: bool,
    pub first_at: Option<i64>,
    pub events: Vec<Event>,
    pub failures: Vec<(String, i64)>,
}

impl Batch {
    pub fn new(offset: i64, project: Option<String>, source: String) -> Self {
        Self {
            offset,
            project,
            source,
            ..Self::default()
        }
    }

    pub fn note(&mut self, at: i64) -> i64 {
        self.first_at = Some(match self.first_at {
            Some(first) => first.min(at),
            None => at,
        });
        at
    }

    fn stamp(&mut self, record: &Map<String, Value>, key: &str) -> Option<i64> {
        let at = moment(record.get(key))?;
        Some(self.note(at))
    }
}

fn moment(value: Option<&Value>) -> Option<i64> {
    match value {
        Some(Value::Number(number)) => number
            .as_i64()
            .or_else(|| number.as_f64().map(|value| value.trunc() as i64)),
        other => parse_iso(text(other)),
    }
}

fn parse_iso(value: &str) -> Option<i64> {
    let normalized = match value.strip_suffix(['Z', 'z']) {
        Some(head) => format!("{head}+00:00"),
        None => value.to_string(),
    };
    let nanos = if let Ok(stamped) = chrono::DateTime::parse_from_rfc3339(&normalized) {
        i128::from(stamped.timestamp()) * 1_000_000_000
            + i128::from(stamped.timestamp_subsec_nanos())
    } else {
        let naive = chrono::NaiveDateTime::parse_from_str(&normalized, "%Y-%m-%dT%H:%M:%S%.f")
            .or_else(|_| chrono::NaiveDateTime::parse_from_str(&normalized, "%Y-%m-%d %H:%M:%S%.f"))
            .or_else(|_| {
                chrono::NaiveDate::parse_from_str(&normalized, "%Y-%m-%d")
                    .map(|day| day.and_hms_opt(0, 0, 0).unwrap_or_default())
            })
            .ok()?;
        i128::from(naive.and_utc().timestamp()) * 1_000_000_000
            + i128::from(naive.and_utc().timestamp_subsec_nanos())
    };
    i64::try_from(nanos.div_euclid(1_000_000)).ok()
}

pub fn text(value: Option<&Value>) -> &str {
    match value {
        Some(Value::String(value)) => value,
        _ => "",
    }
}

pub fn mapping(value: Option<&Value>) -> &Map<String, Value> {
    static EMPTY: OnceLock<Map<String, Value>> = OnceLock::new();
    match value {
        Some(Value::Object(entries)) => entries,
        _ => EMPTY.get_or_init(Map::new),
    }
}

fn array(value: Option<&Value>) -> &[Value] {
    match value {
        Some(Value::Array(items)) => items,
        _ => &[],
    }
}

fn integer(value: Option<&Value>) -> Option<i64> {
    match value {
        Some(Value::Number(number)) => number.as_i64(),
        _ => None,
    }
}

fn is_true(value: Option<&Value>) -> bool {
    value == Some(&Value::Bool(true))
}

fn basename(path: &str) -> &str {
    match path.rfind('/') {
        Some(index) => &path[index + 1..],
        None => path,
    }
}

fn dirname(path: &str) -> &str {
    let index = path.rfind('/').map_or(0, |index| index + 1);
    let head = &path[..index];
    if head.is_empty() || head.bytes().all(|byte| byte == b'/') {
        return head;
    }
    head.trim_end_matches('/')
}

pub fn normpath(path: &str) -> String {
    if path.is_empty() {
        return ".".to_string();
    }
    let leading = path.bytes().take_while(|byte| *byte == b'/').count();
    let initial = match leading {
        0 => 0,
        2 => 2,
        _ => 1,
    };
    let mut parts: Vec<&str> = Vec::new();
    for part in path.split('/') {
        if part.is_empty() || part == "." {
            continue;
        }
        if part != ".." || (initial == 0 && parts.is_empty()) || parts.last() == Some(&"..") {
            parts.push(part);
        } else if !parts.is_empty() {
            parts.pop();
        }
    }
    let mut result = "/".repeat(initial);
    result.push_str(&parts.join("/"));
    if result.is_empty() {
        ".".to_string()
    } else {
        result
    }
}

pub fn skill_from_path(path: &str) -> String {
    let normalized = normpath(path);
    if basename(&normalized) != SKILL_FILE {
        return String::new();
    }
    basename(dirname(&normalized)).to_string()
}

fn command_names(body: &str) -> Vec<String> {
    static PATTERN: OnceLock<regex::Regex> = OnceLock::new();
    let expression = PATTERN.get_or_init(|| {
        regex::Regex::new(r"<command-name>/([A-Za-z0-9:_-]+)</command-name>")
            .expect("valid pattern")
    });
    expression
        .captures_iter(body)
        .map(|found| found[1].to_string())
        .collect()
}

fn leading_slash_word(body: &str) -> Option<String> {
    static PATTERN: OnceLock<regex::Regex> = OnceLock::new();
    let expression =
        PATTERN.get_or_init(|| regex::Regex::new(r"^/([A-Za-z0-9:_.-]+)").expect("valid pattern"));
    expression.captures(body).map(|found| found[1].to_string())
}

fn codex_skill_name(body: &str) -> Option<String> {
    static PATTERN: OnceLock<regex::Regex> = OnceLock::new();
    let expression = PATTERN.get_or_init(|| {
        regex::Regex::new(r"<skill>\s*<name>([^<\n]+)</name>").expect("valid pattern")
    });
    expression
        .captures(body)
        .map(|found| found[1].trim().to_string())
}

fn skill_path_words(body: &str) -> Vec<&str> {
    static PATTERN: OnceLock<regex::Regex> = OnceLock::new();
    let expression = PATTERN.get_or_init(|| {
        regex::Regex::new("^(?:/|~/)[^\\s'\"`;|&<>()]*?/SKILL\\.md").expect("valid pattern")
    });
    let mut found = Vec::new();
    let mut index = 0usize;
    while index < body.len() {
        if !body.is_char_boundary(index) {
            index += 1;
            continue;
        }
        let allowed = match body[..index].chars().next_back() {
            None => true,
            Some(previous) => {
                !(previous.is_alphanumeric()
                    || previous == '_'
                    || previous == '.'
                    || previous == '/'
                    || previous == '-')
            }
        };
        if allowed && let Some(matched) = expression.find(&body[index..]) {
            found.push(&body[index + matched.start()..index + matched.end()]);
            index += matched.end();
            continue;
        }
        index += 1;
    }
    found
}

fn skills_in_command(command: &str) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for word in skill_path_words(command) {
        let skill = skill_from_path(word);
        if !skill.is_empty() && !found.contains(&skill) {
            found.push(skill);
        }
    }
    found
}

fn truncated_uri(uri: &str) -> String {
    let head = uri.split(['?', '#']).next().unwrap_or("");
    let mut end = head.len().min(MAX_URI_BYTES);
    while end > 0 && !head.is_char_boundary(end) {
        end -= 1;
    }
    head[..end].to_string()
}

fn tool(part: &Map<String, Value>) -> Option<(String, String, String)> {
    let name = text(part.get("name"));
    let arguments = mapping(part.get("input"));
    if name == "Skill" {
        let skill = text(arguments.get("skill"));
        return (!skill.is_empty())
            .then(|| ("skill".to_string(), String::new(), skill.to_string()));
    }
    if name == "ListMcpResourcesTool" {
        return Some((
            "resource-list".to_string(),
            text(arguments.get("server")).to_string(),
            String::new(),
        ));
    }
    if name == "ReadMcpResourceTool" || name == "ReadMcpResourceDirTool" {
        return Some((
            "resource".to_string(),
            text(arguments.get("server")).to_string(),
            truncated_uri(text(arguments.get("uri"))),
        ));
    }
    let segments: Vec<&str> = name.split("__").collect();
    if segments[0] == "mcp" && segments.len() >= 3 && !segments[1].is_empty() {
        return Some((
            "tool".to_string(),
            segments[1].to_string(),
            segments[2..].join("__"),
        ));
    }
    None
}

fn contains(raw: &[u8], needle: &[u8]) -> bool {
    raw.windows(needle.len()).any(|window| window == needle)
}

pub fn claude_line(raw: &[u8]) -> bool {
    contains(raw, b"\"Skill\"")
        || contains(raw, b"mcp__")
        || contains(raw, b"McpResource")
        || contains(raw, b"command-name")
        || contains(raw, b"\"is_error\"")
}

pub fn codex_line(raw: &[u8]) -> bool {
    contains(raw, b"McpToolCall")
        || contains(raw, b"SKILL.md")
        || contains(raw, b"selected_skill_instructions")
        || contains(raw, b"<skill>")
        || contains(raw, b"session_meta")
}

pub fn copilot_line(raw: &[u8]) -> bool {
    contains(raw, b"skill.invoked")
        || contains(raw, b"tool.execution_")
        || contains(raw, b"session.start")
        || contains(raw, b"session.context_changed")
}

pub fn antigravity_line(raw: &[u8]) -> bool {
    contains(raw, b"SKILL.md") || contains(raw, b"slash_command")
}

pub fn pi_line(raw: &[u8]) -> bool {
    contains(raw, b"SKILL.md") || contains(raw, b"toolResult") || contains(raw, b"\"session\"")
}

pub fn claude(record: &Map<String, Value>, batch: &mut Batch) {
    let at = batch.stamp(record, "timestamp");
    let content = mapping(record.get("message")).get("content").cloned();
    let project = match record.get("cwd") {
        Some(Value::String(path)) => Some(path.clone()),
        _ => None,
    };
    let subagent = i64::from(is_true(record.get("isSidechain")));
    if text(record.get("type")) == "assistant"
        && let Some(Value::Array(parts)) = &content
        && let Some(at) = at
    {
        for part in parts {
            let Some(part) = part.as_object() else {
                continue;
            };
            if text(part.get("type")) != "tool_use" || text(part.get("id")).is_empty() {
                continue;
            }
            if let Some((kind, server, name)) = tool(part) {
                batch.events.push(Event {
                    agent: "claude".to_string(),
                    call: text(part.get("id")).to_string(),
                    at,
                    kind,
                    origin: "agent".to_string(),
                    server,
                    name,
                    subagent,
                    project: project.clone(),
                    failed: 0,
                });
            }
        }
    }
    if text(record.get("type")) != "user" {
        return;
    }
    let mut texts: Vec<String> = Vec::new();
    match &content {
        Some(Value::String(body)) => texts.push(body.clone()),
        Some(Value::Array(parts)) => {
            for part in parts {
                let Some(part) = part.as_object() else {
                    continue;
                };
                let kind = text(part.get("type"));
                if kind == "tool_result"
                    && is_true(part.get("is_error"))
                    && !text(part.get("tool_use_id")).is_empty()
                    && let Some(at) = at
                {
                    batch
                        .failures
                        .push((text(part.get("tool_use_id")).to_string(), at));
                } else if kind == "text" {
                    texts.push(text(part.get("text")).to_string());
                }
            }
        }
        _ => {}
    }
    let names = command_names(&texts.join("\n"));
    let uuid = text(record.get("uuid")).to_string();
    let (Some(at), false, false) = (at, names.is_empty(), uuid.is_empty()) else {
        return;
    };
    let origin = match record.get("scheduledTaskId") {
        Some(value) if truthy(value) => "scheduled",
        _ => "user",
    };
    for (index, name) in names.iter().enumerate() {
        let call = if names.len() == 1 {
            uuid.clone()
        } else {
            format!("{uuid}:{}", index + 1)
        };
        batch.events.push(Event {
            agent: "claude".to_string(),
            call,
            at,
            kind: "command".to_string(),
            origin: origin.to_string(),
            server: String::new(),
            name: name.clone(),
            subagent,
            project: project.clone(),
            failed: 0,
        });
    }
}

fn truthy(value: &Value) -> bool {
    match value {
        Value::Null => false,
        Value::Bool(value) => *value,
        Value::Number(number) => number.as_f64().is_some_and(|value| value != 0.0),
        Value::String(value) => !value.is_empty(),
        Value::Array(items) => !items.is_empty(),
        Value::Object(entries) => !entries.is_empty(),
    }
}

fn codex_explicit(payload: &Map<String, Value>, at: i64, batch: &mut Batch) {
    let passthrough = mapping(payload.get("internal_chat_message_metadata_passthrough"));
    let kinds = array(passthrough.get("content_item_kinds"));
    let content = array(payload.get("content"));
    let mut identity = text(payload.get("id")).to_string();
    if identity.is_empty() {
        identity = text(passthrough.get("turn_id")).to_string();
    }
    if identity.is_empty() {
        return;
    }
    for (index, part) in content.iter().enumerate() {
        let body = text(mapping(Some(part)).get("text"));
        let tagged = index < kinds.len() && text(Some(&kinds[index])) == CODEX_SKILL_KIND;
        if !(tagged || body.trim_start().starts_with("<skill>")) {
            continue;
        }
        if let Some(name) = codex_skill_name(body) {
            batch.events.push(Event {
                agent: "codex".to_string(),
                call: format!("{identity}:{index}"),
                at,
                kind: "command".to_string(),
                origin: "user".to_string(),
                server: String::new(),
                name,
                subagent: 0,
                project: batch.project.clone(),
                failed: 0,
            });
        }
    }
}

fn codex_item(payload: &Map<String, Value>, at: i64, batch: &mut Batch) {
    let item = mapping(payload.get("item"));
    let identity = text(item.get("id")).to_string();
    if identity.is_empty() {
        return;
    }
    let kind = text(item.get("type"));
    if kind == "McpToolCall"
        && !text(item.get("server")).is_empty()
        && !text(item.get("tool")).is_empty()
    {
        let failed = text(item.get("status")) != "completed"
            || is_true(mapping(item.get("result")).get("isError"));
        batch.events.push(Event {
            agent: "codex".to_string(),
            call: identity,
            at,
            kind: "tool".to_string(),
            origin: "agent".to_string(),
            server: text(item.get("server")).to_string(),
            name: text(item.get("tool")).to_string(),
            subagent: 0,
            project: batch.project.clone(),
            failed: i64::from(failed),
        });
        return;
    }
    if kind != "CommandExecution" {
        return;
    }
    let mut skills: Vec<String> = array(item.get("parsed_cmd"))
        .iter()
        .filter(|entry| text(mapping(Some(entry)).get("type")) == "read")
        .map(|entry| skill_from_path(text(mapping(Some(entry)).get("path"))))
        .filter(|skill| !skill.is_empty())
        .collect();
    if skills.is_empty() {
        let command = match item.get("command") {
            Some(Value::Array(parts)) => parts
                .iter()
                .map(|part| text(Some(part)).to_string())
                .collect::<Vec<String>>()
                .join(" "),
            other => text(other).to_string(),
        };
        skills = skills_in_command(&command);
    }
    let turn = text(payload.get("turn_id")).to_string();
    let status = text(item.get("status"));
    let failed = i64::from(!status.is_empty() && status != "completed");
    let mut seen: Vec<String> = Vec::new();
    for skill in skills {
        if seen.contains(&skill) {
            continue;
        }
        let call = if turn.is_empty() {
            format!("{identity}:{skill}")
        } else {
            format!("{turn}:{skill}")
        };
        batch.events.push(Event {
            agent: "codex".to_string(),
            call,
            at,
            kind: "skill".to_string(),
            origin: "agent".to_string(),
            server: String::new(),
            name: skill.clone(),
            subagent: 0,
            project: batch.project.clone(),
            failed,
        });
        seen.push(skill);
    }
}

fn codex_legacy(payload: &Map<String, Value>, at: i64, batch: &mut Batch) {
    let mut call = text(payload.get("call_id")).to_string();
    if call.is_empty() {
        call = text(payload.get("id")).to_string();
    }
    if call.is_empty()
        || text(payload.get("type")) != "custom_tool_call"
        || text(payload.get("name")) != "exec"
    {
        return;
    }
    for skill in skills_in_command(text(payload.get("input"))) {
        batch.events.push(Event {
            agent: "codex".to_string(),
            call: format!("{call}:{skill}"),
            at,
            kind: "skill".to_string(),
            origin: "agent".to_string(),
            server: String::new(),
            name: skill,
            subagent: 0,
            project: batch.project.clone(),
            failed: 0,
        });
    }
}

pub fn codex(record: &Map<String, Value>, batch: &mut Batch) {
    let at = batch.stamp(record, "timestamp");
    let payload = mapping(record.get("payload")).clone();
    let kind = text(record.get("type")).to_string();
    if kind == "session_meta"
        && let Some(Value::String(cwd)) = payload.get("cwd")
    {
        batch.project = Some(cwd.clone());
    }
    let Some(at) = at else { return };
    if kind == "event_msg" && text(payload.get("type")) == "item_completed" {
        codex_item(&payload, at, batch);
    } else if kind == "response_item"
        && text(payload.get("type")) == "message"
        && text(payload.get("role")) == "user"
    {
        codex_explicit(&payload, at, batch);
    } else if kind == "response_item" {
        codex_legacy(&payload, at, batch);
    }
}

pub fn copilot(record: &Map<String, Value>, batch: &mut Batch) {
    let at = batch.stamp(record, "timestamp");
    let kind = text(record.get("type")).to_string();
    let data = mapping(record.get("data")).clone();
    let identity = text(record.get("id")).to_string();
    let subagent = i64::from(!text(record.get("agentId")).is_empty());
    if kind == "session.start" || kind == "session.context_changed" {
        let mut cwd = text(mapping(data.get("context")).get("cwd")).to_string();
        if cwd.is_empty() {
            cwd = text(data.get("cwd")).to_string();
        }
        if !cwd.is_empty() {
            batch.project = Some(cwd);
        }
        return;
    }
    let (Some(at), false) = (at, identity.is_empty()) else {
        return;
    };
    if kind == "skill.invoked" {
        let name = text(data.get("name")).to_string();
        let trigger = text(data.get("trigger"));
        if name.is_empty() || COPILOT_SKIPPED_TRIGGERS.contains(&trigger) {
            return;
        }
        let user = trigger == "user-invoked";
        batch.events.push(Event {
            agent: "copilot".to_string(),
            call: identity,
            at,
            kind: if user { "command" } else { "skill" }.to_string(),
            origin: if user { "user" } else { "agent" }.to_string(),
            server: String::new(),
            name,
            subagent,
            project: batch.project.clone(),
            failed: 0,
        });
        return;
    }
    let call = text(data.get("toolCallId")).to_string();
    if kind == "tool.execution_start" && !call.is_empty() {
        let mut server = text(data.get("mcpConfigServerName")).to_string();
        if server.is_empty() {
            server = text(data.get("mcpServerName")).to_string();
        }
        if server.is_empty() {
            return;
        }
        let mut name = text(data.get("mcpToolName")).to_string();
        if name.is_empty() {
            name = text(data.get("toolName")).to_string();
        }
        batch.events.push(Event {
            agent: "copilot".to_string(),
            call,
            at,
            kind: "tool".to_string(),
            origin: "agent".to_string(),
            server,
            name,
            subagent,
            project: batch.project.clone(),
            failed: 0,
        });
    } else if kind == "tool.execution_complete"
        && !call.is_empty()
        && data.get("success") == Some(&Value::Bool(false))
    {
        batch.failures.push((call, at));
    }
}

pub fn antigravity(record: &Map<String, Value>, batch: &mut Batch) {
    if record.contains_key("display") && record.contains_key("timestamp") {
        let Some(at) = batch.stamp(record, "timestamp") else {
            return;
        };
        if text(record.get("type")) != "slash_command" {
            return;
        }
        let Some(name) = leading_slash_word(text(record.get("display")).trim()) else {
            return;
        };
        let workspace = text(record.get("workspace")).to_string();
        let project = (!workspace.is_empty()).then(|| workspace.clone());
        let conversation = text(record.get("conversationId")).to_string();
        let anchor = if conversation.is_empty() {
            workspace
        } else {
            conversation
        };
        batch.events.push(Event {
            agent: "antigravity".to_string(),
            call: format!("{anchor}:{at}"),
            at,
            kind: "command".to_string(),
            origin: "user".to_string(),
            server: String::new(),
            name,
            subagent: 0,
            project,
            failed: 0,
        });
        return;
    }
    let at = batch.stamp(record, "created_at");
    let calls = match record.get("tool_calls") {
        Some(Value::Array(items)) => items.clone(),
        _ => return,
    };
    let (Some(at), Some(step)) = (at, integer(record.get("step_index"))) else {
        return;
    };
    let failed = i64::from(text(record.get("status")).to_uppercase() == "ERROR");
    for (index, call) in calls.iter().enumerate() {
        let Some(call) = call.as_object() else {
            continue;
        };
        if text(call.get("name")) != "view_file" {
            continue;
        }
        let skill = skill_from_path(text(mapping(call.get("args")).get("AbsolutePath")));
        if skill.is_empty() {
            continue;
        }
        batch.events.push(Event {
            agent: "antigravity".to_string(),
            call: format!("{}:{step}:{index}", batch.source),
            at,
            kind: "skill".to_string(),
            origin: "agent".to_string(),
            server: String::new(),
            name: skill,
            subagent: 0,
            project: batch.project.clone(),
            failed,
        });
    }
}

pub fn pi(record: &Map<String, Value>, batch: &mut Batch) {
    let kind = text(record.get("type")).to_string();
    if kind == "session" {
        batch.stamp(record, "timestamp");
        if let Some(Value::String(cwd)) = record.get("cwd") {
            batch.project = Some(cwd.clone());
        }
        return;
    }
    if kind != "message" {
        return;
    }
    let Some(at) = batch.stamp(record, "timestamp") else {
        return;
    };
    let message = mapping(record.get("message")).clone();
    let role = text(message.get("role"));
    if role == "toolResult" {
        let call = text(message.get("toolCallId")).to_string();
        if !call.is_empty() && is_true(message.get("isError")) {
            batch.failures.push((call, at));
        }
        return;
    }
    if role != "assistant" {
        return;
    }
    for part in array(message.get("content")) {
        let Some(part) = part.as_object() else {
            continue;
        };
        if text(part.get("type")) != "toolCall" || text(part.get("id")).is_empty() {
            continue;
        }
        let arguments = mapping(part.get("arguments"));
        let mut descriptor = text(arguments.get("path")).to_string();
        if descriptor.is_empty() {
            descriptor = text(arguments.get("file_path")).to_string();
        }
        let mut skill = skill_from_path(&descriptor);
        if skill.is_empty() && text(part.get("name")) == "bash" {
            skill = text(arguments.get("command"))
                .split_whitespace()
                .find(|word| word.ends_with(SKILL_FILE))
                .map(skill_from_path)
                .unwrap_or_default();
        }
        if skill.is_empty() {
            continue;
        }
        batch.events.push(Event {
            agent: "pi".to_string(),
            call: text(part.get("id")).to_string(),
            at,
            kind: "skill".to_string(),
            origin: "agent".to_string(),
            server: String::new(),
            name: skill,
            subagent: 0,
            project: batch.project.clone(),
            failed: 0,
        });
    }
}

pub fn opencode_part(
    data: &Map<String, Value>,
    call_default: &str,
    at: i64,
    project: Option<&str>,
    batch: &mut Batch,
) {
    let state = mapping(data.get("state"));
    let status = text(state.get("status"));
    if text(data.get("type")) != "tool" || !FINISHED.contains(&status) {
        return;
    }
    let mut call = text(data.get("callID")).to_string();
    if call.is_empty() {
        call = call_default.to_string();
    }
    let mut name = text(data.get("tool")).to_string();
    if name.is_empty() {
        name = text(data.get("name")).to_string();
    }
    if call.is_empty() || name.is_empty() {
        return;
    }
    let failed = i64::from(status == "error");
    if name == "skill" {
        let skill = text(mapping(state.get("input")).get("name")).to_string();
        if !skill.is_empty() {
            batch.events.push(Event {
                agent: "opencode".to_string(),
                call,
                at,
                kind: "skill".to_string(),
                origin: "agent".to_string(),
                server: String::new(),
                name: skill,
                subagent: 0,
                project: project.map(str::to_string),
                failed,
            });
        }
        return;
    }
    if OPENCODE_BUILTIN_TOOLS.contains(&name.as_str()) || !name.contains('_') {
        return;
    }
    batch.events.push(Event {
        agent: "opencode".to_string(),
        call,
        at,
        kind: "tool".to_string(),
        origin: "agent".to_string(),
        server: String::new(),
        name,
        subagent: 0,
        project: project.map(str::to_string),
        failed,
    });
}

pub type LineReader = (fn(&[u8]) -> bool, fn(&Map<String, Value>, &mut Batch));

pub fn line_reader(agent: &str) -> Option<LineReader> {
    match agent {
        "claude" => Some((claude_line, claude)),
        "codex" => Some((codex_line, codex)),
        "copilot" => Some((copilot_line, copilot)),
        "antigravity" => Some((antigravity_line, antigravity)),
        "pi" => Some((pi_line, pi)),
        _ => None,
    }
}
