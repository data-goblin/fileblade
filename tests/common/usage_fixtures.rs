#![allow(dead_code)]

use chrono::{DateTime, SecondsFormat, TimeZone, Utc};
use fileblade::core_modules::usage::store::Environment;
use serde_json::{Map, Value, json};
use std::collections::HashMap;
use std::ffi::OsString;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Once;

unsafe extern "C" {
    fn tzset();
}

pub fn use_zone(zone: &str) {
    unsafe {
        std::env::set_var("TZ", zone);
        tzset();
    }
}

pub fn use_utc() {
    static ONCE: Once = Once::new();
    ONCE.call_once(|| use_zone("UTC0"));
}

pub fn iso(moment: DateTime<Utc>) -> String {
    moment.to_rfc3339_opts(SecondsFormat::Millis, true)
}

pub fn lines(records: &[Value]) -> String {
    records
        .iter()
        .map(|record| format!("{}\n", serde_json::to_string(record).expect("record")))
        .collect()
}

pub fn envelope(
    kind: &str,
    uuid: &str,
    at: DateTime<Utc>,
    content: Value,
    sidechain: bool,
) -> Value {
    json!({
        "parentUuid": Value::Null,
        "isSidechain": sidechain,
        "userType": "external",
        "cwd": "/work/project",
        "sessionId": "0f1e2d3c",
        "version": "2.1.258",
        "gitBranch": "main",
        "type": kind,
        "uuid": uuid,
        "timestamp": iso(at),
        "message": {"role": kind, "content": content},
    })
}

pub fn called(at: DateTime<Utc>, identity: &str, name: &str, arguments: Value) -> Value {
    called_from(at, identity, name, arguments, false)
}

pub fn called_from(
    at: DateTime<Utc>,
    identity: &str,
    name: &str,
    arguments: Value,
    sidechain: bool,
) -> Value {
    let part = json!({
        "type": "tool_use",
        "id": identity,
        "name": name,
        "input": arguments,
        "caller": {"type": "direct"},
    });
    envelope(
        "assistant",
        &format!("a-{identity}"),
        at,
        json!([part]),
        sidechain,
    )
}

pub fn skill_call(at: DateTime<Utc>, identity: &str, skill: &str) -> Value {
    called(at, identity, "Skill", json!({"skill": skill}))
}

pub fn failed(at: DateTime<Utc>, identity: &str) -> Value {
    envelope(
        "user",
        &format!("r-{identity}"),
        at,
        json!([{"tool_use_id": identity, "type": "tool_result", "content": "boom", "is_error": true}]),
        false,
    )
}

pub fn typed(at: DateTime<Utc>, uuid: &str, names: &[&str], scheduled: Option<&str>) -> Value {
    let content = names
        .iter()
        .map(|name| {
            format!(
                "<command-message>{name}</command-message>\n<command-name>/{name}</command-name>\n<command-args></command-args>"
            )
        })
        .collect::<Vec<String>>()
        .join("\n");
    let mut record = envelope("user", uuid, at, Value::from(content), false);
    if let (Some(task), Some(entries)) = (scheduled, record.as_object_mut()) {
        entries.insert("scheduledTaskId".to_string(), Value::from(task));
    }
    record
}

pub fn opening() -> Value {
    json!({"type": "permission-mode", "permissionMode": "default", "sessionId": "0f1e2d3c"})
}

pub fn session_meta(at: DateTime<Utc>) -> Value {
    json!({
        "timestamp": iso(at),
        "type": "session_meta",
        "payload": {"id": "c0de", "timestamp": iso(at), "cwd": "/work/project",
                    "originator": "codex_cli_rs", "cli_version": "0.60.0"},
    })
}

pub fn codex_call(
    at: DateTime<Utc>,
    identity: &str,
    server: &str,
    tool: &str,
    status: &str,
) -> Value {
    json!({
        "timestamp": iso(at),
        "type": "event_msg",
        "payload": {"type": "item_completed", "thread_id": "t", "turn_id": "u",
                    "item": {"type": "McpToolCall", "id": identity, "server": server, "tool": tool,
                             "arguments": {"query": "x"}, "readOnlyHint": true, "status": status,
                             "result": {"content": []}, "duration": {"secs": 0, "nanos": 5}}},
    })
}

pub fn stub(id: &str, name: &str, source: &str) -> Value {
    json!({"id": id, "name": name, "source": source})
}

pub fn definition(id: &str, name: &str, agent: &str) -> Value {
    json!({"id": id, "name": name, "agent": agent})
}

pub struct Fixture {
    pub base: tempfile::TempDir,
    pub home: PathBuf,
    pub claude: PathBuf,
    pub transcripts: PathBuf,
    pub state: PathBuf,
    pub cache: PathBuf,
    pub store: PathBuf,
    pub day: DateTime<Utc>,
    variables: HashMap<OsString, OsString>,
}

impl Fixture {
    pub fn new() -> Self {
        let base = tempfile::TempDir::with_prefix("fileblade-usage-").expect("temporary directory");
        let root = base.path().to_path_buf();
        let home = root.join("home");
        let claude = home.join(".claude");
        let transcripts = claude.join("projects").join("-work-project");
        let state = root.join("state");
        let cache = root.join("cache");
        std::fs::create_dir_all(&transcripts).expect("transcript directory");
        let day = (Utc::now() - chrono::Duration::days(3))
            .with_timezone(&Utc)
            .date_naive()
            .and_hms_opt(11, 0, 0)
            .and_then(|naive| Utc.from_local_datetime(&naive).single())
            .expect("fixture day");
        let mut variables: HashMap<OsString, OsString> = HashMap::new();
        for (key, value) in [
            ("HOME", home.clone()),
            ("XDG_STATE_HOME", state.clone()),
            ("XDG_CACHE_HOME", cache.clone()),
            ("XDG_CONFIG_HOME", home.join(".config")),
            ("XDG_DATA_HOME", home.join(".local/share")),
            ("CLAUDE_CONFIG_DIR", claude.clone()),
            ("CODEX_HOME", home.join(".codex")),
        ] {
            variables.insert(OsString::from(key), value.into_os_string());
        }
        let store = state.join("omarchy/fileblade/agent-usage.sqlite3");
        Self {
            base,
            home,
            claude,
            transcripts,
            state,
            cache,
            store,
            day,
            variables,
        }
    }

    pub fn environment(&self) -> Environment {
        Environment::new(self.variables.clone())
    }

    pub fn transcript(&self, name: &str, records: &[Value]) -> PathBuf {
        self.write_transcript(name, records, false)
    }

    pub fn append(&self, name: &str, records: &[Value]) -> PathBuf {
        self.write_transcript(name, records, true)
    }

    fn write_transcript(&self, name: &str, records: &[Value], append: bool) -> PathBuf {
        let path = self.transcripts.join(name);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("transcript parent");
        }
        let mut handle = std::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(append)
            .truncate(!append)
            .open(&path)
            .expect("transcript file");
        handle
            .write_all(lines(records).as_bytes())
            .expect("transcript body");
        path
    }

    pub fn open_store(&self) -> rusqlite::Connection {
        rusqlite::Connection::open(&self.store).expect("usage store")
    }

    pub fn counts(&self, items: &[Value]) -> Value {
        fileblade::core_modules::usage::query::skill_counts(&self.environment(), items)
    }

    pub fn uses(&self, items: &[Value], id: &str) -> Map<String, Value> {
        let document = self.counts(items);
        let row = document["counts"][id].clone();
        let mut values = Map::new();
        for key in ["uses", "usesAgent", "usesUser", "usesScheduled", "failed"] {
            values.insert(key.to_string(), row[key].clone());
        }
        values
    }

    pub fn history(&self, items: &[Value]) -> Value {
        fileblade::core_modules::usage::query::skill_usage(&self.environment(), items, false)
    }

    pub fn mcp_history(&self) -> Value {
        fileblade::core_modules::usage::query::mcp_usage(&self.environment())
    }

    pub fn forget(&self, before: Option<&str>) -> Value {
        fileblade::core_modules::usage::query::forget(&self.environment(), before)
    }

    pub fn attach_mcp(&self, definitions: &mut [Value]) -> Map<String, Value> {
        fileblade::core_modules::usage::query::attach_mcp(&self.environment(), definitions)
    }

    pub fn date(&self, moment: DateTime<Utc>) -> String {
        moment.date_naive().format("%Y-%m-%d").to_string()
    }
}

pub fn counts_of(values: [i64; 5]) -> Map<String, Value> {
    let mut row = Map::new();
    for (key, value) in ["uses", "usesAgent", "usesUser", "usesScheduled", "failed"]
        .iter()
        .zip(values)
    {
        row.insert((*key).to_string(), json!(value));
    }
    row
}

pub fn mode_of(path: &Path) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .expect("metadata")
        .permissions()
        .mode()
        & 0o7777
}
