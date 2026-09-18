use crate::core_modules::metrics::{creation_time, minute_stamp};
use crate::core_modules::text::word_count;
use crate::core_modules::watch::WatchPlan;
use rustix::fs::{Mode, OFlags};
use serde_json::{Map, Value, json};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::{Instant, UNIX_EPOCH};

pub struct ReadResult {
    pub data: Option<Vec<u8>>,
    pub error: Option<&'static str>,
}

impl ReadResult {
    fn failed(error: &'static str) -> Self {
        Self {
            data: None,
            error: Some(error),
        }
    }

    fn ok(data: Vec<u8>) -> Self {
        Self {
            data: Some(data),
            error: None,
        }
    }
}

pub struct Deadline {
    end: Instant,
    pub truncated: bool,
}

impl Deadline {
    pub fn new(milliseconds: u64) -> Self {
        Self {
            end: Instant::now() + std::time::Duration::from_millis(milliseconds),
            truncated: false,
        }
    }

    pub fn expired(&self) -> bool {
        Instant::now() > self.end
    }
}

pub fn absolute(path: &Path) -> PathBuf {
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        std::env::current_dir()
            .unwrap_or_else(|_| PathBuf::from("/"))
            .join(path)
    }
}

pub fn artifact_metrics(path: &Path, data: &[u8]) -> Map<String, Value> {
    let Ok(target) = std::fs::canonicalize(path) else {
        return Map::new();
    };
    let Ok(metadata) = std::fs::symlink_metadata(&target) else {
        return Map::new();
    };
    if !metadata.is_file() {
        return Map::new();
    }
    let text = String::from_utf8_lossy(data);
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
    metrics.insert("created".to_string(), json!(creation_time(&target)));
    metrics.insert("bytes".to_string(), json!(data.len()));
    metrics.insert("characters".to_string(), json!(text.chars().count()));
    metrics.insert("words".to_string(), json!(word_count(&text)));
    metrics.insert("tokens".to_string(), json!(data.len().div_ceil(4)));
    metrics
}

fn open_directory_nofollow(path: &Path) -> Option<rustix::fd::OwnedFd> {
    let flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::DIRECTORY | OFlags::NOFOLLOW;
    let mut descriptor = rustix::fs::open("/", flags, Mode::empty()).ok()?;
    for part in absolute(path)
        .as_os_str()
        .as_bytes()
        .split(|byte| *byte == b'/')
    {
        if part.is_empty() {
            continue;
        }
        let name = std::ffi::OsStr::from_bytes(part);
        let child = rustix::fs::openat(&descriptor, name, flags, Mode::empty()).ok()?;
        descriptor = child;
    }
    Some(descriptor)
}

#[derive(Default)]
pub struct Options {
    pub required_owner_uid: Option<u32>,
    pub reject_group_or_world_writable: bool,
}

impl Options {
    pub fn managed(owner: u32) -> Self {
        Self {
            required_owner_uid: Some(owner),
            reject_group_or_world_writable: true,
        }
    }
}

pub fn bounded_read(
    plan: &mut WatchPlan,
    path: &Path,
    limit: usize,
    options: &Options,
) -> ReadResult {
    plan.watch_path(path, false);
    let Ok(resolved) = std::fs::canonicalize(path) else {
        return match std::fs::symlink_metadata(path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                ReadResult::failed("missing")
            }
            _ => ReadResult::failed("unreadable"),
        };
    };
    let Some(parent) = resolved.parent() else {
        return ReadResult::failed("unreadable");
    };
    let Some(name) = resolved.file_name() else {
        return ReadResult::failed("unreadable");
    };
    let Some(directory) = open_directory_nofollow(parent) else {
        return ReadResult::failed("unreadable");
    };
    let flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK;
    let descriptor = match rustix::fs::openat(&directory, name, flags, Mode::empty()) {
        Ok(descriptor) => descriptor,
        Err(rustix::io::Errno::NOENT) => return ReadResult::failed("missing"),
        Err(_) => return ReadResult::failed("unreadable"),
    };
    let Ok(metadata) = rustix::fs::fstat(&descriptor) else {
        return ReadResult::failed("unreadable");
    };
    if metadata.st_mode & libc::S_IFMT != libc::S_IFREG {
        return ReadResult::failed("not-regular");
    }
    if let Some(owner) = options.required_owner_uid
        && metadata.st_uid != owner
    {
        return ReadResult::failed("insecure-owner");
    }
    if options.reject_group_or_world_writable
        && metadata.st_mode & (libc::S_IWGRP | libc::S_IWOTH) != 0
    {
        return ReadResult::failed("insecure-mode");
    }
    if metadata.st_size as usize > limit {
        return ReadResult::failed("oversized");
    }
    let mut data: Vec<u8> = Vec::new();
    let mut buffer = vec![0u8; 65536];
    let mut remaining = limit + 1;
    while remaining > 0 {
        let wanted = remaining.min(buffer.len());
        let Ok(count) = rustix::io::read(&descriptor, &mut buffer[..wanted]) else {
            return ReadResult::failed("unreadable");
        };
        if count == 0 {
            break;
        }
        data.extend_from_slice(&buffer[..count]);
        remaining -= count;
    }
    if data.len() > limit {
        return ReadResult::failed("oversized");
    }
    ReadResult::ok(data)
}

pub fn safe_relative_file(root: &Path, relative: &str) -> Option<PathBuf> {
    use std::path::Component;
    let mut components: Vec<&std::ffi::OsStr> = Vec::new();
    for component in Path::new(relative).components() {
        match component {
            Component::CurDir => continue,
            Component::Normal(part) => components.push(part),
            _ => return None,
        }
    }
    if components.is_empty() {
        return None;
    }
    let mut current = root.to_path_buf();
    for part in &components[..components.len() - 1] {
        current = current.join(part);
        let metadata = std::fs::symlink_metadata(&current).ok()?;
        if metadata.is_symlink() || !metadata.is_dir() {
            return None;
        }
    }
    let mut result = root.to_path_buf();
    for part in &components {
        result = result.join(part);
    }
    Some(result)
}

fn bounded_entries(
    plan: &mut WatchPlan,
    root: &Path,
    maximum: usize,
    deadline: &mut Deadline,
    directories: bool,
) -> Vec<PathBuf> {
    plan.watch_path(root, true);
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut names: Vec<std::ffi::OsString> = Vec::new();
    for (index, entry) in entries.enumerate() {
        if deadline.expired() {
            return Vec::new();
        }
        if index >= maximum {
            deadline.truncated = true;
            break;
        }
        let Ok(entry) = entry else { continue };
        let Ok(metadata) = std::fs::symlink_metadata(entry.path()) else {
            continue;
        };
        let keep = if directories {
            metadata.is_dir()
        } else {
            metadata.is_file()
        };
        if keep {
            names.push(entry.file_name());
        }
    }
    names.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    names.into_iter().map(|name| root.join(name)).collect()
}

pub fn bounded_directories(
    plan: &mut WatchPlan,
    root: &Path,
    maximum: usize,
    deadline: &mut Deadline,
) -> Vec<PathBuf> {
    bounded_entries(plan, root, maximum, deadline, true)
}

pub fn bounded_files(
    plan: &mut WatchPlan,
    root: &Path,
    maximum: usize,
    deadline: &mut Deadline,
) -> Vec<PathBuf> {
    bounded_entries(plan, root, maximum, deadline, false)
}
