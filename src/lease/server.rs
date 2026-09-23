use super::*;
use crate::lease::Authority;
use std::io::Read;
use std::net::Shutdown;
use std::os::fd::AsRawFd;
use std::os::unix::fs::{FileTypeExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};

pub(super) fn run(options: ServeArgs, authority: Arc<Authority>) -> AppResult<()> {
    let _signals = input::InputSignals::install()?;
    let socket = authority.root().join("authority.sock");
    if let Ok(metadata) = std::fs::symlink_metadata(&socket) {
        if !metadata.file_type().is_socket() {
            return Err(AppError::command(
                "native authority socket path contains an unrelated file",
            ));
        }
        std::fs::remove_file(&socket)?;
    }
    let recovered = if let crate::lease::WriteMode::ReadOnly { reason } = authority.write_mode() {
        json!({"ok": false, "skipped": true, "error_id": "migration-refused", "error": reason})
    } else if options.no_recover {
        json!({"ok": true, "skipped": true})
    } else {
        crate::recovery::sweep()
    };
    let (directory, relative) = authority
        .storage_anchor(&socket)?
        .ok_or_else(|| AppError::command("native authority socket has no owned directory"))?;
    let address = PathBuf::from(format!("/proc/self/fd/{}", directory.as_raw_fd())).join(relative);
    let listener = UnixListener::bind(address)?;
    std::fs::set_permissions(&socket, std::fs::Permissions::from_mode(0o600))?;
    listener.set_nonblocking(true)?;
    let operations = Arc::new(Operations::new(
        Arc::clone(&authority),
        options.max_concurrency,
    ));
    let mut clients = Vec::new();
    let stopping = Arc::new(AtomicBool::new(false));
    let result = (|| {
        while !input::interrupted() && !operations.closing() {
            if authority.verify().is_err() {
                operations.stop_after_identity_loss();
            }
            reap(&mut clients);
            match listener.accept() {
                Ok((stream, _)) if clients.len() < 32 => {
                    let operations = Arc::clone(&operations);
                    let recovered = recovered.clone();
                    let max_concurrency = options.max_concurrency;
                    let stopping = Arc::clone(&stopping);
                    clients.push(thread::spawn(move || {
                        let _ = session(stream, max_concurrency, &recovered, operations, stopping);
                    }));
                }
                Ok((stream, _)) => {
                    let _ = stream.shutdown(Shutdown::Both);
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                    thread::sleep(Duration::from_millis(20))
                }
                Err(error) => return Err(error.into()),
            }
        }
        Ok(())
    })();
    stopping.store(true, Ordering::Relaxed);
    for client in clients {
        let _ = client.join();
    }
    while operations.busy() {
        thread::sleep(Duration::from_millis(20));
    }
    if authority.verify().is_ok() {
        crate::hyprland::restore_owned_borders();
        std::fs::remove_file(socket)?;
    }
    result
}

struct SocketInput {
    stream: UnixStream,
    stopping: Arc<AtomicBool>,
    handshake_deadline: Option<Instant>,
}

impl Read for SocketInput {
    fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        loop {
            if input::interrupted() || self.stopping.load(Ordering::Relaxed) {
                return Ok(0);
            }
            if self
                .handshake_deadline
                .is_some_and(|deadline| Instant::now() >= deadline)
            {
                return Ok(0);
            }
            match self.stream.read(buffer) {
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::TimedOut
                            | io::ErrorKind::WouldBlock
                            | io::ErrorKind::Interrupted
                    ) =>
                {
                    continue;
                }
                result => return result,
            }
        }
    }
}

fn session(
    stream: UnixStream,
    max_concurrency: usize,
    recovered: &Value,
    operations: Arc<Operations>,
    authority_stopping: Arc<AtomicBool>,
) -> AppResult<()> {
    stream.set_read_timeout(Some(Duration::from_millis(50)))?;
    let output = Arc::new(Output::socket(stream.try_clone()?)?);
    let mut reader = BufReader::new(SocketInput {
        stream,
        stopping: authority_stopping,
        handshake_deadline: Some(Instant::now() + Duration::from_secs(2)),
    });
    let active: Arc<Mutex<HashMap<RequestKey, ActiveRequest>>> = Arc::default();
    let stopping = Arc::new(AtomicBool::new(false));
    let monitor = monitor_deadlines(Arc::clone(&active), Arc::clone(&stopping));
    let mut seen = RecentKeys::default();
    let mut workers = Vec::new();
    let mut view = false;
    let drain_owner = uuid::Uuid::new_v4().to_string();
    let result = (|| {
        match read_bounded_line(&mut reader, MAX_LINE_BYTES)? {
            InputLine::Line(line) => {
                let value: Value = serde_json::from_slice(&line)?;
                let object = value
                    .as_object()
                    .ok_or_else(|| AppError::invalid("handshake must be an object"))?;
                require_version(object)?;
                if text(object, "type") != "hello" {
                    return Err(AppError::invalid("native session requires hello"));
                }
                if object.get("view") == Some(&Value::Bool(true)) {
                    operations.attach_view()?;
                    view = true;
                }
                emit(
                    &output,
                    &json!({"v": VERSION, "type": "hello", "ok": true, "authority": true,
                    "protocol": "fileblade", "version": env!("CARGO_PKG_VERSION"), "recovered": recovered, "write_mode": operations.write_mode(),
                    "limits": {"concurrency": max_concurrency, "line_bytes": MAX_LINE_BYTES, "response_bytes": MAX_RESPONSE_BYTES,
                    "arguments": MAX_ARGUMENTS, "deadline_ms": MAX_DEADLINE_MS,
                    "identifier_bytes": MAX_IDENTIFIER_BYTES, "request_keys": RECENT_REQUEST_KEYS,
                    "watch_paths": MAX_WATCH_PATHS}}),
                )?;
            }
            _ => {
                return Err(AppError::invalid(
                    "native session handshake missing or too long",
                ));
            }
        }
        reader.get_mut().handshake_deadline = None;
        loop {
            reap(&mut workers);
            match read_bounded_line(&mut reader, MAX_LINE_BYTES)? {
                InputLine::End => break,
                InputLine::TooLong => emit(
                    &output,
                    &json!({"v": VERSION, "type": "error", "ok": false, "error": "request exceeds the line limit"}),
                )?,
                InputLine::Line(line) if line.iter().all(u8::is_ascii_whitespace) => {}
                InputLine::Line(line)
                    if lifecycle_request(&line, &drain_owner, &output, &operations)? => {}
                InputLine::Line(line) => process_line(
                    &line,
                    max_concurrency,
                    &output,
                    &active,
                    &mut seen,
                    &mut workers,
                    Some(&operations),
                )?,
            }
        }
        Ok(())
    })();
    operations.resume(&drain_owner);
    output.detach();
    if view {
        operations.detach_view();
    }
    for request in lock(&active)
        .values()
        .filter(|request| !request.authority_owned)
    {
        request.cancelled.store(true, Ordering::Relaxed);
    }
    for worker in workers {
        let _ = worker.join();
    }
    stopping.store(true, Ordering::Relaxed);
    let _ = monitor.join();
    result
}

fn lifecycle_request(
    line: &[u8],
    owner: &str,
    output: &Arc<Output>,
    operations: &Operations,
) -> AppResult<bool> {
    let Ok(Value::Object(object)) = serde_json::from_slice::<Value>(line) else {
        return Ok(false);
    };
    let kind = text(&object, "type");
    if kind != "drain" {
        if operations.draining() && matches!(kind.as_str(), "request" | "subscribe") {
            emit(
                output,
                &error_frame(
                    &object,
                    "native authority is draining; retry after it resumes",
                ),
            )?;
            return Ok(true);
        }
        return Ok(false);
    }
    require_version(&object)?;
    if operations.authority_lost() {
        emit(
            output,
            &error_frame(
                &object,
                "authority-lost: storage identity changed during drain",
            ),
        )?;
        return Ok(true);
    }
    let action = text(&object, "action");
    let ok = match action.as_str() {
        "status" => true,
        "quiesce" => {
            let timeout = object
                .get("timeout_ms")
                .and_then(Value::as_u64)
                .filter(|value| (1..=300000).contains(value));
            match timeout {
                Some(timeout) => operations.quiesce(owner, Duration::from_millis(timeout)),
                None => false,
            }
        }
        "resume" => operations.resume(owner),
        "exit" => operations.commit_exit(owner),
        _ => false,
    };
    emit(
        output,
        &json!({"v": VERSION, "type": "drain", "action": action, "ok": ok,
        "payload": operations.drain_status()}),
    )?;
    Ok(true)
}

pub(super) fn operation_request(
    object: &Map<String, Value>,
    output: &Arc<Output>,
    operations: &Operations,
) -> AppResult<()> {
    let id = text(object, "op");
    let action = text(object, "action");
    let result = match action.as_str() {
        "get" | "fetch" => operations.get(&id),
        "list" => Ok(operations.list()),
        "subscribe" => operations.subscribe(&id, output),
        _ => Err(AppError::invalid(
            "operation action must be get, fetch, subscribe or list",
        )),
    };
    match result {
        Ok(payload) => {
            let complete = payload["complete"] == true;
            let frame = json!({"v": VERSION, "type": "operation", "id": object.get("id"), "generation": object.get("generation"), "op": id, "ok": true, "payload": payload});
            if serde_json::to_vec(&frame)?.len() > MAX_RESPONSE_BYTES {
                return emit(
                    output,
                    &error_frame(
                        object,
                        "operation result exceeds the protocol limit; result retained",
                    ),
                );
            }
            emit(output, &frame)?;
            if action == "fetch" && complete {
                operations.fetched(&id);
            }
            Ok(())
        }
        Err(error) => emit(output, &error_frame(object, &error.to_string())),
    }
}
