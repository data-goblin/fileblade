use super::*;

pub(super) fn start_subscription(
    object: &Map<String, Value>,
    max_concurrency: usize,
    output: &Arc<Output>,
    active: &Arc<Mutex<HashMap<RequestKey, ActiveRequest>>>,
    seen: &mut RecentKeys,
    workers: &mut Vec<JoinHandle<()>>,
) -> AppResult<()> {
    let key = match request_key(object) {
        Ok(value) => value,
        Err(error) => return emit(output, &error_frame(object, &error.to_string())),
    };
    let topic = text(object, "topic").to_string();
    if topic != "filesystem" && topic != "mounts" {
        return emit(
            output,
            &error_frame(
                object,
                "only the filesystem and mounts subscription topics are supported",
            ),
        );
    }
    let paths = if topic == "mounts" {
        Vec::new()
    } else {
        match watch_paths(object) {
            Ok(value) => value,
            Err(error) => return emit(output, &error_frame(object, &error.to_string())),
        }
    };
    if seen.contains(&key) {
        return emit(
            output,
            &error_frame(object, "request id and generation must be unique"),
        );
    }
    let deadline = match optional_deadline(object) {
        Ok(value) => value,
        Err(error) => return emit(output, &error_frame(object, &error.to_string())),
    };
    let cancelled = Arc::new(AtomicBool::new(false));
    let deadline_exceeded = Arc::new(AtomicBool::new(false));
    {
        let mut requests = lock(active);
        if requests.contains_key(&key) {
            return emit(
                output,
                &error_frame(object, "request id and generation must be unique"),
            );
        }
        if live_requests(&requests) >= max_concurrency {
            return emit(
                output,
                &error_frame(object, "server concurrency limit reached"),
            );
        }
        requests.insert(
            key.clone(),
            ActiveRequest {
                cancelled: Arc::clone(&cancelled),
                deadline,
                deadline_exceeded: Arc::clone(&deadline_exceeded),
                cancel_on_deadline: true,
                standing: true,
                chooser_watch: false,
                authority_owned: false,
            },
        );
    }
    seen.remember(key.clone());
    let generation = object.get("generation").cloned().unwrap_or(Value::Null);
    let include_writes = object.get("includeWrites").and_then(Value::as_bool) == Some(true);
    let output = Arc::clone(output);
    let active = Arc::clone(active);
    workers.push(thread::spawn(move || {
        if topic == "mounts" {
            watch_mounts(&key, generation, &cancelled, &deadline_exceeded, &output);
        } else {
            watch_filesystem(
                &key,
                generation,
                &paths,
                include_writes,
                &cancelled,
                &deadline_exceeded,
                &output,
            );
        }
        lock(&active).remove(&key);
    }));
    Ok(())
}

pub(super) fn watch_paths(object: &Map<String, Value>) -> AppResult<Vec<PathBuf>> {
    let values = object
        .get("paths")
        .and_then(Value::as_array)
        .ok_or_else(|| AppError::invalid("filesystem subscription paths must be an array"))?;
    if values.is_empty() || values.len() > MAX_WATCH_PATHS {
        return Err(AppError::invalid(format!(
            "filesystem subscription requires 1 to {MAX_WATCH_PATHS} paths"
        )));
    }
    let mut paths = Vec::with_capacity(values.len());
    let mut seen = HashSet::new();
    for value in values {
        let raw = value
            .as_str()
            .ok_or_else(|| AppError::invalid("filesystem subscription paths must be strings"))?;
        let path = crate::common::parse_path(raw)?;
        if seen.insert(path.clone()) {
            paths.push(path);
        }
    }
    Ok(paths)
}

pub(super) fn watch_filesystem(
    key: &RequestKey,
    generation: Value,
    paths: &[PathBuf],
    include_writes: bool,
    cancelled: &AtomicBool,
    deadline_exceeded: &AtomicBool,
    output: &Output,
) {
    let result = filesystem_events(key, &generation, paths, include_writes, cancelled, output);
    let frame = closing_frame(
        key,
        generation,
        "filesystem",
        result,
        cancelled,
        deadline_exceeded,
    );
    let _ = emit(output, &frame);
}

pub(super) fn filesystem_events(
    key: &RequestKey,
    generation: &Value,
    paths: &[PathBuf],
    include_writes: bool,
    cancelled: &AtomicBool,
    output: &Output,
) -> AppResult<()> {
    let descriptor =
        inotify::init(CreateFlags::CLOEXEC | CreateFlags::NONBLOCK).map_err(|error| {
            AppError::command(format!("could not start filesystem subscription: {error}"))
        })?;
    let mut flags = WatchFlags::CREATE
        | WatchFlags::DELETE
        | WatchFlags::MOVED_FROM
        | WatchFlags::MOVED_TO
        | WatchFlags::CLOSE_WRITE
        | WatchFlags::ATTRIB
        | WatchFlags::DELETE_SELF
        | WatchFlags::MOVE_SELF
        | WatchFlags::DONT_FOLLOW
        | WatchFlags::EXCL_UNLINK
        | WatchFlags::ONLYDIR;
    if include_writes {
        flags |= WatchFlags::MODIFY;
    }
    let mut watches = HashMap::new();
    let mut skipped = Vec::new();
    for path in paths {
        let target = match std::fs::symlink_metadata(path) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                match std::fs::canonicalize(path) {
                    Ok(resolved) => resolved,
                    Err(error) => {
                        skipped.push(json!({"path": crate::common::path_text(path), "error": error.to_string()}));
                        continue;
                    }
                }
            }
            Ok(_) => path.clone(),
            Err(error) => {
                skipped.push(
                    json!({"path": crate::common::path_text(path), "error": error.to_string()}),
                );
                continue;
            }
        };
        match inotify::add_watch(&descriptor, &target, flags) {
            Ok(watch) => {
                watches.insert(watch, path.clone());
            }
            Err(error) => skipped.push(json!({
                "path": crate::common::path_text(path),
                "error": error.to_string(),
            })),
        }
    }
    if watches.is_empty() {
        return Err(AppError::command(format!(
            "could not watch any of the {} requested paths: {}",
            paths.len(),
            skipped
                .first()
                .and_then(|entry| entry["error"].as_str())
                .unwrap_or("no paths")
        )));
    }
    let watched: Vec<String> = watches
        .values()
        .map(|path| crate::common::path_text(path))
        .collect();
    emit(
        output,
        &json!({
            "v": VERSION,
            "type": "subscribed",
            "id": key.id,
            "generation": generation,
            "ok": true,
            "topic": "filesystem",
            "paths": watched,
            "skipped": skipped,
        }),
    )?;
    let mut storage = [MaybeUninit::<u8>::uninit(); 64 * 1024];
    let mut reader = inotify::Reader::new(&descriptor, &mut storage);
    while !cancelled.load(Ordering::Relaxed) {
        match reader.next() {
            Ok(event) => {
                let flags = event.events();
                let root = watches
                    .get(&event.wd())
                    .map(PathBuf::as_path)
                    .unwrap_or_else(|| Path::new(""));
                let name = event
                    .file_name()
                    .map(|name| std::ffi::OsString::from_vec(name.to_bytes().to_vec()))
                    .unwrap_or_default();
                let path = if name.is_empty() {
                    root.to_path_buf()
                } else {
                    root.join(&name)
                };
                if flags.contains(ReadFlags::QUEUE_OVERFLOW) {
                    crate::listing::invalidate_all();
                } else {
                    crate::listing::invalidate_within(&path);
                }
                emit(
                    output,
                    &json!({
                        "v": VERSION,
                        "type": "event",
                        "id": key.id,
                        "generation": generation,
                        "topic": "filesystem",
                        "path": crate::common::path_text(&path),
                        "root": crate::common::path_text(root),
                        "name": crate::common::display_path(Path::new(&name)),
                        "name_bytes": BASE64_STANDARD.encode(name.as_bytes()),
                        "events": event_names(flags),
                        "cookie": event.cookie(),
                        "directory": flags.contains(ReadFlags::ISDIR),
                        "overflow": flags.contains(ReadFlags::QUEUE_OVERFLOW),
                    }),
                )?;
            }
            Err(Errno::AGAIN) => {
                let mut fds = [PollFd::new(&descriptor, PollFlags::IN)];
                if let Err(error) = poll(&mut fds, Some(&WATCH_POLL_TIMEOUT))
                    && error != Errno::INTR
                {
                    return Err(AppError::command(format!(
                        "filesystem subscription failed: {error}"
                    )));
                }
            }
            Err(error) => {
                return Err(AppError::command(format!(
                    "filesystem subscription failed: {error}"
                )));
            }
        }
    }
    Ok(())
}

pub(super) fn event_names(flags: ReadFlags) -> Vec<&'static str> {
    [
        (ReadFlags::CREATE, "create"),
        (ReadFlags::DELETE, "delete"),
        (ReadFlags::MOVED_FROM, "moved_from"),
        (ReadFlags::MOVED_TO, "moved_to"),
        (ReadFlags::CLOSE_WRITE, "close_write"),
        (ReadFlags::MODIFY, "modify"),
        (ReadFlags::ATTRIB, "attrib"),
        (ReadFlags::DELETE_SELF, "delete_self"),
        (ReadFlags::MOVE_SELF, "move_self"),
        (ReadFlags::UNMOUNT, "unmount"),
        (ReadFlags::IGNORED, "ignored"),
        (ReadFlags::QUEUE_OVERFLOW, "queue_overflow"),
    ]
    .into_iter()
    .filter_map(|(flag, name)| flags.contains(flag).then_some(name))
    .collect()
}

pub(super) fn closing_frame(
    key: &RequestKey,
    generation: Value,
    topic: &str,
    result: AppResult<()>,
    cancelled: &AtomicBool,
    deadline_exceeded: &AtomicBool,
) -> Value {
    if deadline_exceeded.load(Ordering::Relaxed) {
        json!({
            "v": VERSION,
            "type": "response",
            "id": key.id,
            "generation": generation,
            "ok": false,
            "deadline_exceeded": true,
            "error": "subscription deadline exceeded",
        })
    } else if cancelled.load(Ordering::Relaxed) {
        json!({
            "v": VERSION,
            "type": "response",
            "id": key.id,
            "generation": generation,
            "ok": false,
            "cancelled": true,
            "error": "subscription cancelled",
        })
    } else {
        match result {
            Ok(()) => json!({
                "v": VERSION,
                "type": "response",
                "id": key.id,
                "generation": generation,
                "ok": true,
                "payload": {"topic": topic, "closed": true},
            }),
            Err(error) => json!({
                "v": VERSION,
                "type": "response",
                "id": key.id,
                "generation": generation,
                "ok": false,
                "error": error.to_string(),
            }),
        }
    }
}
