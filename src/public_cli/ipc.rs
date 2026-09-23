use super::*;

#[derive(Debug)]
pub struct PublicResult {
    pub(super) documents: Vec<Value>,
    pub(super) error: Option<String>,
    pub(super) document: Option<Value>,
}

impl PublicResult {
    pub(super) fn one(value: impl Into<Value>) -> Self {
        Self {
            documents: vec![value.into()],
            error: None,
            document: None,
        }
    }

    pub(super) fn many(documents: Vec<Value>) -> Self {
        Self {
            documents,
            error: None,
            document: None,
        }
    }

    pub(super) fn lines(lines: Vec<String>, document: Value) -> Self {
        Self {
            documents: lines.into_iter().map(Value::String).collect(),
            error: None,
            document: Some(document),
        }
    }

    pub(super) fn checked(value: Value, fallback: &str) -> Self {
        let error = (!value.get("ok").and_then(Value::as_bool).unwrap_or(false))
            .then(|| error_text(&value, fallback));
        Self {
            documents: vec![value],
            error,
            document: None,
        }
    }

    pub fn emit(self, output: &Output) -> AppResult<()> {
        if !output.quiet() {
            match output.format() {
                Format::Text => {
                    for document in &self.documents {
                        output.value(document)?;
                    }
                }
                Format::Json => {
                    let value = match (&self.document, self.documents.as_slice()) {
                        (Some(document), _) => document.clone(),
                        (None, []) => Value::Null,
                        (None, [single]) => single.clone(),
                        (None, [first, second]) => json!({"response": first, "result": second}),
                        (None, values) => json!({"responses": values}),
                    };
                    output.json(&value)?;
                }
            }
        }
        self.error
            .map_or(Ok(()), |error| Err(AppError::command(error)))
    }
}

pub(super) fn ipc(method: &str, arguments: &[String]) -> AppResult<String> {
    let target = ipc_target(method);
    let response = ipc_on(target, method, arguments)?;
    if target == CONTROL_TARGET
        && (response.starts_with("invalid-")
            || matches!(response.as_str(), "no-screen" | "unknown-monitor"))
    {
        return Err(AppError::command(format!("{method}: {response}")));
    }
    Ok(response)
}

pub(super) fn ipc_target(method: &str) -> &'static str {
    match method {
        "status" | "selection" | "favorites" | "operationResult" | "tree" | "searchResults"
        | "folderColor" | "history" | "blades" | "bladeModules" | "pickerResult" | "actions"
        | "actionResult" => READ_TARGET,
        _ => CONTROL_TARGET,
    }
}

fn native_config() -> Option<PathBuf> {
    std::env::var_os("FILEBLADE_APP_ROOT")
        .map(PathBuf::from)
        .filter(|root| root.is_absolute())
        .map(|root| root.join("app"))
}

fn shell_config() -> Option<PathBuf> {
    std::env::var_os("OMARCHY_PATH")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .map(|root| root.join("shell"))
}

fn ipc_configs() -> AppResult<Vec<PathBuf>> {
    let native = crate::lease::selected_root()?.is_some();
    let mut candidates = Vec::new();
    if native {
        candidates.push(native_config().ok_or_else(|| {
            AppError::command("native IPC requires an absolute FILEBLADE_APP_ROOT")
        })?);
    }
    if let Some(shell) = shell_config()
        && !candidates.contains(&shell)
    {
        candidates.push(shell);
    }
    if candidates.is_empty() {
        return Err(AppError::command("OMARCHY_PATH is not set"));
    }
    let present: Vec<PathBuf> = candidates
        .iter()
        .filter(|config| config.join("shell.qml").is_file())
        .cloned()
        .collect();
    if present.is_empty() {
        return Err(AppError::command(format!(
            "no FileBlade shell config found: {}",
            candidates
                .iter()
                .map(|config| config.join("shell.qml").display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )));
    }
    Ok(present)
}

pub(super) fn ipc_on(target: &str, method: &str, arguments: &[String]) -> AppResult<String> {
    let configs = ipc_configs()?;
    let program = which("qs").or_else(|| which("quickshell")).ok_or_else(|| {
        AppError::command("quickshell is not installed; install it to use the CLI")
    })?;
    let last = configs.len() - 1;
    for (position, config) in configs.iter().enumerate() {
        match call_config(
            &program,
            config,
            target,
            method,
            arguments,
            position == last,
        )? {
            Some(response) => return Ok(response),
            None => continue,
        }
    }
    Err(AppError::command(format!(
        "FileBlade is not running; no instance answered on {}",
        configs
            .iter()
            .map(|config| config.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    )))
}

fn call_config(
    program: &std::path::Path,
    config: &std::path::Path,
    target: &str,
    method: &str,
    arguments: &[String],
    final_candidate: bool,
) -> AppResult<Option<String>> {
    let mut command = CommandSpec::new(program.to_path_buf())
        .args(["ipc", "-n", "-p"])
        .args([config.to_path_buf()])
        .args(["call", "--", target, method])
        .args(arguments.iter().cloned())
        .timeout(IPC_TIMEOUT)
        .limits(MAX_IPC_STDOUT, MAX_IPC_STDERR);
    if std::env::var_os("WAYLAND_DISPLAY").is_none_or(|value| value.is_empty()) {
        let candidates = wayland_display_candidates();
        match candidates.as_slice() {
            [only] => command = command.env("WAYLAND_DISPLAY", only.clone()),
            [] => {}
            several => {
                return Err(AppError::command(format!(
                    "WAYLAND_DISPLAY is not set and several sessions exist ({}); export the one to drive",
                    several.join(", ")
                )));
            }
        }
    }
    let output = command.run().map_err(|error| {
        let message = error.to_string();
        if message.contains("did not respond within") {
            AppError::command(format!(
                "file-tree IPC method {method} did not respond within {}s",
                IPC_TIMEOUT.as_secs()
            ))
        } else {
            AppError::command(format!(
                "FileBlade IPC failed for {method}: {message} (is FileBlade running?)"
            ))
        }
    })?;
    if output.stdout_truncated || output.stderr_truncated {
        return Err(AppError::command(format!(
            "file-tree IPC method {method} exceeded its output limit"
        )));
    }
    if !output.status.success() {
        return Ok(None);
    }
    let response = String::from_utf8(output.stdout)
        .map(|value| value.trim().to_string())
        .map_err(|error| {
            AppError::command(format!(
                "file-tree IPC method {method} returned non-UTF-8 output: {error}"
            ))
        })?;
    if response.starts_with("No running instances")
        || (response == "Target not found." && !final_candidate)
    {
        return Ok(None);
    }
    if matches!(
        response.as_str(),
        "Target not found." | "Function not found."
    ) || response.starts_with("Too few arguments provided")
        || response.starts_with("Too many arguments provided")
    {
        return Err(AppError::command(response));
    }
    if response.starts_with("Not ready to accept queries yet") {
        return Err(AppError::command("FileBlade is not ready"));
    }
    Ok(Some(response))
}

pub(super) fn wayland_display_candidates() -> Vec<String> {
    let directory = std::env::var_os("XDG_RUNTIME_DIR")
        .filter(|value| !value.is_empty())
        .map(PathBuf::from)
        .filter(|path| path.is_absolute())
        .unwrap_or_else(|| {
            PathBuf::from(format!("/run/user/{}", rustix::process::getuid().as_raw()))
        });
    let mut candidates = Vec::new();
    let Ok(entries) = std::fs::read_dir(directory) else {
        return candidates;
    };
    for entry in entries.flatten().take(4096) {
        let Some(name) = entry.file_name().to_str().map(ToOwned::to_owned) else {
            continue;
        };
        let Some(suffix) = name.strip_prefix("wayland-") else {
            continue;
        };
        if suffix.is_empty() || !suffix.bytes().all(|byte| byte.is_ascii_digit()) {
            continue;
        }
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if !file_type.is_socket() {
            continue;
        }
        candidates.push(name);
    }
    candidates.sort();
    candidates
}

pub(super) fn simple_ipc(method: &str, arguments: &[String]) -> AppResult<PublicResult> {
    Ok(PublicResult::one(ipc(method, arguments)?))
}

pub(super) fn json_ipc(method: &str, arguments: &[String]) -> AppResult<PublicResult> {
    Ok(PublicResult::one(response_value(&ipc(method, arguments)?)))
}

pub(super) fn response_value(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|_| Value::String(text.to_string()))
}

pub(super) fn object_response(method: &str, arguments: &[String]) -> AppResult<Map<String, Value>> {
    let response = ipc(method, arguments)?;
    serde_json::from_str::<Value>(&response)?
        .as_object()
        .cloned()
        .ok_or_else(|| {
            AppError::command(format!(
                "file-tree IPC method {method} returned an unexpected document"
            ))
        })
}

pub(super) fn backend_json(arguments: &[String]) -> AppResult<Value> {
    backend_json_with_timeout(arguments, BACKEND_TIMEOUT)
}

pub(super) fn backend_json_with_timeout(
    arguments: &[String],
    timeout: Duration,
) -> AppResult<Value> {
    let command = backend::parse(
        ["fileblade _backend".to_string()]
            .into_iter()
            .chain(arguments.iter().cloned()),
    )
    .map_err(|error| AppError::invalid(error.to_string().trim().to_string()))?;
    if crate::lease::selected_root()?.is_some() {
        let mut document = Value::Null;
        let mut bytes = 0usize;
        crate::native::backend_request(arguments, timeout, &mut |value| {
            let size = bounded_json_size(value, MAX_IPC_STDOUT.saturating_sub(bytes))
                .ok_or_else(|| AppError::command("filesystem backend exceeded its output limit"))?;
            bytes += size;
            document = value.clone();
            Ok(())
        })?;
        if !document.is_object() {
            return Err(AppError::command(
                "filesystem backend returned an unexpected document",
            ));
        }
        return Ok(document);
    }
    let cancelled = Arc::new(AtomicBool::new(false));
    let expired = Arc::new(AtomicBool::new(false));
    let started = Instant::now();
    let deadline = started + timeout;
    let (finished, completion) = mpsc::channel();
    let monitor_cancelled = Arc::clone(&cancelled);
    let monitor_expired = Arc::clone(&expired);
    let monitor = thread::spawn(move || {
        if completion.recv_timeout(timeout).is_err() {
            monitor_expired.store(true, Ordering::Relaxed);
            monitor_cancelled.store(true, Ordering::Relaxed);
        }
    });
    let mut progress_bytes = 0usize;
    let mut progress = |value: Value| {
        let remaining = MAX_IPC_STDOUT.saturating_sub(progress_bytes);
        let size = remaining
            .checked_sub(1)
            .and_then(|limit| bounded_json_size(&value, limit));
        let Some(size) = size else {
            cancelled.store(true, Ordering::Relaxed);
            return Err(AppError::command(
                "filesystem backend exceeded its progress output limit",
            ));
        };
        progress_bytes += size + 1;
        Ok(())
    };
    let result = backend::dispatch(command, cancelled.as_ref(), &mut progress);
    let _ = crate::audit::record(&crate::audit::Event {
        via: "cli",
        actor: "cli",
        command: arguments.first().map_or("", String::as_str),
        arguments: arguments.get(1..).unwrap_or_default(),
        outcome: &result,
        started,
    });
    let completed_at = Instant::now();
    let _ = finished.send(());
    let _ = monitor.join();
    if expired.load(Ordering::Relaxed) || completed_at >= deadline {
        return Err(AppError::command(format!(
            "filesystem backend did not respond within {}s",
            timeout.as_secs_f64()
        )));
    }
    let value = result?;
    if bounded_json_size(&value, MAX_IPC_STDOUT).is_none() {
        return Err(AppError::command(
            "filesystem backend exceeded its result output limit",
        ));
    }
    if !value.is_object() {
        return Err(AppError::command(
            "filesystem backend returned an unexpected document",
        ));
    }
    Ok(value)
}

pub(super) struct JsonByteCounter {
    pub(super) bytes: usize,
    pub(super) limit: usize,
}

impl Write for JsonByteCounter {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.len() > self.limit.saturating_sub(self.bytes) {
            return Err(io::Error::other("JSON document exceeds its byte limit"));
        }
        self.bytes += buffer.len();
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

pub(super) fn bounded_json_size(value: &Value, limit: usize) -> Option<usize> {
    let mut counter = JsonByteCounter { bytes: 0, limit };
    serde_json::to_writer(&mut counter, value).ok()?;
    Some(counter.bytes)
}

pub(super) fn absolute_path(path: &str) -> String {
    parse_path(path)
        .map(|path| path_text(&path))
        .unwrap_or_else(|_| path.to_string())
}

pub(super) fn location_resource(path: &str) -> String {
    if path == "trash:///" || path == "recent:///" || path == "drives:///" {
        path.to_string()
    } else {
        absolute_path(path)
    }
}

pub(super) fn encoded_document(value: &Value) -> AppResult<String> {
    let encoded = serde_json::to_vec(value)?;
    Ok(format!("base64:{}", BASE64_STANDARD.encode(encoded)))
}

pub(super) fn error_text(value: &Value, fallback: &str) -> String {
    value
        .get("error")
        .and_then(Value::as_str)
        .filter(|text| !text.is_empty())
        .unwrap_or(fallback)
        .to_string()
}

pub(super) fn poll(
    timeout_seconds: f64,
    mut check: impl FnMut() -> AppResult<Option<Value>>,
    timeout_message: &str,
) -> AppResult<Value> {
    if !timeout_seconds.is_finite() || timeout_seconds <= 0.0 || timeout_seconds > MAX_POLL_SECONDS
    {
        return Err(AppError::invalid(format!(
            "timeout must be above 0 and at most {MAX_POLL_SECONDS} seconds"
        )));
    }
    let deadline = Instant::now() + Duration::from_secs_f64(timeout_seconds);
    let mut attempt = 0_u64;
    while Instant::now() < deadline {
        if let Some(value) = check()? {
            return Ok(value);
        }
        let bucket = (attempt / 10).min(3);
        let delay = Duration::from_millis((100_u64 << bucket).min(500));
        thread::sleep(delay.min(deadline.saturating_duration_since(Instant::now())));
        attempt += 1;
    }
    Err(AppError::command(format!(
        "{timeout_message} after {timeout_seconds}s"
    )))
}

pub(super) fn text_field(value: &Map<String, Value>, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

pub(super) fn integer_field(value: &Map<String, Value>, key: &str) -> i64 {
    value.get(key).and_then(Value::as_i64).unwrap_or_default()
}

pub(super) fn nonempty(value: &Map<String, Value>, key: &str) -> bool {
    !text_field(value, key).is_empty()
}

pub(super) fn value_text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string()
}

pub(super) fn value_i64(value: &Value, key: &str) -> i64 {
    value.get(key).and_then(Value::as_i64).unwrap_or_default()
}

pub(super) fn nonempty_value(value: &Value, key: &str) -> bool {
    !value_text(value, key).is_empty()
}
