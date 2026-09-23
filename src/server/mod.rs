use crate::backend;
use crate::lease::operations::{Operation, Operations};
use crate::lease::transport::Output;
use crate::{AppError, AppResult};
use base64::Engine as _;
use base64::prelude::BASE64_STANDARD;
use clap::Args;
use rustix::event::{PollFd, PollFlags, poll};
use rustix::fs::Timespec;
use rustix::fs::inotify::{self, CreateFlags, ReadFlags, WatchFlags};
use rustix::io::Errno;
use rustix::process::{Signal, getppid, set_parent_process_death_signal};
use serde_json::{Map, Value, json};
use std::collections::{HashMap, HashSet, VecDeque};
use std::io::{self, BufRead, BufReader};
use std::mem::MaybeUninit;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

mod input;
mod lifecycle;
mod mounts;
#[path = "../lease/server.rs"]
mod native;
mod request;
mod subscription;
use lifecycle::*;
use mounts::*;
pub use request::native_mutating;
use request::*;
use subscription::*;
const VERSION: u64 = 1;
const MAX_LINE_BYTES: usize = 1024 * 1024;
const MAX_RESPONSE_BYTES: usize = 32 * 1024 * 1024;
const MAX_ARGUMENTS: usize = 4096;
const MAX_WATCH_PATHS: usize = 512;
const RECENT_REQUEST_KEYS: usize = 4_096;
const MAX_IDENTIFIER_BYTES: usize = 128;
const DEFAULT_DEADLINE_MS: u64 = 15_000;
const MAX_DEADLINE_MS: u64 = 900_000;
const MONITOR_IDLE_TICK: Duration = Duration::from_millis(500);
const WATCH_POLL_TIMEOUT: Timespec = Timespec {
    tv_sec: 0,
    tv_nsec: 500_000_000,
};

#[derive(Clone, Debug, Args)]
pub struct ServeArgs {
    #[arg(long, default_value_t = 8, value_parser = concurrency)]
    pub max_concurrency: usize,
    #[arg(long)]
    pub no_recover: bool,
    #[arg(long)]
    pub native_authority: bool,
    #[arg(long)]
    pub native_probe: bool,
    #[arg(long, requires = "native_authority")]
    pub native_isolated: bool,
}

#[derive(Clone)]
struct ActiveRequest {
    cancelled: Arc<AtomicBool>,
    deadline: Option<Arc<Deadline>>,
    deadline_exceeded: Arc<AtomicBool>,
    cancel_on_deadline: bool,
    standing: bool,
    chooser_watch: bool,
    authority_owned: bool,
}

struct Deadline {
    at: Mutex<Instant>,
    budget: Duration,
}

impl Deadline {
    fn new(budget: Duration) -> Self {
        Self {
            at: Mutex::new(Instant::now() + budget),
            budget,
        }
    }

    fn expired(&self, now: Instant) -> bool {
        now >= *lock(&self.at)
    }

    fn renew(&self) {
        *lock(&self.at) = Instant::now() + self.budget;
    }

    fn next(&self) -> Instant {
        *lock(&self.at)
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
struct RequestKey {
    id: String,
    generation: String,
}

#[derive(Default)]
struct RecentKeys {
    order: VecDeque<RequestKey>,
    set: HashSet<RequestKey>,
}

impl RecentKeys {
    fn contains(&self, key: &RequestKey) -> bool {
        self.set.contains(key)
    }

    fn remember(&mut self, key: RequestKey) {
        if !self.set.insert(key.clone()) {
            return;
        }
        self.order.push_back(key);
        while self.order.len() > RECENT_REQUEST_KEYS {
            if let Some(oldest) = self.order.pop_front() {
                self.set.remove(&oldest);
            }
        }
    }
}

struct Request {
    key: RequestKey,
    generation: Value,
    name: String,
    arguments: Vec<String>,
    actor: String,
    command: backend::BackendCommand,
    deadline: Arc<Deadline>,
}

enum InputLine {
    End,
    Line(Vec<u8>),
    TooLong,
}

pub fn run(options: ServeArgs, output: Arc<fileblade_output::Output>) -> AppResult<()> {
    if let Some(root) = crate::lease::selected_root()? {
        if options.native_authority {
            let authority = Arc::new(
                crate::lease::Authority::acquire_bound(
                    &root,
                    crate::lease::native_config_root(),
                    crate::lease::native_recovery_root(),
                )
                .map_err(|error| AppError::command(error.to_string()))?,
            );
            let _clipboard = crate::clipboard::Session::open()?;
            if options.native_isolated {
                let home = std::env::var_os("FILEBLADE_SPIKE_HOME")
                    .map(PathBuf::from)
                    .filter(|home| home.is_absolute())
                    .ok_or_else(|| {
                        AppError::command("isolated authority requires FILEBLADE_SPIKE_HOME")
                    })?;
                for (role, selected) in [
                    ("state", home.join("state/omarchy/fileblade")),
                    ("config", home.join("config/omarchy/fileblade")),
                    ("recovery", home.join("state/fileblade")),
                ] {
                    if std::fs::canonicalize(selected)? != authority.roots()[role].path {
                        return Err(AppError::command(
                            "isolated authority roots must belong to FILEBLADE_SPIKE_HOME",
                        ));
                    }
                }
                authority
                    .set_write_mode(crate::lease::WriteMode::Full)
                    .map_err(|error| AppError::command(error.to_string()))?;
            } else {
                let roots = crate::migration::Roots {
                    state: root.clone(),
                    config: crate::lease::native_config_root(),
                    recovery: crate::lease::native_recovery_root(),
                };
                let preparation = crate::migration::prepare(&roots, &roots, &authority)?;
                let mode = match preparation.status {
                    crate::migration::Status::Ready => crate::lease::WriteMode::Full,
                    crate::migration::Status::ReadOnly { reason }
                    | crate::migration::Status::Refused { reason } => {
                        crate::lease::WriteMode::ReadOnly {
                            reason: format!(
                                "{reason}; migration receipt: {}",
                                preparation.receipt_path.display()
                            ),
                        }
                    }
                };
                authority
                    .set_write_mode(mode)
                    .map_err(|error| AppError::command(error.to_string()))?;
            }
            let _persistence =
                crate::lease::persistence::PersistenceSession::open(Arc::clone(&authority))?;
            return native::run(options, authority);
        }
        if options.native_probe {
            return crate::lease::transport::probe(&root).map_err(AppError::Io);
        }
        return crate::lease::transport::bridge(&root).map_err(AppError::Io);
    }
    if options.native_authority || options.native_probe {
        return Err(AppError::command(
            "native authority requires FILEBLADE_NATIVE_STATE_ROOT",
        ));
    }
    let output = Arc::new(Output::Stdio(output));
    let _signals = input::InputSignals::install()?;
    let parent = getppid();
    set_parent_process_death_signal(Some(Signal::TERM)).map_err(|error| {
        AppError::command(format!(
            "could not bind server lifetime to its parent: {error}"
        ))
    })?;
    if getppid() != parent {
        return Ok(());
    }

    let _clipboard = crate::clipboard::Session::open()?;
    let recovery_started = Instant::now();
    let recovered = if options.no_recover {
        json!({"ok": true, "skipped": true})
    } else {
        let recovered = crate::recovery::sweep();
        let _ = crate::audit::record(&crate::audit::Event {
            via: "serve",
            actor: "serve",
            command: "recover",
            arguments: &[],
            outcome: &Ok(recovered.clone()),
            started: recovery_started,
        });
        recovered
    };
    let active = Arc::new(Mutex::new(HashMap::<RequestKey, ActiveRequest>::new()));
    let stopping = Arc::new(AtomicBool::new(false));
    let deadline_monitor = monitor_deadlines(Arc::clone(&active), Arc::clone(&stopping));
    let mut reader = BufReader::new(input::InterruptibleStdin);
    let mut workers = Vec::new();
    let mut seen = RecentKeys::default();

    match read_bounded_line(&mut reader, MAX_LINE_BYTES)? {
        InputLine::Line(line) => {
            if let Err(error) = handshake(&line, &output, options.max_concurrency, &recovered) {
                emit(
                    &output,
                    &json!({"v": VERSION, "type": "error", "ok": false, "error": error.to_string()}),
                )?;
                shutdown(&active, &stopping, workers, deadline_monitor);
                return Ok(());
            }
        }
        InputLine::TooLong => {
            emit(
                &output,
                &json!({"v": VERSION, "type": "error", "ok": false, "error": "handshake exceeds the line limit"}),
            )?;
            shutdown(&active, &stopping, workers, deadline_monitor);
            return Ok(());
        }
        InputLine::End => {
            shutdown(&active, &stopping, workers, deadline_monitor);
            return Ok(());
        }
    }

    let result = (|| -> AppResult<()> {
        loop {
            if input::interrupted() {
                break;
            }
            reap(&mut workers);
            match read_bounded_line(&mut reader, MAX_LINE_BYTES)? {
                InputLine::End => break,
                InputLine::TooLong => emit(
                    &output,
                    &json!({"v": VERSION, "type": "error", "ok": false, "error": "request exceeds the line limit"}),
                )?,
                InputLine::Line(line) if line.iter().all(u8::is_ascii_whitespace) => {}
                InputLine::Line(line) => process_line(
                    &line,
                    options.max_concurrency,
                    &output,
                    &active,
                    &mut seen,
                    &mut workers,
                    None,
                )?,
            }
        }
        Ok(())
    })();

    shutdown(&active, &stopping, workers, deadline_monitor);
    result
}

fn concurrency(value: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| "concurrency must be an integer".to_string())?;
    (1..=32)
        .contains(&parsed)
        .then_some(parsed)
        .ok_or_else(|| "concurrency must be between 1 and 32".to_string())
}

fn handshake(
    line: &[u8],
    output: &Output,
    max_concurrency: usize,
    recovered: &Value,
) -> AppResult<()> {
    let value: Value = serde_json::from_slice(line)
        .map_err(|_| AppError::invalid("server handshake is not valid JSON"))?;
    let object = value
        .as_object()
        .ok_or_else(|| AppError::invalid("server handshake must be an object"))?;
    require_version(object)?;
    if text(object, "type") != "hello" {
        return Err(AppError::invalid("server handshake must have type hello"));
    }
    emit(
        output,
        &json!({
            "v": VERSION,
            "type": "hello",
            "ok": true,
            "protocol": "fileblade",
            "limits": {
                "line_bytes": MAX_LINE_BYTES,
                "response_bytes": MAX_RESPONSE_BYTES,
                "arguments": MAX_ARGUMENTS,
                "concurrency": max_concurrency,
                "deadline_ms": MAX_DEADLINE_MS,
                "identifier_bytes": MAX_IDENTIFIER_BYTES,
                "request_keys": RECENT_REQUEST_KEYS,
                "watch_paths": MAX_WATCH_PATHS,
            },
            "paths": {
                "screenshots": crate::common::path_text(&crate::paths::screenshots_dir()),
            },
            "recovered": recovered,
            "version": env!("CARGO_PKG_VERSION"),
        }),
    )
}

fn process_line(
    line: &[u8],
    max_concurrency: usize,
    output: &Arc<Output>,
    active: &Arc<Mutex<HashMap<RequestKey, ActiveRequest>>>,
    seen: &mut RecentKeys,
    workers: &mut Vec<JoinHandle<()>>,
    operations: Option<&Arc<Operations>>,
) -> AppResult<()> {
    let value: Value = match serde_json::from_slice(line) {
        Ok(value) => value,
        Err(_) => {
            return emit(
                output,
                &json!({"v": VERSION, "type": "error", "ok": false, "error": "request is not valid JSON"}),
            );
        }
    };
    let Some(object) = value.as_object() else {
        return emit(
            output,
            &json!({"v": VERSION, "type": "error", "ok": false, "error": "request must be an object"}),
        );
    };
    if let Err(error) = require_version(object) {
        return emit(output, &error_frame(object, &error.to_string()));
    }
    match text(object, "type").as_str() {
        "request" => start_request(
            object,
            max_concurrency,
            output,
            active,
            seen,
            workers,
            operations,
        ),
        "subscribe" => start_subscription(object, max_concurrency, output, active, seen, workers),
        "cancel" => cancel_request(object, output, active, operations),
        "operation" if operations.is_some() => {
            native::operation_request(object, output, operations.unwrap())
        }
        "hello" => emit(
            output,
            &error_frame(object, "handshake is already complete"),
        ),
        _ => emit(
            output,
            &error_frame(object, "unknown protocol message type"),
        ),
    }
}

fn emit(output: &Output, value: &Value) -> AppResult<()> {
    let encoded = serde_json::to_vec(value)?;
    if encoded.len() > MAX_RESPONSE_BYTES {
        let fallback = json!({
            "v": VERSION,
            "type": "error",
            "id": value.get("id").cloned().unwrap_or(Value::Null),
            "generation": value.get("generation").cloned().unwrap_or(Value::Null),
            "ok": false,
            "error": "response exceeds the protocol limit",
        });
        output.machine(&fallback)?;
    } else {
        output.machine(value)?;
    }
    Ok(())
}

const IDLE_TRIM_DELAY: Duration = Duration::from_secs(2);
const IDLE_SWEEP_INTERVAL: Duration = Duration::from_secs(30);
