use crate::common::{
    expanded_os_path, expanded_path, file_name, parse_path, path_error, path_text,
};
use serde_json::{Value, json};
use std::os::unix::fs::MetadataExt;
use std::path::{Path, PathBuf};

const PRIMARY_MARKERS: [&str; 1] = [".git"];
const AGENT_MARKERS: [&str; 9] = [
    ".claude",
    ".agents",
    ".codex",
    ".opencode",
    ".pi",
    ".github",
    ".mcp.json",
    "AGENTS.md",
    "CLAUDE.md",
];
const MAX_WALK: usize = 64;

pub const SKILLS_MARKERS: [&str; 1] = [".git"];
pub const AGENT_MARKERS_WITH_GIT: [&str; 7] = [
    ".git",
    ".claude",
    ".agents",
    ".codex",
    ".opencode",
    ".pi",
    ".github",
];
pub const SKILLS_WALK: usize = 32;
pub const AGENT_WALK: usize = 24;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum WalkBoundary {
    FilesystemRoot,
    HomeParent,
}

#[derive(Clone, Copy)]
pub struct RootSearch<'a> {
    pub markers: &'a [&'a str],
    pub max_walk: usize,
    pub boundary: WalkBoundary,
}

pub const SKILLS_SEARCH: RootSearch<'static> = RootSearch {
    markers: &SKILLS_MARKERS,
    max_walk: SKILLS_WALK,
    boundary: WalkBoundary::FilesystemRoot,
};

pub const AGENT_SEARCH: RootSearch<'static> = RootSearch {
    markers: &AGENT_MARKERS_WITH_GIT,
    max_walk: AGENT_WALK,
    boundary: WalkBoundary::HomeParent,
};

pub fn marker_root(start: &Path, search: RootSearch<'_>) -> Option<(PathBuf, String)> {
    let boundary = match search.boundary {
        WalkBoundary::FilesystemRoot => None,
        WalkBoundary::HomeParent => {
            let home = expanded_path("~");
            Some(home.parent().unwrap_or(&home).to_path_buf())
        }
    };
    if start.as_os_str().is_empty() {
        return None;
    }
    let start = expanded_os_path(start);
    for directory in walked(start, boundary.as_deref(), search.max_walk) {
        if !eligible_marker_parent(&directory) {
            continue;
        }
        if let Some(marker) = has_marker(&directory, search.markers) {
            return Some((directory, marker.to_string()));
        }
    }
    None
}

pub fn project_root(raw_path: &str) -> Value {
    let path = match parse_path(raw_path) {
        Ok(path) => path,
        Err(error) => return path_error(raw_path, &error),
    };
    let start = if path.is_dir() {
        path
    } else {
        path.parent()
            .unwrap_or_else(|| Path::new("/"))
            .to_path_buf()
    };
    let home = expanded_path("~")
        .canonicalize()
        .unwrap_or_else(|_| expanded_path("~"));
    let start = start.canonicalize().unwrap_or(start);
    let chain = walked(start, Some(&home), MAX_WALK);
    for markers in [&PRIMARY_MARKERS[..], &AGENT_MARKERS[..]] {
        for directory in &chain {
            if !eligible_marker_parent(directory) {
                continue;
            }
            if let Some(marker) = has_marker(directory, markers) {
                return json!({
                    "ok": true,
                    "root": path_text(directory),
                    "name": file_name(directory),
                    "marker": marker,
                    "inferred": false
                });
            }
        }
    }
    let fallback = chain.first().cloned().unwrap_or(home);
    json!({
        "ok": true,
        "root": path_text(&fallback),
        "name": file_name(&fallback),
        "marker": "",
        "inferred": true
    })
}

fn walked(start: PathBuf, boundary: Option<&Path>, max_walk: usize) -> Vec<PathBuf> {
    let mut chain = Vec::new();
    let mut current = start;
    for _ in 0..max_walk {
        if boundary.is_some_and(|edge| current == edge) || current.parent().is_none() {
            break;
        }
        chain.push(current.clone());
        if !current.pop() {
            break;
        }
    }
    chain
}

fn has_marker<'a>(directory: &Path, markers: &'a [&str]) -> Option<&'a str> {
    markers
        .iter()
        .copied()
        .find(|marker| std::fs::symlink_metadata(directory.join(marker)).is_ok())
}

fn eligible_marker_parent(directory: &Path) -> bool {
    std::fs::metadata(directory)
        .map(|metadata| metadata.mode() & 0o1002 != 0o1002)
        .unwrap_or(true)
}
