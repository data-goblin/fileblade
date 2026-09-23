use super::*;

pub fn drop_context(
    x: Option<i64>,
    y: Option<i64>,
    paths: &[String],
    blade_titles: &[String],
) -> Value {
    let facts = file_facts(paths);
    let point = match resolve_point(x, y) {
        Ok(point) => point,
        Err(error) => {
            return json!({
                "ok": false,
                "error": error.to_string(),
                "at": {"x": x.unwrap_or(0), "y": y.unwrap_or(0)},
                "target": classify_target(None, &[], false),
                "files": facts,
                "actions": [],
            });
        }
    };
    let target_result = target_at(point.0, point.1, blade_titles);
    drop_context_at(point, facts, target_result)
}

fn drop_context_at(point: (i64, i64), facts: Value, target_result: AppResult<Value>) -> Value {
    let (target, target_warning) = match target_result {
        Ok(target) => (target, None),
        Err(error) => (classify_target(None, &[], false), Some(error.to_string())),
    };
    if facts.get("count").and_then(Value::as_u64).unwrap_or(0) == 0 {
        return json!({
            "ok": false,
            "error": "Nothing to drop",
            "at": {"x": point.0, "y": point.1},
            "target": target,
            "files": facts,
            "actions": [],
        });
    }
    let (actions, diagnostics) = config::load(&actions_for(&target, &facts), &target, &facts);
    let mut result = json!({
        "ok": true,
        "at": {"x": point.0, "y": point.1},
        "target": target,
        "files": facts,
        "actions": actions,
        "diagnostics": diagnostics,
    });
    if let Some(warning) = target_warning {
        result["target_warning"] = Value::String(warning);
    }
    result
}

pub(super) fn target_at(x: i64, y: i64, blade_titles: &[String]) -> AppResult<Value> {
    let clients = hypr_query("clients")?
        .as_array()
        .cloned()
        .ok_or_else(|| AppError::command("Hyprland clients response was not a list"))?;
    if !has_window_at_point(&clients, x, y, blade_titles) {
        return Ok(classify_target(None, &[], false));
    }
    let workspace_id = crate::hyprland::workspace_visible_at_point(x, y)?;
    let client = window_under_cursor(&clients, x, y, workspace_id, blade_titles);
    let processes = client
        .as_ref()
        .and_then(|value| value.get("pid"))
        .and_then(Value::as_u64)
        .map(|pid| process_descendants(pid as u32))
        .unwrap_or_default();
    let shared = client
        .as_ref()
        .map(|target| shares_process(&clients, target))
        .unwrap_or(false);
    Ok(classify_target(client.as_ref(), &processes, shared))
}

pub(super) fn shares_process(clients: &[Value], target: &Value) -> bool {
    let pid = target.get("pid").and_then(Value::as_i64).unwrap_or(0);
    let address = text_field(target, "address");
    pid > 0
        && clients.iter().any(|client| {
            client.get("pid").and_then(Value::as_i64) == Some(pid)
                && text_field(client, "address") != address
                && client
                    .get("mapped")
                    .and_then(Value::as_bool)
                    .unwrap_or(true)
        })
}

pub(super) fn classify_target(
    client: Option<&Value>,
    processes: &[ProcessRow],
    shared: bool,
) -> Value {
    let Some(client) = client else {
        return json!({"kind": "desktop", "label": "Desktop", "address": "", "class": "", "title": "", "pid": 0});
    };
    let class = text_field(client, "class");
    let title = text_field(client, "title");
    let base = json!({
        "address": text_field(client, "address"),
        "class": class,
        "title": title,
        "pid": client.get("pid").and_then(Value::as_i64).unwrap_or(0),
    });
    if terminal_class(&class) {
        let classified = classify_terminal(&class, processes, shared, &title);
        let label = terminal_label(&classified);
        return extend_result(base, extend_result(classified, json!({"label": label})));
    }
    let entry = desktop_entry_for_class(&class).unwrap_or_else(|| DesktopEntry {
        name: class.clone(),
        ..DesktopEntry::default()
    });
    extend_result(
        base,
        json!({
            "kind": app_kind(&class),
            "label": if entry.name.is_empty() { class } else { entry.name.clone() },
            "app": {
                "desktop_id": entry.desktop_id,
                "name": entry.name,
                "icon": entry.icon,
                "icon_source": entry.icon_source,
                "icon_mask": entry.icon_mask,
                "accepts_files": accepts_files(&entry.executable),
            },
        }),
    )
}

pub(super) fn classify_terminal(
    class: &str,
    processes: &[ProcessRow],
    shared: bool,
    title: &str,
) -> Value {
    let comms: HashSet<&str> = processes.iter().map(|row| row.comm.as_str()).collect();
    if class == "org.omarchy.nvim"
        || (comms.contains("nvim") && !comms.contains("herdr") && !comms.contains("tmux: client"))
    {
        if shared {
            return json!({"kind": "editor", "editor": {"kind": "nvim", "server": "", "ambiguous": true,
                "reason": "cannot identify this window's nvim: the terminal process is shared with other windows"}});
        }
        return json!({"kind": "editor", "editor": {"kind": "nvim", "server": nvim_server(processes)}});
    }
    if comms.contains("herdr") {
        let mut terminal = json!({"multiplexer": "herdr"});
        let context = if shared {
            herdr_window_context(processes, title)
        } else {
            herdr_process_context(processes)
        };
        terminal = extend_result(terminal, context);
        return json!({"kind": "terminal", "terminal": terminal});
    }
    if comms.contains("tmux: client") {
        let mut terminal = json!({"multiplexer": "tmux"});
        let context = if shared {
            tmux_focused_client(processes)
        } else {
            tmux_client(processes)
        };
        terminal = extend_result(terminal, context);
        return json!({"kind": "terminal", "terminal": terminal});
    }
    json!({"kind": "terminal", "terminal": {"multiplexer": "none"}})
}

pub(super) fn terminal_label(value: &Value) -> String {
    if value.get("kind").and_then(Value::as_str) == Some("editor")
        && value
            .get("editor")
            .and_then(|editor| editor.get("ambiguous"))
            .and_then(Value::as_bool)
            .unwrap_or(false)
    {
        return "nvim (shared window)".to_string();
    }
    if value.get("kind").and_then(Value::as_str) == Some("editor") {
        return "nvim".to_string();
    }
    let multiplexer = value
        .get("terminal")
        .and_then(|terminal| terminal.get("multiplexer"))
        .and_then(Value::as_str)
        .unwrap_or("none");
    if multiplexer == "none" {
        "terminal".to_string()
    } else if value
        .get("terminal")
        .and_then(|terminal| terminal.get("ambiguous"))
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        format!("terminal ({multiplexer}, shared window)")
    } else {
        format!("terminal ({multiplexer})")
    }
}

pub(super) fn file_facts(raw_paths: &[String]) -> Value {
    let native = raw_paths
        .iter()
        .filter(|value| !value.is_empty())
        .map(|value| parse_path(value))
        .collect::<std::io::Result<Vec<_>>>();
    let native = match native {
        Ok(paths) => paths,
        Err(error) => {
            return json!({"ok": false, "count": 0, "paths": [], "error": error.to_string()});
        }
    };
    let native = native
        .into_iter()
        .filter(|path| fs::symlink_metadata(path).is_ok())
        .collect::<Vec<_>>();
    let paths = native
        .iter()
        .map(|path| path_text(path))
        .collect::<Vec<_>>();
    let files = native
        .iter()
        .filter(|path| !path.is_dir())
        .map(|path| path_text(path))
        .collect::<Vec<_>>();
    let directories = native
        .iter()
        .filter(|path| path.is_dir())
        .map(|path| path_text(path))
        .collect::<Vec<_>>();
    let mimes = paths
        .iter()
        .take(MAX_MIME_PROBES)
        .map(|path| (path.clone(), probe_mime(path)))
        .collect::<serde_json::Map<_, _>>();
    let all_probed = paths.len() <= MAX_MIME_PROBES;
    let text_like = all_probed
        && !mimes.is_empty()
        && mimes.values().all(|mime| {
            mime.as_str()
                .is_some_and(|value| is_text_like(value) || value == "inode/directory")
        });
    let git_root = repository_root(&paths);
    let git_changes = !git_root.is_empty()
        && crate::git::git_worktree_status(&git_root, true).is_some_and(|repository| {
            repository.ok
                && repository
                    .entries
                    .iter()
                    .any(|entry| native.iter().any(|path| entry.path.starts_with(path)))
        });
    let folder = if paths.len() == 1 && !directories.is_empty() {
        directories[0].clone()
    } else {
        native
            .first()
            .and_then(|path| path.parent())
            .map(path_text)
            .unwrap_or_default()
    };
    json!({
        "paths": paths,
        "count": paths.len(),
        "files": files,
        "directories": directories,
        "mime": paths.first().and_then(|path| mimes.get(path)).and_then(Value::as_str).unwrap_or_default(),
        "mimes": mimes,
        "text_like": text_like,
        "git_root": git_root,
        "git_changes": git_changes,
        "review_pair": files.len() == 2 && directories.is_empty(),
        "folder": folder,
    })
}

pub(super) fn probe_mime(path: &str) -> Value {
    if parse_path(path).is_ok_and(|path| path.is_dir()) {
        return Value::String("inode/directory".to_string());
    }
    let output = which("gio")
        .and_then(|binary| {
            CommandSpec::new(binary)
                .args(["info", "-a", "standard::content-type", "--", path])
                .timeout(Duration::from_secs(2))
                .limits(64 * 1024, 64 * 1024)
                .run()
                .ok()
        })
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .and_then(|output| {
            output.lines().find_map(|line| {
                line.trim()
                    .strip_prefix("standard::content-type:")
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
            })
        })
        .unwrap_or_else(|| guessed_mime(path));
    Value::String(output)
}

pub(super) fn guessed_mime(path: &str) -> String {
    let Ok(path) = parse_path(path) else {
        return "application/octet-stream".to_string();
    };
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_lowercase();
    match extension.as_str() {
        "txt" | "md" | "rs" | "py" | "qml" | "js" | "ts" | "css" | "html" | "toml" | "yaml"
        | "yml" => "text/plain",
        "json" => "application/json",
        "xml" => "application/xml",
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "svg" => "image/svg+xml",
        "mp3" => "audio/mpeg",
        "mp4" => "video/mp4",
        _ => "application/octet-stream",
    }
    .to_string()
}

pub(super) fn repository_root(paths: &[String]) -> String {
    let Some(first) = paths.first() else {
        return String::new();
    };
    let Some(repository) = crate::git::git_repository(first) else {
        return String::new();
    };
    if paths
        .iter()
        .all(|path| parse_path(path).is_ok_and(|path| path.starts_with(&repository.root)))
    {
        path_text(&repository.root)
    } else {
        String::new()
    }
}

pub(super) fn revalidate_target(target: &Value) -> AppResult<()> {
    let address = text_field(target, "address");
    if address.is_empty() {
        return Ok(());
    }
    window_selector(&address)?;
    let clients = hypr_query("clients")?;
    let current = clients
        .as_array()
        .and_then(|clients| {
            clients
                .iter()
                .find(|client| text_field(client, "address") == address)
        })
        .ok_or_else(|| AppError::command("The drop target is no longer available"))?;
    let expected_pid = target.get("pid").and_then(Value::as_i64).unwrap_or(0);
    let expected_class = text_field(target, "class");
    if current.get("pid").and_then(Value::as_i64).unwrap_or(0) != expected_pid
        || text_field(current, "class") != expected_class
    {
        return Err(AppError::command(
            "The drop target changed before the action ran",
        ));
    }
    Ok(())
}
