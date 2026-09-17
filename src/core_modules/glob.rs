use super::watch::WatchPlan;
use std::collections::HashSet;
use std::collections::VecDeque;
use std::os::unix::ffi::OsStrExt;
use std::path::{Component, Path, PathBuf};

pub const MAX_GLOB_MATCHES: usize = 64;
pub const MAX_DISCOVERY_DIRS: usize = 200;
pub const MAX_DIR_ENTRIES: usize = 512;
const MAX_PATTERN_PARTS: usize = 32;

pub fn fnmatch_case(value: &str, pattern: &str) -> bool {
    fnmatch_bytes(value.as_bytes(), pattern)
}

pub fn fnmatch_bytes(value: &[u8], pattern: &str) -> bool {
    let value = code_points(value);
    let pattern: Vec<u32> = pattern.chars().map(u32::from).collect();
    matches(&value, &pattern)
}

pub fn code_points(bytes: &[u8]) -> Vec<u32> {
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        match std::str::from_utf8(&bytes[index..]) {
            Ok(text) => {
                out.extend(text.chars().map(u32::from));
                break;
            }
            Err(error) => {
                let valid = error.valid_up_to();
                if let Ok(text) = std::str::from_utf8(&bytes[index..index + valid]) {
                    out.extend(text.chars().map(u32::from));
                }
                let skipped = error
                    .error_len()
                    .unwrap_or(bytes.len() - index - valid)
                    .max(1);
                let stop = (index + valid + skipped).min(bytes.len());
                for byte in &bytes[index + valid..stop] {
                    out.push(0xdc00 + u32::from(*byte));
                }
                index = stop;
            }
        }
    }
    out
}

const STAR: u32 = '*' as u32;
const QUESTION: u32 = '?' as u32;
const OPEN: u32 = '[' as u32;
const CLOSE: u32 = ']' as u32;
const BANG: u32 = '!' as u32;
const DASH: u32 = '-' as u32;

fn matches(value: &[u32], pattern: &[u32]) -> bool {
    let Some((first, rest)) = pattern.split_first() else {
        return value.is_empty();
    };
    match *first {
        STAR => {
            let rest = match rest.iter().position(|character| *character != STAR) {
                Some(offset) => &rest[offset..],
                None => return true,
            };
            (0..=value.len()).any(|index| matches(&value[index..], rest))
        }
        QUESTION => !value.is_empty() && matches(&value[1..], rest),
        OPEN => {
            let Some((set, tail)) = bracket(rest) else {
                return !value.is_empty() && value[0] == OPEN && matches(&value[1..], rest);
            };
            !value.is_empty() && in_set(value[0], set) && matches(&value[1..], tail)
        }
        character => !value.is_empty() && value[0] == character && matches(&value[1..], rest),
    }
}

fn bracket(pattern: &[u32]) -> Option<(&[u32], &[u32])> {
    let start = if pattern.first() == Some(&BANG) { 1 } else { 0 };
    let start = if pattern.get(start) == Some(&CLOSE) {
        start + 1
    } else {
        start
    };
    let offset = pattern[start..]
        .iter()
        .position(|character| *character == CLOSE)?;
    let close = start + offset;
    Some((&pattern[..close], &pattern[close + 1..]))
}

fn in_set(character: u32, set: &[u32]) -> bool {
    let (negated, set) = match set.split_first() {
        Some((&BANG, rest)) => (true, rest),
        _ => (false, set),
    };
    let mut index = 0;
    let mut hit = false;
    while index < set.len() {
        if index + 2 < set.len() && set[index + 1] == DASH {
            if set[index] <= character && character <= set[index + 2] {
                hit = true;
            }
            index += 3;
        } else {
            if set[index] == character {
                hit = true;
            }
            index += 1;
        }
    }
    hit != negated
}

fn realpath_of(path: &Path) -> PathBuf {
    std::fs::canonicalize(path).unwrap_or_else(|_| crate::common::normalize_path(path))
}

fn parts_of(pattern: &str) -> Option<Vec<String>> {
    let mut parts = Vec::new();
    for component in Path::new(pattern).components() {
        match component {
            Component::Normal(name) => parts.push(name.to_string_lossy().into_owned()),
            Component::CurDir => continue,
            _ => return None,
        }
    }
    Some(parts)
}

pub fn bounded_glob(plan: &mut WatchPlan, base: &Path, pattern: &str) -> Vec<PathBuf> {
    plan.watch_path(base, true);
    let cleaned = pattern.trim();
    if cleaned.is_empty()
        || cleaned.starts_with('/')
        || cleaned.starts_with('~')
        || cleaned.contains("..")
    {
        return Vec::new();
    }
    let Some(parts) = parts_of(cleaned) else {
        return Vec::new();
    };
    if parts.len() > MAX_PATTERN_PARTS
        || parts.iter().any(|part| part.contains("**") && part != "**")
    {
        return Vec::new();
    }
    let mut found: HashSet<PathBuf> = HashSet::new();
    let root = realpath_of(base);
    let mut pending: VecDeque<(PathBuf, usize)> = VecDeque::new();
    pending.push_back((base.to_path_buf(), 0));
    let mut visited: HashSet<(PathBuf, usize)> = HashSet::new();
    while !pending.is_empty()
        && visited.len() < MAX_DISCOVERY_DIRS
        && found.len() < MAX_GLOB_MATCHES
    {
        let (directory, index) = pending.pop_front().expect("pending entry");
        if index >= parts.len() {
            continue;
        }
        let resolved = realpath_of(&directory);
        let key = (resolved.clone(), index);
        if (resolved != root && !resolved.starts_with(&root)) || visited.contains(&key) {
            continue;
        }
        visited.insert(key);
        plan.watch_path(&directory, true);
        let part = parts[index].as_str();
        if part == "**" {
            pending.push_back((directory.clone(), index + 1));
        }
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for (count, entry) in entries.enumerate() {
            if count >= MAX_DIR_ENTRIES || found.len() >= MAX_GLOB_MATCHES {
                break;
            }
            let Ok(entry) = entry else { continue };
            let name = entry.file_name();
            let candidate = directory.join(&name);
            if part != "**" && !fnmatch_bytes(name.as_bytes(), part) {
                continue;
            }
            let kind = std::fs::metadata(&candidate).ok();
            if part != "**"
                && index == parts.len() - 1
                && kind.as_ref().is_some_and(|metadata| metadata.is_file())
            {
                let target = realpath_of(&candidate);
                if target
                    .parent()
                    .is_some_and(|parent| parent.starts_with(&root))
                {
                    found.insert(candidate);
                }
            } else if kind.as_ref().is_some_and(|metadata| metadata.is_dir())
                && pending.len() + visited.len() < MAX_DISCOVERY_DIRS
            {
                pending.push_back((candidate, if part == "**" { index } else { index + 1 }));
            }
        }
    }
    let mut result: Vec<PathBuf> = found.into_iter().collect();
    result.sort_by(|left, right| {
        left.as_os_str()
            .as_bytes()
            .cmp(right.as_os_str().as_bytes())
    });
    result
}
