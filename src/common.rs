use chrono::{DateTime, Local};
use serde_json::{Value, json};
use std::ffi::OsStr;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

mod path;
pub use path::{display_path, parse_display_name, parse_path, path_error, path_text};

pub const CONTROL_TIMEOUT: Duration = Duration::from_secs(3);

pub fn own_binary() -> io::Result<PathBuf> {
    std::env::current_exe().map(|path| replaced_binary_path(&path))
}

pub fn replaced_binary_path(path: &Path) -> PathBuf {
    const DELETED: &[u8] = b" (deleted)";
    let bytes = std::os::unix::ffi::OsStrExt::as_bytes(path.as_os_str());
    match bytes.strip_suffix(DELETED) {
        Some(stripped) if !stripped.is_empty() => {
            let original = PathBuf::from(
                <std::ffi::OsString as std::os::unix::ffi::OsStringExt>::from_vec(
                    stripped.to_vec(),
                ),
            );
            if original.is_file() {
                original
            } else {
                path.to_path_buf()
            }
        }
        _ => path.to_path_buf(),
    }
}

pub fn expanded_path(raw: &str) -> PathBuf {
    let input = if raw.is_empty() { "~" } else { raw };
    expanded_os_path(Path::new(input))
}

pub fn expanded_os_path(path: &Path) -> PathBuf {
    use std::os::unix::ffi::OsStrExt;
    let bytes = path.as_os_str().as_bytes();
    let expanded = if bytes == b"~" || bytes.starts_with(b"~/") {
        let home = std::env::var_os("HOME").unwrap_or_else(|| "/".into());
        let mut result = PathBuf::from(home);
        if bytes.len() > 2 {
            result.push(Path::new(OsStr::from_bytes(&bytes[2..])));
        }
        result
    } else {
        path.to_path_buf()
    };
    let absolute = if expanded.is_absolute() {
        expanded
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("/"))
            .join(expanded)
    };
    normalize_path(&absolute)
}

pub fn normalize_path(path: &Path) -> PathBuf {
    let mut result = PathBuf::new();
    for component in path.components() {
        match component {
            Component::Prefix(prefix) => result.push(prefix.as_os_str()),
            Component::RootDir => result.push(Path::new("/")),
            Component::CurDir => {}
            Component::ParentDir => {
                if result != Path::new("/") {
                    result.pop();
                }
            }
            Component::Normal(part) => result.push(part),
        }
    }
    if result.as_os_str().is_empty() {
        PathBuf::from("/")
    } else {
        result
    }
}

pub fn timestamp(value: SystemTime) -> String {
    let time: DateTime<Local> = value.into();
    time.format("%Y-%m-%d %H:%M:%S").to_string()
}

pub fn timestamp_seconds(seconds: i64) -> String {
    DateTime::<Local>::from(UNIX_EPOCH + Duration::from_secs(seconds.max(0) as u64))
        .format("%Y-%m-%d %H:%M:%S")
        .to_string()
}

pub fn human_size(size: Option<u64>) -> String {
    let Some(size) = size else {
        return "—".to_string();
    };
    let units = ["B", "KB", "MB", "GB", "TB", "PB"];
    let mut value = size as f64;
    for unit in units {
        if value < 1024.0 || unit == "PB" {
            return if unit == "B" {
                format!("{} {unit}", value as u64)
            } else {
                format!("{value:.1} {unit}")
            };
        }
        value /= 1024.0;
    }
    format!("{size} B")
}

pub fn error_payload(path: &Path, error: &(dyn std::error::Error + 'static)) -> Value {
    let mut payload = json!({
        "ok": false,
        "path": path_text(path),
        "error": error.to_string(),
    });
    if let Some(io_error) = error.downcast_ref::<io::Error>() {
        let code = io_error.raw_os_error().unwrap_or_default();
        payload["errno"] = json!(code);
        payload["missing"] = json!(matches!(
            io_error.kind(),
            io::ErrorKind::NotFound | io::ErrorKind::NotADirectory
        ));
        payload["permission_denied"] = json!(io_error.kind() == io::ErrorKind::PermissionDenied);
    }
    payload
}

pub fn file_name(path: &Path) -> String {
    display_path(Path::new(
        path.file_name().unwrap_or_else(|| OsStr::new("/")),
    ))
}
