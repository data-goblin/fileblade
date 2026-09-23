use super::{Capability, Connection, Descriptor, Kind, tailnet};
use crate::command::{CommandSpec, which};
use crate::{AppError, AppResult};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Saved {
    pub host: String,
    pub user: String,
    pub path: String,
}

impl Saved {
    pub fn uri(&self) -> AppResult<String> {
        if !tailnet::valid_host(&self.host)
            || self.user.is_empty()
            || self.user.len() > 128
            || self
                .user
                .bytes()
                .any(|ch| !ch.is_ascii_alphanumeric() && !b"._-".contains(&ch))
            || !self.path.starts_with('/')
            || self.path.len() > 4096
            || self.path.contains('\0')
        {
            return Err(AppError::invalid(
                "SFTP location requires a host, SSH user and absolute remote path",
            ));
        }
        let host = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        let mut uri = url::Url::parse(&format!("sftp://{host}/"))
            .map_err(|error| AppError::invalid(error.to_string()))?;
        uri.set_username(&self.user)
            .map_err(|_| AppError::invalid("invalid SSH user"))?;
        uri.path_segments_mut()
            .map_err(|_| AppError::invalid("invalid SFTP path"))?
            .clear()
            .extend(self.path[1..].split('/'));
        Ok(uri.to_string())
    }
}

static CONNECTED: Mutex<Option<HashMap<String, Descriptor>>> = Mutex::new(None);
static CONNECTING: Mutex<BTreeMap<String, Option<String>>> = Mutex::new(BTreeMap::new());

struct Pending(String);
impl Pending {
    fn reserve(id: &str, host: Option<&str>) -> AppResult<Self> {
        let mut pending = CONNECTING
            .lock()
            .map_err(|_| AppError::command("SFTP connections unavailable"))?;
        if pending.contains_key(id) {
            return Err(AppError::invalid(
                "a connection change is already pending for this peer",
            ));
        }
        if host.is_some() {
            let sessions = CONNECTED
                .lock()
                .map_err(|_| AppError::command("SFTP connections unavailable"))?;
            let connected = sessions.as_ref();
            if connected.is_some_and(|sessions| sessions.contains_key(id)) {
                return Err(AppError::invalid(
                    "disconnect this peer before changing its connection",
                ));
            }
            let reserved = pending
                .keys()
                .filter(|id| connected.is_none_or(|sessions| !sessions.contains_key(*id)))
                .count();
            if connected.map_or(0, HashMap::len) + reserved >= 64 {
                return Err(AppError::invalid("at most 64 SFTP peers can be connected"));
            }
        }
        pending.insert(id.to_owned(), host.map(str::to_owned));
        Ok(Self(id.to_owned()))
    }
}

impl Drop for Pending {
    fn drop(&mut self) {
        if let Ok(mut pending) = CONNECTING.lock() {
            pending.remove(&self.0);
        }
    }
}

fn gio(arguments: &[&str], cancelled: &AtomicBool) -> AppResult<String> {
    let program = which("gio").ok_or_else(|| AppError::command("GIO is not installed"))?;
    let output = CommandSpec::new(program)
        .args(arguments.iter().copied())
        .env("LC_ALL", "C")
        .timeout(Duration::from_secs(30))
        .limits(8 * 1024 * 1024, 16384)
        .stop_on_output_limit()
        .run_cancellable(cancelled)?;
    if !output.status.success() {
        let error = String::from_utf8_lossy(&output.stderr);
        let prompt = String::from_utf8_lossy(&output.stdout);
        return Err(AppError::command(format!(
            "SFTP request failed: {}{}",
            error.trim(),
            if prompt.is_empty() {
                ""
            } else {
                "; existing SSH authorization is required; interactive credential and host-key prompts are not accepted"
            }
        )));
    }
    String::from_utf8(output.stdout).map_err(|error| AppError::invalid(error.to_string()))
}

pub fn connect(
    candidate: &tailnet::Candidate,
    saved: &Saved,
    cancelled: &AtomicBool,
) -> AppResult<Descriptor> {
    if candidate.host != saved.host
        && (candidate.ssh_host.is_empty() || candidate.ssh_host != saved.host)
    {
        return Err(AppError::invalid(
            "selected peer changed; refresh Locations",
        ));
    }
    let uri = saved.uri()?;
    let id = candidate.location.id.clone();
    let _pending = Pending::reserve(&id, Some(&candidate.host))?;
    let query = [
        "info",
        "--attributes=standard::type,access::can-read",
        "--",
        uri.as_str(),
    ];
    let mut descriptor = Descriptor {
        schema: 1,
        id: id.clone(),
        kind: Kind::Sftp,
        canonical_uri: uri.clone(),
        label: candidate.location.label.clone(),
        connection: Connection::Connected,
        session_generation: uuid::Uuid::new_v4().to_string(),
        local_representation: None,
        capabilities: BTreeSet::from([Capability::List]),
        error: None,
    };
    let mut mounted = false;
    let result = (|| {
        let info = match gio(&query, cancelled) {
            Ok(info) => info,
            Err(_) if !cancelled.load(Ordering::Relaxed) => {
                gio(&["mount", "--", &uri], cancelled)?;
                mounted = true;
                gio(&query, cancelled)?
            }
            Err(error) => return Err(error),
        };
        if !info.lines().any(|line| line.trim() == "standard::type: 2") {
            return Err(AppError::invalid("selected remote path is not a directory"));
        }
        match info
            .lines()
            .find_map(|line| line.trim().strip_prefix("access::can-read: "))
        {
            Some("FALSE" | "false" | "0") => {
                return Err(AppError::invalid("remote directory denies read access"));
            }
            Some("TRUE" | "true" | "1") => {}
            _ => {
                gio(&["list", "--nofollow-symlinks", "--", &uri], cancelled)?;
            }
        }
        if cancelled.load(Ordering::Relaxed) {
            return Err(AppError::Cancelled);
        }
        let pending = CONNECTING
            .lock()
            .map_err(|_| AppError::command("SFTP connections unavailable"))?;
        if pending.get(&id).and_then(Option::as_deref) != Some(candidate.host.as_str()) {
            return Err(AppError::invalid(
                "peer discovery changed during connect; refresh Locations",
            ));
        }
        CONNECTED
            .lock()
            .map_err(|_| AppError::command("SFTP sessions unavailable"))?
            .get_or_insert_with(HashMap::new)
            .insert(id.clone(), descriptor.clone());
        Ok(descriptor.clone())
    })();
    if let Err(error) = &result
        && mounted
        && let Err(cleanup) = gio(&["mount", "--unmount", "--", &uri], &AtomicBool::new(false))
    {
        descriptor.connection = Connection::Unavailable;
        descriptor.capabilities.clear();
        descriptor.error = Some(format!("{error}; disconnect required: {cleanup}"));
        CONNECTED
            .lock()
            .map_err(|_| AppError::command("SFTP sessions unavailable"))?
            .get_or_insert_with(HashMap::new)
            .insert(id, descriptor.clone());
        return Ok(descriptor);
    }
    result
}

pub fn retain_candidates(candidates: &[tailnet::Candidate]) {
    let Ok(mut pending) = CONNECTING.lock() else {
        return;
    };
    let valid = |id: &str, host: &str| {
        candidates
            .iter()
            .any(|candidate| candidate.location.id == id && candidate.host == host)
    };
    for (id, host) in pending.iter_mut() {
        if host.as_ref().is_some_and(|host| !valid(id, host)) {
            *host = None;
        }
    }
    if let Ok(mut connected) = CONNECTED.lock()
        && let Some(connected) = connected.as_mut()
    {
        connected.retain(|id, session| {
            session.connection == Connection::Unavailable
                || candidates.iter().any(|candidate| {
                    candidate.location.id == *id
                        && url::Url::parse(&candidate.location.canonical_uri)
                            .ok()
                            .zip(url::Url::parse(&session.canonical_uri).ok())
                            .is_some_and(|(discovered, session)| {
                                discovered.host() == session.host()
                                    || (!candidate.ssh_host.is_empty()
                                        && session.host_str() == Some(candidate.ssh_host.as_str()))
                            })
                })
        });
    }
}

pub fn cleanup_locations() -> Vec<Descriptor> {
    CONNECTED
        .lock()
        .ok()
        .and_then(|sessions| {
            sessions.as_ref().map(|sessions| {
                sessions
                    .values()
                    .filter(|session| session.connection == Connection::Unavailable)
                    .cloned()
                    .collect()
            })
        })
        .unwrap_or_default()
}

pub fn snapshot(id: &str) -> Option<Descriptor> {
    CONNECTED.lock().ok()?.as_ref()?.get(id).cloned()
}

fn invalidate(id: &str, generation: &str) {
    if let Ok(mut sessions) = CONNECTED.lock()
        && let Some(sessions) = sessions.as_mut()
        && sessions
            .get(id)
            .is_some_and(|value| value.session_generation == generation)
    {
        sessions.remove(id);
    }
}

pub fn disconnect(id: &str, generation: &str, cancelled: &AtomicBool) -> Value {
    let _pending = match Pending::reserve(id, None) {
        Ok(pending) => pending,
        Err(error) => return json!({"ok":false,"error":error.to_string(),"location":id}),
    };
    let Some(current) = snapshot(id).filter(|value| value.session_generation == generation) else {
        return stale(id, generation);
    };
    invalidate(id, generation);
    match gio(
        &["mount", "--unmount", "--", &current.canonical_uri],
        cancelled,
    ) {
        Ok(_) => json!({"ok":true,"location":id,"disconnected":true}),
        Err(error) => {
            let mut cleanup = current;
            cleanup.connection = Connection::Unavailable;
            cleanup.capabilities.clear();
            cleanup.error = Some(error.to_string());
            if let Ok(mut sessions) = CONNECTED.lock() {
                sessions
                    .get_or_insert_with(HashMap::new)
                    .insert(id.to_owned(), cleanup.clone());
            }
            json!({"ok":false,"location":cleanup,"disconnected":false,"error":error.to_string()})
        }
    }
}

pub fn stale(id: &str, generation: &str) -> Value {
    json!({"ok":false,"error_id":"stale-location","error":"SFTP session changed or disconnected; refresh Locations",
        "location":id,"generation":generation,"entries":[]})
}

pub fn list(options: &crate::backend::LocationListArgs, cancelled: &AtomicBool) -> Value {
    let Some(current) =
        snapshot(&options.location).filter(|value| value.session_generation == options.generation)
    else {
        return stale(&options.location, &options.generation);
    };
    if !current.capabilities.contains(&Capability::List) {
        return stale(&options.location, &options.generation);
    }
    let mut uri = match url::Url::parse(&current.canonical_uri) {
        Ok(uri) => uri,
        Err(_) => return stale(&options.location, &options.generation),
    };
    let path = std::path::Path::new(&options.path);
    if path.components().any(|part| {
        !matches!(
            part,
            std::path::Component::Normal(_) | std::path::Component::CurDir
        )
    }) || options.path.contains("://")
    {
        return json!({"ok":false,"error_id":"invalid-location-path","error":"remote path must be relative without parent traversal","entries":[]});
    }
    if options.path != "." && !options.path.is_empty() {
        let mut segments = uri.path_segments_mut().unwrap();
        segments.pop_if_empty();
        for part in path.components() {
            if let std::path::Component::Normal(name) = part {
                segments.push(&name.to_string_lossy());
            }
        }
    }
    let output = gio(
        &[
            "list",
            "--print-uris",
            "--nofollow-symlinks",
            "--hidden",
            "--attributes=time::modified,time::created",
            "--",
            uri.as_str(),
        ],
        cancelled,
    );
    let output = match output {
        Ok(output) => output,
        Err(error) => {
            invalidate(&options.location, &options.generation);
            let mut result = stale(&options.location, &options.generation);
            result["error"] = json!(error.to_string());
            return result;
        }
    };
    if snapshot(&options.location).is_none_or(|value| {
        value.session_generation != options.generation
            || !value.capabilities.contains(&Capability::List)
    }) {
        return stale(&options.location, &options.generation);
    }
    match parse_listing(&output, &uri, options) {
        Ok(entries) => entries,
        Err(error) => json!({"ok":false,"error":error.to_string(),"entries":[]}),
    }
}

fn parse_listing(
    output: &str,
    parent: &url::Url,
    options: &crate::backend::LocationListArgs,
) -> AppResult<Value> {
    let filter: Value = serde_json::from_str(&options.filter)?;
    if !filter.is_object() || filter.as_object().is_some_and(|filter| !filter.is_empty()) {
        return Err(AppError::invalid(
            "remote metadata filtering is not available yet",
        ));
    }
    if !["name", "size", "modified", "created", "type"].contains(&options.sort.as_str()) {
        return Err(AppError::invalid("unsupported remote listing sort"));
    }
    let mut entries = Vec::new();
    for (index, line) in output.lines().enumerate() {
        if index >= 10000 {
            return Err(AppError::invalid("remote listing exceeds 10000 entries"));
        }
        let fields: Vec<_> = line.splitn(4, '\t').collect();
        if fields.len() < 3 {
            return Err(AppError::invalid("GIO returned an invalid listing"));
        }
        let uri =
            url::Url::parse(fields[0]).map_err(|error| AppError::invalid(error.to_string()))?;
        if uri.scheme() != "sftp"
            || uri.host_str() != parent.host_str()
            || uri.username() != parent.username()
            || uri.port() != parent.port()
            || uri.password().is_some()
            || uri.query().is_some()
            || uri.fragment().is_some()
        {
            return Err(AppError::invalid(
                "GIO returned a different remote authority",
            ));
        }
        let mut parent_segments = parent
            .path_segments()
            .ok_or_else(|| AppError::invalid("invalid remote parent"))?
            .collect::<Vec<_>>();
        while parent_segments.last() == Some(&"") {
            parent_segments.pop();
        }
        let child_segments = uri
            .path_segments()
            .ok_or_else(|| AppError::invalid("invalid remote child"))?
            .collect::<Vec<_>>();
        if child_segments.len() != parent_segments.len() + 1
            || !child_segments.starts_with(&parent_segments)
        {
            return Err(AppError::invalid(
                "GIO returned an entry outside the requested directory",
            ));
        }
        let name = uri
            .path_segments()
            .and_then(|mut segments| segments.next_back())
            .unwrap_or("");
        let decoded = crate::common::parse_path(&format!("file:///{name}"))?;
        if decoded.parent() != Some(std::path::Path::new("/")) {
            return Err(AppError::invalid("GIO returned a path in a filename"));
        }
        let name = decoded
            .file_name()
            .ok_or_else(|| AppError::invalid("GIO returned an invalid name"))?
            .to_string_lossy()
            .into_owned();
        if !options.show_hidden && name.starts_with('.') {
            continue;
        }
        let size = fields[1]
            .parse::<u64>()
            .map_err(|_| AppError::invalid("GIO returned an invalid size"))?;
        let timestamp = |key: &str| {
            fields
                .get(3)
                .into_iter()
                .flat_map(|value| value.split(' '))
                .find_map(|field| field.strip_prefix(key)?.parse::<i64>().ok())
        };
        let date = |value| {
            chrono::DateTime::from_timestamp(value, 0)
                .map(|date| date.to_rfc3339_opts(chrono::SecondsFormat::Secs, true))
                .unwrap_or_default()
        };
        let modified = timestamp("time::modified=").map(date).unwrap_or_default();
        let created = timestamp("time::created=").map(date).unwrap_or_default();
        entries.push(json!({"name":name,"path":uri.as_str(),"is_dir":fields[2]=="(directory)","is_symlink":fields[2]=="(symlink)",
            "size":size,"modified":modified,"created":created,"location":options.location,"generation":options.generation}));
    }
    entries.sort_by(|a, b| {
        let directories = b["is_dir"].as_bool().cmp(&a["is_dir"].as_bool());
        let key = options.sort.as_str();
        let values = if key == "size" {
            a[key].as_u64().cmp(&b[key].as_u64())
        } else if key == "type" {
            a["name"]
                .as_str()
                .unwrap_or("")
                .rsplit('.')
                .next()
                .cmp(&b["name"].as_str().unwrap_or("").rsplit('.').next())
        } else {
            a[key].as_str().cmp(&b[key].as_str())
        };
        directories
            .then(if options.desc {
                values.reverse()
            } else {
                values
            })
            .then(a["name"].as_str().cmp(&b["name"].as_str()))
    });
    let total = entries.len();
    let start = options.start.min(total);
    let count = options.count.clamp(1, 10000);
    let rows = entries
        .into_iter()
        .skip(start)
        .take(count)
        .collect::<Vec<_>>();
    Ok(
        json!({"ok":true,"location":options.location,"generation":options.generation,"path":parent.as_str(),
        "entries":rows,"total":total,"start":start,"windowed":true,"truncated":start.saturating_add(count)<total}),
    )
}
