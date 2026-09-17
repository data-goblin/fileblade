use crate::AppResult;
use crate::command::{CommandSpec, which};
use crate::common::display_path;
use base64::Engine as _;
use base64::prelude::BASE64_STANDARD;
use serde_json::{Value, json};
use std::ffi::OsString;
use std::os::unix::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

pub const GREP_FILE_CAP: usize = 200;
pub const GREP_HIT_CAP: usize = 1000;
const GREP_OUTPUT_BYTES: usize = 8 * 1024 * 1024;
const GREP_HITS_PER_FILE: &str = "5";
const GREP_MAX_FILESIZE: &str = "2M";
const GREP_SNIPPET_BEFORE: usize = 48;
const GREP_SNIPPET_CHARS: usize = 200;

pub struct GrepRequest<'a> {
    pub root: &'a Path,
    pub needle: &'a str,
    pub regex: bool,
    pub case_sensitive: bool,
    pub show_hidden: bool,
    pub globs: &'a [String],
    pub timeout: Duration,
}

#[derive(Clone, Debug, Default)]
pub struct GrepFile {
    pub relative: String,
    pub native: PathBuf,
    pub hits: Vec<Value>,
}

pub fn available() -> bool {
    which("rg").is_some()
}

pub fn grep(request: &GrepRequest<'_>, cancelled: &AtomicBool) -> AppResult<Vec<GrepFile>> {
    let rg = which("rg").ok_or_else(|| crate::AppError::command("rg is not installed"))?;
    let mut arguments = vec![
        "--json".to_string(),
        "--no-messages".to_string(),
        "--max-count".to_string(),
        GREP_HITS_PER_FILE.to_string(),
        "--max-filesize".to_string(),
        GREP_MAX_FILESIZE.to_string(),
    ];
    arguments.push(
        if request.case_sensitive {
            "--case-sensitive"
        } else {
            "--ignore-case"
        }
        .to_string(),
    );
    if !request.regex {
        arguments.push("--fixed-strings".to_string());
    }
    if request.show_hidden {
        arguments.push("--hidden".to_string());
    }
    for glob in request.globs {
        arguments.extend(["--glob".to_string(), glob.clone()]);
    }
    arguments.extend([
        "-e".to_string(),
        request.needle.to_string(),
        "--".to_string(),
        ".".to_string(),
    ]);
    let output = CommandSpec::new(rg)
        .args(arguments)
        .cwd(request.root)
        .timeout(request.timeout)
        .limits(GREP_OUTPUT_BYTES, 64 * 1024)
        .run_cancellable(cancelled)?;
    if !matches!(output.status.code(), Some(0..=2)) {
        return Err(crate::AppError::command(format!(
            "rg exited with {}",
            output.status.code().unwrap_or(-1)
        )));
    }
    let mut visibility = crate::visibility::Visibility::new(request.root, request.show_hidden);
    Ok(parse(
        &String::from_utf8_lossy(&output.stdout),
        &mut |path| visibility.path(&request.root.join(path)),
    ))
}

fn parse(text: &str, visible: &mut dyn FnMut(&Path) -> bool) -> Vec<GrepFile> {
    let mut files: Vec<GrepFile> = Vec::new();
    let mut hits = 0_usize;
    for line in text.lines() {
        let Ok(message) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        let data = &message["data"];
        let native = if let Some(text) = data["path"]["text"].as_str() {
            PathBuf::from(text)
        } else if let Some(bytes) = data["path"]["bytes"]
            .as_str()
            .and_then(|text| BASE64_STANDARD.decode(text).ok())
        {
            PathBuf::from(OsString::from_vec(bytes))
        } else {
            continue;
        };
        let native = native.strip_prefix(".").unwrap_or(&native).to_path_buf();
        if !visible(&native) {
            continue;
        }
        let relative = display_path(&native);
        match message["type"].as_str() {
            Some("begin") => {
                if files.len() >= GREP_FILE_CAP || hits >= GREP_HIT_CAP {
                    break;
                }
                files.push(GrepFile {
                    relative,
                    native,
                    hits: Vec::new(),
                });
            }
            Some("match") => {
                if let Some(file) = files.last_mut().filter(|file| file.native == native) {
                    file.hits.push(hit(data));
                    hits += 1;
                }
            }
            _ => {}
        }
    }
    files.retain(|file| !file.hits.is_empty());
    files
}

fn hit(data: &Value) -> Value {
    let text = data["lines"]["text"]
        .as_str()
        .unwrap_or("")
        .trim_end_matches(['\n', '\r']);
    let first = data["submatches"][0]["start"].as_u64().unwrap_or(0) as usize;
    let skip = char_index(text, first).saturating_sub(GREP_SNIPPET_BEFORE);
    let snippet = text
        .chars()
        .skip(skip)
        .take(GREP_SNIPPET_CHARS)
        .collect::<String>();
    let spans = data["submatches"]
        .as_array()
        .map(|matches| {
            matches
                .iter()
                .filter_map(|found| {
                    let start = char_index(text, found["start"].as_u64()? as usize);
                    let end = char_index(text, found["end"].as_u64()? as usize);
                    let clipped = (
                        start.saturating_sub(skip),
                        end.saturating_sub(skip).min(GREP_SNIPPET_CHARS),
                    );
                    (clipped.1 > clipped.0).then(|| format!("{}-{}", clipped.0, clipped.1))
                })
                .collect::<Vec<_>>()
                .join(",")
        })
        .unwrap_or_default();
    json!({
        "line": data["line_number"].as_u64().unwrap_or(0),
        "column": char_index(text, first) + 1,
        "text": snippet.trim_end(),
        "spans": spans
    })
}

fn char_index(text: &str, byte: usize) -> usize {
    text.char_indices()
        .take_while(|(index, _)| *index < byte)
        .count()
}
