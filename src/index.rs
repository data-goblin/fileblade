use crate::common::display_path;
use ignore::{WalkBuilder, WalkState};
use nucleo::pattern::{CaseMatching, Normalization, Pattern};
use nucleo::{Config, Injector, Matcher, Nucleo, Utf32Str, Utf32String};
use std::borrow::Cow;
use std::collections::{BinaryHeap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, OnceLock};
use std::thread;
use std::time::{Duration, Instant};

pub const INDEX_ENTRY_CAP: usize = 250_000;
pub const INDEX_WALK_DEADLINE: Duration = Duration::from_secs(30);
const INDEX_CAP: usize = 3;
const INDEX_IDLE: Duration = Duration::from_secs(180);
const INDEX_STALE: Duration = Duration::from_secs(60);
const INDEX_PATH_BYTES: usize = 16 * 1024;
const INDEX_THREADS: usize = 4;
const INDEX_PATCH_CAP: usize = 64;

#[derive(Clone, Debug)]
pub struct IndexEntry {
    pub relative: String,
    pub native: Option<Arc<Path>>,
    pub is_dir: bool,
    pub is_symlink: bool,
}

impl IndexEntry {
    pub fn path(&self, root: &Path) -> PathBuf {
        root.join(
            self.native
                .as_deref()
                .unwrap_or_else(|| Path::new(&self.relative)),
        )
    }
}

#[derive(Clone, Debug)]
pub struct IndexFlags {
    pub native: Option<Arc<Path>>,
    pub is_dir: bool,
    pub is_symlink: bool,
}

fn haystack_text(column: &Utf32String) -> Cow<'_, str> {
    match column {
        Utf32String::Ascii(text) => Cow::Borrowed(&**text),
        Utf32String::Unicode(points) => Cow::Owned(points.iter().collect()),
    }
}

fn entry_of(column: &Utf32String, flags: &IndexFlags) -> IndexEntry {
    IndexEntry {
        relative: haystack_text(column).into_owned(),
        native: flags.native.clone(),
        is_dir: flags.is_dir,
        is_symlink: flags.is_symlink,
    }
}

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct IndexKey {
    pub root: PathBuf,
    pub show_hidden: bool,
}

#[derive(Clone, Debug)]
pub struct Hit {
    pub entry: IndexEntry,
    pub score: u32,
    pub indices: Vec<u32>,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct WalkStatus {
    pub running: bool,
    pub walked: usize,
    pub truncated: bool,
}

#[derive(Default)]
struct WalkProgress {
    running: AtomicBool,
    walked: AtomicUsize,
    truncated: AtomicBool,
    abort: AtomicBool,
    error: Mutex<String>,
}

pub struct PathIndex {
    nucleo: Nucleo<IndexFlags>,
    progress: Arc<WalkProgress>,
    ranked: bool,
    root: PathBuf,
    show_hidden: bool,
    known: Option<HashSet<u64>>,
    patched: bool,
}

struct Slot {
    index: Arc<Mutex<PathIndex>>,
    progress: Arc<WalkProgress>,
    dirty: bool,
    pending: HashSet<PathBuf>,
    built: Instant,
    last_used: Instant,
}

type Registry = Mutex<HashMap<IndexKey, Slot>>;

fn registry() -> &'static Registry {
    static REGISTRY: OnceLock<Registry> = OnceLock::new();
    REGISTRY.get_or_init(|| Mutex::new(HashMap::new()))
}

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub fn matcher_config() -> Config {
    Config::DEFAULT.match_paths()
}

pub fn case_matching(case_sensitive: bool) -> CaseMatching {
    if case_sensitive {
        CaseMatching::Respect
    } else {
        CaseMatching::Ignore
    }
}

pub fn acquire(root: &Path, show_hidden: bool, fresh: bool) -> Arc<Mutex<PathIndex>> {
    let key = IndexKey {
        root: root.to_path_buf(),
        show_hidden,
    };
    let mut slots = lock(registry());
    let now = Instant::now();
    slots.retain(|_, slot| {
        Arc::strong_count(&slot.index) > 1
            || now.duration_since(slot.last_used) < INDEX_IDLE
            || slot.abort()
    });
    let stale = |slot: &Slot| {
        slot.dirty
            || (fresh
                && !slot.progress.running.load(Ordering::Relaxed)
                && now.duration_since(slot.built) > INDEX_STALE)
    };
    if slots.get(&key).is_some_and(stale) {
        evict(&mut slots, &key);
    }
    while !slots.contains_key(&key) && slots.len() >= INDEX_CAP {
        let Some(oldest) = slots
            .iter()
            .filter(|(_, slot)| Arc::strong_count(&slot.index) == 1)
            .min_by_key(|(_, slot)| slot.last_used)
            .map(|(key, _)| key.clone())
        else {
            break;
        };
        evict(&mut slots, &oldest);
    }
    let slot = slots
        .entry(key)
        .or_insert_with(|| Slot::build(root, show_hidden, now));
    slot.last_used = now;
    let index = Arc::clone(&slot.index);
    let pending = if slot.progress.running.load(Ordering::Relaxed) {
        HashSet::new()
    } else {
        std::mem::take(&mut slot.pending)
    };
    drop(slots);
    if !pending.is_empty() {
        lock(&index).patch(&pending);
    }
    index
}

pub fn sweep_idle() -> bool {
    let mut slots = lock(registry());
    let now = Instant::now();
    let before = slots.len();
    slots.retain(|_, slot| {
        Arc::strong_count(&slot.index) > 1
            || now.duration_since(slot.last_used) < INDEX_IDLE
            || slot.abort()
    });
    slots.len() != before
}

pub fn has_slots() -> bool {
    !lock(registry()).is_empty()
}

fn evict(slots: &mut HashMap<IndexKey, Slot>, key: &IndexKey) {
    if let Some(slot) = slots.remove(key) {
        slot.abort();
    }
}

pub fn invalidate_all() {
    for slot in lock(registry()).values_mut() {
        slot.dirty = true;
    }
}

pub fn invalidate_within(path: &Path) {
    for (key, slot) in lock(registry()).iter_mut() {
        if key.root.starts_with(path) {
            slot.dirty = true;
            continue;
        }
        if !path.starts_with(&key.root) {
            continue;
        }
        if slot.progress.running.load(Ordering::Relaxed) || slot.pending.len() >= INDEX_PATCH_CAP {
            slot.dirty = true;
            slot.pending.clear();
            continue;
        }
        if let Some(parent) = path.parent().filter(|parent| parent.starts_with(&key.root)) {
            slot.pending.insert(parent.to_path_buf());
        }
        if path.is_dir() {
            slot.pending.insert(path.to_path_buf());
        }
    }
}

pub fn invalidate_from(result: &serde_json::Value) {
    let mut paths = Vec::new();
    let mut push = |value: &serde_json::Value| {
        if let Some(text) = value.as_str()
            && let Ok(path) = crate::common::parse_path(text)
        {
            paths.push(path);
        }
    };
    for key in ["path", "source", "destination", "target"] {
        push(&result[key]);
    }
    for key in ["paths", "entries", "mappings", "restored", "removed"] {
        if let Some(values) = result[key].as_array() {
            for value in values.iter().take(INDEX_PATCH_CAP) {
                push(value);
                for nested in ["path", "source", "destination", "target"] {
                    push(&value[nested]);
                }
            }
        }
    }
    if paths.is_empty() {
        invalidate_all();
        return;
    }
    for path in paths {
        invalidate_within(&path);
    }
}

impl Slot {
    fn build(root: &Path, show_hidden: bool, now: Instant) -> Self {
        let threads = thread::available_parallelism()
            .map(|value| value.get().clamp(1, INDEX_THREADS))
            .unwrap_or(1);
        let nucleo = Nucleo::new(matcher_config(), Arc::new(|| {}), Some(threads), 1);
        let progress = Arc::new(WalkProgress::default());
        progress.running.store(true, Ordering::Relaxed);
        let walker = Walker {
            root: root.to_path_buf(),
            show_hidden,
            threads,
            injector: nucleo.injector(),
            progress: Arc::clone(&progress),
        };
        if thread::Builder::new()
            .name("fileblade-index".to_string())
            .spawn(move || walker.run())
            .is_err()
        {
            *lock(&progress.error) = "unable to start the index walker".to_string();
            progress.running.store(false, Ordering::Relaxed);
        }
        Self {
            index: Arc::new(Mutex::new(PathIndex {
                nucleo,
                progress: Arc::clone(&progress),
                ranked: false,
                root: root.to_path_buf(),
                show_hidden,
                known: None,
                patched: false,
            })),
            progress,
            dirty: false,
            pending: HashSet::new(),
            built: now,
            last_used: now,
        }
    }

    fn abort(&self) -> bool {
        self.progress.abort.store(true, Ordering::Relaxed);
        false
    }
}

impl PathIndex {
    pub fn status(&self) -> WalkStatus {
        WalkStatus {
            running: self.progress.running.load(Ordering::Relaxed),
            walked: self.progress.walked.load(Ordering::Relaxed),
            truncated: self.progress.truncated.load(Ordering::Relaxed),
        }
    }

    pub fn error(&self) -> String {
        lock(&self.progress.error).clone()
    }

    pub fn indexed(&self) -> usize {
        self.nucleo.snapshot().item_count() as usize
    }

    pub fn matched(&self) -> usize {
        self.nucleo.snapshot().matched_item_count() as usize
    }

    pub fn set_pattern(&mut self, pattern: &str, case: CaseMatching) {
        self.ranked = !pattern.trim().is_empty();
        self.nucleo
            .pattern
            .reparse(0, pattern, case, Normalization::Smart, false);
    }

    pub fn tick(&mut self, timeout_ms: u64) -> nucleo::Status {
        self.nucleo.tick(timeout_ms)
    }

    fn digest(text: &str) -> u64 {
        use std::hash::{BuildHasher, RandomState};
        static SEED: OnceLock<RandomState> = OnceLock::new();
        SEED.get_or_init(RandomState::new).hash_one(text)
    }

    fn known_digests(&mut self) -> &mut HashSet<u64> {
        if self.known.is_none() {
            let snapshot = self.nucleo.snapshot();
            let count = snapshot.item_count();
            let mut digests = HashSet::with_capacity(count as usize);
            for position in 0..count {
                let Some(item) = snapshot.get_item(position) else {
                    continue;
                };
                digests.insert(Self::digest(&haystack_text(&item.matcher_columns[0])));
            }
            self.known = Some(digests);
        }
        self.known.as_mut().expect("digests")
    }

    fn patch(&mut self, directories: &HashSet<PathBuf>) {
        let root = self.root.clone();
        let show_hidden = self.show_hidden;
        let injector = self.nucleo.injector();
        let known = self.known_digests();
        for directory in directories.iter().take(INDEX_PATCH_CAP) {
            let mut builder = WalkBuilder::new(directory);
            builder
                .hidden(!show_hidden)
                .follow_links(false)
                .max_depth(Some(1))
                .threads(1)
                .add_custom_ignore_filename(".fdignore");
            for entry in builder.build().flatten() {
                let Ok(native) = entry.path().strip_prefix(&root) else {
                    continue;
                };
                let relative = display_path(native);
                if relative.is_empty() || relative.len() > INDEX_PATH_BYTES {
                    continue;
                }
                if !known.insert(Self::digest(&relative)) {
                    continue;
                }
                let native =
                    (native.to_str() != Some(&relative)).then(|| Arc::<Path>::from(native));
                let file_type = entry.file_type();
                injector.push(
                    IndexFlags {
                        native,
                        is_dir: file_type.is_some_and(|kind| kind.is_dir()),
                        is_symlink: file_type.is_some_and(|kind| kind.is_symlink()),
                    },
                    move |_, columns| columns[0] = Utf32String::from(relative),
                );
            }
        }
        self.patched = true;
    }

    fn present(&self, entry: &IndexEntry) -> bool {
        !self.patched || std::fs::symlink_metadata(entry.path(&self.root)).is_ok()
    }

    pub fn ranked(&self) -> bool {
        self.ranked
    }

    pub fn hits(
        &self,
        limit: usize,
        accept: &mut dyn FnMut(&str, &IndexFlags) -> bool,
    ) -> Vec<Hit> {
        let snapshot = self.nucleo.snapshot();
        let pattern = snapshot.pattern().column_pattern(0);
        let mut matcher = Matcher::new(matcher_config());
        let items = snapshot
            .matched_items(..)
            .filter(|item| accept(&haystack_text(&item.matcher_columns[0]), item.data));
        let headroom = if self.patched {
            limit.saturating_add(64)
        } else {
            limit
        };
        if !self.ranked {
            let mut rows = alphabetical(
                items.map(|item| entry_of(&item.matcher_columns[0], item.data)),
                headroom,
            );
            if self.patched {
                rows.retain(|hit| self.present(&hit.entry));
                rows.truncate(limit);
            }
            return rows;
        }
        let mut hits = items
            .filter_map(|item| {
                let mut indices = Vec::new();
                let score = pattern.indices(
                    item.matcher_columns[0].slice(..),
                    &mut matcher,
                    &mut indices,
                )?;
                indices.sort_unstable();
                indices.dedup();
                Some(Hit {
                    entry: entry_of(&item.matcher_columns[0], item.data),
                    score,
                    indices,
                })
            })
            .take(headroom)
            .collect::<Vec<_>>();
        if self.patched {
            hits.retain(|hit| self.present(&hit.entry));
        }
        hits.sort_by(|left, right| {
            right
                .score
                .cmp(&left.score)
                .then_with(|| left.entry.relative.len().cmp(&right.entry.relative.len()))
                .then_with(|| left.entry.relative.cmp(&right.entry.relative))
        });
        hits.truncate(limit);
        hits
    }
}

struct Alphabetical(String, IndexEntry);

impl PartialEq for Alphabetical {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0 && self.1.relative == other.1.relative
    }
}

impl Eq for Alphabetical {}

impl PartialOrd for Alphabetical {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Alphabetical {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.0
            .cmp(&other.0)
            .then_with(|| self.1.relative.cmp(&other.1.relative))
    }
}

fn alphabetical(entries: impl Iterator<Item = IndexEntry>, limit: usize) -> Vec<Hit> {
    let mut heap = BinaryHeap::with_capacity(limit + 1);
    for entry in entries {
        heap.push(Alphabetical(entry.relative.to_lowercase(), entry));
        if heap.len() > limit {
            heap.pop();
        }
    }
    let mut sorted = heap.into_sorted_vec();
    sorted.dedup();
    sorted
        .into_iter()
        .map(|item| Hit {
            entry: item.1,
            score: 0,
            indices: Vec::new(),
        })
        .collect()
}

pub fn score_text(pattern: &Pattern, text: &str, matcher: &mut Matcher) -> Option<(u32, Vec<u32>)> {
    let mut buffer = Vec::new();
    let mut indices = Vec::new();
    let score = pattern.indices(Utf32Str::new(text, &mut buffer), matcher, &mut indices)?;
    indices.sort_unstable();
    indices.dedup();
    Some((score, indices))
}

pub fn score_path_name(
    pattern: &Pattern,
    path: &str,
    matcher: &mut Matcher,
) -> Option<(u32, Vec<u32>)> {
    let Some(slash) = path.rfind('/') else {
        return score_text(pattern, path, matcher);
    };
    let name = &path[slash + 1..];
    if name.is_empty() {
        return score_text(pattern, path, matcher);
    }
    let offset = path[..=slash].chars().count() as u32;
    let (score, mut indices) = score_text(pattern, name, matcher)?;
    indices.iter_mut().for_each(|index| *index += offset);
    Some((score, indices))
}

pub fn literal_tier(name: &str, needle: &str) -> u8 {
    let needle = needle.trim();
    if needle.is_empty() {
        return 0;
    }
    let name = name.to_lowercase();
    let needle = needle.to_lowercase();
    if name == needle {
        0
    } else if name.starts_with(&needle) {
        1
    } else if name.contains(&needle) {
        2
    } else {
        3
    }
}

pub fn literal_tier_path(path: &str, needle: &str) -> u8 {
    literal_tier(path.rsplit('/').next().unwrap_or(path), needle)
}

pub fn parse_pattern(text: &str, case: CaseMatching) -> Pattern {
    Pattern::parse(text, case, Normalization::Smart)
}

pub fn spans_from_indices(indices: &[u32]) -> Vec<(usize, usize)> {
    let mut spans: Vec<(usize, usize)> = Vec::new();
    for index in indices.iter().map(|index| *index as usize) {
        match spans.last_mut() {
            Some(last) if last.1 == index => last.1 = index + 1,
            _ => spans.push((index, index + 1)),
        }
    }
    spans
}

pub fn spans_text(indices: &[u32], offset: usize) -> String {
    let shifted = indices
        .iter()
        .filter(|index| **index as usize >= offset)
        .map(|index| index - offset as u32)
        .collect::<Vec<_>>();
    spans_from_indices(&shifted)
        .into_iter()
        .map(|(start, end)| format!("{start}-{end}"))
        .collect::<Vec<_>>()
        .join(",")
}

pub fn escape_atom(text: &str) -> String {
    let count = text.chars().count();
    let mut escaped = String::with_capacity(text.len() + 4);
    for (position, character) in text.chars().enumerate() {
        let lead = position == 0 && matches!(character, '!' | '^' | '\'');
        let tail = position + 1 == count && character == '$';
        if character.is_whitespace() || character == '\\' || lead || tail {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

struct Walker {
    root: PathBuf,
    show_hidden: bool,
    threads: usize,
    injector: Injector<IndexFlags>,
    progress: Arc<WalkProgress>,
}

impl Walker {
    fn run(self) {
        let started = Instant::now();
        let mut visibility = crate::visibility::Visibility::new(&self.root, self.show_hidden);
        if !visibility.path(&self.root) {
            self.progress.running.store(false, Ordering::Relaxed);
            return;
        }
        let mut builder = WalkBuilder::new(&self.root);
        builder
            .hidden(!self.show_hidden)
            .follow_links(false)
            .threads(self.threads)
            .add_custom_ignore_filename(".fdignore");
        let root_error = Mutex::new(String::new());
        builder.build_parallel().run(|| {
            Box::new(|result| match result {
                Ok(entry) => self.visit(entry, started, &visibility),
                Err(error) => {
                    if self.progress.walked.load(Ordering::Relaxed) == 0 {
                        let mut slot = lock(&root_error);
                        if slot.is_empty() {
                            *slot = error.to_string();
                        }
                    }
                    WalkState::Continue
                }
            })
        });
        if self.progress.walked.load(Ordering::Relaxed) == 0 {
            *lock(&self.progress.error) = lock(&root_error).clone();
        }
        self.progress.running.store(false, Ordering::Relaxed);
    }

    fn visit(
        &self,
        entry: ignore::DirEntry,
        started: Instant,
        visibility: &crate::visibility::Visibility,
    ) -> WalkState {
        if self.progress.abort.load(Ordering::Relaxed) {
            return WalkState::Quit;
        }
        let over_deadline = started.elapsed() >= INDEX_WALK_DEADLINE;
        let Ok(native) = entry.path().strip_prefix(&self.root) else {
            return WalkState::Continue;
        };
        let relative = display_path(native);
        let native = (native.to_str() != Some(&relative)).then(|| Arc::<Path>::from(native));
        if entry.depth() == 0 || relative.is_empty() || relative.len() > INDEX_PATH_BYTES {
            return WalkState::Continue;
        }
        if over_deadline || self.progress.walked.load(Ordering::Relaxed) >= INDEX_ENTRY_CAP {
            self.progress.truncated.store(true, Ordering::Relaxed);
            return WalkState::Quit;
        }
        if !visibility.entry(
            entry.path(),
            entry
                .file_type()
                .is_some_and(|kind| kind.is_dir() || kind.is_symlink()),
        ) {
            return WalkState::Skip;
        }
        self.progress.walked.fetch_add(1, Ordering::Relaxed);
        let file_type = entry.file_type();
        self.injector.push(
            IndexFlags {
                native,
                is_dir: file_type.is_some_and(|kind| kind.is_dir()),
                is_symlink: file_type.is_some_and(|kind| kind.is_symlink()),
            },
            move |_, columns| columns[0] = Utf32String::from(relative),
        );
        WalkState::Continue
    }
}
