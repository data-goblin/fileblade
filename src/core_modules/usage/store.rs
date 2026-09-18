use super::records::{self, Batch};
use super::sql::{self, Bound, Sql, literal};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::fs::File;
use std::io::{BufReader, Read, Seek, SeekFrom};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::MetadataExt;
use std::os::unix::io::AsRawFd;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

pub const VERSION: i64 = 3;
pub const MAX_FILES: usize = 8192;
pub const MAX_RECORD_BYTES: usize = 4 * 1024 * 1024;
pub const CHUNK_BYTES: i64 = 8 * 1024 * 1024;
pub const CHUNK_ROWS: usize = 1024;
pub const BUDGET: Duration = Duration::from_secs(3);
pub const LOCK_SECONDS: Duration = Duration::from_secs(4);
pub const MAX_WATCH_DIRECTORIES: usize = 96;
const MAX_WALK_DEPTH: usize = 64;

pub const SKILL_AGENTS: [&str; 6] = [
    "claude",
    "codex",
    "opencode",
    "copilot",
    "antigravity",
    "pi",
];
pub const MCP_AGENTS: [&str; 4] = ["claude", "codex", "opencode", "copilot"];

const SCHEMA: [&str; 5] = [
    "CREATE TABLE source (\n  id INTEGER PRIMARY KEY,\n  agent TEXT NOT NULL,\n  path TEXT NOT NULL UNIQUE,\n  device INTEGER NOT NULL,\n  inode INTEGER NOT NULL,\n  size INTEGER NOT NULL,\n  mtime INTEGER NOT NULL,\n  offset INTEGER NOT NULL,\n  project INTEGER REFERENCES project(id)\n)",
    "CREATE TABLE project (id INTEGER PRIMARY KEY, path TEXT NOT NULL UNIQUE)",
    "CREATE TABLE coverage (agent TEXT PRIMARY KEY, first_at INTEGER NOT NULL)",
    "CREATE TABLE event (\n  agent TEXT NOT NULL,\n  call TEXT NOT NULL,\n  at INTEGER NOT NULL,\n  kind TEXT NOT NULL,\n  origin TEXT NOT NULL,\n  server TEXT NOT NULL DEFAULT '',\n  name TEXT NOT NULL,\n  subagent INTEGER NOT NULL DEFAULT 0,\n  project INTEGER REFERENCES project(id),\n  failed INTEGER NOT NULL DEFAULT 0,\n  PRIMARY KEY (agent, call)\n) WITHOUT ROWID",
    "CREATE INDEX event_time ON event (kind, at)",
];
const EVENT_NAME_INDEX: &str = "CREATE INDEX event_name ON event (kind, server, name, at)";
const RETENTION: &str =
    "CREATE TABLE retention (id INTEGER PRIMARY KEY CHECK (id = 1), before INTEGER NOT NULL)";
const FAILURE: &str =
    "CREATE TABLE failure (call TEXT PRIMARY KEY, at INTEGER NOT NULL) WITHOUT ROWID";
const FAILURE_AGENT: &str = "ALTER TABLE failure ADD COLUMN agent TEXT NOT NULL DEFAULT 'claude'";
const FORGOTTEN: &str = "CREATE TABLE forgotten (identity BLOB PRIMARY KEY) WITHOUT ROWID";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Identity {
    pub device: i64,
    pub inode: i64,
    pub size: i64,
    pub mtime: i64,
}

pub struct Source {
    pub agent: &'static str,
    pub directory: PathBuf,
    pub pattern: &'static str,
    pub recursive: bool,
}

#[derive(Clone)]
pub struct Environment {
    home: PathBuf,
    variables: HashMap<OsString, OsString>,
}

impl Environment {
    pub fn current() -> Self {
        let variables: HashMap<OsString, OsString> = std::env::vars_os().collect();
        Self::new(variables)
    }

    pub fn new(variables: HashMap<OsString, OsString>) -> Self {
        let home = variables
            .get(OsStr::new("HOME"))
            .map(PathBuf::from)
            .unwrap_or_default();
        Self { home, variables }
    }

    pub fn home(&self) -> &Path {
        &self.home
    }

    pub fn directory(&self, variable: &str, fallback: &str) -> PathBuf {
        match self.variables.get(OsStr::new(variable)) {
            Some(value) if Path::new(value).is_absolute() => PathBuf::from(value),
            _ => self.home.join(fallback),
        }
    }

    pub fn state(&self) -> PathBuf {
        self.directory("XDG_STATE_HOME", ".local/state")
            .join("omarchy/fileblade")
    }

    pub fn cache(&self) -> PathBuf {
        self.directory("XDG_CACHE_HOME", ".cache")
            .join("omarchy/fileblade")
    }

    pub fn sources(&self) -> Vec<Source> {
        let claude = self.directory("CLAUDE_CONFIG_DIR", ".claude");
        let codex = self.directory("CODEX_HOME", ".codex");
        let copilot = self.directory("COPILOT_HOME", ".copilot");
        let antigravity = self
            .directory("GEMINI_HOME", ".gemini")
            .join("antigravity-cli");
        let pi = self.directory("PI_HOME", ".pi").join("agent");
        vec![
            Source {
                agent: "claude",
                directory: claude.join("projects"),
                pattern: "*.jsonl",
                recursive: true,
            },
            Source {
                agent: "codex",
                directory: codex.join("sessions"),
                pattern: "*.jsonl",
                recursive: true,
            },
            Source {
                agent: "codex",
                directory: codex.join("archived_sessions"),
                pattern: "*.jsonl",
                recursive: true,
            },
            Source {
                agent: "opencode",
                directory: self
                    .directory("XDG_DATA_HOME", ".local/share")
                    .join("opencode"),
                pattern: "opencode*.db",
                recursive: false,
            },
            Source {
                agent: "copilot",
                directory: copilot.join("session-state"),
                pattern: "*/events.jsonl",
                recursive: false,
            },
            Source {
                agent: "antigravity",
                directory: antigravity.clone(),
                pattern: "history.jsonl",
                recursive: false,
            },
            Source {
                agent: "antigravity",
                directory: antigravity.join("brain"),
                pattern: "*/.system_generated/logs/transcript_full.jsonl",
                recursive: false,
            },
            Source {
                agent: "pi",
                directory: pi.join("sessions"),
                pattern: "*.jsonl",
                recursive: true,
            },
        ]
    }
}

pub fn clean_bytes(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len());
    let mut index = 0usize;
    while index < bytes.len() {
        match std::str::from_utf8(&bytes[index..]) {
            Ok(text) => {
                out.push_str(text);
                break;
            }
            Err(error) => {
                let valid = error.valid_up_to();
                out.push_str(std::str::from_utf8(&bytes[index..index + valid]).unwrap_or(""));
                let skipped = error
                    .error_len()
                    .unwrap_or(bytes.len() - index - valid)
                    .max(1);
                let stop = (index + valid + skipped).min(bytes.len());
                for _ in index + valid..stop {
                    out.push('?');
                }
                index = stop;
            }
        }
    }
    out
}

pub fn clean_path(path: &Path) -> String {
    clean_bytes(path.as_os_str().as_bytes())
}

pub fn identity(agent: &str, call: &str) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(agent.as_bytes());
    hasher.update(b"\0");
    hasher.update(call.as_bytes());
    hasher.finalize().to_vec()
}

fn sorted_entries(directory: &Path) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = entries.flatten().map(|entry| entry.path()).collect();
    found.sort_by(|left, right| {
        left.as_os_str()
            .as_bytes()
            .cmp(right.as_os_str().as_bytes())
    });
    found
}

fn is_real_directory(path: &Path) -> bool {
    std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.is_dir())
}

fn glob_into(directory: &Path, parts: &[&str], found: &mut Vec<PathBuf>) {
    let Some((head, rest)) = parts.split_first() else {
        return;
    };
    let wild = head.contains(['*', '?', '[']);
    if !wild {
        let candidate = directory.join(head);
        if rest.is_empty() {
            if std::fs::symlink_metadata(&candidate).is_ok() {
                found.push(candidate);
            }
        } else if candidate.is_dir() {
            glob_into(&candidate, rest, found);
        }
        return;
    }
    for entry in sorted_entries(directory) {
        let Some(name) = entry.file_name() else {
            continue;
        };
        if !crate::core_modules::glob::fnmatch_bytes(name.as_bytes(), head) {
            continue;
        }
        if rest.is_empty() {
            found.push(entry);
        } else if entry.is_dir() {
            glob_into(&entry, rest, found);
        }
    }
}

fn walk_directories(directory: &Path, depth: usize, found: &mut Vec<PathBuf>) {
    found.push(directory.to_path_buf());
    if depth >= MAX_WALK_DEPTH {
        return;
    }
    for entry in sorted_entries(directory) {
        if is_real_directory(&entry) {
            walk_directories(&entry, depth + 1, found);
        }
    }
}

fn glob(source: &Source) -> Option<Vec<PathBuf>> {
    if !source.directory.is_dir() {
        return None;
    }
    let parts: Vec<&str> = source.pattern.split('/').collect();
    let mut found = Vec::new();
    if source.recursive {
        let mut directories = Vec::new();
        walk_directories(&source.directory, 0, &mut directories);
        for directory in directories {
            glob_into(&directory, &parts, &mut found);
        }
    } else {
        glob_into(&source.directory, &parts, &mut found);
    }
    Some(found)
}

fn stat_identity(path: &Path) -> std::io::Result<(std::fs::Metadata, Identity)> {
    let metadata = std::fs::metadata(path)?;
    let status = Identity {
        device: metadata.dev() as i64,
        inode: metadata.ino() as i64,
        size: metadata.len() as i64,
        mtime: metadata.mtime() * 1_000_000_000 + metadata.mtime_nsec(),
    };
    Ok((metadata, status))
}

fn sidecar_identity(path: &Path, base: Identity) -> Identity {
    let mut status = base;
    for suffix in ["-wal", "-shm"] {
        let mut name = path.as_os_str().to_os_string();
        name.push(suffix);
        if let Ok((_, extra)) = stat_identity(Path::new(&name)) {
            status.size += extra.size;
            status.mtime = status.mtime.max(extra.mtime);
        }
    }
    status
}

pub fn transcripts(environment: &Environment) -> (Vec<(&'static str, PathBuf, Identity)>, i64) {
    let mut found: Vec<(&'static str, PathBuf, Identity)> = Vec::new();
    let mut unreadable = 0i64;
    let mut counts: HashMap<&'static str, usize> = HashMap::new();
    for source in environment.sources() {
        let Some(candidates) = glob(&source) else {
            continue;
        };
        for path in candidates {
            if counts.get(source.agent).copied().unwrap_or(0) >= MAX_FILES {
                break;
            }
            let Ok((metadata, status)) = stat_identity(&path) else {
                unreadable += 1;
                continue;
            };
            if !metadata.is_file() {
                continue;
            }
            let status = if source.agent == "opencode" {
                sidecar_identity(&path, status)
            } else {
                status
            };
            found.push((source.agent, path, status));
            *counts.entry(source.agent).or_insert(0) += 1;
        }
    }
    found.sort_by_key(|entry| std::cmp::Reverse(entry.2.mtime));
    (found, unreadable)
}

fn recent_directories(directory: &Path, limit: usize, suffix: &str) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut ranked: Vec<(i64, PathBuf)> = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let Ok((_, status)) = stat_identity(&path) else {
            continue;
        };
        ranked.push((status.mtime, path));
    }
    ranked.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    ranked
        .into_iter()
        .take(limit)
        .map(|(_, path)| {
            if suffix.is_empty() {
                path
            } else {
                path.join(suffix)
            }
        })
        .filter(|path| path.is_dir())
        .collect()
}

pub fn watch_paths(environment: &Environment) -> Vec<PathBuf> {
    let mut found: Vec<PathBuf> = Vec::new();
    let today = chrono::Local::now().date_naive();
    for source in environment.sources() {
        if !source.directory.is_dir() {
            continue;
        }
        found.push(source.directory.clone());
        match source.agent {
            "claude" => found.extend(recent_directories(&source.directory, 24, "")),
            "codex" => {
                for offset in [0i64, 1] {
                    let Some(day) = today.checked_sub_signed(chrono::Duration::days(offset)) else {
                        continue;
                    };
                    let daily = source
                        .directory
                        .join(day.format("%Y").to_string())
                        .join(day.format("%m").to_string())
                        .join(day.format("%d").to_string());
                    if daily.is_dir() {
                        found.push(daily);
                    }
                }
            }
            "copilot" => found.extend(recent_directories(&source.directory, 16, "")),
            "antigravity" if source.directory.file_name() == Some(OsStr::new("brain")) => {
                found.extend(recent_directories(
                    &source.directory,
                    8,
                    ".system_generated/logs",
                ));
            }
            "pi" => found.extend(recent_directories(&source.directory, 8, "")),
            _ => {}
        }
    }
    let mut unique: Vec<PathBuf> = Vec::new();
    for path in found {
        let Ok(resolved) = std::fs::canonicalize(&path) else {
            continue;
        };
        if !unique.contains(&resolved) {
            unique.push(resolved);
        }
    }
    unique.truncate(MAX_WATCH_DIRECTORIES);
    unique
}

pub fn connect(directory: &Path) -> sql::Result<Sql> {
    connect_with_cache(directory, None)
}

fn connect_with_cache(directory: &Path, cache: Option<&Path>) -> sql::Result<Sql> {
    if let Some(cache) = cache {
        for name in ["agent-usage.json", "agent-usage.tmp"] {
            let _ = std::fs::remove_file(cache.join(name));
        }
    }
    create_private_directory(directory)?;
    let path = directory.join("agent-usage.sqlite3");
    create_private_file(&path)?;
    let database = Sql::open(&path)?;
    prepare(&database)?;
    Ok(database)
}

fn create_private_directory(directory: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::DirBuilderExt;
    if directory.is_dir() {
        return Ok(());
    }
    std::fs::DirBuilder::new()
        .recursive(true)
        .mode(0o700)
        .create(directory)
}

fn create_private_file(path: &Path) -> std::io::Result<()> {
    use std::os::unix::fs::OpenOptionsExt;
    std::fs::OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .mode(0o600)
        .open(path)
        .map(|_| ())
}

fn prepare(database: &Sql) -> sql::Result<()> {
    database.execute("PRAGMA journal_mode = WAL")?;
    let version = user_version(database)?;
    if version == VERSION {
        return Ok(());
    }
    if !(version == 0 || version == 1 || version == 2) {
        return Err(sql::Error::new("unsupported usage schema"));
    }
    let mut script = String::from("BEGIN IMMEDIATE;\n");
    if version == 0 {
        for statement in SCHEMA {
            script.push_str(statement);
            script.push_str(";\n");
        }
        script.push_str(EVENT_NAME_INDEX);
        script.push_str(";\n");
    }
    if version < 2 {
        for statement in [RETENTION, FAILURE, FORGOTTEN] {
            script.push_str(statement);
            script.push_str(";\n");
        }
    }
    if version < 3 {
        script.push_str(FAILURE_AGENT);
        script.push_str(";\n");
    }
    script.push_str(&format!("PRAGMA user_version = {VERSION};\nCOMMIT;\n"));
    match database.execute(&script) {
        Ok(()) => Ok(()),
        Err(error) => {
            if user_version(database)? == VERSION {
                Ok(())
            } else {
                Err(error)
            }
        }
    }
}

fn user_version(database: &Sql) -> sql::Result<i64> {
    database
        .query_one("PRAGMA user_version")?
        .map(|row| row.integer(0))
        .ok_or_else(|| sql::Error::new("usage store has no schema version"))
}

struct Lock {
    file: File,
}

impl Lock {
    fn acquire(path: &Path) -> std::io::Result<Option<Self>> {
        use std::os::unix::fs::OpenOptionsExt;
        let file = std::fs::OpenOptions::new()
            .read(true)
            .write(true)
            .create(true)
            .truncate(false)
            .mode(0o600)
            .open(path)?;
        let deadline = Instant::now() + LOCK_SECONDS;
        loop {
            let taken = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
            if taken == 0 {
                return Ok(Some(Self { file }));
            }
            if Instant::now() >= deadline {
                return Ok(None);
            }
            std::thread::sleep(Duration::from_millis(50));
        }
    }
}

impl Drop for Lock {
    fn drop(&mut self) {
        unsafe { libc::flock(self.file.as_raw_fd(), libc::LOCK_UN) };
    }
}

#[derive(Clone, Debug)]
struct Known {
    device: i64,
    inode: i64,
    size: i64,
    mtime: i64,
    offset: i64,
    project: Option<String>,
}

fn read_lines(
    agent: &str,
    path: &Path,
    status: Identity,
    row: Option<&Known>,
    deadline: Instant,
) -> std::io::Result<Batch> {
    let restart = match row {
        None => true,
        Some(row) => {
            row.device != status.device || row.inode != status.inode || status.size < row.offset
        }
    };
    let mut batch = Batch::new(
        if restart {
            0
        } else {
            row.map_or(0, |row| row.offset)
        },
        if restart {
            None
        } else {
            row.and_then(|row| row.project.clone())
        },
        clean_path(path),
    );
    let Some((wanted, consume)) = records::line_reader(agent) else {
        return Ok(batch);
    };
    let file = File::open(path)?;
    let mut handle = BufReader::new(file);
    let mut skipping = false;
    if batch.offset > 0 {
        handle.seek(SeekFrom::Start(batch.offset as u64 - 1))?;
        let mut byte = [0u8; 1];
        let read = handle.read(&mut byte)?;
        skipping = read != 1 || byte[0] != b'\n';
    } else {
        handle.seek(SeekFrom::Start(0))?;
    }
    let start = batch.offset;
    let mut raw: Vec<u8> = Vec::new();
    loop {
        if Instant::now() >= deadline
            || batch.offset - start >= CHUNK_BYTES
            || batch.events.len() >= CHUNK_ROWS
        {
            batch.pending = batch.offset < status.size;
            break;
        }
        raw.clear();
        read_bounded_line(&mut handle, MAX_RECORD_BYTES + 1, &mut raw)?;
        if raw.is_empty() {
            break;
        }
        let terminated = raw.last() == Some(&b'\n');
        if skipping || raw.len() > MAX_RECORD_BYTES {
            skipping = !terminated;
            batch.offset += raw.len() as i64;
            continue;
        }
        if !terminated {
            break;
        }
        let opening = restart
            && batch.first_at.is_none()
            && (contains_bytes(&raw, b"\"timestamp\"") || contains_bytes(&raw, b"\"created_at\""));
        batch.offset += raw.len() as i64;
        if !(opening || wanted(&raw)) {
            continue;
        }
        let Ok(record) = serde_json::from_slice::<serde_json::Value>(&raw) else {
            continue;
        };
        if let serde_json::Value::Object(record) = record {
            consume(&record, &mut batch);
        }
    }
    Ok(batch)
}

fn contains_bytes(raw: &[u8], needle: &[u8]) -> bool {
    raw.windows(needle.len()).any(|window| window == needle)
}

fn read_bounded_line(
    handle: &mut BufReader<File>,
    limit: usize,
    out: &mut Vec<u8>,
) -> std::io::Result<()> {
    use std::io::BufRead;
    while out.len() < limit {
        let available = match handle.fill_buf() {
            Ok(available) => available,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        if available.is_empty() {
            break;
        }
        let room = limit - out.len();
        let newline = available.iter().position(|byte| *byte == b'\n');
        match newline {
            Some(index) if index < room => {
                out.extend_from_slice(&available[..=index]);
                handle.consume(index + 1);
                break;
            }
            _ => {
                let taken = available.len().min(room);
                out.extend_from_slice(&available[..taken]);
                handle.consume(taken);
            }
        }
    }
    Ok(())
}

fn remaining(deadline: Instant) -> Duration {
    deadline.saturating_duration_since(Instant::now())
}

fn read_sqlite(path: &Path, row: Option<&Known>, deadline: Instant) -> std::io::Result<Batch> {
    let watermark = row.map_or(0, |row| row.offset.max(0));
    let mut batch = Batch::new(watermark, None, clean_path(path));
    let database = Sql::open_readonly(path)
        .map_err(sqlite_io)?
        .with_busy(1000)
        .with_timeout(remaining(deadline));
    read_opencode(&database, watermark, &mut batch, deadline)?;
    Ok(batch)
}

fn sqlite_io(error: sql::Error) -> std::io::Error {
    std::io::Error::other(error.to_string())
}

fn read_opencode(
    database: &Sql,
    watermark: i64,
    batch: &mut Batch,
    deadline: Instant,
) -> std::io::Result<()> {
    let tables: Vec<String> = database
        .query("SELECT name FROM sqlite_master WHERE type = 'table'")
        .map_err(sqlite_io)?
        .iter()
        .map(|row| row.text(0))
        .collect();
    let has = |name: &str| tables.iter().any(|entry| entry == name);
    let (query, table) = if has("part") && has("session") {
        (
            "SELECT part.id, part.time_updated, part.time_created, part.data, session.directory FROM part \
             LEFT JOIN session ON session.id = part.session_id WHERE part.time_updated > ? \
             AND instr(part.data, '\"tool\"') > 0 ORDER BY part.time_updated LIMIT ?",
            "session",
        )
    } else if has("session_message") && has("session_v2") {
        (
            "SELECT m.id, m.time_updated, m.time_created, m.data, s.directory FROM session_message m \
             LEFT JOIN session_v2 s ON s.id = m.session_id WHERE m.time_updated > ? AND m.type = 'assistant' \
             AND instr(m.data, '\"tool\"') > 0 ORDER BY m.time_updated LIMIT ?",
            "session_v2",
        )
    } else {
        return Ok(());
    };
    if watermark == 0 {
        let earliest = database
            .query_one(&format!("SELECT min(time_created) FROM {table}"))
            .map_err(sqlite_io)?
            .and_then(|row| row.optional_integer(0));
        if let Some(earliest) = earliest {
            batch.note(earliest);
        }
    }
    let statement = sql::bind(
        query,
        &[
            Bound::Integer(watermark),
            Bound::Integer(CHUNK_ROWS as i64 + 1),
        ],
    )
    .map_err(sqlite_io)?;
    let mut rows = database.query(&statement).map_err(sqlite_io)?;
    batch.pending = rows.len() > CHUNK_ROWS;
    rows.truncate(CHUNK_ROWS);
    for row in rows {
        if Instant::now() >= deadline {
            batch.pending = true;
            break;
        }
        let created = row.optional_integer(2);
        batch.offset = batch.offset.max(row.optional_integer(1).unwrap_or(0));
        let Some(document) = payload(row.value(3)) else {
            continue;
        };
        let project = row.optional_text(4).filter(|value| !value.is_empty());
        let identity = value_text(row.value(0));
        if table == "session" {
            let state = records::mapping(document.get("state")).clone();
            let time = records::mapping(state.get("time")).clone();
            let at = chosen_moment(time.get("end"), time.get("start"), created);
            if let Some(at) = at {
                let at = batch.note(at);
                records::opencode_part(&document, &identity, at, project.as_deref(), batch);
            }
            continue;
        }
        let content = match document.get("content") {
            Some(Value::Array(items)) => items.clone(),
            _ => Vec::new(),
        };
        for (index, item) in content.iter().enumerate() {
            let Some(item) = item.as_object() else {
                continue;
            };
            if records::text(item.get("type")) != "tool" {
                continue;
            }
            let time = records::mapping(item.get("time")).clone();
            let at = chosen_moment(time.get("completed"), time.get("created"), created);
            let Some(at) = at else { continue };
            let call = {
                let declared = records::text(item.get("id"));
                if declared.is_empty() {
                    format!("{identity}:{index}")
                } else {
                    declared.to_string()
                }
            };
            let mut shaped = serde_json::Map::new();
            shaped.insert("type".to_string(), Value::from("tool"));
            shaped.insert("callID".to_string(), Value::from(call));
            shaped.insert(
                "tool".to_string(),
                Value::from(records::text(item.get("name"))),
            );
            shaped.insert(
                "state".to_string(),
                Value::Object(records::mapping(item.get("state")).clone()),
            );
            let at = batch.note(at);
            records::opencode_part(
                &shaped,
                &format!("{identity}:{index}"),
                at,
                project.as_deref(),
                batch,
            );
        }
    }
    Ok(())
}

fn payload(value: &Value) -> Option<serde_json::Map<String, Value>> {
    let Value::String(text) = value else {
        return None;
    };
    if let Ok(Value::Object(document)) = serde_json::from_str::<Value>(text) {
        return Some(document);
    }
    let bytes: Vec<u8> = text
        .chars()
        .map(|character| (character as u32 & 0xff) as u8)
        .collect();
    match serde_json::from_str::<Value>(&clean_bytes(&bytes)) {
        Ok(Value::Object(document)) => Some(document),
        _ => None,
    }
}

fn chosen_moment(
    first: Option<&serde_json::Value>,
    second: Option<&serde_json::Value>,
    fallback: Option<i64>,
) -> Option<i64> {
    let truthy = |value: Option<&serde_json::Value>| match value {
        None | Some(serde_json::Value::Null) => false,
        Some(serde_json::Value::Bool(value)) => *value,
        Some(serde_json::Value::Number(number)) => {
            number.as_f64().is_some_and(|value| value != 0.0)
        }
        Some(serde_json::Value::String(value)) => !value.is_empty(),
        Some(serde_json::Value::Array(items)) => !items.is_empty(),
        Some(serde_json::Value::Object(entries)) => !entries.is_empty(),
    };
    let chosen = if truthy(first) {
        first.cloned()
    } else if truthy(second) {
        second.cloned()
    } else {
        fallback.map(serde_json::Value::from)
    };
    match chosen {
        Some(serde_json::Value::Number(number)) => number.as_i64(),
        _ => None,
    }
}

fn value_text(value: &Value) -> String {
    match value {
        Value::Null => "None".to_string(),
        Value::String(text) => text.clone(),
        other => other.to_string(),
    }
}

fn project_reference(script: &mut String, path: Option<&str>) -> String {
    let Some(path) = path else {
        return "NULL".to_string();
    };
    let value = literal(&Bound::Text(clean_bytes(path.as_bytes())));
    script.push_str(&format!(
        "INSERT OR IGNORE INTO project (path) VALUES ({value});\n"
    ));
    format!("(SELECT id FROM project WHERE path = {value})")
}

fn write(
    database: &Sql,
    agent: &str,
    path: &str,
    status: Identity,
    batch: &Batch,
) -> sql::Result<()> {
    let cutoff = database
        .query_one("SELECT coalesce(max(before), -9223372036854775808) FROM retention")?
        .map_or(i64::MIN, |row| row.integer(0));
    let forgotten: Vec<Vec<u8>> = database
        .query("SELECT identity FROM forgotten")?
        .iter()
        .map(|row| row.blob(0))
        .collect();
    let kept = |agent: &str, call: &str| {
        forgotten.is_empty() || !forgotten.contains(&identity(agent, call))
    };
    let events: Vec<&records::Event> = batch
        .events
        .iter()
        .filter(|event| event.at >= cutoff && kept(&event.agent, &event.call))
        .collect();
    let covered = match batch.first_at {
        None => false,
        Some(first) => !events.is_empty() || (batch.events.is_empty() && first >= cutoff),
    };
    let mut script = String::from("BEGIN IMMEDIATE;\n");
    for event in &events {
        let project = project_reference(&mut script, event.project.as_deref());
        let values = [
            literal(&Bound::Text(clean_bytes(event.agent.as_bytes()))),
            literal(&Bound::Text(clean_bytes(event.call.as_bytes()))),
            literal(&Bound::Integer(event.at)),
            literal(&Bound::Text(clean_bytes(event.kind.as_bytes()))),
            literal(&Bound::Text(clean_bytes(event.origin.as_bytes()))),
            literal(&Bound::Text(clean_bytes(event.server.as_bytes()))),
            literal(&Bound::Text(clean_bytes(event.name.as_bytes()))),
            literal(&Bound::Integer(event.subagent)),
            project,
            literal(&Bound::Integer(event.failed)),
        ];
        script.push_str(&format!(
            "INSERT OR IGNORE INTO event VALUES ({});\n",
            values.join(", ")
        ));
    }
    if !events.is_empty() || !batch.failures.is_empty() {
        for (call, at) in &batch.failures {
            if *at < cutoff || !kept(agent, call) {
                continue;
            }
            script.push_str(&format!(
                "INSERT OR IGNORE INTO failure (call, at, agent) VALUES ({}, {}, {});\n",
                literal(&Bound::Text(clean_bytes(call.as_bytes()))),
                literal(&Bound::Integer(*at)),
                literal(&Bound::Text(agent.to_string()))
            ));
        }
        script.push_str(
            "UPDATE event SET failed = 1 WHERE (agent, call) IN (SELECT agent, call FROM failure);\n\
             DELETE FROM failure WHERE EXISTS (SELECT 1 FROM event WHERE event.agent = failure.agent AND event.call = failure.call);\n",
        );
    }
    if covered {
        script.push_str(&format!(
            "INSERT INTO coverage VALUES ({}, {}) ON CONFLICT (agent) DO UPDATE \
             SET first_at = min(first_at, excluded.first_at);\n",
            literal(&Bound::Text(agent.to_string())),
            literal(&Bound::Integer(
                cutoff.max(batch.first_at.unwrap_or(cutoff))
            ))
        ));
    }
    let project = if covered {
        project_reference(&mut script, batch.project.as_deref())
    } else {
        format!(
            "(SELECT project FROM source WHERE path = {})",
            literal(&Bound::Text(path.to_string()))
        )
    };
    let size = if agent == "opencode" && batch.pending {
        -1
    } else {
        status.size
    };
    let values = [
        literal(&Bound::Text(agent.to_string())),
        literal(&Bound::Text(path.to_string())),
        literal(&Bound::Integer(status.device)),
        literal(&Bound::Integer(status.inode)),
        literal(&Bound::Integer(size)),
        literal(&Bound::Integer(status.mtime)),
        literal(&Bound::Integer(batch.offset)),
        project,
    ];
    script.push_str(&format!(
        "INSERT INTO source (agent, path, device, inode, size, mtime, offset, project) VALUES ({}) \
         ON CONFLICT (path) DO UPDATE SET agent = excluded.agent, device = excluded.device, inode = excluded.inode, \
         size = excluded.size, mtime = excluded.mtime, offset = excluded.offset, project = excluded.project;\n",
        values.join(", ")
    ));
    script.push_str("COMMIT;\n");
    database.execute(&script)
}

fn complete(agent: &str, row: &Known, status: Identity) -> bool {
    if row.device != status.device
        || row.inode != status.inode
        || row.size != status.size
        || row.mtime != status.mtime
    {
        return false;
    }
    agent == "opencode" || row.offset == status.size
}

pub fn ingest(
    database: &Sql,
    environment: &Environment,
    directory: &Path,
) -> sql::Result<(bool, i64)> {
    let deadline = Instant::now() + BUDGET;
    let lock = Lock::acquire(&directory.join("agent-usage.sqlite3.lock"))?;
    let Some(_lock) = lock else {
        return Ok((true, 0));
    };
    let (found, mut unreadable) = transcripts(environment);
    let mut known: HashMap<String, Known> = HashMap::new();
    for row in database.query(
        "SELECT CAST(source.path AS BLOB), device, inode, size, mtime, offset, \
         CAST(project.path AS BLOB) FROM source \
         LEFT JOIN project ON project.id = source.project",
    )? {
        known.insert(
            row.text_bytes(0),
            Known {
                device: row.integer(1),
                inode: row.integer(2),
                size: row.integer(3),
                mtime: row.integer(4),
                offset: row.integer(5),
                project: row.optional_text_bytes(6),
            },
        );
    }
    let mut pending = false;
    for (agent, path, status) in &found {
        let text = clean_path(path);
        let mut row = known.get(&text).cloned();
        if row
            .as_ref()
            .is_some_and(|row| complete(agent, row, *status))
        {
            continue;
        }
        if Instant::now() >= deadline {
            pending = true;
            continue;
        }
        loop {
            let batch = if *agent == "opencode" {
                read_sqlite(path, row.as_ref(), deadline)
            } else {
                read_lines(agent, path, *status, row.as_ref(), deadline)
            };
            let batch = match batch {
                Ok(batch) => batch,
                Err(_) => {
                    unreadable += 1;
                    break;
                }
            };
            write(database, agent, &text, *status, &batch)?;
            if !batch.pending || Instant::now() >= deadline {
                pending = pending || batch.pending;
                break;
            }
            row = Some(Known {
                device: status.device,
                inode: status.inode,
                size: status.size,
                mtime: status.mtime,
                offset: batch.offset,
                project: batch.project.clone(),
            });
        }
    }
    let seen: std::collections::HashSet<String> =
        found.iter().map(|(_, path, _)| clean_path(path)).collect();
    let vanished: Vec<String> = known
        .keys()
        .filter(|path| !seen.contains(path.as_str()) && !Path::new(path).exists())
        .cloned()
        .collect();
    if !vanished.is_empty() && !pending {
        let mut script = String::from("BEGIN IMMEDIATE;\n");
        for path in &vanished {
            script.push_str(&format!(
                "DELETE FROM source WHERE path = {};\n",
                literal(&Bound::Text(path.clone()))
            ));
        }
        script.push_str("COMMIT;\n");
        database.execute(&script)?;
    }
    Ok((pending, unreadable))
}

pub struct Session {
    pub database: Sql,
    pub pending: bool,
    pub unreadable: i64,
}

pub fn session(environment: &Environment, ingest_history: bool) -> sql::Result<Session> {
    let directory = environment.state();
    let cache = environment.cache();
    let database = connect_with_cache(&directory, Some(&cache))?;
    let (pending, unreadable) = if ingest_history {
        ingest(&database, environment, &directory)?
    } else {
        (false, 0)
    };
    Ok(Session {
        database,
        pending,
        unreadable,
    })
}
