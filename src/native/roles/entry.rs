use crate::secure::{self, FileVersion};
use crate::{AppError, AppResult};
use base64::{Engine, prelude::BASE64_STANDARD};
use std::io;
use std::path::{Path, PathBuf};

pub const FILE_BYTES: usize = 1024 * 1024;
pub const MARKER: &str = "-- fileblade desktop role";

pub fn exec_path(path: &Path) -> String {
    let quoted = path
        .to_string_lossy()
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('`', "\\`")
        .replace('$', "\\$");
    format!("\"{quoted}\"")
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

#[derive(Clone, Debug)]
pub struct Planned {
    pub path: PathBuf,
    pub key: Option<String>,
    pub owned: String,
}

impl Planned {
    pub fn whole(path: PathBuf, owned: String) -> Self {
        Self {
            path,
            key: None,
            owned,
        }
    }

    pub fn ini(path: PathBuf, section: &str, key: &str, value: &str) -> Self {
        Self {
            path,
            key: Some(format!("[{section}]{key}")),
            owned: value.to_string(),
        }
    }

    pub fn marker(path: PathBuf, line: String) -> Self {
        Self {
            path,
            key: Some(MARKER.to_string()),
            owned: line,
        }
    }
}

enum Kind<'a> {
    Whole,
    Ini { section: &'a str, key: &'a str },
    Marker,
}

fn kind(key: Option<&str>) -> AppResult<Kind<'_>> {
    match key {
        None => Ok(Kind::Whole),
        Some(MARKER) => Ok(Kind::Marker),
        Some(text) => text
            .strip_prefix('[')
            .and_then(|rest| rest.split_once(']'))
            .filter(|(section, key)| !section.is_empty() && !key.is_empty())
            .map(|(section, key)| Kind::Ini { section, key })
            .ok_or_else(|| AppError::invalid(format!("unknown desktop role entry key {text}"))),
    }
}

fn snapshot(path: &Path) -> AppResult<Option<(FileVersion, String)>> {
    let bytes = match secure::read_bounded_nofollow(path, FILE_BYTES) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return Ok(None),
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let stat = secure::entry_stat(path)?;
    let text = String::from_utf8(bytes.clone())
        .map_err(|_| AppError::invalid(format!("{} is not UTF-8 text", path.display())))?;
    Ok(Some((
        FileVersion {
            dev: stat.dev,
            ino: stat.ino,
            data: BASE64_STANDARD.encode(bytes),
        },
        text,
    )))
}

pub fn current(path: &Path, key: Option<&str>) -> AppResult<Option<String>> {
    let Some((_, text)) = snapshot(path)? else {
        return Ok(None);
    };
    Ok(match kind(key)? {
        Kind::Whole => Some(text),
        Kind::Ini { section, key } => ini_get(&text, section, key),
        Kind::Marker => marker_get(&text),
    })
}

pub fn write(path: &Path, key: Option<&str>, value: Option<&str>) -> AppResult<()> {
    let found = snapshot(path)?;
    let text = match (kind(key)?, value) {
        (Kind::Whole, Some(value)) => value.to_string(),
        (Kind::Whole, None) => {
            if let Some((version, _)) = &found {
                let identity = secure::EntryIdentity {
                    dev: version.dev,
                    ino: version.ino,
                    kind: secure::EntryKind::File,
                };
                secure::remove_nondirectory_matching(path, identity)?;
            }
            return Ok(());
        }
        (Kind::Ini { section, key }, value) => ini_set(
            found.as_ref().map_or("", |(_, text)| text),
            section,
            key,
            value,
        ),
        (Kind::Marker, value) => marker_set(found.as_ref().map_or("", |(_, text)| text), value),
    };
    if let Some(parent) = path.parent() {
        secure::ensure_directories(parent, 0o755)?;
    }
    let target = secure::resolved_parent(path)?;
    secure::write_config_expected(
        target,
        found.as_ref().map(|(version, _)| version),
        text.as_bytes(),
        FILE_BYTES,
    )?;
    Ok(())
}

fn is_header(line: &str) -> bool {
    let line = line.trim();
    line.starts_with('[') && line.ends_with(']')
}

fn in_section<'a>(text: &'a str, section: &str) -> impl Iterator<Item = (usize, &'a str)> {
    let header = format!("[{section}]");
    let mut inside = false;
    text.lines().enumerate().filter(move |(_, line)| {
        if is_header(line) {
            inside = line.trim() == header;
            return false;
        }
        inside
    })
}

fn key_of(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once('=')?;
    Some((key.trim(), value.trim()))
}

pub fn ini_get(text: &str, section: &str, key: &str) -> Option<String> {
    in_section(text, section)
        .filter_map(|(_, line)| key_of(line))
        .find(|(found, _)| *found == key)
        .map(|(_, value)| value.to_string())
}

pub fn ini_set(text: &str, section: &str, key: &str, value: Option<&str>) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let header = format!("[{section}]");
    let existing = in_section(text, section)
        .find(|(_, line)| key_of(line).is_some_and(|(found, _)| found == key));
    match (existing, value) {
        (Some((index, _)), Some(value)) => lines[index] = format!("{key}={value}"),
        (Some((index, _)), None) => {
            lines.remove(index);
            let start = lines.iter().position(|line| line.trim() == header);
            if let Some(start) = start
                && lines[start + 1..]
                    .iter()
                    .take_while(|line| !is_header(line))
                    .all(|line| line.trim().is_empty())
            {
                let end = lines[start + 1..]
                    .iter()
                    .position(|line| is_header(line))
                    .map_or(lines.len(), |offset| start + 1 + offset);
                lines.drain(start..end);
            }
        }
        (None, Some(value)) => {
            let line = format!("{key}={value}");
            match lines.iter().position(|line| line.trim() == header) {
                Some(start) => {
                    let end = lines[start + 1..]
                        .iter()
                        .position(|line| is_header(line))
                        .map_or(lines.len(), |offset| start + 1 + offset);
                    lines.insert(end, line);
                }
                None => {
                    lines.push(header);
                    lines.push(line);
                }
            }
        }
        (None, None) => {}
    }
    join(lines)
}

pub fn marker_get(text: &str) -> Option<String> {
    text.lines()
        .find(|line| line.trim_end().ends_with(MARKER))
        .map(str::to_string)
}

pub fn marker_set(text: &str, value: Option<&str>) -> String {
    let mut lines: Vec<String> = text.lines().map(str::to_string).collect();
    let existing = lines
        .iter()
        .position(|line| line.trim_end().ends_with(MARKER));
    match (existing, value) {
        (Some(index), Some(value)) => lines[index] = value.to_string(),
        (Some(index), None) => {
            lines.remove(index);
        }
        (None, Some(value)) => lines.push(value.to_string()),
        (None, None) => {}
    }
    join(lines)
}

fn join(lines: Vec<String>) -> String {
    if lines.is_empty() {
        return String::new();
    }
    let mut text = lines.join("\n");
    text.push('\n');
    text
}
