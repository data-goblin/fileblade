use fileblade::{
    backend,
    locations::{sftp, tailnet},
};
use serde_json::json;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::sync::atomic::AtomicBool;

#[test]
#[ignore = "requires an isolated D VM SSH/GVfs fixture"]
fn candidate_connects_lists_and_revoked_access_invalidates_generation() {
    let root = PathBuf::from(
        std::env::var_os("FILEBLADE_SFTP_PROBE_ROOT").expect("VM fixture root required"),
    );
    let cancelled = AtomicBool::new(false);
    let candidates = tailnet::discover(&cancelled).unwrap();
    assert_eq!(candidates.len(), 1);
    assert!(candidates[0].location.session_generation.is_empty());
    assert!(candidates[0].location.capabilities.is_empty());
    let discovered = backend::dispatch(
        backend::parse(["fileblade", "locations"]).unwrap(),
        &cancelled,
        &mut |_| Ok(()),
    )
    .unwrap();
    let candidate = discovered["locations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|location| location["id"] == candidates[0].location.id)
        .unwrap();
    assert_eq!(candidate["connection"], "disconnected");
    assert_eq!(candidate["capabilities"], json!([]));
    assert_eq!(candidate["session_generation"], "");
    assert!(sftp::snapshot(&candidates[0].location.id).is_none());
    assert_eq!(
        fs::read_to_string(root.join("discovery-call"))
            .unwrap()
            .trim(),
        "status --json"
    );
    let remote = root.join("remote");
    let saved = sftp::Saved {
        host: candidates[0].host.clone(),
        user: "omarchy".into(),
        path: remote.to_str().unwrap().into(),
    };
    let command = backend::parse([
        "fileblade",
        "location-connect",
        "--location",
        &candidates[0].location.id,
        "--expected-host",
        &candidates[0].host,
        "--user",
        "omarchy",
        "--path",
        remote.to_str().unwrap(),
        "--save",
    ])
    .unwrap();
    assert!(backend::mutating(&command));
    let response = backend::dispatch(command, &cancelled, &mut |_| Ok(())).unwrap();
    assert_eq!(response["ok"], true, "{response}");
    assert_eq!(response["saved"], true, "{response}");
    assert_eq!(
        fileblade::locations::saved::read().unwrap(),
        vec![saved.clone()]
    );
    let connected = sftp::snapshot(&candidates[0].location.id).unwrap();
    assert_eq!(
        serde_json::to_value(&connected.capabilities).unwrap(),
        json!(["list"])
    );
    let inventory = backend::dispatch(
        backend::parse(["fileblade", "locations"]).unwrap(),
        &cancelled,
        &mut |_| Ok(()),
    )
    .unwrap();
    let visible = inventory["locations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|location| location["id"] == connected.id)
        .unwrap();
    assert_eq!(visible["connection"], "connected", "{visible}");
    assert_eq!(visible["session_generation"], connected.session_generation);
    assert!(connected.local_representation.is_none());
    assert!(!connected.session_generation.is_empty());
    let command = backend::parse([
        "fileblade",
        "list",
        "--location",
        &connected.id,
        "--generation",
        &connected.session_generation,
        "--no-git",
    ])
    .unwrap();
    let listed = backend::dispatch(command, &cancelled, &mut |_| Ok(())).unwrap();
    assert_eq!(listed["ok"], true, "{listed}");
    let entries = listed["entries"].as_array().unwrap();
    assert_eq!(entries.len(), 3, "{listed}");
    for name in ["space #percent%.txt", "line\nbreak.txt", "雪.txt"] {
        let entry = entries
            .iter()
            .find(|entry| entry["name"] == name)
            .expect("remote name preserved");
        assert!(entry["path"].as_str().unwrap().starts_with("sftp://"));
    }
    fs::set_permissions(&remote, fs::Permissions::from_mode(0o000)).unwrap();
    let command = backend::parse([
        "fileblade",
        "list",
        "--location",
        &connected.id,
        "--generation",
        &connected.session_generation,
        "--no-git",
    ])
    .unwrap();
    let denied = backend::dispatch(command, &cancelled, &mut |_| Ok(())).unwrap();
    let denied_connect = sftp::connect(&candidates[0], &saved, &cancelled);
    fs::set_permissions(&remote, fs::Permissions::from_mode(0o700)).unwrap();
    assert!(denied_connect.is_err(), "{denied_connect:?}");
    assert_eq!(denied["ok"], false, "{denied}");
    assert_eq!(denied["error_id"], "stale-location", "{denied}");
    assert!(sftp::snapshot(&connected.id).is_none());
    let reconnected = sftp::connect(&candidates[0], &saved, &cancelled).unwrap();
    assert_ne!(reconnected.session_generation, connected.session_generation);
    let old_disconnect = sftp::disconnect(&connected.id, &connected.session_generation, &cancelled);
    assert_eq!(old_disconnect["error_id"], "stale-location");
    assert!(sftp::snapshot(&reconnected.id).is_some());
    let disconnected =
        sftp::disconnect(&reconnected.id, &reconnected.session_generation, &cancelled);
    assert_eq!(disconnected["ok"], true, "{disconnected}");
    assert!(sftp::snapshot(&reconnected.id).is_none());
    fs::write(root.join("results.json"), serde_json::to_vec_pretty(&json!({"candidate":candidate,"connected":connected,"listing":listed,"denied":denied,"denied_connect":denied_connect.unwrap_err().to_string(),"disconnected":disconnected,"discovery":"synthetic status fixture","transport":"real loopback SSH through GVfs"})).unwrap()).unwrap();
}

#[test]
#[ignore = "requires an explicitly authorized live tailnet peer and read-only directory"]
fn configured_alias_survives_refresh_and_lists_remote_directory() {
    let alias = std::env::var("FILEBLADE_SFTP_ALIAS").expect("authorized SSH alias required");
    let path = std::env::var("FILEBLADE_SFTP_DIRECTORY").expect("authorized directory required");
    let cancelled = AtomicBool::new(false);
    let candidates = tailnet::discover(&cancelled).unwrap();
    let candidate = candidates
        .iter()
        .find(|peer| peer.ssh_host == alias)
        .unwrap();
    assert_ne!(candidate.host, candidate.ssh_host);
    let connected = backend::dispatch(
        backend::parse([
            "fileblade",
            "location-connect",
            "--location",
            &candidate.location.id,
            "--expected-host",
            &alias,
            "--user",
            &candidate.ssh_user,
            "--path",
            &path,
        ])
        .unwrap(),
        &cancelled,
        &mut |_| Ok(()),
    )
    .unwrap();
    assert_eq!(connected["ok"], true, "{connected}");
    let generation = connected["location"]["session_generation"]
        .as_str()
        .unwrap();
    let inventory = backend::dispatch(
        backend::parse(["fileblade", "locations"]).unwrap(),
        &cancelled,
        &mut |_| Ok(()),
    )
    .unwrap();
    let listed = backend::dispatch(
        backend::parse([
            "fileblade",
            "list",
            "--location",
            &candidate.location.id,
            "--generation",
            generation,
            "--no-git",
        ])
        .unwrap(),
        &cancelled,
        &mut |_| Ok(()),
    )
    .unwrap();
    let disconnected = sftp::disconnect(&candidate.location.id, generation, &cancelled);
    let visible = inventory["locations"]
        .as_array()
        .unwrap()
        .iter()
        .find(|peer| peer["id"] == candidate.location.id)
        .unwrap();
    assert_eq!(visible["connection"], "connected", "{visible}");
    assert_eq!(visible["session_generation"], generation);
    assert_eq!(listed["ok"], true, "{listed}");
    assert!(
        !listed["entries"].as_array().unwrap().is_empty(),
        "{listed}"
    );
    assert_eq!(disconnected["ok"], true, "{disconnected}");
}
