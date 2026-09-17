use crate::core_modules::metrics::{creation_time, minute_stamp};
use crate::core_modules::text::{estimated_tokens, word_count};
use crate::core_modules::watch::WatchPlan;
use rustix::fs::{Mode, OFlags};
use serde_json::{Map, Value, json};
use std::ffi::OsString;
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

pub const MAX_ITEMS: usize = 256;
pub const MAX_NAME: usize = 256;
pub const MAX_DETAIL: usize = 512;
pub const MAX_PATH: usize = 4096;
pub const MAX_ENV_PATH: usize = 4096;
pub const MAX_DESCRIPTOR_BYTES: usize = 64 * 1024;
pub const MAX_ENTRIES_PER_ROOT: usize = 512;
pub const MAX_TOTAL_ENTRIES: usize = 8192;
pub const MAX_ROOTS: usize = 256;
pub const MAX_PROJECT_WALK: usize = 32;
pub const MAX_PLUGINS: usize = 256;
pub const MAX_CONFIG_BYTES: usize = 256 * 1024;

pub fn env_path_value(environ: &[(OsString, OsString)], name: &str) -> String {
    let Some((_, value)) = environ.iter().find(|(key, _)| key == name) else {
        return String::new();
    };
    let Some(text) = value.to_str() else {
        return String::new();
    };
    if text.is_empty() || text.contains('\0') || text.chars().count() > MAX_ENV_PATH {
        return String::new();
    }
    text.to_string()
}

pub fn bounded_names(plan: &mut WatchPlan, path: &Path, limit: isize) -> (Vec<OsString>, bool) {
    plan.watch_path(path, true);
    let mut names: Vec<OsString> = Vec::new();
    let Ok(entries) = std::fs::read_dir(path) else {
        return (sorted(names), false);
    };
    for entry in entries {
        let Ok(entry) = entry else { continue };
        if names.len() as isize >= limit {
            return (sorted(names), true);
        }
        names.push(entry.file_name());
    }
    (sorted(names), false)
}

fn sorted(mut names: Vec<OsString>) -> Vec<OsString> {
    names.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    names
}

pub fn read_descriptor(
    plan: &mut WatchPlan,
    path: &Path,
    limit: usize,
    secure: bool,
) -> Option<Vec<u8>> {
    plan.watch_path(path, false);
    let flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK;
    let descriptor = rustix::fs::open(path, flags, Mode::empty()).ok()?;
    let metadata = rustix::fs::fstat(&descriptor).ok()?;
    let kind = metadata.st_mode & libc::S_IFMT;
    if kind != libc::S_IFREG || metadata.st_size as u128 > limit as u128 {
        return None;
    }
    if secure && (metadata.st_uid != 0 || metadata.st_mode & 0o022 != 0) {
        return None;
    }
    let mut buffer = vec![0u8; limit];
    let count = rustix::io::read(&descriptor, &mut buffer).ok()?;
    buffer.truncate(count);
    Some(buffer)
}

pub fn read_document(plan: &mut WatchPlan, path: &Path, limit: usize) -> String {
    read_descriptor(plan, path, limit, false)
        .map(|data| String::from_utf8_lossy(&data).into_owned())
        .unwrap_or_default()
}

pub fn read_secure_document(plan: &mut WatchPlan, path: &Path, enforce: bool) -> String {
    read_descriptor(plan, path, MAX_CONFIG_BYTES, enforce)
        .map(|data| String::from_utf8_lossy(&data).into_owned())
        .unwrap_or_default()
}

pub fn artifact_metrics(path: &Path, text: &str, description: &str) -> Map<String, Value> {
    let Ok(metadata) = std::fs::metadata(path) else {
        return Map::new();
    };
    let size = metadata.len();
    let complete = size as usize <= MAX_DESCRIPTOR_BYTES;
    let counted = |value: Value| if complete { value } else { Value::Null };
    let updated = metadata
        .modified()
        .ok()
        .map(|moment| match moment.duration_since(UNIX_EPOCH) {
            Ok(elapsed) => minute_stamp(elapsed.as_secs() as i64, elapsed.subsec_nanos()),
            Err(error) => minute_stamp(-(error.duration().as_secs() as i64), 0),
        })
        .unwrap_or_default();
    let mut metrics = Map::new();
    metrics.insert("updated".to_string(), json!(updated));
    metrics.insert("created".to_string(), json!(creation_time(path)));
    metrics.insert("bytes".to_string(), json!(size));
    metrics.insert(
        "characters".to_string(),
        counted(json!(text.chars().count())),
    );
    metrics.insert("words".to_string(), counted(json!(word_count(text))));
    metrics.insert(
        "tokens".to_string(),
        counted(json!(estimated_tokens(description))),
    );
    metrics.insert(
        "fileTokens".to_string(),
        counted(json!(estimated_tokens(text))),
    );
    metrics
}

pub fn under(prefix: &Path, path: &str) -> PathBuf {
    if prefix.as_os_str().is_empty() {
        return PathBuf::from(path);
    }
    prefix.join(path.trim_start_matches(['/', '\\']))
}

pub fn realpath(path: &Path) -> PathBuf {
    let mut pending: Vec<OsString> = Vec::new();
    for part in path
        .as_os_str()
        .as_bytes()
        .split(|byte| *byte == b'/')
        .rev()
    {
        if !part.is_empty() {
            pending.push(OsString::from_vec(part.to_vec()));
        }
    }
    let mut resolved = PathBuf::from("/");
    let mut followed = 0usize;
    while let Some(part) = pending.pop() {
        if part == *"." {
            continue;
        }
        if part == *".." {
            resolved.pop();
            continue;
        }
        let candidate = resolved.join(&part);
        match std::fs::read_link(&candidate) {
            Ok(target) if followed < 40 => {
                followed += 1;
                if target.is_absolute() {
                    resolved = PathBuf::from("/");
                }
                for piece in target
                    .as_os_str()
                    .as_bytes()
                    .split(|byte| *byte == b'/')
                    .rev()
                {
                    if !piece.is_empty() {
                        pending.push(OsString::from_vec(piece.to_vec()));
                    }
                }
            }
            _ => resolved = candidate,
        }
    }
    resolved
}
