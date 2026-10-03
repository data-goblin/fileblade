use crate::command::{CommandSpec, which};
use crate::common::{parse_path, path_text};
use crate::secure::read_bounded_nofollow;
use serde_json::{Value, json};
use std::path::Path;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

pub const PREVIEW_BYTES: usize = 256 * 1024;
pub const PREVIEW_LINES: usize = 120;
const PREVIEW_OUTPUT_BYTES: usize = 4 * 1024 * 1024;
const PREVIEW_TIMEOUT: Duration = Duration::from_secs(5);

pub fn preview(raw_path: &str, lines: usize, cancelled: &AtomicBool) -> Value {
    let path = match parse_path(raw_path) {
        Ok(path) => path,
        Err(error) => return failure(raw_path, &error.to_string()),
    };
    let text = path_text(&path);
    let lines = lines.clamp(1, 2000);
    let bytes = match read_bounded_nofollow(&path, PREVIEW_BYTES) {
        Ok(Some(bytes)) => bytes,
        Ok(None) => return failure(&text, "file is missing"),
        Err(error) => return failure(&text, &read_refusal(&error)),
    };
    if bytes.iter().take(8192).any(|byte| *byte == 0) {
        return json!({"ok": true, "path": text, "backend": "none", "binary": true, "truncated": false, "lines": []});
    }
    let plain = String::from_utf8_lossy(&bytes);
    let total = plain.lines().count();
    let (backend, rendered) = match highlighted(&path, lines, cancelled) {
        Some(output) => ("bat", output),
        None => (
            "plain",
            plain
                .lines()
                .take(lines)
                .map(|line| vec![run(line, -1, false)])
                .collect(),
        ),
    };
    json!({
        "ok": true,
        "path": text,
        "backend": backend,
        "binary": false,
        "truncated": total > lines,
        "lines": rendered
    })
}

fn failure(path: &str, error: &str) -> Value {
    json!({"ok": false, "path": path, "backend": "none", "binary": false, "truncated": false, "lines": [], "error": error})
}

pub(crate) fn read_refusal(error: &std::io::Error) -> String {
    if error.raw_os_error() == Some(libc::ELOOP) {
        "Files reached through a link open externally to avoid following a link inside the shell."
            .into()
    } else {
        error.to_string()
    }
}

fn run(text: &str, color: i32, bold: bool) -> Value {
    json!({"text": text, "color": color, "bold": bold})
}

fn highlighted(path: &Path, lines: usize, cancelled: &AtomicBool) -> Option<Vec<Vec<Value>>> {
    let bat = which("bat")?;
    let output = CommandSpec::new(bat)
        .args([
            "--color=always",
            "--style=plain",
            "--paging=never",
            "--wrap=never",
            "--theme=ansi",
            &format!("--line-range=:{lines}"),
            "--",
        ])
        .args([path])
        .timeout(PREVIEW_TIMEOUT)
        .limits(PREVIEW_OUTPUT_BYTES, 64 * 1024)
        .run_cancellable(cancelled)
        .ok()
        .filter(|output| output.status.success() && !output.stdout_truncated)?;
    let text = String::from_utf8_lossy(&output.stdout);
    Some(text.lines().map(parse_line).collect())
}

fn parse_line(line: &str) -> Vec<Value> {
    let mut runs = Vec::new();
    let mut color = -1;
    let mut bold = false;
    let mut buffer = String::new();
    let mut rest = line;
    while let Some(start) = rest.find("\u{1b}[") {
        buffer.push_str(&rest[..start]);
        let after = &rest[start + 2..];
        let Some(end) = after.find('m') else {
            break;
        };
        if !buffer.is_empty() {
            runs.push(run(&buffer, color, bold));
            buffer.clear();
        }
        for code in after[..end]
            .split(';')
            .filter_map(|value| value.parse::<i32>().ok())
        {
            match code {
                0 => {
                    color = -1;
                    bold = false;
                }
                1 => bold = true,
                22 => bold = false,
                30..=37 => color = code - 30,
                39 => color = -1,
                90..=97 => color = code - 90 + 8,
                _ => {}
            }
        }
        rest = &after[end + 1..];
    }
    buffer.push_str(rest);
    if !buffer.is_empty() || runs.is_empty() {
        runs.push(run(&buffer, color, bold));
    }
    runs
}
