use crate::command::{CommandSpec, which};
use crate::common::{
    CONTROL_TIMEOUT, error_payload, expanded_path, parse_path, path_error, path_text,
};
use crate::filesystem::{content_type_cancellable, read_regular_file, valid_mime};
use serde_json::{Value, json};
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

pub const MAX_CLIPBOARD_BYTES: usize = 1024 * 1024;
const MAX_DESKTOP_FILE_BYTES: usize = 64 * 1024;
const MAX_APPLICATION_OUTPUT_BYTES: usize = 1024 * 1024;
const MAX_APPLICATIONS: usize = 512;
const MAX_DATA_DIRECTORIES: usize = 16;

pub fn local_path_from_file_uri(uri: &str) -> Option<PathBuf> {
    let uri = uri.trim();
    if !uri
        .get(..7)
        .is_some_and(|prefix| prefix.eq_ignore_ascii_case("file://"))
    {
        return None;
    }
    let path = parse_path(uri).ok()?;
    std::fs::symlink_metadata(&path).ok().map(|_| path)
}

pub fn clipboard_files_cancellable(limit: usize, cancelled: &AtomicBool) -> Value {
    if cancelled.load(Ordering::Relaxed) {
        return clipboard_error("", "operation cancelled");
    }
    let Some(wl_paste) = which("wl-paste") else {
        return clipboard_error("", "wl-paste is not installed");
    };
    let maximum = limit.clamp(1, 4096);
    let advertised = CommandSpec::new(wl_paste.clone())
        .args(["--list-types"])
        .timeout(Duration::from_millis(1500))
        .limits(64 * 1024, 64 * 1024)
        .run_cancellable(cancelled);
    let advertised = match advertised {
        Ok(output) => output,
        Err(error) => return clipboard_error("", &error.to_string()),
    };
    if advertised.stdout_truncated || advertised.stderr_truncated {
        return clipboard_error("", "Wayland clipboard types exceed 64 KiB");
    }
    if !advertised.status.success() {
        let detail = String::from_utf8_lossy(&advertised.stderr)
            .trim()
            .to_string();
        return clipboard_error(
            "",
            if detail.is_empty() {
                "Unable to inspect the Wayland clipboard"
            } else {
                &detail
            },
        );
    }
    let advertised = String::from_utf8_lossy(&advertised.stdout);
    let mime = clipboard_mime(&advertised);
    if mime.is_empty() {
        return json!({"ok": true, "paths": [], "mode": "copy", "mime": "", "error": ""});
    }
    let output = CommandSpec::new(wl_paste)
        .args(["--no-newline", "--type", mime.as_str()])
        .timeout(Duration::from_millis(1500))
        .limits(MAX_CLIPBOARD_BYTES, 64 * 1024)
        .run_cancellable(cancelled);
    let output = match output {
        Ok(output) => output,
        Err(error) => return clipboard_error(&mime, &error.to_string()),
    };
    if output.stdout_truncated {
        return clipboard_error(&mime, "File clipboard exceeds 1 MiB");
    }
    if !output.status.success() {
        return clipboard_error(&mime, "Unable to read files from the Wayland clipboard");
    }
    let text = match String::from_utf8(output.stdout) {
        Ok(text) => text,
        Err(error) => return clipboard_error(&mime, &error.to_string()),
    };
    let (paths, mode) = clipboard_paths(&text, &mime, maximum);
    json!({
        "ok": true,
        "paths": paths.iter().map(|path| path_text(path)).collect::<Vec<_>>(),
        "mode": mode,
        "mime": mime,
        "truncated": paths.len() >= maximum,
        "error": ""
    })
}

pub fn data_directories() -> Vec<PathBuf> {
    let mut result = Vec::new();
    let mut seen = HashSet::new();
    let home = std::env::var_os("XDG_DATA_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| expanded_path("~/.local/share"));
    push_data_directory(&mut result, &mut seen, home);
    let configured = std::env::var("XDG_DATA_DIRS")
        .unwrap_or_else(|_| "/usr/local/share:/usr/share".to_string());
    for value in configured.split(':').filter(|value| !value.is_empty()) {
        push_data_directory(&mut result, &mut seen, PathBuf::from(value));
        if result.len() >= MAX_DATA_DIRECTORIES {
            break;
        }
    }
    let zed = expanded_path("~/.local/zed.app/share");
    if zed.exists() && result.len() < MAX_DATA_DIRECTORIES {
        push_data_directory(&mut result, &mut seen, zed);
    }
    result
}

pub fn desktop_file_for(desktop_id: &str) -> Option<PathBuf> {
    if !valid_desktop_id(desktop_id) {
        return None;
    }
    data_directories().into_iter().find_map(|directory| {
        let candidate = directory.join("applications").join(desktop_id);
        std::fs::symlink_metadata(&candidate)
            .ok()
            .filter(|metadata| metadata.is_file() && !metadata.file_type().is_symlink())
            .map(|_| candidate)
    })
}

pub fn applications_for(path: &str, mime_hint: &str) -> Value {
    applications_for_cancellable(path, mime_hint, &AtomicBool::new(false))
}

pub fn applications_for_cancellable(
    raw_path: &str,
    mime_hint: &str,
    cancelled: &AtomicBool,
) -> Value {
    let path = match parse_path(raw_path) {
        Ok(path) => path,
        Err(error) => return path_error(raw_path, &error),
    };
    if cancelled.load(Ordering::Relaxed) {
        return json!({
            "ok": false,
            "path": path_text(&path),
            "error": "operation cancelled",
            "mime": "",
            "applications": []
        });
    }
    let hinted = mime_hint.trim().to_lowercase();
    let mime = if valid_mime(&hinted) {
        hinted
    } else {
        content_type_cancellable(&path_text(&path), cancelled)
    };
    let Some(gio) = which("gio") else {
        let error = std::io::Error::new(std::io::ErrorKind::NotFound, "gio is not installed");
        let mut payload = error_payload(&path, &error);
        payload["mime"] = json!(mime);
        payload["applications"] = json!([]);
        return payload;
    };
    let output = CommandSpec::new(gio)
        .args(["mime".to_string(), mime.clone()])
        .env("LC_ALL", "C.UTF-8")
        .timeout(CONTROL_TIMEOUT)
        .limits(MAX_APPLICATION_OUTPUT_BYTES, 64 * 1024)
        .run_cancellable(cancelled);
    let output = match output {
        Ok(output) => output,
        Err(error) => {
            let mut payload = json!({
                "ok": false,
                "path": path_text(&path),
                "error": error.to_string(),
                "mime": mime,
                "applications": []
            });
            if matches!(error, crate::AppError::Io(ref io_error) if io_error.kind() == std::io::ErrorKind::NotFound)
            {
                payload["missing"] = json!(true);
            }
            return payload;
        }
    };
    if output.stdout_truncated {
        return json!({
            "ok": false,
            "path": path_text(&path),
            "mime": mime,
            "applications": [],
            "error": "Application list exceeds 1 MiB"
        });
    }
    let (default_id, ordered) = parse_applications(&String::from_utf8_lossy(&output.stdout));
    let applications = ordered
        .iter()
        .map(|desktop_id| {
            let (name, icon) = desktop_identity(desktop_id);
            json!({
                "desktop_id": desktop_id,
                "name": name,
                "icon": icon,
                "is_default": desktop_id == &default_id
            })
        })
        .collect::<Vec<_>>();
    let error = if output.status.success() {
        String::new()
    } else {
        String::from_utf8_lossy(&output.stderr).trim().to_string()
    };
    json!({
        "ok": output.status.success(),
        "path": path_text(&path),
        "mime": mime,
        "default": default_id,
        "applications": applications,
        "error": error
    })
}

pub fn valid_desktop_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 255
        && value.ends_with(".desktop")
        && !value.starts_with('-')
        && !value.contains(['/', '\\'])
        && !value.chars().any(char::is_control)
}

fn clipboard_mime(output: &str) -> String {
    let mut types = output
        .lines()
        .map(str::trim)
        .filter(|value| !value.is_empty() && value.len() <= 255)
        .take(1024)
        .collect::<HashSet<_>>();
    if types.contains("x-special/gnome-copied-files") {
        return "x-special/gnome-copied-files".to_string();
    }
    let mut uri_types = types
        .drain()
        .filter(|value| value.split(';').next().unwrap_or_default().trim() == "text/uri-list")
        .collect::<Vec<_>>();
    uri_types.sort_unstable();
    uri_types.first().copied().unwrap_or_default().to_string()
}

fn clipboard_paths(output: &str, mime: &str, limit: usize) -> (Vec<PathBuf>, String) {
    let mut lines = output.lines();
    let first = lines.next();
    let mut mode = "copy".to_string();
    let values = if mime == "x-special/gnome-copied-files"
        && first.is_some_and(|value| matches!(value.trim().to_lowercase().as_str(), "copy" | "cut"))
    {
        mode = first.unwrap_or_default().trim().to_lowercase();
        lines.collect::<Vec<_>>()
    } else {
        first.into_iter().chain(lines).collect::<Vec<_>>()
    };
    let mut paths = Vec::new();
    let mut seen = HashSet::new();
    for value in values.into_iter().take(65_536) {
        let value = value.trim();
        if value.is_empty() || value.starts_with('#') || value.len() > 16 * 1024 {
            continue;
        }
        if let Some(path) = local_path_from_file_uri(value)
            && seen.insert(path.clone())
        {
            paths.push(path);
        }
        if paths.len() >= limit {
            break;
        }
    }
    (paths, mode)
}

fn desktop_identity(desktop_id: &str) -> (String, String) {
    let fallback = desktop_id
        .strip_suffix(".desktop")
        .unwrap_or(desktop_id)
        .to_string();
    let Some(path) = desktop_file_for(desktop_id) else {
        return (fallback.clone(), fallback);
    };
    let Ok(data) = read_regular_file(&path, MAX_DESKTOP_FILE_BYTES) else {
        return (fallback.clone(), fallback);
    };
    let Ok(text) = String::from_utf8(data) else {
        return (fallback.clone(), fallback);
    };
    let mut in_desktop_entry = false;
    let mut name = String::new();
    let mut icon = String::new();
    for line in text.lines().take(8192) {
        let line = line.trim();
        if line.starts_with('[') && line.ends_with(']') {
            in_desktop_entry = line == "[Desktop Entry]";
            continue;
        }
        if in_desktop_entry && let Some((key, value)) = line.split_once('=') {
            let value = value.trim();
            if !value.is_empty() && value.len() <= 4096 {
                if name.is_empty() && key.eq_ignore_ascii_case("Name") {
                    name = value.to_string();
                } else if icon.is_empty() && key.eq_ignore_ascii_case("Icon") {
                    icon = value.to_string();
                }
            }
        }
    }
    if icon.is_empty() {
        icon = fallback.clone();
    }
    (if name.is_empty() { fallback } else { name }, icon)
}

fn parse_applications(output: &str) -> (String, Vec<String>) {
    let mut default_id = String::new();
    let mut registered = Vec::new();
    let mut seen = HashSet::new();
    let mut in_registered = false;
    for raw_line in output.lines().take(65_536) {
        let line = raw_line.trim();
        if line.len() > 4096 {
            continue;
        }
        if line.starts_with("Default application") {
            if let Some(value) = line.rsplit_once(':').map(|(_, value)| value.trim())
                && valid_desktop_id(value)
            {
                default_id = value.to_string();
            }
            in_registered = false;
        } else if line == "Registered applications:" {
            in_registered = true;
        } else if line.ends_with("applications:") {
            in_registered = false;
        } else if in_registered && valid_desktop_id(line) && seen.insert(line.to_string()) {
            registered.push(line.to_string());
            if registered.len() >= MAX_APPLICATIONS {
                break;
            }
        }
    }
    let mut ordered = Vec::new();
    if !default_id.is_empty() {
        ordered.push(default_id.clone());
    }
    for desktop_id in registered {
        if !ordered.contains(&desktop_id) && ordered.len() < MAX_APPLICATIONS {
            ordered.push(desktop_id);
        }
    }
    (default_id, ordered)
}

fn push_data_directory(result: &mut Vec<PathBuf>, seen: &mut HashSet<PathBuf>, path: PathBuf) {
    if path.is_absolute() && seen.insert(path.clone()) && result.len() < MAX_DATA_DIRECTORIES {
        result.push(path);
    }
}

fn clipboard_error(mime: &str, error: &str) -> Value {
    json!({"ok": false, "paths": [], "mode": "copy", "mime": mime, "error": error})
}
