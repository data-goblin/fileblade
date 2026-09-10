use crate::command::{CommandSpec, which};
use crate::common::{CONTROL_TIMEOUT, display_path, parse_path, path_text};
use crate::filesystem::entry_for_path;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::Path;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

pub const MAX_QUICKNAV_BYTES: usize = 1024 * 1024;
const MAX_QUERY_BYTES: usize = 4096;
const QUICKNAV_INDEX_DEADLINE: Duration = Duration::from_secs(4);
const QUICKNAV_INDEX_LIMIT: usize = 4000;
const ZOXIDE_CACHE_TTL: Duration = Duration::from_secs(10);

type ZoxideListing = (Instant, Vec<(String, f64)>);

fn zoxide_cache() -> MutexGuard<'static, Option<ZoxideListing>> {
    static CACHE: OnceLock<Mutex<Option<ZoxideListing>>> = OnceLock::new();
    CACHE
        .get_or_init(|| Mutex::new(None))
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn quicknav(query: &str, exclude: &str, limit: usize) -> Value {
    quicknav_from_root(query, exclude, "~", false, limit)
}

pub fn quicknav_from_root(
    query: &str,
    exclude: &str,
    root: &str,
    show_hidden: bool,
    limit: usize,
) -> Value {
    quicknav_cancellable(
        query,
        exclude,
        root,
        show_hidden,
        limit,
        &AtomicBool::new(false),
    )
}

pub fn quicknav_cancellable(
    query: &str,
    exclude: &str,
    root: &str,
    show_hidden: bool,
    limit: usize,
    cancelled: &AtomicBool,
) -> Value {
    let needle = query.trim();
    if cancelled.load(Ordering::Relaxed) {
        return quicknav_error(needle, "operation cancelled");
    }
    if needle.len() > MAX_QUERY_BYTES {
        return quicknav_error(needle, "Quick navigation query exceeds 4096 bytes");
    }
    let zoxide = zoxide_listing(cancelled);
    let discovered = directory_listing(root, show_hidden, needle, cancelled);
    if let (Err(zoxide_error), Err(discovery_error)) = (&zoxide, &discovered) {
        return quicknav_error(needle, &format!("{discovery_error}; {zoxide_error}"));
    }
    let excluded = if exclude.trim().is_empty() {
        None
    } else {
        match parse_path(exclude) {
            Ok(path) => Some(path_text(&path)),
            Err(error) => return quicknav_error(needle, &error.to_string()),
        }
    };
    let mut frequencies = HashMap::new();
    for (path, frequency) in zoxide.unwrap_or_default() {
        if excluded.as_deref() != Some(path.as_str()) {
            frequencies.insert(path, frequency);
        }
    }
    for path in discovered.unwrap_or_default() {
        if excluded.as_deref() != Some(path.as_str()) {
            frequencies.entry(path).or_insert(0.0);
        }
    }
    let visibility_root = match parse_path(root) {
        Ok(path) => path,
        Err(error) => return quicknav_error(needle, &error.to_string()),
    };
    let mut visibility = crate::visibility::Visibility::new(&visibility_root, show_hidden);
    let candidates = frequencies.into_iter();
    let mut ranked = if needle.is_empty() {
        candidates
            .map(|(path, frequency)| (path, frequency, 0, Vec::new()))
            .collect::<Vec<_>>()
    } else {
        let pattern = crate::index::parse_pattern(needle, crate::index::case_matching(false));
        let mut matcher = nucleo::Matcher::new(crate::index::matcher_config());
        candidates
            .filter_map(|(path, frequency)| {
                let text = display_path(&parse_path(&path).ok()?);
                crate::index::score_path_name(&pattern, &text, &mut matcher)
                    .map(|(score, indices)| (path, frequency, score, indices))
            })
            .collect()
    };
    ranked.sort_by(|left, right| {
        right
            .2
            .cmp(&left.2)
            .then_with(|| right.1.total_cmp(&left.1))
            .then_with(|| left.0.cmp(&right.0))
    });
    let entries = ranked
        .into_iter()
        .take_while(|_| !cancelled.load(Ordering::Relaxed))
        .filter(|(path, _, _, _)| parse_path(path).is_ok_and(|path| visibility.path(&path)))
        .filter_map(|(path, frequency, _, indices)| quicknav_row(&path, frequency, &indices))
        .take(limit.clamp(1, 200))
        .collect::<Vec<_>>();
    json!({
        "ok": true,
        "query": needle,
        "backend": "zoxide",
        "entries": entries
    })
}

fn directory_listing(
    raw_root: &str,
    show_hidden: bool,
    query: &str,
    cancelled: &AtomicBool,
) -> Result<Vec<String>, String> {
    let root = parse_path(raw_root).map_err(|error| error.to_string())?;
    if !root.is_dir() {
        return Err(format!("{} is not a directory", path_text(&root)));
    }
    let shared = crate::index::acquire(&root, show_hidden, true);
    let mut index = shared
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner());
    index.set_pattern(query, crate::index::case_matching(false));
    let started = Instant::now();
    loop {
        let status = index.tick(20);
        let walk = index.status();
        if cancelled.load(Ordering::Relaxed) {
            return Err("operation cancelled".to_string());
        }
        let settled = !status.running && !walk.running && index.indexed() >= walk.walked;
        if settled || started.elapsed() >= QUICKNAV_INDEX_DEADLINE {
            let error = index.error();
            if !error.is_empty() && index.indexed() == 0 && !walk.running {
                return Err(error);
            }
            break;
        }
        if !status.running && walk.running {
            thread::sleep(Duration::from_millis(12));
        }
    }
    let mut accept = |_: &str, flags: &crate::index::IndexFlags| flags.is_dir || flags.is_symlink;
    let mut paths = index
        .hits(QUICKNAV_INDEX_LIMIT, &mut accept)
        .into_iter()
        .map(|hit| path_text(&hit.entry.path(&root)))
        .collect::<Vec<_>>();
    paths.push(path_text(&root));
    Ok(paths)
}

fn zoxide_listing(cancelled: &AtomicBool) -> Result<Vec<(String, f64)>, String> {
    let mut cache = zoxide_cache();
    if let Some((built, rows)) = cache
        .as_ref()
        .filter(|(built, _)| built.elapsed() < ZOXIDE_CACHE_TTL)
    {
        let _ = built;
        return Ok(rows.clone());
    }
    let zoxide = which("zoxide").ok_or_else(|| "zoxide is not installed".to_string())?;
    let output = CommandSpec::new(zoxide)
        .args(["query", "--list", "--score"])
        .timeout(CONTROL_TIMEOUT)
        .limits(MAX_QUICKNAV_BYTES, 64 * 1024)
        .run_cancellable(cancelled)
        .map_err(|error| error.to_string())?;
    if output.stdout_truncated {
        return Err("zoxide results exceed 1 MiB".to_string());
    }
    if !matches!(output.status.code(), Some(0 | 1)) {
        return Err(format!(
            "zoxide exited with {}",
            output.status.code().unwrap_or(-1)
        ));
    }
    let rows = output
        .stdout
        .split(|byte| *byte == b'\n')
        .take(65_536)
        .filter_map(|line| {
            let line = line.trim_ascii_start();
            let split = line.iter().position(u8::is_ascii_whitespace)?;
            let score = std::str::from_utf8(&line[..split]).ok()?;
            let path = Path::new(OsStr::from_bytes(&line[split + 1..]));
            (valid_score(score) && path.is_absolute() && path.as_os_str().len() <= 16 * 1024)
                .then(|| (path_text(path), score.parse().unwrap_or(0.0)))
        })
        .collect::<Vec<_>>();
    *cache = Some((Instant::now(), rows.clone()));
    Ok(rows)
}

fn quicknav_row(path: &str, frequency: f64, indices: &[u32]) -> Option<Value> {
    let target = parse_path(path).ok()?;
    if !target.is_dir() {
        return None;
    }
    let mut item = entry_for_path(&target, false).ok()?;
    let display = display_path(&target);
    let path = display.as_str();
    let name_offset = path
        .rfind('/')
        .map_or(0, |slash| path[..=slash].chars().count());
    item["relative"] = json!(path);
    item["score"] = json!(frequency);
    item["relative_spans"] = json!(crate::index::spans_text(indices, 0));
    item["name_spans"] = json!(crate::index::spans_text(indices, name_offset));
    Some(item)
}

pub fn record_zoxide_visit_cancellable(raw_path: &str, cancelled: &AtomicBool) -> Value {
    let path = match parse_path(raw_path) {
        Ok(path) => path,
        Err(error) => return crate::common::path_error(raw_path, &error),
    };
    if cancelled.load(Ordering::Relaxed) {
        return json!({"ok": false, "path": path_text(&path), "error": "operation cancelled"});
    }
    if !path.is_dir() {
        return json!({"ok": false, "path": path_text(&path), "error": "Not a directory"});
    }
    let Some(zoxide) = which("zoxide") else {
        return json!({"ok": false, "path": path_text(&path), "error": "zoxide is not installed"});
    };
    let result = CommandSpec::new(zoxide)
        .args(["add", "--"])
        .args([&path])
        .timeout(Duration::from_secs(2))
        .limits(16 * 1024, 64 * 1024)
        .run_cancellable(cancelled);
    match result {
        Ok(output) => {
            let error = if output.status.success() {
                String::new()
            } else {
                let detail = String::from_utf8_lossy(&output.stderr).trim().to_string();
                if detail.is_empty() {
                    "Unable to update zoxide".to_string()
                } else {
                    detail
                }
            };
            if output.status.success() {
                *zoxide_cache() = None;
            }
            json!({"ok": output.status.success(), "path": path_text(&path), "error": error})
        }
        Err(error) => json!({"ok": false, "path": path_text(&path), "error": error.to_string()}),
    }
}

fn valid_score(value: &str) -> bool {
    let mut dot = false;
    let mut before = 0_usize;
    let mut after = 0_usize;
    for byte in value.bytes() {
        if byte == b'.' && !dot {
            dot = true;
        } else if byte.is_ascii_digit() {
            if dot {
                after += 1;
            } else {
                before += 1;
            }
        } else {
            return false;
        }
    }
    before > 0 && (!dot || after > 0)
}

fn quicknav_error(query: &str, error: &str) -> Value {
    json!({"ok": false, "query": query, "backend": "zoxide", "entries": [], "error": error})
}
