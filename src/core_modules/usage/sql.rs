use crate::command::which;
use rustix::event::{PollFd, PollFlags, poll};
use rustix::fs::{OFlags, Timespec, fcntl_getfl, fcntl_setfl};
use rustix::io::Errno;
use serde::de::{MapAccess, Visitor};
use serde_json::Value;
use std::collections::HashMap;
use std::fmt;
use std::fmt::Write as _;
use std::io::Read;
use std::os::fd::AsFd;
use std::os::unix::process::CommandExt;
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock, PoisonError};
use std::thread;
use std::time::{Duration, Instant};

pub const PROGRAM: &str = "sqlite3";
pub const TIMEOUT: Duration = Duration::from_secs(15);
pub const BUSY_MILLISECONDS: u64 = 5000;
pub const STDOUT_LIMIT: usize = 32 * 1024 * 1024;
pub const STDERR_LIMIT: usize = 64 * 1024;

#[derive(Debug)]
pub struct Error {
    message: String,
}

impl Error {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn message(&self) -> &str {
        &self.message
    }
}

impl fmt::Display for Error {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(error: std::io::Error) -> Self {
        Self::new(error.to_string())
    }
}

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Clone, Debug)]
pub enum Bound {
    Null,
    Integer(i64),
    Text(String),
    Blob(Vec<u8>),
}

impl From<&str> for Bound {
    fn from(value: &str) -> Self {
        Self::Text(value.to_string())
    }
}

fn hex(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2 + 3);
    out.push_str("X'");
    for byte in bytes {
        let _ = write!(out, "{byte:02x}");
    }
    out.push('\'');
    out
}

pub fn literal(value: &Bound) -> String {
    match value {
        Bound::Null => "NULL".to_string(),
        Bound::Integer(number) => number.to_string(),
        Bound::Text(text) => format!("CAST({} AS TEXT)", hex(text.as_bytes())),
        Bound::Blob(bytes) => hex(bytes),
    }
}

pub fn bind(statement: &str, values: &[Bound]) -> Result<String> {
    let mut out = String::with_capacity(statement.len());
    let mut taken = 0usize;
    let mut quoted = false;
    for character in statement.chars() {
        if character == '\'' {
            quoted = !quoted;
            out.push(character);
            continue;
        }
        if character != '?' || quoted {
            out.push(character);
            continue;
        }
        let Some(value) = values.get(taken) else {
            return Err(Error::new("usage statement wants more values than given"));
        };
        out.push_str(&literal(value));
        taken += 1;
    }
    if taken != values.len() {
        return Err(Error::new("usage statement leaves values unused"));
    }
    Ok(out)
}

#[derive(Clone, Debug, Default)]
pub struct Row {
    values: Vec<Value>,
}

impl Row {
    pub fn value(&self, index: usize) -> &Value {
        self.values.get(index).unwrap_or(&Value::Null)
    }

    pub fn integer(&self, index: usize) -> i64 {
        self.optional_integer(index).unwrap_or(0)
    }

    pub fn optional_integer(&self, index: usize) -> Option<i64> {
        match self.value(index) {
            Value::Number(number) => number
                .as_i64()
                .or_else(|| number.as_f64().map(|number| number as i64)),
            Value::String(text) => text.parse().ok(),
            _ => None,
        }
    }

    pub fn text(&self, index: usize) -> String {
        self.optional_text(index).unwrap_or_default()
    }

    pub fn optional_text(&self, index: usize) -> Option<String> {
        match self.value(index) {
            Value::Null => None,
            Value::String(text) => Some(text.clone()),
            other => Some(other.to_string()),
        }
    }

    pub fn text_bytes(&self, index: usize) -> String {
        self.optional_text_bytes(index).unwrap_or_default()
    }

    pub fn optional_text_bytes(&self, index: usize) -> Option<String> {
        match self.value(index) {
            Value::Null => None,
            Value::String(_) => Some(String::from_utf8_lossy(&self.blob(index)).into_owned()),
            other => Some(other.to_string()),
        }
    }

    pub fn blob(&self, index: usize) -> Vec<u8> {
        match self.value(index) {
            Value::String(text) => text
                .chars()
                .map(|character| (character as u32 & 0xff) as u8)
                .collect(),
            _ => Vec::new(),
        }
    }
}

impl<'de> serde::Deserialize<'de> for Row {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        struct Shape;

        impl<'de> Visitor<'de> for Shape {
            type Value = Row;

            fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                formatter.write_str("a sqlite result row")
            }

            fn visit_map<A>(self, mut entries: A) -> std::result::Result<Row, A::Error>
            where
                A: MapAccess<'de>,
            {
                let mut values = Vec::new();
                while let Some((_, value)) = entries.next_entry::<String, Value>()? {
                    values.push(value);
                }
                Ok(Row { values })
            }
        }

        deserializer.deserialize_map(Shape)
    }
}

#[derive(Clone)]
pub struct Sql {
    program: PathBuf,
    path: PathBuf,
    readonly: bool,
    timeout: Duration,
    busy: u64,
}

type Slot = Arc<Mutex<Option<Session>>>;

fn sessions() -> &'static Mutex<HashMap<(PathBuf, bool), Slot>> {
    static SESSIONS: OnceLock<Mutex<HashMap<(PathBuf, bool), Slot>>> = OnceLock::new();
    SESSIONS.get_or_init(Mutex::default)
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

type Zone = (Option<std::ffi::OsString>, Option<PathBuf>);

fn zone() -> Zone {
    (
        std::env::var_os("TZ"),
        std::fs::read_link("/etc/localtime").ok(),
    )
}

struct Session {
    zone: Zone,
    child: Child,
    input: ChildStdin,
    output: ChildStdout,
    errors: Arc<Mutex<Vec<u8>>>,
    collector: Option<thread::JoinHandle<()>>,
    serial: u64,
}

impl Session {
    fn spawn(program: &Path, path: &Path, readonly: bool) -> Result<Self> {
        let mut command = Command::new(program);
        command.args(["-batch", "-bail", "-json"]);
        if readonly {
            command.arg("-readonly");
        }
        command
            .arg(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .process_group(0);
        let mut child = command
            .spawn()
            .map_err(|error| Error::new(format!("usage store: {error}")))?;
        let (Some(input), Some(output), Some(mut stderr)) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            let _ = child.kill();
            let _ = child.wait();
            return Err(Error::new("usage store has no pipes"));
        };
        for descriptor in [input.as_fd(), output.as_fd()] {
            fcntl_getfl(descriptor)
                .and_then(|flags| fcntl_setfl(descriptor, flags | OFlags::NONBLOCK))
                .map_err(std::io::Error::from)?;
        }
        let errors = Arc::new(Mutex::new(Vec::new()));
        let collected = Arc::clone(&errors);
        let collector = thread::Builder::new()
            .name("fileblade-usage-stderr".to_string())
            .stack_size(64 * 1024)
            .spawn(move || {
                let mut chunk = [0u8; 4096];
                while let Ok(count) = stderr.read(&mut chunk) {
                    if count == 0 {
                        break;
                    }
                    let mut buffer = lock(&collected);
                    let room = STDERR_LIMIT.saturating_sub(buffer.len());
                    buffer.extend_from_slice(&chunk[..count.min(room)]);
                }
            })?;
        Ok(Self {
            zone: zone(),
            child,
            input,
            output,
            errors,
            collector: Some(collector),
            serial: 0,
        })
    }

    fn reusable(&mut self) -> bool {
        self.zone == zone() && matches!(self.child.try_wait(), Ok(None))
    }

    fn stop(mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }

    fn failure(mut self) -> Error {
        drop(self.input);
        let _ = self.child.wait();
        if let Some(collector) = self.collector.take() {
            let _ = collector.join();
        }
        let message = String::from_utf8_lossy(&lock(&self.errors))
            .trim()
            .to_string();
        Error::new(if message.is_empty() {
            "usage store refused the statement".to_string()
        } else {
            message
        })
    }

    fn exchange(&mut self, body: &[u8], marker: &[u8], deadline: Instant) -> Result<Exchange> {
        let mut written = 0usize;
        let mut stdout = Vec::new();
        let mut chunk = vec![0u8; 64 * 1024];
        loop {
            if let Some(end) = finished(&stdout, marker) {
                stdout.truncate(end);
                return Ok(Exchange::Answer(stdout));
            }
            if stdout.len() > STDOUT_LIMIT {
                return Ok(Exchange::TooLarge);
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return Ok(Exchange::TimedOut);
            }
            let wants_input = written < body.len();
            let mut descriptors = vec![PollFd::new(&self.output, PollFlags::IN)];
            if wants_input {
                descriptors.push(PollFd::new(&self.input, PollFlags::OUT));
            }
            let timeout = Timespec {
                tv_sec: remaining.as_secs() as i64,
                tv_nsec: i64::from(remaining.subsec_nanos()),
            };
            match poll(&mut descriptors, Some(&timeout)) {
                Ok(_) | Err(Errno::INTR) => {}
                Err(error) => return Err(Error::from(std::io::Error::from(error))),
            }
            let readable = !descriptors[0].revents().is_empty();
            let writable = wants_input && !descriptors[1].revents().is_empty();
            if writable {
                match rustix::io::write(&self.input, &body[written..]) {
                    Ok(count) => written += count,
                    Err(Errno::AGAIN | Errno::INTR) => {}
                    Err(_) => return Ok(Exchange::Closed),
                }
            }
            if readable {
                match rustix::io::read(&self.output, &mut chunk) {
                    Ok(0) => return Ok(Exchange::Closed),
                    Ok(count) => stdout.extend_from_slice(&chunk[..count]),
                    Err(Errno::AGAIN | Errno::INTR) => {}
                    Err(_) => return Ok(Exchange::Closed),
                }
            }
        }
    }
}

enum Exchange {
    Answer(Vec<u8>),
    Closed,
    TimedOut,
    TooLarge,
}

fn finished(stdout: &[u8], marker: &[u8]) -> Option<usize> {
    if stdout.len() < marker.len() + 1 || !stdout.ends_with(b"\n") {
        return None;
    }
    let line = &stdout[..stdout.len() - 1];
    if !line.ends_with(marker) {
        return None;
    }
    let start = line.len() - marker.len();
    (start == 0 || line[start - 1] == b'\n').then_some(start)
}

impl Sql {
    pub fn open(path: impl Into<PathBuf>) -> Result<Self> {
        Self::locate(path, false)
    }

    pub fn open_readonly(path: impl Into<PathBuf>) -> Result<Self> {
        Self::locate(path, true)
    }

    fn locate(path: impl Into<PathBuf>, readonly: bool) -> Result<Self> {
        let program = which(PROGRAM).ok_or_else(|| Error::new("sqlite3 is not installed"))?;
        Ok(Self {
            program,
            path: path.into(),
            readonly,
            timeout: TIMEOUT,
            busy: BUSY_MILLISECONDS,
        })
    }

    pub fn with_timeout(mut self, value: Duration) -> Self {
        self.timeout = value.max(Duration::from_secs(1));
        self
    }

    pub fn with_busy(mut self, milliseconds: u64) -> Self {
        self.busy = milliseconds;
        self
    }

    pub fn execute(&self, script: &str) -> Result<()> {
        self.invoke(script).map(|_| ())
    }

    pub fn query(&self, script: &str) -> Result<Vec<Row>> {
        let stdout = self.invoke(script)?;
        let text = std::str::from_utf8(&stdout)
            .map_err(|_| Error::new("usage store returned invalid text"))?;
        let mut last: Vec<Row> = Vec::new();
        for set in serde_json::Deserializer::from_str(text).into_iter::<Vec<Row>>() {
            last = set.map_err(|error| Error::new(format!("usage store answer: {error}")))?;
        }
        Ok(last)
    }

    pub fn query_one(&self, script: &str) -> Result<Option<Row>> {
        Ok(self.query(script)?.into_iter().next())
    }

    fn invoke(&self, script: &str) -> Result<Vec<u8>> {
        let deadline = Instant::now() + self.timeout;
        let slot = Arc::clone(
            lock(sessions())
                .entry((self.path.clone(), self.readonly))
                .or_default(),
        );
        let mut guard = lock(&slot);
        let reusable = guard.take().and_then(|mut session| {
            if session.reusable() {
                Some(session)
            } else {
                session.stop();
                None
            }
        });
        let session = match reusable {
            Some(session) => guard.insert(session),
            None => guard.insert(Session::spawn(&self.program, &self.path, self.readonly)?),
        };
        session.serial += 1;
        lock(&session.errors).clear();
        let marker = format!("fileblade-usage-end-{}", session.serial);
        let terminator = if script.trim_end().ends_with(';') {
            ""
        } else {
            ";"
        };
        let body = format!(
            ".timeout {}\n{script}{terminator}\n.print {marker}\n",
            self.busy
        );
        let exchange = session.exchange(body.as_bytes(), marker.as_bytes(), deadline)?;
        if let Exchange::Answer(stdout) = exchange {
            return Ok(stdout);
        }
        let Some(session) = guard.take() else {
            return Err(Error::new("usage store session vanished"));
        };
        Err(match exchange {
            Exchange::Closed => session.failure(),
            Exchange::TimedOut => {
                session.stop();
                Error::new("usage store timed out")
            }
            Exchange::TooLarge | Exchange::Answer(_) => {
                session.stop();
                Error::new("usage store answer is too large")
            }
        })
    }
}
