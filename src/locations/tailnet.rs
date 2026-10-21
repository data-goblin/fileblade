use super::{Connection, Descriptor, Kind, disconnected};
use crate::command::{CommandSpec, which};
use crate::{AppError, AppResult};
use serde::Serialize;
use serde_json::Value;
use std::collections::BTreeSet;
use std::net::IpAddr;
use std::sync::atomic::AtomicBool;
use std::time::Duration;

const MAX_PEERS: usize = 4096;
const MAX_SSH_CONFIG_LOOKUPS: usize = 64;

#[derive(Clone, Debug, Serialize)]
pub struct Candidate {
    #[serde(flatten)]
    pub location: Descriptor,
    pub host: String,
    pub online: bool,
    pub ssh_host: String,
    pub ssh_user: String,
}

pub fn discover(cancelled: &AtomicBool) -> AppResult<Vec<Candidate>> {
    let program =
        which("tailscale").ok_or_else(|| AppError::command("tailscale is not installed"))?;
    let output = CommandSpec::new(program)
        .args(["status", "--json"])
        .timeout(Duration::from_secs(3))
        .limits(8 * 1024 * 1024, 8192)
        .stop_on_output_limit()
        .run_cancellable(cancelled)?;
    if !output.status.success() {
        return Err(AppError::command(
            "tailscale status failed; check the existing Tailscale connection",
        ));
    }
    candidates(&serde_json::from_slice(&output.stdout)?)
}

pub fn candidates(status: &Value) -> AppResult<Vec<Candidate>> {
    if status["BackendState"].as_str() != Some("Running") {
        return Err(AppError::invalid(
            "Tailscale is not running; connect it before discovering peers",
        ));
    }
    let peers = status["Peer"]
        .as_object()
        .ok_or_else(|| AppError::invalid("Tailscale returned no peer inventory"))?;
    if peers.len() > MAX_PEERS {
        return Err(AppError::invalid(
            "tailnet peer inventory exceeds 4096 peers",
        ));
    }
    let self_id = status["Self"]["ID"].as_str();
    let mut ids = BTreeSet::new();
    let mut result = Vec::new();
    for (key, peer) in peers {
        let id = peer["ID"]
            .as_str()
            .filter(|id| !id.is_empty())
            .unwrap_or(key);
        if Some(id) == self_id
            || id.len() > 256
            || id.chars().any(char::is_control)
            || !ids.insert(id.to_string())
        {
            continue;
        }
        let host = peer["DNSName"]
            .as_str()
            .map(|host| host.trim_end_matches('.'))
            .filter(|host| valid_host(host))
            .map(str::to_string)
            .or_else(|| {
                peer["TailscaleIPs"]
                    .as_array()?
                    .iter()
                    .filter_map(|value| value.as_str()?.parse::<IpAddr>().ok())
                    .next()
                    .map(|ip| ip.to_string())
            });
        let Some(host) = host else { continue };
        let authority = if host.contains(':') {
            format!("[{host}]")
        } else {
            host.clone()
        };
        let label = peer["HostName"]
            .as_str()
            .filter(|name| !name.is_empty())
            .unwrap_or(&host)
            .chars()
            .filter(|ch| {
                !ch.is_control()
                    && !matches!(*ch, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
            })
            .take(128)
            .collect::<String>();
        let location = disconnected(
            &format!("tailnet:{id}"),
            Kind::Sftp,
            &format!("sftp://{authority}/"),
            &label,
            Connection::Disconnected,
        )?;
        let (ssh_host, ssh_user) = if result.len() < MAX_SSH_CONFIG_LOOKUPS {
            configured_ssh(&host)
        } else {
            (String::new(), String::new())
        };
        result.push(Candidate {
            location,
            host,
            online: peer["Online"].as_bool().unwrap_or(false),
            ssh_host,
            ssh_user,
        });
    }
    result.sort_by(|left, right| {
        left.location
            .label
            .to_lowercase()
            .cmp(&right.location.label.to_lowercase())
            .then(left.location.id.cmp(&right.location.id))
    });
    Ok(result)
}

pub(super) fn configured_ssh(host: &str) -> (String, String) {
    let alias = host.split('.').next().unwrap_or_default();
    if alias.is_empty() || !valid_host(alias) {
        return (String::new(), String::new());
    }
    let Some(program) = which("ssh") else {
        return (String::new(), String::new());
    };
    let Ok(output) = CommandSpec::new(program)
        .args(["-G", "--", alias])
        .env("LC_ALL", "C.UTF-8")
        .timeout(Duration::from_secs(2))
        .limits(256 * 1024, 4096)
        .stop_on_output_limit()
        .run_cancellable(&AtomicBool::new(false))
    else {
        return (String::new(), String::new());
    };
    if !output.status.success() {
        return (String::new(), String::new());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let field = |name: &str| {
        text.lines()
            .find_map(|line| line.strip_prefix(name)?.split_whitespace().next())
            .unwrap_or_default()
            .to_string()
    };
    let resolved = field("hostname ");
    let user = field("user ");
    let matches_peer = resolved.eq_ignore_ascii_case(host) || resolved.eq_ignore_ascii_case(alias);
    if !matches_peer || user.is_empty() || user.len() > 128 {
        return (String::new(), String::new());
    }
    (alias.to_string(), user)
}

pub(super) fn valid_host(host: &str) -> bool {
    if host.parse::<IpAddr>().is_ok() {
        return true;
    }
    !host.is_empty()
        && host.len() <= 253
        && host.is_ascii()
        && host.split('.').all(|label| {
            !label.is_empty()
                && label.len() <= 63
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}
