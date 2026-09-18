use crate::core_modules::metrics::{creation_time, minute_stamp};
use crate::core_modules::text::word_count;
use crate::core_modules::watch::WatchPlan;
use rustix::fs::{Mode, OFlags};
use serde_json::{Map, Value, json};
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::time::{Instant, UNIX_EPOCH};

#[derive(Debug)]
pub struct Timeout;

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
) -> Result<Vec<u8>, &'static str> {
    plan.watch_path(path, false);
    let resolved = match std::fs::canonicalize(path) {
        Ok(resolved) => resolved,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Err("missing");
        }
        Err(_) => return Err("unreadable"),
    };
    let Some(parent) = resolved.parent() else {
        return Err("unreadable");
    };
    let Some(name) = resolved.file_name() else {
        return Err("unreadable");
    };
    let Some(directory) = open_directory_nofollow(parent) else {
        return Err("unreadable");
    };
    let flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NOFOLLOW | OFlags::NONBLOCK;
    let descriptor = match rustix::fs::openat(&directory, name, flags, Mode::empty()) {
        Ok(descriptor) => descriptor,
        Err(rustix::io::Errno::NOENT) => return Err("missing"),
        Err(_) => return Err("unreadable"),
    };
    let Ok(metadata) = rustix::fs::fstat(&descriptor) else {
        return Err("unreadable");
    };
    if metadata.st_mode & libc::S_IFMT != libc::S_IFREG {
        return Err("not-regular");
    }
    if let Some(owner) = options.required_owner_uid
        && metadata.st_uid != owner
    {
        return Err("insecure-owner");
    }
    if options.reject_group_or_world_writable
        && metadata.st_mode & (libc::S_IWGRP | libc::S_IWOTH) != 0
    {
        return Err("insecure-mode");
    }
    if metadata.st_size as usize > limit {
        return Err("oversized");
    }
    let mut data: Vec<u8> = Vec::new();
    let mut buffer = vec![0u8; 65536];
    let mut remaining = limit + 1;
    while remaining > 0 {
        let wanted = remaining.min(buffer.len());
        let Ok(count) = rustix::io::read(&descriptor, &mut buffer[..wanted]) else {
            return Err("unreadable");
        };
        if count == 0 {
            break;
        }
        data.extend_from_slice(&buffer[..count]);
        remaining -= count;
    }
    if data.len() > limit {
        return Err("oversized");
    }
    Ok(data)
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
) -> Result<Vec<PathBuf>, Timeout> {
    plan.watch_path(root, true);
    let Some(descriptor) = open_directory_nofollow(root) else {
        return Ok(Vec::new());
    };
    let Ok(stream) = rustix::fs::Dir::read_from(&descriptor) else {
        return Ok(Vec::new());
    };
    let mut names: Vec<std::ffi::OsString> = Vec::new();
    let mut index = 0usize;
    for entry in stream {
        let Ok(entry) = entry else { continue };
        let name = entry.file_name().to_bytes();
        if name == b"." || name == b".." {
            continue;
        }
        if deadline.expired() {
            return Err(Timeout);
        }
        if index >= maximum {
            deadline.truncated = true;
            break;
        }
        index += 1;
        let name = std::ffi::OsStr::from_bytes(name);
        let Ok(metadata) =
            rustix::fs::statat(&descriptor, name, rustix::fs::AtFlags::SYMLINK_NOFOLLOW)
        else {
            continue;
        };
        let kind = metadata.st_mode & libc::S_IFMT;
        let keep = if directories {
            kind == libc::S_IFDIR
        } else {
            kind == libc::S_IFREG
        };
        if keep {
            names.push(name.to_os_string());
        }
    }
    names.sort_by(|left, right| left.as_bytes().cmp(right.as_bytes()));
    Ok(names.into_iter().map(|name| root.join(name)).collect())
}

pub fn bounded_directories(
    plan: &mut WatchPlan,
    root: &Path,
    maximum: usize,
    deadline: &mut Deadline,
) -> Result<Vec<PathBuf>, Timeout> {
    bounded_entries(plan, root, maximum, deadline, true)
}

pub fn bounded_files(
    plan: &mut WatchPlan,
    root: &Path,
    maximum: usize,
    deadline: &mut Deadline,
) -> Result<Vec<PathBuf>, Timeout> {
    bounded_entries(plan, root, maximum, deadline, false)
}
