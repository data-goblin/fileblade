use super::*;

pub(super) fn transfer(
    copy: bool,
    sources: &[String],
    destination: &str,
    journal_id: &str,
    cancelled: &AtomicBool,
    progress: &mut dyn FnMut(Value) -> AppResult<()>,
) -> AppResult<Value> {
    let mut output_error = None;
    let mut callback = |value| {
        if output_error.is_some() {
            return;
        }
        if let Err(error) = progress(value) {
            cancelled.store(true, Ordering::Relaxed);
            output_error = Some(error);
        }
    };
    let result = if copy {
        crate::operations::copy_paths(sources, destination, &mut callback, cancelled, journal_id)
    } else {
        crate::operations::move_paths(sources, destination, &mut callback, cancelled, journal_id)
    };
    match output_error {
        Some(error) => Err(error),
        None => Ok(result),
    }
}

pub(super) fn clipboard_write(paths: &[String], cut: bool, cancelled: &AtomicBool) -> Value {
    const MAX_PATHS: usize = 4096;
    const MAX_BYTES: usize = 1024 * 1024;
    if paths.len() > MAX_PATHS {
        return json!({
            "ok": false,
            "error": format!("at most {MAX_PATHS} clipboard paths are supported"),
        });
    }
    let mut input = Vec::new();
    for raw_path in paths {
        let path = match crate::common::parse_path(raw_path) {
            Ok(path) => path,
            Err(error) => return crate::common::path_error(raw_path, &error),
        };
        let Ok(uri) = url::Url::from_file_path(&path) else {
            return json!({
                "ok": false,
                "error": format!("could not encode clipboard path: {}", path.display()),
            });
        };
        let line = uri.as_str().as_bytes();
        if input.len().saturating_add(line.len()).saturating_add(2) > MAX_BYTES {
            return json!({
                "ok": false,
                "error": "clipboard URI list exceeds 1 MiB",
            });
        }
        input.extend_from_slice(line);
        input.extend_from_slice(b"\r\n");
    }
    if let Err(error) = wl_copy("text/uri-list", input, cancelled) {
        return error;
    }
    json!({
        "ok": true,
        "paths": paths.len(),
        "mode": if cut { "cut" } else { "copy" },
        "mime": "text/uri-list",
    })
}

pub(super) fn clipboard_text(paths: &[String], cancelled: &AtomicBool) -> Value {
    const MAX_PATHS: usize = 4096;
    const MAX_BYTES: usize = 1024 * 1024;
    if paths.is_empty() {
        return json!({"ok": false, "error": "no paths to copy"});
    }
    if paths.len() > MAX_PATHS {
        return json!({
            "ok": false,
            "error": format!("at most {MAX_PATHS} clipboard paths are supported"),
        });
    }
    let lines = paths
        .iter()
        .map(|raw_path| {
            crate::common::parse_path(raw_path).map(|path| crate::common::path_text(&path))
        })
        .collect::<std::io::Result<Vec<_>>>();
    let lines = match lines {
        Ok(lines) => lines,
        Err(error) => return json!({"ok": false, "error": error.to_string()}),
    };
    let text = lines.join("\n");
    if text.len() > MAX_BYTES {
        return json!({"ok": false, "error": "clipboard text exceeds 1 MiB"});
    }
    if let Err(error) = wl_copy("text/plain", text.into_bytes(), cancelled) {
        return error;
    }
    json!({
        "ok": true,
        "paths": paths.len(),
        "mime": "text/plain",
    })
}

pub(super) fn wl_copy(mime: &str, input: Vec<u8>, cancelled: &AtomicBool) -> Result<(), Value> {
    let Some(program) = crate::command::which("wl-copy") else {
        return Err(json!({"ok": false, "error": "wl-copy is not installed"}));
    };
    crate::command::CommandSpec::new(program)
        .args(["--type", mime])
        .stdin(input)
        .timeout(Duration::from_secs(3))
        .spawn_detached_with_stdin(cancelled)
        .map_err(|error| json!({"ok": false, "error": error.to_string()}))?;
    Ok(())
}

pub(super) fn state_path() -> std::path::PathBuf {
    crate::paths::state_dir().join("state.json")
}

pub(super) fn layout_path() -> std::path::PathBuf {
    crate::paths::config_dir().join("blades.json")
}

pub(super) fn private_document_read(path: &std::path::Path) -> Value {
    if let Err(error) = prepare_private_path(path) {
        return json!({"ok": false, "text": "", "error": error.to_string()});
    }
    match crate::secure::read_private_bounded(path, 256 * 1024) {
        Ok(Some(bytes)) => match String::from_utf8(bytes) {
            Ok(text) => match serde_json::from_str::<Value>(&text) {
                Ok(Value::Object(_)) => json!({"ok": true, "text": text}),
                Ok(_) => quarantined_document(path, "document must be a JSON object"),
                Err(error) => {
                    quarantined_document(path, &format!("invalid JSON document: {error}"))
                }
            },
            Err(error) => quarantined_document(path, &error.to_string()),
        },
        Ok(None) => json!({"ok": true, "text": ""}),
        Err(error) => json!({"ok": false, "text": "", "error": error.to_string()}),
    }
}

pub(super) fn quarantined_document(path: &std::path::Path, reason: &str) -> Value {
    let Some(name) = path.file_name().and_then(|value| value.to_str()) else {
        return json!({"ok": false, "text": "", "error": reason});
    };
    let destination = path.with_file_name(format!(
        "{name}.corrupt-{}-{}",
        chrono::Local::now().format("%Y%m%d-%H%M%S"),
        uuid::Uuid::new_v4().simple()
    ));
    match crate::secure::rename_noreplace(path, &destination) {
        Ok(()) => json!({
            "ok": true,
            "text": "",
            "quarantined": destination,
            "error": reason,
        }),
        Err(error) => json!({
            "ok": false,
            "text": "",
            "error": format!("{reason}; could not quarantine the document: {error}"),
        }),
    }
}

pub(super) fn private_document_write(path: &std::path::Path, document: &str) -> Value {
    if document.len() > 256 * 1024 {
        return json!({"ok": false, "error": "document exceeds 256 KiB"});
    }
    match serde_json::from_str::<Value>(document) {
        Ok(Value::Object(_)) => {}
        Ok(_) => return json!({"ok": false, "error": "document must be a JSON object"}),
        Err(error) => {
            return json!({"ok": false, "error": format!("invalid JSON document: {error}")});
        }
    }
    if let Err(error) = prepare_private_path(path) {
        return json!({"ok": false, "error": error.to_string()});
    }
    match crate::secure::write_private_atomic(path, document.as_bytes()) {
        Ok(()) => json!({"ok": true}),
        Err(error) => json!({"ok": false, "error": error.to_string()}),
    }
}

pub(super) fn prepare_private_path(path: &std::path::Path) -> std::io::Result<()> {
    let parent = path.parent().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "private path has no parent",
        )
    })?;
    crate::secure::ensure_private_directory(parent)?;
    match crate::secure::entry_stat(path) {
        Ok(stat)
            if stat.kind == crate::secure::EntryKind::File
                && stat.uid == rustix::process::geteuid().as_raw() =>
        {
            if stat.mode & 0o777 != crate::secure::PRIVATE_FILE_MODE {
                crate::secure::set_mode(path, crate::secure::PRIVATE_FILE_MODE)?;
            }
            Ok(())
        }
        Ok(stat) if stat.kind != crate::secure::EntryKind::File => Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "private state has the wrong file type",
        )),
        Ok(_) => Err(std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "private state is owned by another user",
        )),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error),
    }
}
