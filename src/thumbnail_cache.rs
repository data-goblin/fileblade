use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

pub const CAPACITY_BYTES: u64 = 256 * 1024 * 1024;
pub const RETAINED_BYTES: u64 = 192 * 1024 * 1024;
const SCAN_INTERVAL: Duration = Duration::from_secs(10 * 60);
const ABANDONED_TEMPORARY: Duration = Duration::from_secs(24 * 60 * 60);

static RUNNING: AtomicBool = AtomicBool::new(false);
static LAST_SCAN: AtomicI64 = AtomicI64::new(i64::MIN);

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Eviction {
    pub examined: usize,
    pub removed: usize,
    pub bytes_before: u64,
    pub bytes_after: u64,
}

struct Entry {
    path: PathBuf,
    bytes: u64,
    used: i64,
}

fn now_seconds() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs() as i64)
        .unwrap_or(0)
}

fn cached_name(name: &str) -> bool {
    name.strip_suffix(".png")
        .is_some_and(|stem| stem.len() == 32 && stem.bytes().all(|byte| byte.is_ascii_hexdigit()))
}

fn temporary_name(name: &str) -> bool {
    name.starts_with(".fileblade-") && name.ends_with(".tmp")
}

pub fn evict(directory: &Path, capacity: u64, retained: u64) -> std::io::Result<Eviction> {
    let now = now_seconds();
    let mut entries = Vec::new();
    let mut eviction = Eviction::default();
    for item in std::fs::read_dir(directory)? {
        let item = item?;
        let Ok(metadata) = item.metadata() else {
            continue;
        };
        if !metadata.file_type().is_file() {
            continue;
        }
        let name = item.file_name();
        let name = name.to_string_lossy();
        let used = metadata.atime().max(metadata.mtime());
        if temporary_name(&name) {
            if now.saturating_sub(metadata.mtime()) >= ABANDONED_TEMPORARY.as_secs() as i64
                && std::fs::remove_file(item.path()).is_ok()
            {
                eviction.removed += 1;
            }
            continue;
        }
        if !cached_name(&name) {
            continue;
        }
        eviction.examined += 1;
        eviction.bytes_before += metadata.len();
        entries.push(Entry {
            path: item.path(),
            bytes: metadata.len(),
            used,
        });
    }
    eviction.bytes_after = eviction.bytes_before;
    if eviction.bytes_before <= capacity {
        return Ok(eviction);
    }
    entries.sort_by_key(|entry| entry.used);
    for entry in entries {
        if eviction.bytes_after <= retained {
            break;
        }
        if std::fs::remove_file(&entry.path).is_ok() {
            eviction.removed += 1;
            eviction.bytes_after = eviction.bytes_after.saturating_sub(entry.bytes);
        }
    }
    Ok(eviction)
}

pub fn schedule(directory: PathBuf) {
    let now = now_seconds();
    let last = LAST_SCAN.load(Ordering::Relaxed);
    if last != i64::MIN && now.saturating_sub(last) < SCAN_INTERVAL.as_secs() as i64 {
        return;
    }
    if RUNNING.swap(true, Ordering::AcqRel) {
        return;
    }
    LAST_SCAN.store(now, Ordering::Relaxed);
    let spawned = std::thread::Builder::new()
        .name("fileblade-thumbnail-eviction".to_string())
        .stack_size(128 * 1024)
        .spawn(move || {
            let _ = evict(&directory, CAPACITY_BYTES, RETAINED_BYTES);
            RUNNING.store(false, Ordering::Release);
        });
    if spawned.is_err() {
        RUNNING.store(false, Ordering::Release);
    }
}
