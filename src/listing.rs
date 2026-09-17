use crate::common::{parse_path, path_text};
use crate::filesystem::{basic_entry_with_git, cached_git_repository};
use crate::git::{
    decorate_git_entry, git_metadata_document, ignored_paths, indexed_git_counts_for_path,
    indexed_git_status_for_path, repository_status_index_cancellable,
};
use serde_json::{Value, json};
use std::cmp::Ordering as CmpOrdering;
use std::collections::HashMap;
use std::fs;
use std::io;
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::time::{Duration, Instant};

pub const DIRECTORY_PAGE: usize = 400;
pub const LISTING_ENTRY_CAP: usize = 200_000;
const LISTING_CAP: usize = 24;
const LISTING_IDLE: Duration = Duration::from_secs(300);
const METADATA_DEADLINE: Duration = Duration::from_secs(2);
const ORDER_IDLE: Duration = Duration::from_secs(5);
const SORT_KEYS: [&str; 5] = ["name", "size", "modified", "created", "type"];

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct ListingKey {
    pub path: PathBuf,
    pub show_hidden: bool,
}

#[derive(Clone, Debug)]
struct Listed {
    name: String,
    path: PathBuf,
    is_dir: bool,
    is_link: bool,
}

#[derive(Debug)]
pub struct DirListing {
    entries: Vec<Listed>,
    capped: bool,
    stamp: (i64, i64, u64),
    last_used: Instant,
    order: Option<OrderedListing>,
}

#[derive(Debug)]
struct OrderedListing {
    sort: String,
    descending: bool,
    filter: Value,
    git_enabled: bool,
    order: Vec<usize>,
    built: Instant,
}

#[derive(Clone, Debug)]
pub struct WindowRequest {
    pub path: String,
    pub show_hidden: bool,
    pub start: usize,
    pub count: usize,
    pub sort: String,
    pub descending: bool,
    pub filter: Value,
    pub include_created: bool,
    pub fresh: bool,
    pub git_enabled: bool,
    pub fresh_git: bool,
}

type Registry = Mutex<HashMap<ListingKey, Arc<Mutex<DirListing>>>>;

fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn directory_stamp(path: &Path) -> io::Result<(i64, i64, u64)> {
    let metadata = fs::symlink_metadata(path)?;
    if !metadata.is_dir() {
        return Err(io::Error::new(
            io::ErrorKind::NotADirectory,
            "Not a directory",
        ));
    }
    Ok((
        metadata.mtime() * 1_000_000_000 + metadata.mtime_nsec(),
        metadata.ctime() * 1_000_000_000 + metadata.ctime_nsec(),
        metadata.ino(),
    ))
}

fn scan(path: &Path, show_hidden: bool, cancelled: &AtomicBool) -> io::Result<(Vec<Listed>, bool)> {
    let mut entries = Vec::new();
    let mut capped = false;
    let mut visibility = crate::visibility::Visibility::new(path, show_hidden);
    if !visibility.path(path) {
        return Ok((entries, false));
    }
    for entry in fs::read_dir(path)? {
        if cancelled.load(Ordering::Relaxed) {
            return Err(io::Error::new(
                io::ErrorKind::Interrupted,
                "operation cancelled",
            ));
        }
        let Ok(entry) = entry else {
            continue;
        };
        let raw = entry.file_name();
        if !show_hidden && raw.as_encoded_bytes().starts_with(b".") {
            continue;
        }
        if entries.len() >= LISTING_ENTRY_CAP {
            capped = true;
            break;
        }
        let Ok(kind) = entry.file_type() else {
            continue;
        };
        let child = entry.path();
        let is_link = kind.is_symlink();
        let is_dir = if is_link {
            child.is_dir()
        } else {
            kind.is_dir()
        };
        if !visibility.entry(&child, is_dir) {
            continue;
        }
        entries.push(Listed {
            name: crate::common::display_path(Path::new(&raw)),
            path: child,
            is_dir,
            is_link,
        });
    }
    entries.sort_by(|left, right| natural(&left.name, &right.name));
    Ok((entries, capped))
}

fn natural(left: &str, right: &str) -> CmpOrdering {
    let mut a = left.chars().peekable();
    let mut b = right.chars().peekable();
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return left.cmp(right),
            (None, Some(_)) => return CmpOrdering::Less,
            (Some(_), None) => return CmpOrdering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let mut na = 0_u128;
                let mut nb = 0_u128;
                while let Some(d) = a.peek().copied().filter(char::is_ascii_digit) {
                    na = na
                        .saturating_mul(10)
                        .saturating_add(d as u128 - '0' as u128);
                    a.next();
                }
                while let Some(d) = b.peek().copied().filter(char::is_ascii_digit) {
                    nb = nb
                        .saturating_mul(10)
                        .saturating_add(d as u128 - '0' as u128);
                    b.next();
                }
                if na != nb {
                    return na.cmp(&nb);
                }
            }
            (Some(x), Some(y)) => {
                let lx = x.to_lowercase().next().unwrap_or(x);
                let ly = y.to_lowercase().next().unwrap_or(y);
                if lx != ly {
                    return lx.cmp(&ly);
                }
                a.next();
                b.next();
            }
        }
    }
}

fn acquire(
    path: &Path,
    show_hidden: bool,
    fresh: bool,
    cancelled: &AtomicBool,
) -> io::Result<Arc<Mutex<DirListing>>> {
    let key = ListingKey {
        path: path.to_path_buf(),
        show_hidden,
    };
    let stamp = directory_stamp(path)?;
    let now = Instant::now();
    let slot = {
        let mut slots = lock(registry());
        slots.retain(|_, slot| {
            Arc::strong_count(slot) > 1 || now.duration_since(lock(slot).last_used) < LISTING_IDLE
        });
        while !slots.contains_key(&key) && slots.len() >= LISTING_CAP {
            let Some(oldest) = slots
                .iter()
                .filter(|(_, slot)| Arc::strong_count(slot) == 1)
                .min_by_key(|(_, slot)| lock(slot).last_used)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            slots.remove(&oldest);
        }
        slots
            .entry(key)
            .or_insert_with(|| {
                Arc::new(Mutex::new(DirListing {
                    entries: Vec::new(),
                    capped: false,
                    stamp: (0, 0, 0),
                    last_used: now,
                    order: None,
                }))
            })
            .clone()
    };
    {
        let mut listing = lock(&slot);
        listing.last_used = now;
        if fresh || listing.stamp != stamp {
            let (entries, capped) = scan(path, show_hidden, cancelled)?;
            listing.entries = entries;
            listing.capped = capped;
            listing.stamp = stamp;
            listing.order = None;
        }
    }
    Ok(slot)
}

pub fn entry_count(raw_path: &str, show_hidden: bool, cancelled: &AtomicBool) -> io::Result<usize> {
    let path = parse_path(raw_path)?;
    let slot = acquire(&path, show_hidden, false, cancelled)?;
    let count = lock(&slot).entries.len();
    Ok(count)
}

pub fn forget(raw_path: &str) {
    let Ok(path) = parse_path(raw_path) else {
        return;
    };
    lock(registry()).retain(|key, _| key.path != path);
}

pub fn invalidate_all() {
    lock(registry()).clear();
}

pub fn invalidate_within(path: &Path) {
    for (key, slot) in lock(registry()).iter() {
        if path.starts_with(&key.path) || key.path.starts_with(path) {
            lock(slot).order = None;
        }
    }
}

fn stat_row(entry: &Listed, include_created: bool, git_enabled: bool) -> Option<Value> {
    let metadata = fs::symlink_metadata(&entry.path).ok()?;
    let mut row = basic_entry_with_git(
        &entry.path,
        &metadata,
        entry.is_dir,
        entry.is_link,
        git_enabled,
    );
    if include_created {
        row["created"] = json!(crate::filesystem::creation_timestamp(&entry.path));
    }
    Some(row)
}

fn sort_field(row: &Value, key: &str) -> Value {
    match key {
        "size" => json!(row["size"].as_i64().unwrap_or(-1)),
        "modified" | "created" => row[key].clone(),
        "type" => json!(format!(
            "{} {}",
            row["kind"].as_str().unwrap_or_default(),
            row["mime"].as_str().unwrap_or_default()
        )),
        _ => Value::Null,
    }
}

fn compare_field(left: &Value, right: &Value) -> CmpOrdering {
    match (left.as_i64(), right.as_i64()) {
        (Some(a), Some(b)) => a.cmp(&b),
        _ => natural(
            left.as_str().unwrap_or_default(),
            right.as_str().unwrap_or_default(),
        ),
    }
}

fn clause_matches(row: &Value, key: &str, clause: &Value) -> bool {
    if key == "size" {
        let size = row["size"].as_i64().unwrap_or(-1);
        if size < 0 {
            return false;
        }
        let min = clause["min"]
            .as_i64()
            .or_else(|| clause["min"].as_str().and_then(|v| v.parse().ok()));
        let max = clause["max"]
            .as_i64()
            .or_else(|| clause["max"].as_str().and_then(|v| v.parse().ok()));
        return min.is_none_or(|value| size >= value) && max.is_none_or(|value| size <= value);
    }
    let day = row[key].as_str().unwrap_or_default();
    let day = &day[..day.len().min(10)];
    if day.is_empty() {
        return false;
    }
    let since = clause["since"].as_str();
    let until = clause["until"].as_str();
    since.is_none_or(|value| day >= value) && until.is_none_or(|value| day <= value)
}

fn passes(row: &Value, filter: &Value) -> bool {
    if row["is_dir"].as_bool().unwrap_or(false) {
        return true;
    }
    let Some(clauses) = filter.as_object() else {
        return true;
    };
    clauses
        .iter()
        .filter(|(key, _)| matches!(key.as_str(), "size" | "modified" | "created"))
        .all(|(key, clause)| clause_matches(row, key, clause))
}

fn needs_metadata(sort: &str, filter: &Value) -> bool {
    sort != "name"
        || filter
            .as_object()
            .is_some_and(|clauses| !clauses.is_empty())
}

fn needs_creation(sort: &str, filter: &Value) -> bool {
    sort == "created"
        || filter
            .as_object()
            .is_some_and(|clauses| clauses.contains_key("created"))
}

pub fn window(request: &WindowRequest, cancelled: &AtomicBool) -> Value {
    let path = match parse_path(&request.path) {
        Ok(path) => path,
        Err(error) => return failure(&request.path, &error),
    };
    let text = path_text(&path);
    let sort = if SORT_KEYS.contains(&request.sort.as_str()) {
        request.sort.clone()
    } else {
        "name".to_string()
    };
    let slot = match acquire(&path, request.show_hidden, request.fresh, cancelled) {
        Ok(slot) => slot,
        Err(error) => return failure(&text, &error),
    };
    let mut listing = lock(&slot);
    let started = Instant::now();
    let cached = listing
        .order
        .as_ref()
        .filter(|view| {
            view.sort == sort
                && view.descending == request.descending
                && view.filter == request.filter
                && view.git_enabled == request.git_enabled
                && view.built.elapsed() < ORDER_IDLE
        })
        .map(|view| view.order.clone());
    let mut rows: Vec<Option<Value>> = if cached.is_none() {
        vec![None; listing.entries.len()]
    } else {
        Vec::new()
    };
    let mut partial = false;
    let metadata_pass = needs_metadata(&sort, &request.filter);
    let include_created = request.include_created || needs_creation(&sort, &request.filter);
    if metadata_pass && cached.is_none() {
        for (index, entry) in listing.entries.iter().enumerate() {
            if cancelled.load(Ordering::Relaxed) {
                return failure(
                    &text,
                    &io::Error::new(io::ErrorKind::Interrupted, "operation cancelled"),
                );
            }
            if started.elapsed() > METADATA_DEADLINE {
                partial = true;
                break;
            }
            rows[index] = stat_row(entry, include_created, request.git_enabled);
        }
    }
    let order = if let Some(order) = cached {
        order
    } else {
        let mut order: Vec<usize> = (0..listing.entries.len())
            .filter(|index| {
                !metadata_pass
                    || rows[*index]
                        .as_ref()
                        .is_none_or(|row| passes(row, &request.filter))
            })
            .collect();
        order.sort_by(|left, right| {
            let a = &listing.entries[*left];
            let b = &listing.entries[*right];
            let dirs = (!a.is_dir).cmp(&(!b.is_dir));
            if dirs != CmpOrdering::Equal {
                return dirs;
            }
            let keyed = if sort == "name" {
                CmpOrdering::Equal
            } else {
                let fa = rows[*left]
                    .as_ref()
                    .map(|row| sort_field(row, &sort))
                    .unwrap_or(Value::Null);
                let fb = rows[*right]
                    .as_ref()
                    .map(|row| sort_field(row, &sort))
                    .unwrap_or(Value::Null);
                let ordering = compare_field(&fa, &fb);
                if request.descending {
                    ordering.reverse()
                } else {
                    ordering
                }
            };
            keyed.then_with(|| {
                let names = natural(&a.name, &b.name);
                if sort == "name" && request.descending {
                    names.reverse()
                } else {
                    names
                }
            })
        });
        if !partial {
            listing.order = Some(OrderedListing {
                sort: sort.clone(),
                descending: request.descending,
                filter: request.filter.clone(),
                git_enabled: request.git_enabled,
                order: order.clone(),
                built: Instant::now(),
            });
        }
        order
    };
    let total = order.len();
    let start = request.start.min(total);
    let end = start.saturating_add(request.count.max(1)).min(total);
    let mut entries = Vec::with_capacity(end - start);
    for index in &order[start..end] {
        if cancelled.load(Ordering::Relaxed) {
            return failure(
                &text,
                &io::Error::new(io::ErrorKind::Interrupted, "operation cancelled"),
            );
        }
        let row = match rows.get_mut(*index).and_then(Option::take) {
            Some(row) => row,
            None => match stat_row(
                &listing.entries[*index],
                include_created,
                request.git_enabled,
            ) {
                Some(row) => row,
                None => continue,
            },
        };
        entries.push(row);
    }
    let capped = listing.capped;
    drop(listing);
    let git = request
        .git_enabled
        .then(|| decorate(&path, &mut entries, request.fresh_git, cancelled))
        .flatten();
    let mut response = json!({
        "ok": true,
        "path": text,
        "entries": entries,
        "start": start,
        "total": total,
        "truncated": end < total,
        "windowed": true,
        "capped": capped,
        "partial_metadata": partial,
        "sort": sort,
        "descending": request.descending,
        "limit": request.count.max(1)
    });
    if let Some(git) = git {
        response["git"] = git;
    }
    response
}

fn decorate(
    path: &Path,
    rows: &mut [Value],
    fresh_git: bool,
    cancelled: &AtomicBool,
) -> Option<Value> {
    let mut cache = HashMap::new();
    let repository = cached_git_repository(path, &mut cache, fresh_git, cancelled)?;
    let index = repository_status_index_cancellable(&repository, cancelled)?;
    let mut probes = rows
        .iter()
        .filter_map(|row| parse_path(row["path"].as_str()?).ok())
        .collect::<Vec<_>>();
    probes.push(path.to_path_buf());
    let ignored = ignored_paths(&repository.root, &probes, cancelled);
    for row in rows.iter_mut() {
        let Ok(row_path) = parse_path(row["path"].as_str().unwrap_or_default()) else {
            continue;
        };
        let is_dir = row["is_dir"].as_bool().unwrap_or(false);
        let status = indexed_git_status_for_path(&index, &row_path, is_dir);
        let counts = indexed_git_counts_for_path(&index, &row_path, is_dir);
        decorate_git_entry(row, &repository, status, counts);
        row["git_ignored"] = json!(ignored.contains(&row_path));
    }
    let status = indexed_git_status_for_path(&index, path, true);
    let counts = indexed_git_counts_for_path(&index, path, true);
    let mut document = git_metadata_document(&repository, status, true, counts);
    document["ignored"] = json!(ignored.contains(path));
    Some(document)
}

fn failure(path: &str, error: &io::Error) -> Value {
    json!({
        "ok": false,
        "path": path,
        "entries": [],
        "start": 0,
        "total": 0,
        "truncated": false,
        "windowed": true,
        "missing": error.kind() == io::ErrorKind::NotFound,
        "error": error.to_string()
    })
}
