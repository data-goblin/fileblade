use crate::core_modules::glob::fnmatch_bytes;
use crate::core_modules::path::realpath;
use crate::core_modules::watch::WatchPlan;
use rustix::fs::{AtFlags, Dir, Mode, OFlags};
use serde_json::Value;
use std::collections::HashSet;
use std::ffi::{OsStr, OsString};
use std::os::unix::ffi::{OsStrExt, OsStringExt};
use std::path::{Path, PathBuf};
use unicode_general_category::{GeneralCategory, get_general_category};

pub const SCHEMA_VERSION: i64 = 1;
pub const MAX_DESCRIPTOR_BYTES: usize = 8 * 1024;
pub const MAX_FILE_BYTES: u64 = 4 * 1024 * 1024;
pub const MAX_ITEMS: usize = 1000;
pub const MAX_SOURCES: usize = 512;
pub const MAX_DIR_ENTRIES: usize = 512;
pub const MAX_RULE_DEPTH: usize = 4;
pub const MAX_ANCESTORS: usize = 24;
pub const MAX_EXTRA_ROOTS: usize = 16;
pub const MAX_DETAIL_CHARS: usize = 160;
pub const MAX_NAME_CHARS: usize = 120;
pub const MAX_CONFIG_BYTES: usize = 64 * 1024;
pub const MAX_FALLBACK_NAMES: usize = 8;
pub const MAX_INSTRUCTION_ENTRIES: usize = 32;
pub const MAX_ENV_DIRS: usize = 8;
pub const MAX_BASENAME_CHARS: usize = 255;
pub const MAX_ENV_PATH_CHARS: usize = 4096;

pub const PROJECT_MARKERS: [&str; 7] = [
    ".git",
    ".claude",
    ".agents",
    ".codex",
    ".opencode",
    ".pi",
    ".github",
];

const REMOTE_PREFIXES: [&str; 4] = ["http://", "https://", "ftp://", "//"];

pub type Environ = Vec<(OsString, OsString)>;

pub struct Budget {
    items: usize,
    sources: usize,
    truncated: bool,
}

impl Default for Budget {
    fn default() -> Self {
        Self {
            items: MAX_ITEMS,
            sources: MAX_SOURCES,
            truncated: false,
        }
    }
}

impl Budget {
    pub fn truncated(&self) -> bool {
        self.truncated
    }

    pub fn take_source(&mut self) -> bool {
        if self.sources == 0 {
            self.truncated = true;
            return false;
        }
        self.sources -= 1;
        true
    }

    pub fn take_item(&mut self) -> bool {
        if self.items == 0 {
            self.truncated = true;
            return false;
        }
        self.items -= 1;
        true
    }
}

pub fn expanded(path: &str) -> PathBuf {
    match crate::common::parse_path(path) {
        Ok(parsed) => crate::common::expanded_os_path(&parsed),
        Err(_) => crate::common::expanded_os_path(Path::new(path)),
    }
}

pub fn expanded_os(value: &OsStr) -> PathBuf {
    match value.to_str() {
        Some(text) => expanded(text),
        None => expanded_path(Path::new(value)),
    }
}

pub fn expanded_path(path: &Path) -> PathBuf {
    crate::common::expanded_os_path(path)
}

pub fn realpath_of(path: &Path) -> PathBuf {
    realpath(path)
}

pub fn stable_id(target: &Path) -> String {
    crate::core_modules::canonical::path_id(target)
}

fn open_regular(plan: &mut WatchPlan, path: &Path) -> Option<(rustix::fd::OwnedFd, u64)> {
    plan.watch_path(path, false);
    let flags = OFlags::RDONLY | OFlags::CLOEXEC | OFlags::NONBLOCK;
    let descriptor = rustix::fs::open(path, flags, Mode::empty()).ok()?;
    let metadata = rustix::fs::fstat(&descriptor).ok()?;
    let size = metadata.st_size as u64;
    if metadata.st_mode & libc::S_IFMT != libc::S_IFREG || size > MAX_FILE_BYTES {
        return None;
    }
    Some((descriptor, size))
}

fn read_capped_fd(descriptor: &rustix::fd::OwnedFd, cap: usize) -> Option<String> {
    let mut buffer = vec![0u8; cap];
    let count = rustix::io::read(descriptor, &mut buffer).ok()?;
    buffer.truncate(count);
    Some(String::from_utf8_lossy(&buffer).into_owned())
}

pub fn descriptor_head(plan: &mut WatchPlan, path: &Path) -> Option<(String, u64)> {
    let (descriptor, size) = open_regular(plan, path)?;
    let text = read_capped_fd(&descriptor, MAX_DESCRIPTOR_BYTES)?;
    Some((text, size))
}

pub fn read_capped(plan: &mut WatchPlan, path: &Path) -> String {
    let Some((descriptor, _)) = open_regular(plan, path) else {
        return String::new();
    };
    read_capped_fd(&descriptor, MAX_CONFIG_BYTES).unwrap_or_default()
}

pub fn load_json_document(plan: &mut WatchPlan, path: &Path) -> Option<Value> {
    let raw = read_capped(plan, path);
    if raw.is_empty() {
        return None;
    }
    serde_json::from_str::<Value>(&raw)
        .ok()
        .filter(Value::is_object)
}

pub fn load_toml_document(plan: &mut WatchPlan, path: &Path) -> Option<toml::Table> {
    let raw = read_capped(plan, path);
    if raw.is_empty() {
        return None;
    }
    raw.parse::<toml::Table>().ok()
}

pub fn safe_basename(value: &str) -> String {
    if value.trim().is_empty() || value.chars().count() > MAX_BASENAME_CHARS {
        return String::new();
    }
    if value == "." || value == ".." {
        return String::new();
    }
    if value.contains('/') || value.contains('\\') || value.contains('\0') {
        return String::new();
    }
    for character in value.chars() {
        if matches!(
            get_general_category(character),
            GeneralCategory::Control | GeneralCategory::Format | GeneralCategory::Surrogate
        ) {
            return String::new();
        }
    }
    value.to_string()
}

pub fn safe_basename_os(value: &OsStr) -> OsString {
    match value.to_str() {
        Some(text) => OsString::from(safe_basename(text)),
        None => OsString::new(),
    }
}

fn sorted(mut paths: Vec<PathBuf>) -> Vec<PathBuf> {
    paths.sort_by(|left, right| {
        left.as_os_str()
            .as_bytes()
            .cmp(right.as_os_str().as_bytes())
    });
    paths
}

pub fn bounded_directories(
    plan: &mut WatchPlan,
    root: &Path,
    limit: usize,
    follow_symlinks: bool,
) -> Vec<PathBuf> {
    plan.watch_path(root, true);
    let Ok(entries) = std::fs::read_dir(root) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = Vec::new();
    for (count, entry) in entries.enumerate() {
        if count >= MAX_DIR_ENTRIES || found.len() >= limit {
            break;
        }
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        let metadata = if follow_symlinks {
            std::fs::metadata(&path)
        } else {
            std::fs::symlink_metadata(&path)
        };
        if metadata.is_ok_and(|data| data.is_dir()) {
            found.push(path);
        }
    }
    sorted(found)
}

pub fn is_remote(value: &str) -> bool {
    let lowered = value.trim().to_lowercase();
    REMOTE_PREFIXES
        .iter()
        .any(|prefix| lowered.starts_with(prefix))
}

fn tail_match(candidate: &Path, pattern: &str) -> bool {
    let pattern = Path::new(pattern);
    let parts: Vec<&OsStr> = pattern
        .components()
        .map(|component| component.as_os_str())
        .collect();
    let values: Vec<&OsStr> = candidate
        .components()
        .map(|component| component.as_os_str())
        .collect();
    if parts.is_empty() || parts.len() > values.len() {
        return false;
    }
    if pattern.is_absolute() && parts.len() != values.len() {
        return false;
    }
    let offset = values.len() - parts.len();
    parts.iter().enumerate().all(|(index, part)| {
        let Some(text) = part.to_str() else {
            return false;
        };
        fnmatch_bytes(values[offset + index].as_bytes(), text)
    })
}

pub fn matches_any(path: &Path, patterns: &[String]) -> bool {
    let resolved = realpath_of(path);
    for pattern in patterns {
        let cleaned = pattern.trim();
        if cleaned.is_empty() {
            continue;
        }
        for candidate in [path, resolved.as_path()] {
            if tail_match(candidate, cleaned)
                || fnmatch_bytes(candidate.as_os_str().as_bytes(), cleaned)
            {
                return true;
            }
        }
    }
    false
}

pub fn summary_line(text: &str) -> String {
    let mut in_frontmatter = false;
    for (index, line) in text.lines().enumerate() {
        let stripped = line.trim();
        if index == 0 && stripped == "---" {
            in_frontmatter = true;
            continue;
        }
        if in_frontmatter {
            if stripped == "---" {
                in_frontmatter = false;
            }
            continue;
        }
        if stripped.is_empty() || stripped.starts_with("<!--") || stripped.starts_with("```") {
            continue;
        }
        let cleaned = stripped.trim_start_matches('#').trim();
        if !cleaned.is_empty() {
            return cleaned.chars().take(MAX_DETAIL_CHARS).collect();
        }
    }
    String::new()
}

fn wanted(name: &OsStr, suffixes: &[&str]) -> bool {
    let bytes = name.as_bytes();
    let Some(index) = bytes.iter().rposition(|byte| *byte == b'.') else {
        return false;
    };
    if index == 0 || index + 1 == bytes.len() {
        return false;
    }
    suffixes
        .iter()
        .any(|suffix| &bytes[index..] == suffix.as_bytes())
}

pub fn listed_files(plan: &mut WatchPlan, directory: &Path, suffixes: &[&str]) -> Vec<PathBuf> {
    plan.watch_path(directory, true);
    let Ok(entries) = std::fs::read_dir(directory) else {
        return Vec::new();
    };
    let mut found: Vec<PathBuf> = Vec::new();
    for (count, entry) in entries.enumerate() {
        if count >= MAX_DIR_ENTRIES {
            break;
        }
        let Ok(entry) = entry else { continue };
        let path = entry.path();
        if std::fs::metadata(&path).is_ok_and(|data| data.is_file())
            && wanted(&entry.file_name(), suffixes)
        {
            found.push(path);
        }
    }
    sorted(found)
}

struct Walk<'a> {
    plan: &'a mut WatchPlan,
    found: Vec<PathBuf>,
    visited: HashSet<(u64, u64)>,
    suffixes: &'a [&'a str],
    depth: usize,
}

fn directory_flags() -> OFlags {
    OFlags::RDONLY | OFlags::DIRECTORY | OFlags::NOFOLLOW | OFlags::CLOEXEC
}

fn names_in(descriptor: &rustix::fd::OwnedFd) -> Vec<OsString> {
    let Ok(duplicate) = rustix::io::dup(descriptor) else {
        return Vec::new();
    };
    let Ok(directory) = Dir::read_from(duplicate) else {
        return Vec::new();
    };
    let mut names: Vec<OsString> = Vec::new();
    for entry in directory {
        let Ok(entry) = entry else { continue };
        let name = entry.file_name().to_bytes();
        if name == b"." || name == b".." {
            continue;
        }
        if names.len() >= MAX_DIR_ENTRIES {
            break;
        }
        names.push(OsString::from_vec(name.to_vec()));
    }
    names
}

fn walk(walker: &mut Walk<'_>, descriptor: &rustix::fd::OwnedFd, current: &Path, level: usize) {
    if level > walker.depth || walker.found.len() >= MAX_DIR_ENTRIES {
        return;
    }
    walker.plan.watch_path(current, true);
    let Ok(opened) = rustix::fs::fstat(descriptor) else {
        return;
    };
    let key = (opened.st_dev, opened.st_ino);
    if !walker.visited.insert(key) {
        return;
    }
    for name in names_in(descriptor) {
        if walker.found.len() >= MAX_DIR_ENTRIES {
            break;
        }
        let child = current.join(&name);
        let Ok(entry) = rustix::fs::statat(descriptor, name.as_os_str(), AtFlags::SYMLINK_NOFOLLOW)
        else {
            continue;
        };
        if entry.st_mode & libc::S_IFMT == libc::S_IFDIR {
            let Ok(child_fd) = rustix::fs::openat(
                descriptor,
                name.as_os_str(),
                directory_flags(),
                Mode::empty(),
            ) else {
                continue;
            };
            let Ok(child_stat) = rustix::fs::fstat(&child_fd) else {
                continue;
            };
            if (entry.st_dev, entry.st_ino) != (child_stat.st_dev, child_stat.st_ino) {
                continue;
            }
            walk(walker, &child_fd, &child, level + 1);
            continue;
        }
        if rustix::fs::statat(descriptor, name.as_os_str(), AtFlags::empty())
            .is_ok_and(|data| data.st_mode & libc::S_IFMT == libc::S_IFREG)
            && wanted(&name, walker.suffixes)
        {
            walker.found.push(child);
        }
    }
}

pub fn walked_files(
    plan: &mut WatchPlan,
    directory: &Path,
    suffixes: &[&str],
    depth: usize,
) -> Vec<PathBuf> {
    plan.watch_path(directory, true);
    let Ok(root) = rustix::fs::open(directory, directory_flags(), Mode::empty()) else {
        return Vec::new();
    };
    let mut walker = Walk {
        plan,
        found: Vec::new(),
        visited: HashSet::new(),
        suffixes,
        depth,
    };
    walk(&mut walker, &root, directory, 0);
    sorted(walker.found)
}

pub fn ancestors_of(start: &Path, stop: Option<&Path>) -> Vec<PathBuf> {
    let chain: Vec<PathBuf> = start
        .ancestors()
        .take(MAX_ANCESTORS)
        .map(Path::to_path_buf)
        .collect();
    let Some(stop) = stop else {
        return chain;
    };
    let mut bounded: Vec<PathBuf> = Vec::new();
    for candidate in chain {
        let reached = candidate == stop;
        bounded.push(candidate);
        if reached {
            break;
        }
    }
    bounded
}

pub fn project_root(start: &Path, home: &Path) -> PathBuf {
    if start.as_os_str().is_empty() {
        return PathBuf::new();
    }
    let mut current = expanded_path(start);
    if current.is_file() {
        current = current.parent().map(Path::to_path_buf).unwrap_or(current);
    }
    let boundary = home.parent().map(Path::to_path_buf);
    for candidate in ancestors_of(&current, None) {
        if boundary.as_deref() == Some(candidate.as_path()) {
            break;
        }
        for marker in PROJECT_MARKERS {
            if candidate.join(marker).exists() {
                return candidate;
            }
        }
    }
    PathBuf::new()
}

fn escaped_chars(bytes: &[u8]) -> usize {
    let mut total = 0;
    let mut rest = bytes;
    loop {
        match std::str::from_utf8(rest) {
            Ok(text) => return total + text.chars().count(),
            Err(error) => {
                let valid = error.valid_up_to();
                total += std::str::from_utf8(&rest[..valid])
                    .unwrap_or_default()
                    .chars()
                    .count();
                let skipped = error.error_len().unwrap_or(rest.len() - valid);
                total += skipped;
                rest = &rest[valid + skipped..];
            }
        }
    }
}

pub fn env_path_value(name: &str, environ: &Environ) -> OsString {
    let Some((_, value)) = environ.iter().find(|(key, _)| key == name) else {
        return OsString::new();
    };
    let bytes = value.as_bytes();
    if bytes.is_empty() || bytes.contains(&0) || escaped_chars(bytes) > MAX_ENV_PATH_CHARS {
        return OsString::new();
    }
    value.clone()
}

pub fn env_path(name: &str, environ: &Environ) -> Option<PathBuf> {
    let value = env_path_value(name, environ);
    if value.is_empty() {
        None
    } else {
        Some(expanded_os(&value))
    }
}
