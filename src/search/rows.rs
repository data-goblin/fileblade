use super::*;

pub(super) fn metadata_matches(run: &SearchRun<'_>, relative: &str, flags: &IndexFlags) -> bool {
    if !run.spec.filters.iter().any(|filter| {
        filter.key == "mime"
            || (filter.key == "type" && !matches!(filter.value.as_str(), "file" | "dir" | "link"))
    }) {
        return true;
    }
    if run.cancelled.load(Ordering::Relaxed) || run.started.elapsed() >= SEARCH_DEADLINE {
        return false;
    }
    let path = run.root.join(
        flags
            .native
            .as_deref()
            .unwrap_or_else(|| Path::new(relative)),
    );
    let Ok(mut item) = entry_for_path_with_git(&path, false, run.git_enabled) else {
        return false;
    };
    item["relative"] = json!(relative);
    spec_matches(run.spec, &item)
}

pub(super) fn candidate_rows(run: &SearchRun<'_>, hits: Vec<Hit>, ranked: bool) -> Vec<Value> {
    let mut rows = Vec::new();
    let mut retained = 0_usize;
    let mut visibility = crate::visibility::Visibility::new(run.root, run.show_hidden);
    for hit in hits {
        if run.cancelled.load(Ordering::Relaxed) || run.started.elapsed() >= SEARCH_DEADLINE {
            break;
        }
        if rows.len() >= run.limit {
            break;
        }
        let path = hit.entry.path(run.root);
        if !visibility.path(&path) {
            continue;
        }
        let Ok(mut item) = entry_for_path_with_git(&path, false, run.git_enabled) else {
            continue;
        };
        item["relative"] = json!(hit.entry.relative);
        if !spec_matches(run.spec, &item) {
            continue;
        }
        annotate_spans(run.spec, &mut item, &hit.indices);
        item["score"] = json!(if ranked { hit.score } else { 0 });
        let size = serde_json::to_vec(&item)
            .map(|data| data.len())
            .unwrap_or(SEARCH_RESPONSE_BYTES);
        if retained.saturating_add(size) > SEARCH_RESPONSE_BYTES {
            break;
        }
        retained += size;
        rows.push(item);
    }
    rows
}

#[derive(Default)]
pub(super) struct SeenRepositories {
    pub(super) repositories: HashMap<PathBuf, Option<GitRepository>>,
    pub(super) roots: HashSet<PathBuf>,
}

pub(super) fn decorate_rows(
    run: &SearchRun<'_>,
    rows: &mut [Value],
    repository_roots: &[String],
) -> SeenRepositories {
    if !run.git_enabled {
        return SeenRepositories::default();
    }
    let mut marker_cache = HashMap::new();
    let root_marker = nearest_git_marker(&path_text(run.root), &mut marker_cache);
    let mut row_markers = rows
        .iter()
        .map(|row| nearest_git_marker(row["path"].as_str().unwrap_or_default(), &mut marker_cache))
        .collect::<Vec<_>>();
    let trusted = known_markers(
        run.root,
        repository_roots.iter().take(64),
        &mut marker_cache,
    );
    let mut markers = Vec::new();
    let mut marker_seen = HashSet::new();
    for marker in root_marker
        .iter()
        .chain(row_markers.iter().flatten())
        .chain(trusted.iter())
    {
        if markers.len() >= SEARCH_REPOSITORY_CAP {
            break;
        }
        if marker_seen.insert(marker.clone()) {
            markers.push(marker.clone());
        }
    }
    let remaining = SEARCH_DEADLINE.saturating_sub(run.started.elapsed());
    let by_marker =
        cached_git_repositories_for_markers_bounded(&markers, run.cancelled, remaining, false);
    let allowed = markers.iter().cloned().collect::<HashSet<_>>();
    for marker in &mut row_markers {
        if marker
            .as_ref()
            .is_some_and(|value| !allowed.contains(value))
        {
            *marker = None;
        }
    }
    let mut roots = HashSet::new();
    let mut indexes = HashMap::new();
    for (row, marker) in rows.iter_mut().zip(&row_markers) {
        if run.cancelled.load(Ordering::Relaxed) || run.started.elapsed() >= SEARCH_DEADLINE {
            break;
        }
        let Some(repository) = repository_for_marker(&by_marker, marker.as_ref()) else {
            continue;
        };
        roots.insert(repository.root.clone());
        let index = match indexes.entry(repository.root.clone()) {
            std::collections::hash_map::Entry::Occupied(entry) => entry.into_mut(),
            std::collections::hash_map::Entry::Vacant(entry) => {
                let Some(index) = repository_status_index_cancellable(repository, run.cancelled)
                else {
                    break;
                };
                entry.insert(index)
            }
        };
        let Ok(path) = parse_path(row["path"].as_str().unwrap_or_default()) else {
            continue;
        };
        let is_dir = row["is_dir"].as_bool().unwrap_or(false);
        let status = indexed_git_status_for_path(index, &path, is_dir);
        let counts = indexed_git_counts_for_path(index, &path, is_dir);
        decorate_git_entry(row, repository, status, counts);
    }
    for marker in root_marker.iter().chain(trusted.iter()) {
        if let Some(repository) = repository_for_marker(&by_marker, Some(marker)) {
            roots.insert(repository.root.clone());
        }
    }
    SeenRepositories {
        repositories: by_marker,
        roots,
    }
}

pub(super) fn spec_matches(spec: &SearchSpec, item: &Value) -> bool {
    let name = item["name"].as_str().unwrap_or_default();
    let relative = item["relative"].as_str().unwrap_or_default();
    if !spec
        .verified_terms()
        .all(|term| term.matches(name, relative))
    {
        return false;
    }
    let kinds = entry_kinds(item);
    let wanted_kinds = spec.values("type", false).collect::<HashSet<_>>();
    let excluded_kinds = spec.values("type", true).collect::<HashSet<_>>();
    if (!wanted_kinds.is_empty() && kinds.is_disjoint(&wanted_kinds))
        || !kinds.is_disjoint(&excluded_kinds)
    {
        return false;
    }
    let extension = extension_of(name);
    if !allowed_value(
        &extension,
        spec.values("format", false),
        spec.values("format", true),
    ) {
        return false;
    }
    let mime = item["mime"].as_str().unwrap_or_default().to_lowercase();
    if !allowed_starts_with(
        &mime,
        spec.values("mime", false),
        spec.values("mime", true),
        false,
    ) {
        return false;
    }
    allowed_starts_with(
        &relative.to_lowercase(),
        spec.values("in", false),
        spec.values("in", true),
        true,
    )
}

pub(super) fn entry_kinds(item: &Value) -> HashSet<&'static str> {
    let mut result = HashSet::new();
    result.insert(if item["is_dir"].as_bool().unwrap_or(false) {
        "dir"
    } else {
        "file"
    });
    if item["is_symlink"].as_bool().unwrap_or(false) {
        result.insert("link");
    }
    if item["is_git_repo"].as_bool().unwrap_or(false) {
        result.insert("repo");
    }
    let mime = item["mime"].as_str().unwrap_or_default();
    for (prefix, kind) in [
        ("image/", "image"),
        ("text/", "text"),
        ("video/", "video"),
        ("audio/", "audio"),
    ] {
        if mime.to_lowercase().starts_with(prefix) {
            result.insert(kind);
        }
    }
    result
}

pub(super) fn allowed_value<'a>(
    value: &str,
    wanted: impl Iterator<Item = &'a str>,
    excluded: impl Iterator<Item = &'a str>,
) -> bool {
    let wanted = wanted.collect::<Vec<_>>();
    let excluded = excluded.collect::<Vec<_>>();
    (wanted.is_empty() || wanted.contains(&value)) && !excluded.contains(&value)
}

pub(super) fn allowed_starts_with<'a>(
    value: &str,
    wanted: impl Iterator<Item = &'a str>,
    excluded: impl Iterator<Item = &'a str>,
    directory_boundary: bool,
) -> bool {
    let matches = |prefix: &str| {
        let prefix = prefix.to_lowercase();
        value.starts_with(&prefix)
            && (!directory_boundary
                || value.len() == prefix.len()
                || value.as_bytes().get(prefix.len()) == Some(&b'/'))
    };
    let wanted = wanted.collect::<Vec<_>>();
    let excluded = excluded.collect::<Vec<_>>();
    (wanted.is_empty() || wanted.iter().any(|prefix| matches(prefix)))
        && !excluded.iter().any(|prefix| matches(prefix))
}

pub(super) fn annotate_spans(spec: &SearchSpec, item: &mut Value, indices: &[u32]) {
    let name = item["name"].as_str().unwrap_or_default().to_string();
    let relative = item["relative"].as_str().unwrap_or_default().to_string();
    let relative_chars = relative.chars().count();
    let name_offset = relative_chars.saturating_sub(name.chars().count());
    let mut relative_spans = index::spans_from_indices(indices);
    let mut name_spans = Vec::new();
    for (start, end) in &relative_spans {
        let clipped_start = (*start).max(name_offset).saturating_sub(name_offset);
        let clipped_end = (*end).min(relative_chars).saturating_sub(name_offset);
        if clipped_end > clipped_start {
            name_spans.push((clipped_start, clipped_end));
        }
    }
    for term in spec.verified_terms().filter(|term| !term.negate) {
        if term.field == "name" {
            for (start, end) in term.occurrences(&name) {
                name_spans.push((start, end));
                relative_spans.push((start + name_offset, end + name_offset));
            }
            continue;
        }
        for (start, end) in term.occurrences(&relative) {
            relative_spans.push((start, end));
            let clipped_start = start.max(name_offset).saturating_sub(name_offset);
            let clipped_end = end.min(relative_chars).saturating_sub(name_offset);
            if clipped_end > clipped_start {
                name_spans.push((clipped_start, clipped_end));
            }
        }
    }
    item["relative_spans"] = json!(serialize_spans(relative_spans));
    item["name_spans"] = json!(serialize_spans(name_spans));
}

pub(super) fn serialize_spans(mut spans: Vec<(usize, usize)>) -> String {
    spans.sort_unstable();
    let mut merged: Vec<(usize, usize)> = Vec::new();
    for (start, end) in spans {
        if end <= start {
            continue;
        }
        if let Some(last) = merged.last_mut().filter(|last| start <= last.1) {
            last.1 = last.1.max(end);
        } else {
            merged.push((start, end));
        }
    }
    merged
        .into_iter()
        .map(|(start, end)| format!("{start}-{end}"))
        .collect::<Vec<_>>()
        .join(",")
}

pub(super) fn known_markers<'a>(
    root: &Path,
    roots: impl Iterator<Item = &'a String>,
    cache: &mut HashMap<PathBuf, Option<PathBuf>>,
) -> Vec<PathBuf> {
    let mut result = Vec::new();
    for raw_root in roots {
        let Ok(candidate) = parse_path(raw_root) else {
            continue;
        };
        if candidate != root && !candidate.starts_with(root) {
            continue;
        }
        if let Some(marker) = nearest_git_marker(&path_text(&candidate), cache)
            && !result.contains(&marker)
        {
            result.push(marker);
        }
    }
    result
}

pub(super) fn repository_for_marker<'a>(
    repositories: &'a HashMap<PathBuf, Option<GitRepository>>,
    marker: Option<&PathBuf>,
) -> Option<&'a GitRepository> {
    marker
        .and_then(|value| repositories.get(value))
        .and_then(Option::as_ref)
}

pub(super) fn append_deleted_matches(
    rows: &mut Vec<Value>,
    run: &SearchRun<'_>,
    seen: &SeenRepositories,
    pattern: Option<&Pattern>,
) {
    let mut existing = rows
        .iter()
        .filter_map(|row| row["path"].as_str().map(ToOwned::to_owned))
        .collect::<HashSet<_>>();
    let mut retained = rows
        .iter()
        .filter_map(|row| serde_json::to_vec(row).ok().map(|data| data.len()))
        .sum::<usize>();
    let mut matcher = Matcher::new(index::matcher_config());
    let mut visited_repositories = HashSet::new();
    let repositories = seen
        .repositories
        .values()
        .flatten()
        .filter(|repository| seen.roots.contains(&repository.root))
        .filter(|repository| visited_repositories.insert(repository.root.clone()));
    for (repository, status) in repositories.flat_map(|repository| {
        repository
            .entries
            .iter()
            .map(move |status| (repository, status))
    }) {
        if run.cancelled.load(Ordering::Relaxed) || run.started.elapsed() >= SEARCH_DEADLINE {
            return;
        }
        let path = path_text(&status.path);
        if !status.deleted || !status.path.starts_with(run.root) || existing.contains(&path) {
            continue;
        }
        let relative = relative_text(&status.path, run.root);
        if relative == "."
            || !crate::visibility::Visibility::new(run.root, run.show_hidden).path(&status.path)
        {
            continue;
        }
        let mut item = deleted_git_entry(status, repository);
        item["relative"] = json!(relative);
        let scored = match pattern {
            Some(pattern) => index::score_text(pattern, &relative, &mut matcher),
            None => Some((0, Vec::new())),
        };
        let Some((score, indices)) = scored.filter(|_| spec_matches(run.spec, &item)) else {
            continue;
        };
        annotate_spans(run.spec, &mut item, &indices);
        item["score"] = json!(score);
        let size = serde_json::to_vec(&item)
            .map(|data| data.len())
            .unwrap_or(SEARCH_RESPONSE_BYTES);
        if retained.saturating_add(size) > SEARCH_RESPONSE_BYTES {
            return;
        }
        retained += size;
        existing.insert(path);
        rows.push(item);
    }
}

pub(super) fn sort_rows(
    rows: &mut [Value],
    ranked: bool,
    frecency: &std::collections::HashMap<String, f64>,
) {
    let visits = |row: &Value| {
        frecency
            .get(row["path"].as_str().unwrap_or_default())
            .copied()
            .unwrap_or(0.0)
    };
    rows.sort_by(|left, right| {
        let left_relative = left["relative"].as_str().unwrap_or_default();
        let right_relative = right["relative"].as_str().unwrap_or_default();
        let alphabetical = || {
            left_relative
                .to_lowercase()
                .cmp(&right_relative.to_lowercase())
                .then_with(|| left_relative.cmp(right_relative))
        };
        if !ranked {
            return alphabetical();
        }
        let left_score = left["score"].as_u64().unwrap_or(0);
        let right_score = right["score"].as_u64().unwrap_or(0);
        right_score
            .cmp(&left_score)
            .then_with(|| visits(right).total_cmp(&visits(left)))
            .then_with(|| left_relative.len().cmp(&right_relative.len()))
            .then_with(alphabetical)
    });
}

pub(super) fn retain_response_budget(rows: &mut Vec<Value>, response_overhead: usize) {
    let mut retained = response_overhead;
    let mut count = 0_usize;
    for row in rows.iter() {
        let size = serde_json::to_vec(row)
            .map(|data| data.len() + 1)
            .unwrap_or(SEARCH_RESPONSE_BYTES);
        if retained.saturating_add(size) > SEARCH_RESPONSE_BYTES {
            break;
        }
        retained += size;
        count += 1;
    }
    rows.truncate(count);
}

pub(super) fn relative_text(path: &Path, root: &Path) -> String {
    path.strip_prefix(root)
        .ok()
        .filter(|value| !value.as_os_str().is_empty())
        .map(display_path)
        .unwrap_or_else(|| ".".to_string())
}
