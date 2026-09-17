use crate::AppResult;
use crate::command::{CommandOutput, CommandSpec, which};
use crate::common::{parse_path, path_text};
use serde_json::{Value, json};
use std::collections::BTreeMap;
use std::ffi::OsStr;
use std::os::unix::ffi::OsStrExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

const MAX_REPOSITORIES: usize = 16;
const MAX_SUBJECTS: usize = 20;
const REMOTE_TIMEOUT: Duration = Duration::from_secs(20);
const LOCAL_TIMEOUT: Duration = Duration::from_secs(5);
const OUTPUT_LIMIT: usize = 64 * 1024;
const MAX_REMOTE_REFS: usize = 512;
const MAX_VERSION_BYTES: usize = 64;

pub struct RepositorySpec {
    pub id: String,
    pub path: PathBuf,
}

pub fn parse_specs(raw: &[String]) -> Vec<RepositorySpec> {
    raw.iter()
        .take(MAX_REPOSITORIES)
        .filter_map(|entry| {
            let (id, path) = entry.split_once('=')?;
            let id = id.trim();
            if !(path.starts_with('/') || path.starts_with("file://")) {
                return None;
            }
            let path = parse_path(path).ok()?;
            let valid_id = !id.is_empty()
                && id.len() <= 128
                && id
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || b"._-".contains(&byte));
            (valid_id && path.is_absolute()).then(|| RepositorySpec {
                id: id.to_string(),
                path,
            })
        })
        .collect()
}

fn git(
    path: &Path,
    arguments: &[&str],
    timeout: Duration,
    cancelled: &AtomicBool,
) -> AppResult<CommandOutput> {
    let program = which("git").ok_or_else(|| crate::AppError::command("git is not installed"))?;
    CommandSpec::new(program)
        .args(["-c", "core.fsmonitor=false", "-C"])
        .args([path])
        .args(arguments)
        .env("GIT_OPTIONAL_LOCKS", "0")
        .env("GIT_NO_LAZY_FETCH", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_ASKPASS", "")
        .env("GIT_SSH_COMMAND", "ssh -o BatchMode=yes")
        .env("LC_ALL", "C")
        .timeout(timeout)
        .limits(OUTPUT_LIMIT, 16 * 1024)
        .stop_on_output_limit()
        .resource_limits(16 * 1024 * 1024, 512 * 1024 * 1024)
        .run_cancellable(cancelled)
}

fn git_text(path: &Path, arguments: &[&str], cancelled: &AtomicBool) -> Option<String> {
    let output = git(path, arguments, LOCAL_TIMEOUT, cancelled).ok()?;
    if !output.status.success() || output.stdout_truncated {
        return None;
    }
    Some(String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn parse_version(text: &str) -> Option<semver::Version> {
    (text.len() <= MAX_VERSION_BYTES)
        .then(|| semver::Version::parse(text).ok())
        .flatten()
}

fn manifest_version(text: &str) -> String {
    serde_json::from_str::<Value>(text)
        .ok()
        .and_then(|value| value["version"].as_str().and_then(parse_version))
        .map(|version| version.to_string())
        .unwrap_or_default()
}

fn current_manifest_version(path: &Path) -> String {
    crate::secure::read_bounded_nofollow(&path.join("manifest.json"), 64 * 1024)
        .ok()
        .flatten()
        .and_then(|bytes| String::from_utf8(bytes).ok())
        .map(|text| manifest_version(&text))
        .unwrap_or_default()
}

struct Standing {
    ahead: u64,
    dirty: bool,
    head: String,
    upstream: String,
}

fn standing(path: &Path, cancelled: &AtomicBool) -> Result<Standing, String> {
    let upstream = git_text(
        path,
        &["rev-parse", "--abbrev-ref", "--symbolic-full-name", "@{u}"],
        cancelled,
    )
    .filter(|value| !value.is_empty())
    .unwrap_or_else(|| "origin/HEAD".into());
    let head = git_text(path, &["rev-parse", "HEAD"], cancelled).ok_or("unable to read HEAD")?;
    let ahead = git_text(
        path,
        &[
            "rev-list",
            "--left-right",
            "--count",
            &format!("HEAD...{upstream}"),
        ],
        cancelled,
    )
    .and_then(|text| text.split_whitespace().next()?.parse().ok())
    .unwrap_or(0);
    let status = git(
        path,
        &["status", "--porcelain=v1", "-z", "--untracked-files=no"],
        LOCAL_TIMEOUT,
        cancelled,
    )
    .map_err(|error| error.to_string())?;
    if !status.status.success() {
        return Err("unable to read the working tree status".to_string());
    }
    Ok(Standing {
        ahead,
        dirty: !status.stdout.is_empty(),
        head,
        upstream,
    })
}

struct RemoteRefs {
    head: String,
    version: String,
}

fn parse_remote_refs(text: &str, reference: &str) -> Result<RemoteRefs, String> {
    let mut refs = BTreeMap::new();
    for (index, line) in text.lines().enumerate() {
        if index >= MAX_REMOTE_REFS {
            return Err("remote reference count exceeded".to_string());
        }
        let (head, name) = line
            .split_once('\t')
            .ok_or_else(|| "invalid remote reference response".to_string())?;
        if !matches!(head.len(), 40 | 64)
            || !head.bytes().all(|byte| byte.is_ascii_hexdigit())
            || name.len() > 1024
            || (name != reference && !name.starts_with("refs/tags/v"))
            || refs.insert(name, head.to_ascii_lowercase()).is_some()
        {
            return Err("invalid remote reference response".to_string());
        }
    }
    let head = refs
        .get(reference)
        .ok_or_else(|| "upstream branch unavailable".to_string())?
        .clone();
    let releases: Vec<_> = refs
        .iter()
        .filter_map(|(name, target)| {
            let version = parse_version(name.strip_prefix("refs/tags/v")?)?;
            let commit = refs.get(format!("{name}^{{}}").as_str()).unwrap_or(target);
            Some((version, commit))
        })
        .collect();
    let newest = releases
        .iter()
        .map(|(version, _)| version)
        .max_by(|a, b| a.cmp_precedence(b));
    let version = releases
        .iter()
        .filter(|(version, commit)| {
            **commit == head && newest.is_some_and(|newest| version.cmp_precedence(newest).is_eq())
        })
        .map(|(version, _)| version)
        .max()
        .map(ToString::to_string)
        .unwrap_or_default();
    Ok(RemoteRefs { head, version })
}

fn remote_refs(root: &Path, cancelled: &AtomicBool) -> Result<RemoteRefs, String> {
    let branch = git_text(
        root,
        &["symbolic-ref", "--quiet", "--short", "HEAD"],
        cancelled,
    )
    .filter(|value| !value.is_empty());
    let (remote, reference) = if let Some(branch) = branch {
        let remote = git_text(
            root,
            &["config", "--get", &format!("branch.{branch}.remote")],
            cancelled,
        )
        .filter(|value| !value.is_empty() && !value.starts_with('-'))
        .ok_or("no upstream branch")?;
        let reference = git_text(
            root,
            &["config", "--get", &format!("branch.{branch}.merge")],
            cancelled,
        )
        .filter(|value| value.starts_with("refs/heads/") && value.len() <= 1024)
        .ok_or("no upstream branch")?;
        (remote, reference)
    } else {
        ("origin".into(), "HEAD".into())
    };
    let output = git(
        root,
        &[
            "ls-remote",
            "--exit-code",
            "--",
            &remote,
            &reference,
            "refs/tags/v*",
        ],
        REMOTE_TIMEOUT,
        cancelled,
    )
    .map_err(|error| format!("remote check failed: {error}"))?;
    if !output.status.success() || output.stdout_truncated || output.stderr_truncated {
        return Err("remote check failed or exceeded its output limit".to_string());
    }
    let text =
        std::str::from_utf8(&output.stdout).map_err(|_| "invalid remote response".to_string())?;
    parse_remote_refs(text, &reference)
}

fn repository_root(path: &Path, cancelled: &AtomicBool) -> Result<PathBuf, String> {
    let resolved = std::fs::canonicalize(path).map_err(|error| error.to_string())?;
    if !resolved.is_dir() {
        return Err("not a directory".to_string());
    }
    let output = git(
        &resolved,
        &["rev-parse", "--show-toplevel"],
        LOCAL_TIMEOUT,
        cancelled,
    )
    .map_err(|error| error.to_string())?;
    if !output.status.success() || output.stdout_truncated {
        return Err("not a git checkout".to_string());
    }
    let top = output.stdout.strip_suffix(b"\n").unwrap_or(&output.stdout);
    if std::fs::canonicalize(Path::new(OsStr::from_bytes(top)))
        .ok()
        .as_deref()
        != Some(resolved.as_path())
    {
        return Err("plugin directory is not the checkout root".to_string());
    }
    Ok(resolved)
}

fn failure(spec: &RepositorySpec, error: String) -> Value {
    json!({
        "id": spec.id,
        "path": path_text(&spec.path),
        "ok": false,
        "updatable": false,
        "error": error
    })
}

pub fn check(specs: &[RepositorySpec], core: &str, cancelled: &AtomicBool) -> Value {
    let mut repositories = Vec::with_capacity(specs.len());
    let mut available = false;
    for spec in specs {
        let root = match repository_root(&spec.path, cancelled) {
            Ok(root) => root,
            Err(error) => {
                repositories.push(failure(spec, error));
                continue;
            }
        };
        let remote = match remote_refs(&root, cancelled) {
            Ok(remote) => remote,
            Err(error) => {
                repositories.push(failure(spec, error));
                continue;
            }
        };
        let remote_head = &remote.head;
        let standing = match standing(&root, cancelled) {
            Ok(standing) => standing,
            Err(error) => {
                repositories.push(failure(spec, error));
                continue;
            }
        };
        let comparison = git_text(
            &root,
            &[
                "rev-list",
                "--left-right",
                "--count",
                &format!("HEAD...{remote_head}"),
            ],
            cancelled,
        )
        .and_then(|text| {
            let mut fields = text.split_whitespace();
            Some((
                fields.next()?.parse::<u64>().ok()?,
                fields.next()?.parse::<u64>().ok()?,
            ))
        });
        let ahead = comparison.map_or(standing.ahead, |counts| counts.0);
        let behind = comparison.map(|counts| counts.1);
        let subjects: Vec<String> = git_text(
            &root,
            &[
                "log",
                "--format=%s",
                "--max-count=20",
                &format!("HEAD..{remote_head}"),
            ],
            cancelled,
        )
        .map(|text| {
            text.lines()
                .take(MAX_SUBJECTS)
                .map(|line| line.chars().take(120).collect())
                .collect()
        })
        .unwrap_or_default();
        let current_version = current_manifest_version(&root);
        let upstream_version = git_text(
            &root,
            &["show", &format!("{remote_head}:manifest.json")],
            cancelled,
        )
        .map(|text| manifest_version(&text))
        .unwrap_or(remote.version);
        let version_change = match (
            parse_version(&current_version),
            parse_version(&upstream_version),
        ) {
            (Some(current), Some(upstream)) => match upstream.cmp_precedence(&current) {
                std::cmp::Ordering::Greater => "newer",
                std::cmp::Ordering::Equal => "same",
                std::cmp::Ordering::Less => "older",
            },
            _ => "unknown",
        };
        let is_core = spec.id == core;
        let backend_stale = is_core && current_version != env!("CARGO_PKG_VERSION");
        let updatable = *remote_head != standing.head && ahead == 0 && !standing.dirty;
        available |= updatable;
        repositories.push(json!({
            "id": spec.id,
            "path": path_text(&root),
            "ok": true,
            "error": "",
            "comparison_known": comparison.is_some(),
            "behind": behind,
            "ahead": ahead,
            "dirty": standing.dirty,
            "head": standing.head,
            "upstream": standing.upstream,
            "upstream_head": remote_head,
            "current_version": current_version,
            "upstream_version": upstream_version,
            "version_change": version_change,
            "subjects": subjects,
            "core": is_core,
            "backend_version": env!("CARGO_PKG_VERSION"),
            "backend_stale": backend_stale,
            "updatable": updatable
        }));
    }
    json!({
        "ok": true,
        "available": available,
        "checked_at": std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|value| value.as_secs())
            .unwrap_or(0),
        "repositories": repositories
    })
}
