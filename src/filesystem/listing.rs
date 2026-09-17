use super::*;

pub fn children(raw_path: &str, show_hidden: bool) -> Value {
    children_cancellable(raw_path, show_hidden, &AtomicBool::new(false))
}

pub fn children_cancellable(raw_path: &str, show_hidden: bool, cancelled: &AtomicBool) -> Value {
    let mut cache = HashMap::new();
    children_with_cache(
        raw_path,
        show_hidden,
        false,
        true,
        false,
        &mut cache,
        cancelled,
    )
}

pub fn children_batch(paths: &[String], show_hidden: bool) -> Value {
    children_batch_cancellable(paths, show_hidden, &AtomicBool::new(false))
}

pub fn children_batch_cancellable(
    paths: &[String],
    show_hidden: bool,
    cancelled: &AtomicBool,
) -> Value {
    let mut unique = Vec::new();
    let mut seen = HashSet::new();
    for raw_path in paths.iter().take(64) {
        let path = match parse_path(raw_path) {
            Ok(path) => path,
            Err(error) => return path_error(raw_path, &error),
        };
        if seen.insert(path.clone()) {
            unique.push(path_text(&path));
        }
    }
    let mut cache = HashMap::new();
    let results = unique
        .iter()
        .map(|path| {
            children_with_cache(path, show_hidden, false, true, false, &mut cache, cancelled)
        })
        .collect::<Vec<_>>();
    json!({"ok": true, "results": results})
}

pub struct ChildrenPage {
    pub limit: usize,
    pub sort: String,
    pub descending: bool,
    pub filter: Value,
    pub include_created: bool,
    pub git_enabled: bool,
    pub fresh_git: bool,
}

pub fn children_batch_paged(
    paths: &[String],
    show_hidden: bool,
    page: &ChildrenPage,
    cancelled: &AtomicBool,
) -> Value {
    let mut unique = Vec::new();
    let mut seen = HashSet::new();
    for raw_path in paths.iter().take(64) {
        let path = match parse_path(raw_path) {
            Ok(path) => path,
            Err(error) => return path_error(raw_path, &error),
        };
        if seen.insert(path.clone()) {
            unique.push(path_text(&path));
        }
    }
    let limit = page.limit.clamp(1, DIRECTORY_ENTRY_LIMIT);
    let mut cache = HashMap::new();
    let results = unique
        .iter()
        .map(|path| {
            let count = crate::listing::entry_count(path, show_hidden, cancelled).unwrap_or(0);
            if count > limit {
                crate::listing::window(
                    &crate::listing::WindowRequest {
                        path: path.clone(),
                        show_hidden,
                        start: 0,
                        count: limit,
                        sort: page.sort.clone(),
                        descending: page.descending,
                        filter: page.filter.clone(),
                        include_created: page.include_created,
                        fresh: false,
                        git_enabled: page.git_enabled,
                        fresh_git: page.fresh_git,
                    },
                    cancelled,
                )
            } else {
                children_with_cache(
                    path,
                    show_hidden,
                    page.include_created,
                    page.git_enabled,
                    page.fresh_git,
                    &mut cache,
                    cancelled,
                )
            }
        })
        .collect::<Vec<_>>();
    json!({"ok": true, "results": results})
}

pub(super) fn visible_rows(
    path: &Path,
    show_hidden: bool,
    include_created: bool,
    git_enabled: bool,
    cancelled: &AtomicBool,
) -> io::Result<(Vec<Value>, bool)> {
    let mut rows = Vec::new();
    let mut scanned = 0_usize;
    let mut visibility = crate::visibility::Visibility::new(path, show_hidden);
    if !visibility.path(path) {
        return Ok((rows, false));
    }
    for entry in fs::read_dir(path)? {
        if cancelled.load(Ordering::Relaxed) {
            break;
        }
        scanned += 1;
        if scanned > DIRECTORY_SCAN_LIMIT || rows.len() >= DIRECTORY_ENTRY_LIMIT {
            return Ok((rows, true));
        }
        let Ok(entry) = entry else {
            continue;
        };
        let name = entry.file_name();
        if !show_hidden && name.as_encoded_bytes().starts_with(b".") {
            continue;
        }
        let path = entry.path();
        let Ok(metadata) = fs::symlink_metadata(&path) else {
            continue;
        };
        let is_link = metadata.file_type().is_symlink();
        let is_dir = path.is_dir();
        if !visibility.entry(&path, is_dir) {
            continue;
        }
        let mut row = basic_entry_with_git(&path, &metadata, is_dir, is_link, git_enabled);
        if include_created {
            row["created"] = json!(creation_timestamp(&path));
        }
        rows.push(row);
    }
    Ok((rows, false))
}

pub(super) fn children_with_cache(
    raw_path: &str,
    show_hidden: bool,
    include_created: bool,
    git_enabled: bool,
    fresh_git: bool,
    cache: &mut HashMap<PathBuf, GitRepository>,
    cancelled: &AtomicBool,
) -> Value {
    let path = match parse_path(raw_path) {
        Ok(path) => path,
        Err(error) => return path_error(raw_path, &error),
    };
    if cancelled.load(Ordering::Relaxed) {
        let error = io::Error::new(io::ErrorKind::Interrupted, "operation cancelled");
        return error_payload(&path, &error);
    }
    let result = visible_rows(&path, show_hidden, include_created, git_enabled, cancelled);
    let (mut rows, mut truncated) = match result {
        Ok(rows) => rows,
        Err(error) => return error_payload(&path, &error),
    };
    if cancelled.load(Ordering::Relaxed) {
        let error = io::Error::new(io::ErrorKind::Interrupted, "operation cancelled");
        return error_payload(&path, &error);
    }
    let repository = git_enabled
        .then(|| cached_git_repository(&path, cache, fresh_git, cancelled))
        .flatten();
    if cancelled.load(Ordering::Relaxed) {
        let error = io::Error::new(io::ErrorKind::Interrupted, "operation cancelled");
        return error_payload(&path, &error);
    }
    let mut status_index = None;
    let mut directory_ignored = false;
    if let Some(repository) = repository.as_ref() {
        let Some(index) = repository_status_index_cancellable(repository, cancelled) else {
            let error = io::Error::new(io::ErrorKind::Interrupted, "operation cancelled");
            return error_payload(&path, &error);
        };
        let mut probes = rows
            .iter()
            .filter_map(|row| parse_path(row["path"].as_str()?).ok())
            .collect::<Vec<_>>();
        probes.push(path.clone());
        let ignored = ignored_paths(&repository.root, &probes, cancelled);
        directory_ignored = ignored.contains(&path);
        for row in &mut rows {
            let Ok(row_path) = parse_path(row["path"].as_str().unwrap_or_default()) else {
                continue;
            };
            let status = indexed_git_status_for_path(
                &index,
                &row_path,
                row["is_dir"].as_bool().unwrap_or(false),
            );
            let counts = indexed_git_counts_for_path(
                &index,
                &row_path,
                row["is_dir"].as_bool().unwrap_or(false),
            );
            decorate_git_entry(row, repository, status, counts);
            row["git_ignored"] = json!(ignored.contains(&row_path));
        }
        let existing = rows
            .iter()
            .filter_map(|row| row["path"].as_str().map(ToOwned::to_owned))
            .collect::<HashSet<_>>();
        for status in &repository.entries {
            if !status.deleted || status.path.parent() != Some(path.as_path()) {
                continue;
            }
            let status_path = path_text(&status.path);
            if existing.contains(&status_path)
                || !crate::visibility::Visibility::new(&path, show_hidden).path(&status.path)
            {
                continue;
            }
            if rows.len() >= DIRECTORY_ENTRY_LIMIT {
                truncated = true;
                break;
            }
            rows.push(deleted_git_entry(status, repository));
        }
        status_index = Some(index);
    }
    rows.sort_by(|left, right| {
        let left_dir = left["is_dir"].as_bool().unwrap_or(false);
        let right_dir = right["is_dir"].as_bool().unwrap_or(false);
        (!left_dir)
            .cmp(&(!right_dir))
            .then_with(|| {
                left["name"]
                    .as_str()
                    .unwrap_or_default()
                    .to_lowercase()
                    .cmp(&right["name"].as_str().unwrap_or_default().to_lowercase())
            })
            .then_with(|| left["name"].as_str().cmp(&right["name"].as_str()))
    });
    let mut response = json!({
        "ok": true,
        "path": path_text(&path),
        "entries": rows,
        "truncated": truncated,
        "limit": DIRECTORY_ENTRY_LIMIT
    });
    if let (Some(repository), Some(index)) = (repository.as_ref(), status_index.as_ref()) {
        let status = indexed_git_status_for_path(index, &path, true);
        let counts = indexed_git_counts_for_path(index, &path, true);
        response["git"] = git_metadata_document(repository, status, true, counts);
        response["git"]["ignored"] = json!(directory_ignored);
    }
    response
}
