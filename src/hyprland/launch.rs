use super::*;
use std::ffi::OsString;

pub fn launch_path(options: &LaunchOptions) -> Value {
    if options.mode == "url" {
        return launch_url(&options.path);
    }
    let path = match parse_path(&options.path) {
        Ok(path) => path,
        Err(error) => return path_error(&options.path, &error),
    };
    let resolved = path_text(&path);
    let outcome = || -> AppResult<Value> {
        let mut command = launch_command(&path, &options.mode, &options.desktop_id, options.line)?
            .into_iter()
            .map(OsString::from)
            .collect::<Vec<_>>();
        if options.mode == "editor" {
            *command.last_mut().unwrap() = path.as_os_str().to_owned();
        }
        if options.mode == "default" && path.is_dir() {
            spawn_launcher(&command)?;
            return Ok(json!({
                "ok": true,
                "operation": "launch",
                "path": resolved,
                "mode": options.mode,
                "placed": false,
                "address": "",
                "fileblade": true,
            }));
        }
        let active_workspace = hypr_query("activeworkspace")?;
        let workspace_id = field_i64(&active_workspace, "id");
        let known: HashSet<String> = clients()?
            .iter()
            .map(|client| field_str(client, "address"))
            .collect();
        let mut launcher = spawn_launcher(&command)?;
        let launcher_pid = launcher.id();
        let candidate = wait_for_launched_client(
            &mut launcher,
            launcher_pid,
            &known,
            workspace_id,
            options.timeout,
        )?;
        let Some(candidate) = candidate else {
            return Ok(json!({
                "ok": true,
                "operation": "launch",
                "path": resolved,
                "mode": options.mode,
                "placed": false,
                "address": "",
                "placement_error": "The application reused an existing window or did not map a correlated window in time",
            }));
        };
        let address = field_str(&candidate, "address");
        let placed = place_window(&address, workspace_id, "left")?;
        let mut result = Map::new();
        result.insert("ok".to_string(), Value::Bool(true));
        result.insert("operation".to_string(), Value::String("launch".to_string()));
        result.insert("path".to_string(), Value::String(resolved.clone()));
        result.insert("mode".to_string(), Value::String(options.mode.clone()));
        result.insert("placed".to_string(), Value::Bool(true));
        if let Value::Object(fields) = placed {
            result.extend(fields);
        }
        Ok(Value::Object(result))
    };
    outcome().unwrap_or_else(|error| {
        json!({
            "ok": false,
            "operation": "launch",
            "path": resolved,
            "mode": options.mode,
            "placed": false,
            "error": error.to_string(),
        })
    })
}

const URL_LIMIT: usize = 2048;

fn web_url(value: &str) -> AppResult<url::Url> {
    if value.is_empty() || value.len() > URL_LIMIT {
        return Err(AppError::invalid(format!(
            "a link must be 1 to {URL_LIMIT} bytes"
        )));
    }
    if value.chars().any(|c| c.is_control() || c.is_whitespace()) {
        return Err(AppError::invalid(
            "a link cannot contain spaces or control characters",
        ));
    }
    let parsed = url::Url::parse(value).map_err(|_| AppError::invalid("not a valid link"))?;
    if !matches!(parsed.scheme(), "http" | "https") || parsed.host_str().is_none_or(str::is_empty) {
        return Err(AppError::invalid(
            "only http and https links open from FileBlade",
        ));
    }
    Ok(parsed)
}

fn launch_url(value: &str) -> Value {
    let outcome = || -> AppResult<String> {
        let parsed = web_url(value)?;
        let program = which("gio").ok_or_else(|| AppError::command("gio is not installed"))?;
        CommandSpec::new(program)
            .args(["open", "--", parsed.as_str()])
            .spawn_detached()?;
        Ok(parsed.to_string())
    };
    match outcome() {
        Ok(opened) => json!({
            "ok": true,
            "operation": "launch",
            "path": opened,
            "mode": "url",
            "placed": false,
            "address": "",
        }),
        Err(error) => json!({
            "ok": false,
            "operation": "launch",
            "path": "",
            "mode": "url",
            "placed": false,
            "error": error.to_string(),
        }),
    }
}

pub fn launch_command(
    path: &Path,
    mode: &str,
    desktop_id: &str,
    line: u64,
) -> AppResult<Vec<String>> {
    if !path.exists() {
        return Err(AppError::invalid(format!(
            "{} does not exist",
            path.display()
        )));
    }
    let resolved = path_text(path);
    if mode == "application" {
        validate_desktop_id(desktop_id)?;
        return Ok(vec![
            "gtk-launch".to_string(),
            desktop_id.to_string(),
            url::Url::from_file_path(path)
                .map_err(|_| AppError::invalid("could not encode file URI"))?
                .to_string(),
        ]);
    }
    if mode == "editor" {
        let mut command = vec!["omarchy-launch-editor".to_string()];
        if line > 0 {
            command.push(format!("+{line}"));
        }
        command.push(resolved);
        return Ok(command);
    }
    let command = if mode == "reveal" {
        vec![
            "nautilus".to_string(),
            "--new-window".to_string(),
            "--select".to_string(),
            "--".to_string(),
            resolved,
        ]
    } else if path.is_dir() {
        vec![
            path_text(&own_binary().map_err(|error| {
                AppError::command(format!("could not locate fileblade: {error}"))
            })?),
            "navigate".to_string(),
            resolved,
        ]
    } else {
        vec![
            "gio".to_string(),
            "open".to_string(),
            "--".to_string(),
            resolved,
        ]
    };
    if command.first().is_some_and(|value| value == "nautilus") && which("uwsm-app").is_some() {
        let mut wrapped = vec!["uwsm-app".to_string(), "--".to_string()];
        wrapped.extend(command);
        Ok(wrapped)
    } else {
        Ok(command)
    }
}

pub fn validate_desktop_id(value: &str) -> AppResult<()> {
    if crate::desktop::valid_desktop_id(value) {
        Ok(())
    } else {
        Err(AppError::invalid("Invalid desktop application id"))
    }
}

pub(super) fn spawn_launcher(command: &[OsString]) -> AppResult<Child> {
    let program = command
        .first()
        .ok_or_else(|| AppError::invalid("empty launch command"))?;
    let binary = if let Some(uri) = program.to_str().filter(|text| text.starts_with("file://")) {
        parse_path(uri)?
    } else {
        program
            .to_str()
            .and_then(which)
            .unwrap_or_else(|| PathBuf::from(program))
    };
    let mut process = Command::new(binary);
    process
        .args(&command[1..])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::piped())
        .process_group(0);
    process.spawn().map_err(|error| {
        AppError::command(format!(
            "could not launch {}: {error}",
            program.to_string_lossy()
        ))
    })
}

pub(super) fn wait_for_launched_client(
    launcher: &mut Child,
    launcher_pid: u32,
    known_addresses: &HashSet<String>,
    workspace_id: i64,
    timeout: Duration,
) -> AppResult<Option<Value>> {
    let deadline =
        Instant::now() + timeout.clamp(Duration::from_millis(250), Duration::from_secs(20));
    while Instant::now() < deadline {
        let processes = process_parents();
        let mut candidates = clients()?
            .into_iter()
            .filter(|client| {
                let address = field_str(client, "address");
                let pid = field_i64(client, "pid").max(0) as u32;
                !known_addresses.contains(&address)
                    && field_bool(client, "mapped", true)
                    && process_descends_from(pid, launcher_pid, &processes)
            })
            .collect::<Vec<_>>();
        candidates.sort_by_key(|client| {
            let off_workspace = client_workspace_id(client) != workspace_id;
            (off_workspace, field_i64(client, "focusHistoryID"))
        });
        if let Some(candidate) = candidates.into_iter().next() {
            return Ok(Some(candidate));
        }
        if let Some(status) = launcher.try_wait()?
            && !status.success()
        {
            let mut error = String::new();
            if let Some(stderr) = launcher.stderr.as_mut() {
                let _ = stderr.take(64 * 1024).read_to_string(&mut error);
            }
            return Err(AppError::command(if error.trim().is_empty() {
                format!("Launcher exited with {status}")
            } else {
                error.trim().to_string()
            }));
        }
        thread::sleep(Duration::from_millis(50));
    }
    Ok(None)
}

pub(super) fn process_parents() -> HashMap<u32, u32> {
    let mut result = HashMap::new();
    let Ok(entries) = fs::read_dir("/proc") else {
        return result;
    };
    for entry in entries.flatten().take(65_536) {
        let Some(pid) = entry
            .file_name()
            .to_str()
            .and_then(|value| value.parse::<u32>().ok())
        else {
            continue;
        };
        let Ok(stat) = fs::read_to_string(entry.path().join("stat")) else {
            continue;
        };
        let Some(end) = stat.rfind(')') else {
            continue;
        };
        let Some(parent) = stat[end + 1..]
            .split_whitespace()
            .nth(1)
            .and_then(|value| value.parse::<u32>().ok())
        else {
            continue;
        };
        result.insert(pid, parent);
    }
    result
}

pub(super) fn process_descends_from(pid: u32, ancestor: u32, parents: &HashMap<u32, u32>) -> bool {
    let mut current = pid;
    for _ in 0..64 {
        if current == ancestor {
            return true;
        }
        let Some(parent) = parents.get(&current).copied() else {
            return false;
        };
        if parent == current || parent == 0 {
            return false;
        }
        current = parent;
    }
    false
}
