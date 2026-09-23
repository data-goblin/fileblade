use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::PathBuf;
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::mpsc::{self, Receiver};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};
use tempfile::TempDir;

struct Server {
    child: Option<Child>,
    stdin: Option<ChildStdin>,
    frames: Receiver<Value>,
    reader: Option<JoinHandle<()>>,
}

#[test]
fn usage_watches_report_appends_before_the_writer_closes_the_file() {
    let root = TempDir::new().unwrap();
    let path = root.path().join("session.jsonl");
    let mut transcript = std::fs::File::create(&path).unwrap();
    let mut server = Server::start_with(&[("XDG_STATE_HOME", root.path())]);
    server.send(json!({"v": 1, "type": "hello"}));
    assert_eq!(server.receive()["ok"], true);
    server.send(json!({
        "v": 1, "type": "subscribe", "id": "usage", "generation": 1,
        "topic": "filesystem", "paths": [root.path()], "includeWrites": true,
    }));
    assert_eq!(server.receive()["type"], "subscribed");
    transcript.write_all(b"{\"type\":\"assistant\"}\n").unwrap();
    transcript.flush().unwrap();
    let event = server.receive_where(|frame| frame["type"] == "event");
    assert_eq!(event["path"], path.to_str().unwrap());
    assert_eq!(event["events"], json!(["modify"]));
    server.finish();
    drop(transcript);
}

#[test]
fn handshake_resolves_the_screenshot_directory_from_xdg_user_dirs() {
    let root = TempDir::new().unwrap();
    let config = root.path().join("config");
    std::fs::create_dir_all(config.join("omarchy/fileblade")).unwrap();
    std::fs::write(
        config.join("user-dirs.dirs"),
        "XDG_PICTURES_DIR=\"$HOME/Bilder\"\n",
    )
    .unwrap();
    std::fs::write(
        config.join("omarchy/fileblade/user-dirs.dirs"),
        "XDG_PICTURES_DIR=\"/wrong\"\n",
    )
    .unwrap();
    let mut server = Server::start_with(&[
        ("HOME", root.path()),
        ("XDG_CONFIG_HOME", &config),
        ("XDG_STATE_HOME", root.path()),
        ("OMARCHY_SCREENSHOT_DIR", std::path::Path::new("")),
        ("XDG_PICTURES_DIR", std::path::Path::new("")),
    ]);
    server.send(json!({"v":1,"type":"hello"}));
    assert_eq!(
        server.receive()["paths"]["screenshots"],
        root.path().join("Bilder").to_str().unwrap()
    );
    server.finish();
}

#[test]
fn navigation_writes_preserve_reads_and_file_mutations_refresh_listing_and_search() {
    let root = TempDir::new().unwrap();
    let directory = root.path().join("files");
    std::fs::create_dir(&directory).unwrap();
    let path = directory.to_str().unwrap();
    let mut server = Server::start_with(&[
        ("XDG_STATE_HOME", root.path()),
        ("XDG_CONFIG_HOME", root.path()),
    ]);
    server.send(json!({"v":1,"type":"hello"}));
    assert_eq!(server.receive()["ok"], true);
    let mut generation = 0;
    let mut request = |command: &str, arguments: Value| {
        generation += 1;
        server.send(
            json!({"v":1,"type":"request","id":"cache","generation":generation,
            "command":command,"arguments":arguments}),
        );
        let response = server.receive_where(|frame| frame["type"] == "response");
        assert_eq!(response["ok"], true, "{response}");
        response["payload"].clone()
    };
    let listing = json!(["--path", path, "--no-git"]);
    let search = json!(["--root", path, "--query", "new.txt", "--no-git"]);
    assert_eq!(request("children-window", listing.clone())["total"], 0);
    assert_eq!(request("search", search.clone())["entries"], json!([]));
    for (command, arguments) in [
        ("state-write", json!(["--document", "{}"])),
        ("layout-write", json!(["--document", "{}"])),
        ("frecency-visit", json!(["--path", path])),
    ] {
        assert_eq!(request(command, arguments)["ok"], true);
        assert_eq!(request("children-window", listing.clone())["total"], 0);
        assert_eq!(request("search", search.clone())["entries"], json!([]));
    }
    assert_eq!(
        request("create", json!(["--parent", path, "--name", "new.txt"]))["ok"],
        true
    );
    assert_eq!(
        request("children-window", listing.clone())["entries"][0]["name"],
        "new.txt"
    );
    assert_eq!(
        request("search", search.clone())["entries"][0]["name"],
        "new.txt"
    );
    let renamed = request(
        "rename",
        json!(["--path", directory.join("new.txt"), "--name", "old.txt"]),
    );
    assert_eq!(renamed["ok"], true, "{renamed}");
    assert_eq!(
        request("children-window", listing)["entries"][0]["name"],
        "old.txt"
    );
    assert_eq!(request("search", search)["entries"], json!([]));
    server.finish();
}

impl Server {
    fn start() -> Self {
        Self::start_with(&[])
    }

    fn start_with(environment: &[(&str, &std::path::Path)]) -> Self {
        Self::start_with_limit(environment, 4)
    }

    fn start_with_limit(environment: &[(&str, &std::path::Path)], concurrency: usize) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_fileblade"))
            .args(["serve", "--max-concurrency", &concurrency.to_string()])
            .envs(environment.iter().map(|(key, value)| (*key, *value)))
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .expect("start resident backend");
        let stdin = child.stdin.take().expect("resident stdin");
        let stdout = child.stdout.take().expect("resident stdout");
        let (sender, frames) = mpsc::channel();
        let reader = thread::spawn(move || {
            for line in BufReader::new(stdout).lines() {
                let Ok(line) = line else { break };
                let Ok(frame) = serde_json::from_str(&line) else {
                    break;
                };
                if sender.send(frame).is_err() {
                    break;
                }
            }
        });
        Self {
            child: Some(child),
            stdin: Some(stdin),
            frames,
            reader: Some(reader),
        }
    }

    fn send(&mut self, frame: Value) {
        let stdin = self.stdin.as_mut().expect("resident stdin is open");
        serde_json::to_writer(&mut *stdin, &frame).expect("write protocol frame");
        stdin.write_all(b"\n").expect("terminate protocol frame");
        stdin.flush().expect("flush protocol frame");
    }

    fn receive(&self) -> Value {
        self.frames
            .recv_timeout(Duration::from_secs(5))
            .expect("resident protocol response")
    }

    fn receive_where(&self, predicate: impl Fn(&Value) -> bool) -> Value {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            let frame = self
                .frames
                .recv_timeout(remaining)
                .expect("matching resident protocol response");
            if predicate(&frame) {
                return frame;
            }
        }
    }

    fn finish(mut self) {
        self.stdin.take();
        let deadline = Instant::now() + Duration::from_secs(3);
        let child = self.child.as_mut().expect("resident child");
        loop {
            if let Some(status) = child.try_wait().expect("resident child status") {
                assert!(status.success(), "resident backend exited with {status}");
                break;
            }
            assert!(
                Instant::now() < deadline,
                "resident backend ignored stdin EOF"
            );
            thread::sleep(Duration::from_millis(20));
        }
        self.child.take();
        if let Some(reader) = self.reader.take() {
            reader.join().expect("resident reader thread");
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        self.stdin.take();
        if let Some(mut child) = self.child.take() {
            let _ = child.kill();
            let _ = child.wait();
        }
        if let Some(reader) = self.reader.take() {
            let _ = reader.join();
        }
    }
}

fn executable(name: &str) -> PathBuf {
    std::env::split_paths(&std::env::var_os("PATH").unwrap_or_default())
        .map(|directory| directory.join(name))
        .find(|path| path.is_file())
        .unwrap_or_else(|| panic!("{name} is not installed"))
}

#[test]
fn resident_navigation_reuses_git_status_until_an_explicit_refresh() {
    let fixture = TempDir::new().expect("temporary fixture");
    let repository = fixture.path().join("repository");
    let nested = repository.join("nested");
    std::fs::create_dir_all(&nested).expect("repository directories");
    std::fs::write(repository.join("changed.txt"), b"changed").expect("repository file");
    let real_git = executable("git");
    assert!(
        Command::new(&real_git)
            .args(["init", "-q"])
            .current_dir(&repository)
            .status()
            .expect("git init")
            .success()
    );

    let bin = fixture.path().join("bin");
    let git_log = fixture.path().join("git.log");
    std::fs::create_dir(&bin).expect("wrapper directory");
    let wrapper = bin.join("git");
    std::fs::write(
        &wrapper,
        "#!/bin/sh\nprintf '%s\\n' \"$*\" >> \"$FILEBLADE_TEST_GIT_LOG\"\nexec \"$FILEBLADE_TEST_REAL_GIT\" \"$@\"\n",
    )
    .expect("git wrapper");
    std::fs::set_permissions(&wrapper, std::fs::Permissions::from_mode(0o755))
        .expect("executable git wrapper");
    let path = std::env::join_paths(std::iter::once(bin).chain(std::env::split_paths(
        &std::env::var_os("PATH").unwrap_or_default(),
    )))
    .expect("wrapper PATH");
    let path = PathBuf::from(path);
    let mut server = Server::start_with(&[
        ("PATH", &path),
        ("FILEBLADE_TEST_REAL_GIT", &real_git),
        ("FILEBLADE_TEST_GIT_LOG", &git_log),
    ]);
    server.send(json!({"v": 1, "type": "hello"}));
    assert_eq!(server.receive()["ok"], true);

    for (id, path, fresh) in [
        ("root", repository.as_path(), false),
        ("nested", nested.as_path(), false),
        ("fresh", repository.as_path(), true),
    ] {
        let mut arguments = vec!["--path".to_string(), path.to_string_lossy().into_owned()];
        if fresh {
            arguments.push("--fresh-git".to_string());
        }
        server.send(json!({
            "v": 1,
            "type": "request",
            "id": id,
            "generation": 1,
            "command": "children-batch",
            "arguments": arguments,
            "deadline_ms": 5000
        }));
        assert_eq!(server.receive_where(|frame| frame["id"] == id)["ok"], true);
        let scans = std::fs::read_to_string(&git_log)
            .expect("git invocation log")
            .lines()
            .filter(|line| line.split_whitespace().any(|argument| argument == "status"))
            .count();
        assert_eq!(scans, if fresh { 2 } else { 1 });
    }
    server.finish();
}

#[test]
fn resident_handshake_requests_subscription_and_eof_are_coherent() {
    let fixture = TempDir::new().expect("temporary fixture");
    std::fs::create_dir(fixture.path().join("folder")).expect("fixture directory");
    std::fs::write(fixture.path().join("alpha.txt"), b"alpha").expect("fixture file");

    let mut server = Server::start();
    server.send(json!({"v": 1, "type": "hello"}));
    let hello = server.receive();
    assert_eq!(hello["type"], "hello");
    assert_eq!(hello["ok"], true);
    assert_eq!(hello["protocol"], "fileblade");
    assert_eq!(hello["limits"]["concurrency"], 4);

    let directory = fixture.path().to_string_lossy();
    server.send(json!({
        "v": 1,
        "type": "request",
        "id": "children-1",
        "generation": 7,
        "command": "children-window",
        "arguments": ["--path", directory, "--show-hidden"],
        "deadline_ms": 2000
    }));
    let children = server.receive_where(|frame| frame["id"] == "children-1");
    assert_eq!(children["type"], "response");
    assert_eq!(children["generation"], 7);
    assert_eq!(children["ok"], true);
    let entries = children["payload"]["entries"]
        .as_array()
        .expect("children entries");
    assert_eq!(entries.len(), 2);

    server.send(json!({
        "v": 1,
        "type": "request",
        "id": "children-1",
        "generation": 7,
        "command": "children-window",
        "arguments": ["--path", directory]
    }));
    let duplicate = server.receive_where(|frame| frame["id"] == "children-1");
    assert_eq!(duplicate["type"], "error");
    assert_eq!(duplicate["ok"], false);
    assert!(
        duplicate["error"]
            .as_str()
            .expect("duplicate error")
            .contains("unique")
    );

    server.send(json!({
        "v": 1,
        "type": "subscribe",
        "id": "watch-1",
        "generation": "tree-9",
        "topic": "filesystem",
        "paths": [directory]
    }));
    let subscribed = server.receive_where(|frame| frame["type"] == "subscribed");
    assert_eq!(subscribed["id"], "watch-1");
    assert_eq!(subscribed["generation"], "tree-9");

    std::fs::write(fixture.path().join("created.txt"), b"created").expect("watched file");
    let event = server.receive_where(|frame| {
        frame["type"] == "event" && frame["id"] == "watch-1" && frame["name"] == "created.txt"
    });
    assert_eq!(event["topic"], "filesystem");
    assert_eq!(event["overflow"], false);
    assert!(
        event["events"]
            .as_array()
            .is_some_and(|events| !events.is_empty())
    );

    server.send(json!({
        "v": 1,
        "type": "cancel",
        "id": "watch-1",
        "generation": "tree-9"
    }));
    let accepted = server.receive_where(|frame| frame["type"] == "cancel");
    assert_eq!(accepted["accepted"], true);
    let cancelled =
        server.receive_where(|frame| frame["type"] == "response" && frame["id"] == "watch-1");
    assert_eq!(cancelled["ok"], false);
    assert_eq!(cancelled["cancelled"], true);

    server.finish();
}

#[test]
fn resident_request_keys_keep_working_past_the_recent_window() {
    let fixture = TempDir::new().expect("temporary fixture");
    let directory = fixture.path().to_string_lossy().to_string();
    let mut server = Server::start();
    server.send(json!({"v": 1, "type": "hello"}));
    let hello = server.receive();
    let window = hello["limits"]["request_keys"]
        .as_u64()
        .expect("recent request key window") as usize;
    let total = window + 16;
    let mut sent = 0;
    while sent < total {
        let batch: Vec<String> = (sent..(sent + 4).min(total))
            .map(|index| format!("children-{index}"))
            .collect();
        for id in &batch {
            server.send(json!({
                "v": 1,
                "type": "request",
                "id": id,
                "generation": 1,
                "command": "children-window",
                "arguments": ["--path", directory]
            }));
        }
        let mut remaining = batch.clone();
        while !remaining.is_empty() {
            let response = server.receive_where(|frame| frame["type"] != "progress");
            let id = response["id"].as_str().unwrap_or("").to_string();
            assert!(remaining.contains(&id), "unexpected frame: {response}");
            assert_eq!(response["type"], "response", "request {id}: {response}");
            assert_eq!(response["ok"], true, "request {id}: {response}");
            remaining.retain(|item| item != &id);
        }
        sent += batch.len();
    }
    server.finish();
}

#[test]
fn a_completed_request_releases_its_slot_before_the_client_refills_it() {
    let fixture = TempDir::new().expect("temporary fixture");
    let mut server = Server::start_with_limit(&[], 1);
    server.send(json!({"v": 1, "type": "hello"}));
    assert_eq!(server.receive()["limits"]["concurrency"], 1);
    for id in 0..512 {
        let id = id.to_string();
        server.send(json!({
            "v": 1, "type": "request", "id": id, "generation": 1,
            "command": "children-window", "arguments": ["--path", fixture.path()]
        }));
        let response = server.receive_where(|frame| frame["type"] != "progress");
        assert_eq!(response["id"], id);
        assert_eq!(response["type"], "response", "{response}");
        assert_eq!(response["ok"], true, "{response}");
    }
    server.finish();
}

#[test]
fn resident_module_dirs_returns_both_private_paths() {
    let homes = TempDir::new().expect("temporary homes");
    let state_home = homes.path().join("state");
    let config_home = homes.path().join("config");
    std::fs::create_dir_all(&state_home).expect("state home");
    std::fs::create_dir_all(&config_home).expect("config home");
    let mut server = Server::start_with(&[
        ("XDG_STATE_HOME", state_home.as_path()),
        ("XDG_CONFIG_HOME", config_home.as_path()),
    ]);
    server.send(json!({"v": 1, "type": "hello"}));
    let hello = server.receive();
    assert_eq!(hello["ok"], true);

    server.send(json!({
        "v": 1,
        "type": "request",
        "id": "dirs-1",
        "generation": 1,
        "command": "module-dirs",
        "arguments": ["--module", "data-goblin.blade-example/clock"]
    }));
    let response = server.receive_where(|frame| frame["id"] == "dirs-1");
    assert_eq!(response["type"], "response", "{response}");
    assert_eq!(response["ok"], true, "{response}");
    let payload = &response["payload"];
    assert_eq!(payload["name"], "data-goblin.blade-example+clock");
    let state_dir = state_home.join("omarchy/fileblade/modules/data-goblin.blade-example+clock");
    let config_dir = config_home.join("omarchy/fileblade/config/data-goblin.blade-example+clock");
    assert_eq!(payload["state_dir"], state_dir.to_string_lossy().as_ref());
    assert_eq!(payload["config_dir"], config_dir.to_string_lossy().as_ref());
    assert!(state_dir.is_dir());
    assert!(config_dir.is_dir());

    server.send(json!({
        "v": 1,
        "type": "request",
        "id": "dirs-2",
        "generation": 1,
        "command": "module-dirs",
        "arguments": ["--module", "../escape"]
    }));
    let refused = server.receive_where(|frame| frame["id"] == "dirs-2");
    assert_eq!(refused["ok"], false, "{refused}");
    assert!(
        !config_home
            .join("omarchy/fileblade/config/../escape")
            .exists()
    );
    server.finish();
}
