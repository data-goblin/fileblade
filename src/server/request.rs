use super::*;

pub(super) fn start_request(
    object: &Map<String, Value>,
    max_concurrency: usize,
    output: &Arc<Output>,
    active: &Arc<Mutex<HashMap<RequestKey, ActiveRequest>>>,
    seen: &mut RecentKeys,
    workers: &mut Vec<JoinHandle<()>>,
    operations: Option<&Arc<Operations>>,
) -> AppResult<()> {
    let request = match parse_request(object) {
        Ok(request) => request,
        Err(error) => return emit(output, &error_frame(object, &error.to_string())),
    };
    if seen.contains(&request.key) {
        return emit(
            output,
            &error_frame(object, "request id and generation must be unique"),
        );
    }
    let cancelled = Arc::new(AtomicBool::new(false));
    if operations.is_some_and(|operations| operations.authority_lost()) {
        let mut frame = error_frame(object, "authority-lost: storage identity changed");
        frame["error_id"] = Value::String("authority-lost".into());
        return emit(output, &frame);
    }
    let deadline_exceeded = Arc::new(AtomicBool::new(false));
    let mutating = if operations.is_some() {
        native_mutating(&request.command)
    } else {
        backend::mutating(&request.command)
    };
    if mutating
        && let Some(operations) = operations
        && let crate::lease::WriteMode::ReadOnly { reason } = operations.write_mode()
    {
        let mut frame = error_frame(object, &reason);
        frame["error_id"] = Value::String("migration-refused".into());
        return emit(output, &frame);
    }
    let chooser_watch = matches!(
        &request.command,
        backend::BackendCommand::Chooser(crate::chooser::transport::ChooserArgs {
            command: crate::chooser::transport::ChooserCommand::Watch { .. }
        })
    );
    let mut operation = None;
    {
        let mut requests = lock(active);
        if requests.contains_key(&request.key) {
            return emit(
                output,
                &error_frame(object, "request id and generation must be unique"),
            );
        }
        if chooser_watch && requests.values().any(|request| request.chooser_watch) {
            return emit(output, &error_frame(object, "chooser watch limit reached"));
        }
        if live_requests(&requests) >= max_concurrency {
            return emit(
                output,
                &error_frame(object, "server concurrency limit reached"),
            );
        }
        if mutating && let Some(operations) = operations {
            operation = match operations.admit(Arc::clone(&cancelled), output) {
                Ok(operation) => Some(operation),
                Err(error) => return emit(output, &error_frame(object, &error.to_string())),
            };
        }
        requests.insert(
            request.key.clone(),
            ActiveRequest {
                cancelled: Arc::clone(&cancelled),
                deadline: Some(Arc::clone(&request.deadline)),
                deadline_exceeded: Arc::clone(&deadline_exceeded),
                cancel_on_deadline: !mutating,
                standing: matches!(
                    &request.command,
                    backend::BackendCommand::Chooser(crate::chooser::transport::ChooserArgs {
                        command: crate::chooser::transport::ChooserCommand::Offer { .. }
                            | crate::chooser::transport::ChooserCommand::Watch { .. }
                    })
                ),
                chooser_watch,
                authority_owned: operation.is_some(),
            },
        );
    }
    seen.remember(request.key.clone());
    if let Some(operation) = &operation {
        let _ = emit(
            output,
            &json!({"v": VERSION, "type": "accepted", "id": request.key.id, "generation": request.generation, "op": operation.id, "ok": true}),
        );
    }
    let output = Arc::clone(output);
    let active = Arc::clone(active);
    let active_key = request.key.clone();
    workers.push(thread::spawn(move || {
        let mut frame = execute(
            request,
            cancelled,
            deadline_exceeded,
            &output,
            operation.as_deref(),
        );
        if let Some(operation) = operation {
            frame["op"] = Value::String(operation.id.clone());
            let deliver = |target: &Output, terminal: &Value| {
                let mut requests = lock(&active);
                let result = target.machine(terminal);
                requests.remove(&active_key);
                result
            };
            if !operation.publish(frame, true, Some(&deliver)) {
                lock(&active).remove(&active_key);
            }
        } else if matches!(output.as_ref(), Output::Stdio(_)) {
            let _ = emit(&output, &frame);
            lock(&active).remove(&active_key);
        } else {
            let mut requests = lock(&active);
            let _ = emit(&output, &frame);
            requests.remove(&active_key);
        }
    }));
    Ok(())
}

pub(super) fn execute(
    request: Request,
    cancelled: Arc<AtomicBool>,
    deadline_exceeded: Arc<AtomicBool>,
    output: &Output,
    operation: Option<&Operation>,
) -> Value {
    let key = request.key.clone();
    let generation = request.generation.clone();
    let deadline = Arc::clone(&request.deadline);
    let mut progress = |payload: Value| {
        deadline.renew();
        let mut frame = json!({
            "v": VERSION,
            "type": "progress",
            "id": key.id,
            "generation": generation,
            "payload": payload,
        });
        if let Some(operation) = operation {
            frame["op"] = Value::String(operation.id.clone());
            operation.publish(frame, false, None);
            Ok(())
        } else {
            emit(output, &frame)
        }
    };
    let started = Instant::now();
    let result = backend::dispatch(request.command, &cancelled, &mut progress);
    let _ = crate::audit::record(&crate::audit::Event {
        via: "serve",
        actor: &request.actor,
        command: &request.name,
        arguments: &request.arguments,
        outcome: &result,
        started,
    });
    if deadline.expired(Instant::now()) {
        deadline_exceeded.store(true, Ordering::Relaxed);
    }
    let late = deadline_exceeded.load(Ordering::Relaxed);
    let was_cancelled = cancelled.load(Ordering::Relaxed);
    match result {
        Ok(mut payload) => {
            if let Value::Object(object) = &mut payload {
                if late {
                    object.insert("late".to_string(), Value::Bool(true));
                }
                if was_cancelled && object.get("ok") != Some(&Value::Bool(true)) {
                    object.insert("cancelled".to_string(), Value::Bool(true));
                }
            }
            json!({
                "v": VERSION,
                "type": "response",
                "id": request.key.id,
                "generation": request.generation,
                "ok": true,
                "late": late,
                "payload": payload,
            })
        }
        Err(AppError::Cancelled) if late => json!({
            "v": VERSION,
            "type": "response",
            "id": request.key.id,
            "generation": request.generation,
            "ok": false,
            "deadline_exceeded": true,
            "error": "request deadline exceeded",
        }),
        Err(AppError::Cancelled) => json!({
            "v": VERSION,
            "type": "response",
            "id": request.key.id,
            "generation": request.generation,
            "ok": false,
            "cancelled": true,
            "error": "request cancelled",
        }),
        Err(error) => json!({
            "v": VERSION,
            "type": "response",
            "id": request.key.id,
            "generation": request.generation,
            "ok": false,
            "late": late,
            "error": error.to_string(),
        }),
    }
}

pub(super) fn cancel_request(
    object: &Map<String, Value>,
    output: &Output,
    active: &Arc<Mutex<HashMap<RequestKey, ActiveRequest>>>,
    operations: Option<&Arc<Operations>>,
) -> AppResult<()> {
    if let Some(id) = object.get("op").and_then(Value::as_str)
        && let Some(operations) = operations
    {
        return emit(
            output,
            &json!({"v": VERSION, "type": "cancel", "op": id, "ok": true, "accepted": operations.cancel(id)}),
        );
    }
    let key = match request_key(object) {
        Ok(value) => value,
        Err(error) => return emit(output, &error_frame(object, &error.to_string())),
    };
    let accepted = lock(active)
        .get(&key)
        .filter(|request| !request.authority_owned)
        .map(|request| {
            request.cancelled.store(true, Ordering::Relaxed);
            true
        })
        .unwrap_or(false);
    emit(
        output,
        &json!({
            "v": VERSION,
            "type": "cancel",
            "id": key.id,
            "generation": object.get("generation").cloned().unwrap_or(Value::Null),
            "ok": true,
            "accepted": accepted,
        }),
    )
}

pub fn native_mutating(command: &backend::BackendCommand) -> bool {
    backend::mutating(command)
        || matches!(
            command,
            backend::BackendCommand::Recover
                | backend::BackendCommand::StateWrite(_)
                | backend::BackendCommand::LayoutWrite(_)
                | backend::BackendCommand::FrecencyVisit(_)
                | backend::BackendCommand::ClipboardWrite(_)
                | backend::BackendCommand::ClipboardText(_)
                | backend::BackendCommand::ClipboardPlain(_)
                | backend::BackendCommand::ModuleDirs(_)
                | backend::BackendCommand::Visit(_)
                | backend::BackendCommand::SetDefault(_)
                | backend::BackendCommand::DimWindows(_)
                | backend::BackendCommand::DropRun(_)
                | backend::BackendCommand::DropPaste(_)
        )
}

pub(super) fn parse_request(object: &Map<String, Value>) -> AppResult<Request> {
    let key = request_key(object)?;
    let generation = object.get("generation").cloned().unwrap_or(Value::Null);
    let command_name = ["command", "method"]
        .into_iter()
        .find_map(|key| object.get(key).and_then(Value::as_str))
        .filter(|value| !value.is_empty())
        .ok_or_else(|| AppError::invalid("request command is required"))?;
    let arguments_value = object.get("arguments").or_else(|| object.get("args"));
    let arguments = match arguments_value {
        None => Vec::new(),
        Some(Value::Array(values)) if values.len() <= MAX_ARGUMENTS => values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(ToOwned::to_owned)
                    .ok_or_else(|| AppError::invalid("request arguments must be strings"))
            })
            .collect::<AppResult<Vec<_>>>()?,
        Some(Value::Array(_)) => return Err(AppError::invalid("request has too many arguments")),
        Some(_) => return Err(AppError::invalid("request arguments must be an array")),
    };
    let mut cli = backend::parse_cli(
        ["fileblade _backend".to_string(), command_name.to_string()]
            .into_iter()
            .chain(arguments.iter().cloned()),
    )
    .map_err(|error| AppError::invalid(error.to_string().trim().to_string()))?;
    if let Some(input) = object.get("input") {
        let input = input
            .as_str()
            .filter(|value| value.len() <= crate::module_helpers::INPUT_LIMIT)
            .ok_or_else(|| AppError::invalid("request input must be a string of at most 64 KiB"))?;
        match &mut cli.command {
            backend::BackendCommand::HelperRead(options)
            | backend::BackendCommand::HelperWrite(options) => {
                options.input = Some(input.to_string());
            }
            backend::BackendCommand::ClipboardPlain(options) => {
                options.input = Some(input.to_string());
            }
            _ => {
                return Err(AppError::invalid(
                    "this request does not accept private input",
                ));
            }
        }
    }
    Ok(Request {
        key,
        generation,
        name: command_name.to_string(),
        arguments,
        actor: cli.actor,
        command: cli.command,
        deadline: request_deadline(object, DEFAULT_DEADLINE_MS)?,
    })
}

pub(super) fn request_deadline(
    object: &Map<String, Value>,
    default_ms: u64,
) -> AppResult<Arc<Deadline>> {
    let deadline_ms = object
        .get("deadline_ms")
        .or_else(|| object.get("timeout_ms"))
        .map(|value| {
            value
                .as_u64()
                .ok_or_else(|| AppError::invalid("deadline_ms must be an integer"))
        })
        .transpose()?
        .unwrap_or(default_ms);
    if deadline_ms == 0 || deadline_ms > MAX_DEADLINE_MS {
        return Err(AppError::invalid(format!(
            "deadline_ms must be between 1 and {MAX_DEADLINE_MS}"
        )));
    }
    Ok(Arc::new(Deadline::new(Duration::from_millis(deadline_ms))))
}

pub(super) fn live_requests(requests: &HashMap<RequestKey, ActiveRequest>) -> usize {
    requests
        .values()
        .filter(|request| !request.standing && !request.deadline_exceeded.load(Ordering::Relaxed))
        .count()
}

pub(super) fn optional_deadline(object: &Map<String, Value>) -> AppResult<Option<Arc<Deadline>>> {
    if object.contains_key("deadline_ms") || object.contains_key("timeout_ms") {
        request_deadline(object, DEFAULT_DEADLINE_MS).map(Some)
    } else {
        Ok(None)
    }
}

pub(super) fn request_key(object: &Map<String, Value>) -> AppResult<RequestKey> {
    let id = object
        .get("id")
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= MAX_IDENTIFIER_BYTES)
        .ok_or_else(|| {
            AppError::invalid("request id must be a non-empty string of at most 128 bytes")
        })?
        .to_string();
    let generation_value = object
        .get("generation")
        .filter(|value| value.is_string() || value.is_number())
        .ok_or_else(|| AppError::invalid("request generation must be a string or number"))?;
    let generation = match generation_value {
        Value::String(value) if !value.is_empty() && value.len() <= MAX_IDENTIFIER_BYTES => {
            value.clone()
        }
        Value::Number(value) => value.to_string(),
        _ => return Err(AppError::invalid("request generation is invalid")),
    };
    Ok(RequestKey { id, generation })
}

pub(super) fn require_version(object: &Map<String, Value>) -> AppResult<()> {
    if object.get("v").and_then(Value::as_u64) == Some(VERSION) {
        Ok(())
    } else {
        Err(AppError::invalid(format!(
            "unsupported protocol version; expected {VERSION}"
        )))
    }
}

pub(super) fn error_frame(object: &Map<String, Value>, message: &str) -> Value {
    json!({
        "v": VERSION,
        "type": "error",
        "id": object.get("id").cloned().unwrap_or(Value::Null),
        "generation": object.get("generation").cloned().unwrap_or(Value::Null),
        "ok": false,
        "error": message,
    })
}

pub(super) fn text(object: &Map<String, Value>, key: &str) -> String {
    object
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}
