use fileblade::lease::{Authority, LeaseError, WriteMode};
use serde_json::{Value, json};
use std::collections::HashMap;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::{MetadataExt, PermissionsExt, symlink};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tempfile::{TempDir, tempdir};

static TEST_LOCK: Mutex<()> = Mutex::new(());

#[test]
fn aliases_contend_on_the_same_kernel_lock_and_release_without_replacing_it() {
    let _serial = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temporary = tempdir().unwrap();
    let root = temporary.path().join("state");
    let first = Authority::acquire(&root).unwrap();
    let inode = fs::metadata(root.join("authority.lock")).unwrap().ino();
    let alias = temporary.path().join("alias");
    symlink(&root, &alias).unwrap();
    match Authority::acquire(&alias).unwrap_err() {
        LeaseError::Held { diagnostic } => assert_eq!(diagnostic.unwrap().pid, std::process::id()),
        other => panic!("{other}"),
    }
    drop(first);
    let second = Authority::acquire(&alias).unwrap();
    assert_eq!(second.root(), root);
    assert_eq!(
        fs::metadata(root.join("authority.lock")).unwrap().ino(),
        inode
    );
}

#[test]
fn symlinks_hardlinks_and_writable_roots_are_refused_without_touching_the_target() {
    let _serial = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temporary = tempdir().unwrap();
    let target = temporary.path().join("target");
    fs::write(&target, b"preserve").unwrap();
    fs::set_permissions(&target, fs::Permissions::from_mode(0o600)).unwrap();
    let root = temporary.path().join("state");
    fs::create_dir(&root).unwrap();
    let lock = root.join("authority.lock");
    symlink(&target, &lock).unwrap();
    assert!(Authority::acquire(&root).is_err());
    fs::remove_file(&lock).unwrap();
    fs::hard_link(&target, &lock).unwrap();
    assert!(Authority::acquire(&root).is_err());
    assert_eq!(fs::read(&target).unwrap(), b"preserve");
    fs::remove_file(&lock).unwrap();
    fs::set_permissions(&root, fs::Permissions::from_mode(0o777)).unwrap();
    assert!(Authority::acquire(&root).is_err());
}

#[test]
fn replaced_storage_invalidates_the_authority() {
    let _serial = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temporary = tempdir().unwrap();
    let root = temporary.path().join("state");
    let authority = Authority::acquire(&root).unwrap();
    fs::rename(&root, temporary.path().join("moved")).unwrap();
    fs::create_dir(&root).unwrap();
    assert!(authority.verify().is_err());
    fs::remove_dir(&root).unwrap();
    fs::rename(temporary.path().join("moved"), &root).unwrap();
    assert!(authority.verify().is_err());
}

#[test]
fn different_state_roots_cannot_own_the_same_config_or_recovery() {
    let _serial = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temporary = tempdir().unwrap();
    let config = temporary.path().join("config");
    let recovery = temporary.path().join("recovery");
    let state = temporary.path().join("state");
    let first = Authority::acquire_bound(&state, &config, &recovery).unwrap();
    for (next_config, next_recovery) in [
        (config.clone(), temporary.path().join("other-recovery")),
        (temporary.path().join("other-config"), recovery.clone()),
    ] {
        assert!(matches!(
            Authority::acquire_bound(
                temporary.path().join("other-state"),
                next_config,
                next_recovery
            ),
            Err(LeaseError::Held { .. })
        ));
    }
    let record: Value =
        serde_json::from_slice(&fs::read(state.join("authority.lock")).unwrap()).unwrap();
    for (role, path) in [("config", &config), ("recovery", &recovery)] {
        let metadata = fs::metadata(path).unwrap();
        assert_eq!(record["roots"][role]["device"], metadata.dev());
        assert_eq!(record["roots"][role]["inode"], metadata.ino());
    }
    drop(first);
    Authority::acquire_bound(temporary.path().join("other-state"), config, recovery).unwrap();
}

#[test]
fn an_alias_change_latches_identity_loss_even_for_a_deduplicated_root() {
    let _serial = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temporary = tempdir().unwrap();
    let state = temporary.path().join("state");
    fs::create_dir(&state).unwrap();
    let alias = temporary.path().join("alias");
    symlink(&state, &alias).unwrap();
    let authority = Authority::acquire_bound(&state, &alias, &state).unwrap();
    fs::remove_file(&alias).unwrap();
    fs::create_dir(&alias).unwrap();
    assert!(authority.verify().is_err());
    fs::remove_dir(&alias).unwrap();
    symlink(&state, &alias).unwrap();
    assert!(authority.verify().is_err());
}

#[test]
fn migration_modes_are_set_once_and_refuse_persistence_with_the_reason() {
    let _serial = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let temporary = tempdir().unwrap();
    let authority = Authority::acquire(temporary.path()).unwrap();
    let path = temporary.path().join("journal.json");
    assert!(
        authority
            .persistence_anchor(&path)
            .unwrap_err()
            .to_string()
            .contains("migration has not been prepared")
    );
    authority
        .set_write_mode(WriteMode::ReadOnly {
            reason: "legacy writer active".into(),
        })
        .unwrap();
    assert!(
        authority
            .persistence_anchor(&path)
            .unwrap_err()
            .to_string()
            .contains("migration-refused: legacy writer active")
    );
    assert!(authority.set_write_mode(WriteMode::Full).is_err());
    assert!(!path.exists());
}

#[test]
fn a_previously_opened_anchor_never_redirects_persistence_to_a_replacement_root() {
    let _serial = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    use std::os::fd::AsRawFd;
    let temporary = tempdir().unwrap();
    let root = temporary.path().join("state");
    let authority = Authority::acquire(&root).unwrap();
    authority.set_write_mode(WriteMode::Full).unwrap();
    let (directory, relative) = authority
        .persistence_anchor(&root.join("journal.json"))
        .unwrap()
        .unwrap();
    assert_eq!(relative, Path::new("journal.json"));
    let moved = temporary.path().join("original-state");
    fs::rename(&root, &moved).unwrap();
    fs::create_dir(&root).unwrap();
    fs::write(
        format!("/proc/self/fd/{}/journal.json", directory.as_raw_fd()),
        b"original",
    )
    .unwrap();
    assert_eq!(fs::read(moved.join("journal.json")).unwrap(), b"original");
    assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
    assert!(authority.verify().is_err());
    assert!(
        authority
            .persistence_anchor(&root.join("audit.jsonl"))
            .unwrap_err()
            .to_string()
            .contains("authority-lost")
    );
    assert_eq!(fs::read_dir(&root).unwrap().count(), 0);
}

struct Resident {
    temporary: TempDir,
    root: PathBuf,
    child: Child,
}

impl Resident {
    fn start() -> Self {
        Self::with_mode(true)
    }

    fn with_mode(isolated: bool) -> Self {
        let temporary = tempdir().unwrap();
        let root = temporary.path().join("state/omarchy/fileblade");
        let mut command = isolated_command(temporary.path(), &root);
        command.env("FILEBLADE_APP_ROOT", env!("CARGO_MANIFEST_DIR"));
        command.args(["serve", "--native-authority", "--no-recover"]);
        if isolated {
            command.arg("--native-isolated");
        }
        let child = command
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .spawn()
            .unwrap();
        let mut resident = Self {
            temporary,
            root,
            child,
        };
        let deadline = Instant::now() + Duration::from_secs(5);
        while fileblade::lease::transport::probe(&resident.root).is_err() {
            assert!(
                resident.child.try_wait().unwrap().is_none(),
                "authority exited before ready"
            );
            assert!(Instant::now() < deadline, "authority did not become ready");
            std::thread::sleep(Duration::from_millis(20));
        }
        resident
    }

    fn session(&self) -> Session {
        Session::open(&self.root)
    }
}

#[test]
fn extension_discovery_records_new_sources_and_preserves_explicit_choices() {
    let _serial = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let resident = Resident::start();
    let config = resident.temporary.path().join("config");
    for id in ["test.chosen", "test.new"] {
        let root = config.join("fileblade/extensions").join(id);
        fs::create_dir_all(&root).unwrap();
        fs::write(root.join("Module.qml"), "import QtQuick\nItem {}\n").unwrap();
        fs::write(
            root.join("manifest.json"),
            json!({"id":id,
                "extensions":{"data-goblin.fileblade/blade":[{"id":"fixture","entry":"Module.qml"}]}
            })
            .to_string(),
        )
        .unwrap();
    }
    let path = config.join("omarchy/fileblade/settings.json");
    fs::write(
        &path,
        json!({"version":1,"future":{"keep":true},
        "extensions":{"test.chosen":{"enabled":false,"future":42}}})
        .to_string(),
    )
    .unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    let mut view = resident.session();
    let mut saved = Vec::new();
    for generation in 1..=2 {
        view.send(
            json!({"v":1,"type":"request","id":"catalog","generation":generation,
            "command":"plugin-catalog","arguments":[]}),
        );
        let response = view.receive();
        assert_eq!(response["payload"]["activation"], "known", "{response}");
        for provider in response["payload"]["providers"].as_array().unwrap() {
            assert_eq!(provider["enabled"], provider["id"] == "test.new");
        }
        let bytes = fs::read(&path).unwrap();
        if generation == 1 {
            saved = bytes;
        } else {
            assert_eq!(bytes, saved);
        }
    }
    let mut document: Value = serde_json::from_slice(&saved).unwrap();
    assert_eq!(document["future"]["keep"], true);
    assert_eq!(document["extensions"]["test.chosen"]["future"], 42);
    assert_eq!(
        document["extensions"]["test.new"]["receipt"]["source"],
        config
            .join("fileblade/extensions/test.new")
            .to_str()
            .unwrap()
    );
    document["extensions"]["test.chosen"]["enabled"] = json!("false");
    fs::write(&path, document.to_string()).unwrap();
    view.send(
        json!({"v":1,"type":"request","id":"invalid-catalog","generation":3,
        "command":"plugin-catalog","arguments":[]}),
    );
    assert_eq!(view.receive()["payload"]["activation"], "unknown");
    assert_eq!(fs::read_to_string(path).unwrap(), document.to_string());
}

#[test]
fn in_flight_root_replacement_never_receives_persistence_and_reports_authority_lost() {
    let _serial = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    for role in ["state", "config", "recovery"] {
        let resident = Resident::start();
        let source = resident.temporary.path().join("source");
        let destination = resident.temporary.path().join("destination");
        fs::create_dir(&destination).unwrap();
        fs::File::create(&source)
            .unwrap()
            .set_len(1024 * 1024 * 1024)
            .unwrap();
        let mut view = resident.session();
        view.send(json!({"v":1,"type":"request","id":"replace-copy","generation":1,"command":"copy",
            "arguments":["--source",source,"--destination",destination,"--journal-id","replacement-copy"]}));
        let accepted = view.receive();
        assert_eq!(accepted["type"], "accepted");
        let op = accepted["op"].as_str().unwrap().to_owned();
        let deadline = Instant::now() + Duration::from_secs(10);
        let partial = loop {
            if let Some(path) = fs::read_dir(&destination)
                .unwrap()
                .flatten()
                .map(|entry| entry.path().join("item"))
                .find(|path| fs::metadata(path).is_ok_and(|metadata| metadata.len() > 0))
            {
                break path;
            }
            assert!(
                Instant::now() < deadline,
                "copy did not reach temporary output"
            );
            std::thread::sleep(Duration::from_millis(1));
        };
        assert_eq!(
            unsafe { libc::kill(resident.child.id() as i32, libc::SIGSTOP) },
            0
        );
        assert!(
            fs::metadata(&partial).unwrap().len() < 1024 * 1024 * 1024,
            "copy finished before replacement barrier"
        );
        let replaced = match role {
            "state" => resident.root.clone(),
            "config" => resident.temporary.path().join("config/omarchy/fileblade"),
            _ => resident.temporary.path().join("state/fileblade"),
        };
        let original = resident.temporary.path().join("pinned-original");
        fs::rename(&replaced, &original).unwrap();
        fs::create_dir(&replaced).unwrap();
        fs::write(replaced.join("sentinel"), b"unchanged").unwrap();
        assert_eq!(
            unsafe { libc::kill(resident.child.id() as i32, libc::SIGCONT) },
            0
        );
        let terminal = loop {
            let frame = view.receive();
            if frame["type"] == "response" {
                break frame;
            }
        };
        assert_eq!(terminal["error_id"], "authority-lost", "{terminal}");
        assert_eq!(terminal["ok"], false);
        assert_eq!(terminal["cancelled"], false);
        assert_eq!(view.result(&op), terminal);
        assert!(source.exists());
        assert_eq!(
            fs::read_dir(&replaced).unwrap().count(),
            1,
            "new {role} received writes"
        );
        assert_eq!(fs::read(replaced.join("sentinel")).unwrap(), b"unchanged");
        view.send(json!({"v":1,"type":"request","id":"late","generation":2,"command":"frecency-visit","arguments":["--path",source]}));
        let refused = view.receive();
        assert_eq!(refused["ok"], false);
        assert!(
            refused["error"]
                .as_str()
                .unwrap()
                .contains("authority-lost"),
            "{refused}"
        );
        fs::remove_file(replaced.join("sentinel")).unwrap();
        fs::remove_dir(&replaced).unwrap();
        fs::rename(&original, &replaced).unwrap();
        view.send(json!({"v":1,"type":"request","id":"restored","generation":3,"command":"frecency-visit","arguments":["--path",source]}));
        assert!(
            view.receive()["error"]
                .as_str()
                .unwrap()
                .contains("authority-lost")
        );
    }
}

#[test]
fn private_record_writers_refuse_replacement_roots_and_keep_open_parents_pinned() {
    let _serial = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    use fileblade::lease::{durable, persistence::PersistenceSession};
    use std::sync::Arc;
    for role in ["state", "config", "recovery"] {
        let temporary = tempdir().unwrap();
        let root = temporary.path().join(role);
        let authority = Arc::new(
            Authority::acquire_bound(
                temporary.path().join("state"),
                temporary.path().join("config"),
                temporary.path().join("recovery"),
            )
            .unwrap(),
        );
        authority.set_write_mode(WriteMode::Full).unwrap();
        let _session = PersistenceSession::open(authority).unwrap();
        let document = root.join("records/document.json");
        durable::write_private_atomic(&document, b"original").unwrap();
        let manifest = root.join("records/manifest.json");
        durable::write_new_private(&manifest, b"manifest").unwrap();
        assert!(durable::write_new_private(&manifest, b"overwrite").is_err());
        let parent = fileblade::secure::resolved_parent(&document).unwrap();
        let moved = temporary.path().join("original");
        fs::rename(&root, &moved).unwrap();
        fs::create_dir(&root).unwrap();
        fs::write(root.join("sentinel"), b"untouched").unwrap();
        for outcome in [
            durable::write_private_atomic(&document, b"replacement"),
            durable::write_new_private(&root.join("new.json"), b"replacement"),
            fileblade::secure::write_private_atomic(&document, b"replacement"),
            fileblade::secure::write_new_private(&root.join("new.json"), b"replacement"),
            fileblade::secure::open_private_append(&document).map(|_| ()),
            fileblade::secure::ensure_private_directory(&root.join("new")).map(|_| ()),
            fileblade::secure::open_record_lock(&root.join("new.lock")).map(|_| ()),
        ] {
            assert!(outcome.unwrap_err().to_string().contains("authority-lost"));
        }
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        assert_eq!(fs::read(root.join("sentinel")).unwrap(), b"untouched");
        assert_eq!(
            fs::read(moved.join("records/document.json")).unwrap(),
            b"original"
        );
        assert_eq!(
            fs::read(moved.join("records/manifest.json")).unwrap(),
            b"manifest"
        );
        let stat = rustix::fs::fstat(&parent.directory).unwrap();
        assert_eq!(
            stat.st_ino,
            fs::metadata(moved.join("records")).unwrap().ino()
        );
    }
}

#[test]
fn an_unprepared_authority_refuses_layout_writes_before_admission() {
    let _serial = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let resident = Resident::with_mode(false);
    let mut view = resident.session();
    view.send(json!({"v":1,"type":"request","id":"layout","generation":1,"command":"layout-write","arguments":["--document","{}"]}));
    let frame = view.receive();
    assert_eq!(frame["error_id"], "migration-refused", "{frame}");
    assert!(
        !resident
            .temporary
            .path()
            .join("config/omarchy/fileblade/blades.json")
            .exists()
    );
}

impl Drop for Resident {
    fn drop(&mut self) {
        unsafe {
            libc::kill(self.child.id() as i32, libc::SIGTERM);
        }
        let deadline = Instant::now() + Duration::from_secs(5);
        while self.child.try_wait().ok().flatten().is_none() {
            if Instant::now() >= deadline {
                let _ = self.child.kill();
                let _ = self.child.wait();
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

fn isolated_command(temporary: &Path, root: &Path) -> Command {
    let binary = std::env::var_os("FILEBLADE_TEST_BIN")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_fileblade").into());
    let mut command = Command::new(binary);
    command
        .env("HOME", temporary)
        .env("XDG_STATE_HOME", temporary.join("state"))
        .env("XDG_CONFIG_HOME", temporary.join("config"))
        .env("XDG_DATA_HOME", temporary.join("data"))
        .env("FILEBLADE_SPIKE_HOME", temporary)
        .env("FILEBLADE_NATIVE_STATE_ROOT", root);
    command
}

struct Session {
    stream: UnixStream,
    reader: BufReader<UnixStream>,
}

impl Session {
    fn open(root: &Path) -> Self {
        let stream = UnixStream::connect(root.join("authority.sock")).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(20)))
            .unwrap();
        let reader = BufReader::new(stream.try_clone().unwrap());
        let mut session = Self { stream, reader };
        session.send(json!({"v":1,"type":"hello"}));
        assert_eq!(session.receive()["authority"], true);
        session
    }

    fn send(&mut self, frame: Value) {
        serde_json::to_writer(&mut self.stream, &frame).unwrap();
        self.stream.write_all(b"\n").unwrap();
    }

    fn receive(&mut self) -> Value {
        let mut line = String::new();
        assert!(
            self.reader.read_line(&mut line).unwrap() > 0,
            "unexpected EOF"
        );
        serde_json::from_str(&line).unwrap()
    }

    fn result(&mut self, op: &str) -> Value {
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            self.send(json!({"v":1,"type":"operation","action":"get","op":op,"id":"query","generation":1}));
            let response = self.receive();
            assert_eq!(response["ok"], true, "{response}");
            if response["payload"]["complete"] == true {
                return response["payload"]["result"].clone();
            }
            assert!(Instant::now() < deadline, "operation did not complete");
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

#[test]
fn accepted_copy_survives_eof_and_result_can_be_fetched_by_a_new_view() {
    let _serial = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let mut resident = Resident::start();
    let source = resident.temporary.path().join("source");
    let destination = resident.temporary.path().join("destination");
    fs::create_dir(&destination).unwrap();
    fs::File::create(&source)
        .unwrap()
        .set_len(256 * 1024 * 1024)
        .unwrap();
    let mut view = resident.session();
    view.send(
        json!({"v":1,"type":"request","id":"copy","generation":1,"command":"copy",
        "arguments":["--source",source,"--destination",destination]}),
    );
    let accepted = view.receive();
    assert_eq!(accepted["type"], "accepted", "{accepted}");
    let op = accepted["op"].as_str().unwrap();
    drop(view);
    assert!(resident.child.try_wait().unwrap().is_none());
    let mut replacement = resident.session();
    let result = replacement.result(op);
    assert_eq!(result["payload"]["ok"], true, "{result}");
    assert_eq!(
        fs::metadata(destination.join("source")).unwrap().len(),
        256 * 1024 * 1024
    );
    assert!(
        result["payload"]["mappings"]
            .as_array()
            .is_some_and(|mappings| !mappings.is_empty()),
        "{result}"
    );
    replacement.send(
        json!({"v":1,"type":"operation","action":"fetch","op":op,"id":"fetch","generation":1}),
    );
    assert_eq!(replacement.receive()["payload"]["result"], result);
    replacement
        .send(json!({"v":1,"type":"operation","action":"get","op":op,"id":"gone","generation":1}));
    assert_eq!(replacement.receive()["ok"], false);
}

#[test]
fn accepted_move_survives_a_lost_progress_reader() {
    let _serial = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let resident = Resident::start();
    let destination = resident.temporary.path().join("destination");
    fs::create_dir(&destination).unwrap();
    let paths: Vec<_> = (0..128)
        .map(|index| {
            let path = resident.temporary.path().join(format!("file-{index}"));
            fs::write(&path, format!("content-{index}")).unwrap();
            path
        })
        .collect();
    let mut arguments = vec![
        "--destination".to_string(),
        destination.display().to_string(),
    ];
    for path in &paths {
        arguments.extend(["--source".into(), path.display().to_string()]);
    }
    let mut view = resident.session();
    view.send(json!({"v":1,"type":"request","id":"move","generation":1,"command":"move","arguments":arguments}));
    let accepted = view.receive();
    assert_eq!(accepted["type"], "accepted", "{accepted}");
    view.stream.shutdown(std::net::Shutdown::Read).unwrap();
    let mut replacement = resident.session();
    let result = replacement.result(accepted["op"].as_str().unwrap());
    assert_eq!(result["payload"]["ok"], true, "{result}");
    for path in paths {
        assert!(!path.exists());
        assert!(destination.join(path.file_name().unwrap()).is_file());
    }
}

#[test]
fn second_authority_and_native_direct_mutations_fail_before_writing() {
    let _serial = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let resident = Resident::start();
    let second = isolated_command(resident.temporary.path(), &resident.root)
        .args(["serve", "--native-authority", "--no-recover"])
        .output()
        .unwrap();
    assert!(!second.status.success());
    let target = resident.temporary.path().join("forbidden");
    for arguments in [
        vec![
            "_backend",
            "create",
            "--parent",
            resident.temporary.path().to_str().unwrap(),
            "--name",
            "forbidden",
        ],
        vec!["_backend", "preferences-set", "--agent-management", "true"],
        vec!["list", "--from", target.to_str().unwrap()],
        vec!["_backend", "dim-windows", "--state", "off"],
        vec!["_companion-mutate"],
    ] {
        let output = isolated_command(resident.temporary.path(), &resident.root)
            .args(arguments)
            .output()
            .unwrap();
        assert!(!output.status.success());
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(combined.contains("owner-unavailable"), "{combined}");
    }
    assert!(!target.exists());
}

#[test]
fn explicit_operation_cancel_works_after_the_accepting_view_detaches() {
    let _serial = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let resident = Resident::start();
    let source = resident.temporary.path().join("large");
    let destination = resident.temporary.path().join("destination");
    fs::create_dir(&destination).unwrap();
    fs::File::create(&source)
        .unwrap()
        .set_len(1024 * 1024 * 1024)
        .unwrap();
    let mut view = resident.session();
    view.send(
        json!({"v":1,"type":"request","id":"copy","generation":1,"command":"copy",
        "arguments":["--source",source,"--destination",destination]}),
    );
    let accepted = view.receive();
    assert_eq!(accepted["type"], "accepted", "{accepted}");
    let op = accepted["op"].as_str().unwrap();
    let mut replacement = resident.session();
    replacement.send(json!({"v":1,"type":"cancel","op":op}));
    let cancelled = replacement.receive();
    assert_eq!(cancelled["accepted"], true, "{cancelled}");
    drop(view);
    let result = replacement.result(op);
    assert!(
        result["cancelled"] == true || result["payload"]["cancelled"] == true,
        "{result}"
    );
    assert!(source.exists());
}

#[test]
fn a_killed_holder_releases_the_kernel_lock_despite_stale_diagnostics() {
    let _serial = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let mut resident = Resident::start();
    let inode = fs::metadata(resident.root.join("authority.lock"))
        .unwrap()
        .ino();
    resident.child.kill().unwrap();
    resident.child.wait().unwrap();
    let authority = Authority::acquire(&resident.root).unwrap();
    authority.verify().unwrap();
    assert_eq!(
        fs::metadata(resident.root.join("authority.lock"))
            .unwrap()
            .ino(),
        inode
    );
}

#[test]
fn an_idle_view_relay_does_not_block_the_qt_pipe_availability_query() {
    let _serial = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    use std::os::fd::AsRawFd;
    let resident = Resident::start();
    let mut child = isolated_command(resident.temporary.path(), &resident.root)
        .args(["serve"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .unwrap();
    let stdout = child.stdout.take().unwrap();
    let (sender, receiver) = std::sync::mpsc::channel();
    let probe = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(200));
        let mut available: libc::c_int = -1;
        let result = unsafe { libc::ioctl(stdout.as_raw_fd(), libc::FIONREAD, &mut available) };
        sender.send((result, available)).unwrap();
        stdout
    });
    let result = receiver.recv_timeout(Duration::from_millis(800));
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let stdout = probe.join().unwrap();
    assert_eq!(result.unwrap(), (0, 0));
    child
        .stdin
        .as_mut()
        .unwrap()
        .write_all(b"{\"v\":1,\"type\":\"hello\"}\n")
        .unwrap();
    let mut response = String::new();
    BufReader::new(stdout).read_line(&mut response).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&response).unwrap()["authority"],
        true
    );
    child.kill().unwrap();
    child.wait().unwrap();
}

#[test]
fn chooser_offers_leave_completion_capacity_and_cancel_on_real_caller_eof() {
    let _serial = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let resident = Resident::start();
    let path = resident.temporary.path().join("upload # space.txt");
    fs::write(&path, "upload").unwrap();
    let mut caller = resident.session();
    let mut observer = resident.session();
    let send = |session: &mut Session, id: &str, arguments: Value| {
        session.send(json!({"v":1,"type":"request","id":id,"generation":1,
            "command":"chooser","arguments":arguments,"deadline_ms":60000}));
    };
    let offer = |number| {
        json!({"handle":format!("chooser-{number}"),"caller":":1.42","parent_window":"",
            "title":"Upload","accept_label":"Choose","modal":true,
            "current_folder":resident.temporary.path(),"current_name":"","mode":"open",
            "multiple":false,"filters":[],"current_filter":null})
        .to_string()
    };
    for number in 0..16 {
        send(
            &mut caller,
            &format!("offer-{number}"),
            json!(["offer", "--document", offer(number)]),
        );
    }
    let mut serial = 0;
    let mut wait_count = |expected| {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            serial += 1;
            send(&mut observer, &format!("watch-{serial}"), json!(["watch"]));
            let frame = observer.receive();
            assert_eq!(frame["ok"], true, "{frame}");
            if frame["payload"]["offers"].as_array().unwrap().len() == expected {
                break;
            }
            assert!(
                Instant::now() < deadline,
                "pending chooser count did not reach {expected}: {frame}"
            );
            std::thread::sleep(Duration::from_millis(20));
        }
    };
    wait_count(16);
    send(
        &mut caller,
        "overflow",
        json!(["offer", "--document", offer(16)]),
    );
    let refused = caller.receive();
    assert_eq!(refused["id"], "overflow");
    assert_eq!(refused["payload"]["ok"], false, "{refused}");
    assert!(
        refused["payload"]["error"]
            .as_str()
            .unwrap()
            .contains("limit or duplicate")
    );
    send(
        &mut caller,
        "choose",
        json!(["choose", "--handle", "chooser-0", "--path", path]),
    );
    let mut completed = [caller.receive(), caller.receive()];
    completed.sort_by_key(|frame| frame["id"].as_str().unwrap().to_owned());
    assert_eq!(completed[0]["id"], "choose");
    assert_eq!(completed[1]["id"], "offer-0");
    for frame in &completed {
        assert_eq!(frame["type"], "response");
        assert_eq!(frame["payload"]["ok"], true, "{frame}");
    }
    assert_eq!(
        completed[1]["payload"]["outcome"]["uris"],
        json!([url::Url::from_file_path(&path).unwrap().to_string()])
    );
    send(
        &mut caller,
        "cancel",
        json!(["cancel", "--handle", "chooser-1"]),
    );
    let mut cancelled = [caller.receive(), caller.receive()];
    cancelled.sort_by_key(|frame| frame["id"].as_str().unwrap().to_owned());
    assert_eq!(cancelled[0]["payload"]["cancelled"], true);
    assert_eq!(cancelled[1]["payload"]["outcome"]["status"], "cancelled");
    drop(caller);
    wait_count(0);
    assert!(fileblade::lease::transport::probe(&resident.root).is_ok());
}

#[test]
fn chooser_watch_admission_is_bounded_and_eof_drains_the_waiter() {
    let _serial = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let resident = Resident::start();
    let threads = || {
        fs::read_dir(format!("/proc/{}/task", resident.child.id()))
            .unwrap()
            .count()
    };
    std::thread::sleep(Duration::from_millis(200));
    let idle_threads = threads();
    let mut caller = resident.session();
    let watch = |caller: &mut Session, id: &str, revision: u64| {
        caller.send(json!({"v":1,"type":"request","id":id,"generation":1,
            "command":"chooser","arguments":["watch","--revision",revision.to_string()],
            "deadline_ms":60000}));
    };
    watch(&mut caller, "snapshot", 0);
    let revision = caller.receive()["payload"]["revision"].as_u64().unwrap();
    watch(&mut caller, "waiting", revision);
    for number in 0..32 {
        let id = format!("excess-{number}");
        watch(&mut caller, &id, revision);
        let frame = caller.receive();
        assert_eq!(frame["id"], id, "{frame}");
        assert_eq!(frame["ok"], false, "{frame}");
        assert_eq!(frame["error"], "chooser watch limit reached", "{frame}");
    }
    caller.send(json!({"v":1,"type":"request","id":"filter","generation":1,
        "command":"chooser","arguments":["filter","--document",r#"{"filter":["All",[[0,"*"]]],"entries":[]}"#]}));
    let completed = caller.receive();
    assert_eq!(completed["id"], "filter", "{completed}");
    assert_eq!(completed["payload"]["ok"], true, "{completed}");
    caller.send(json!({"v":1,"type":"cancel","id":"waiting","generation":1}));
    let mut cancelled = [caller.receive(), caller.receive()];
    cancelled.sort_by_key(|frame| frame["type"].as_str().unwrap().to_owned());
    assert_eq!(cancelled[0]["accepted"], true, "{cancelled:?}");
    assert_eq!(cancelled[1]["payload"]["ok"], false, "{cancelled:?}");
    watch(&mut caller, "eof-waiting", revision);
    watch(&mut caller, "eof-overflow", revision);
    assert_eq!(caller.receive()["error"], "chooser watch limit reached");
    drop(caller);
    let deadline = Instant::now() + Duration::from_secs(3);
    while threads() > idle_threads {
        assert!(
            Instant::now() < deadline,
            "chooser worker survived caller EOF"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(fileblade::lease::transport::probe(&resident.root).is_ok());
}

const PORTAL_FRONTEND: &str = "org.freedesktop.portal.Desktop";
const PORTAL_BACKEND: &str = "org.freedesktop.impl.portal.desktop.fileblade";
const PORTAL_PATH: &str = "/org/freedesktop/portal/desktop";
const PORTAL_INTERFACE: &str = "org.freedesktop.impl.portal.FileChooser";
const REQUEST_INTERFACE: &str = "org.freedesktop.impl.portal.Request";

struct PrivateBus {
    child: Child,
    address: String,
}

impl PrivateBus {
    fn start() -> Self {
        let mut child = Command::new("dbus-daemon")
            .args(["--session", "--nofork", "--nopidfile", "--print-address=1"])
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        let mut address = String::new();
        BufReader::new(child.stdout.take().unwrap())
            .read_line(&mut address)
            .unwrap();
        assert!(
            !address.trim().is_empty(),
            "private D-Bus daemon gave no address"
        );
        Self {
            child,
            address: address.trim().to_owned(),
        }
    }
}

impl Drop for PrivateBus {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

struct PortalProcess {
    child: Child,
}

impl PortalProcess {
    fn start(resident: &Resident, bus: &PrivateBus) -> Self {
        let child = isolated_command(resident.temporary.path(), &resident.root)
            .env("DBUS_SESSION_BUS_ADDRESS", &bus.address)
            .args(["native", "portal"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .unwrap();
        Self { child }
    }
}

impl Drop for PortalProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn bus_connection(bus: &PrivateBus) -> zbus::blocking::Connection {
    zbus::blocking::connection::Builder::address(bus.address.as_str())
        .unwrap()
        .build()
        .unwrap()
}

fn named_bus_connection(bus: &PrivateBus) -> zbus::blocking::Connection {
    zbus::blocking::connection::Builder::address(bus.address.as_str())
        .unwrap()
        .name(PORTAL_FRONTEND)
        .unwrap()
        .build()
        .unwrap()
}

fn wait_for_bus_name(connection: &zbus::blocking::Connection, name: &str) {
    let proxy = zbus::blocking::fdo::DBusProxy::new(connection).unwrap();
    let deadline = Instant::now() + Duration::from_secs(5);
    while !proxy
        .name_has_owner(zbus::names::BusName::try_from(name).unwrap())
        .unwrap_or(false)
    {
        assert!(
            Instant::now() < deadline,
            "D-Bus name did not appear: {name}"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

fn portal_open(
    connection: &zbus::blocking::Connection,
    handle: &str,
) -> zbus::Result<(u32, HashMap<String, zbus::zvariant::OwnedValue>)> {
    let proxy =
        zbus::blocking::Proxy::new(connection, PORTAL_BACKEND, PORTAL_PATH, PORTAL_INTERFACE)?;
    let handle = zbus::zvariant::OwnedObjectPath::try_from(handle).unwrap();
    proxy.call(
        "OpenFile",
        &(
            handle,
            "org.test.browser",
            "",
            "Upload",
            HashMap::<String, zbus::zvariant::OwnedValue>::new(),
        ),
    )
}

fn portal_close(connection: &zbus::blocking::Connection, handle: &str) -> zbus::Result<()> {
    let proxy = zbus::blocking::Proxy::new(connection, PORTAL_BACKEND, handle, REQUEST_INTERFACE)?;
    proxy.call("Close", &())
}

fn wait_for_offer(session: &mut Session, handle: &str, present: bool) {
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut serial = 0;
    loop {
        serial += 1;
        session.send(
            json!({"v":1,"type":"request","id":format!("portal-watch-{handle}-{present}-{serial}"),"generation":1,
            "command":"chooser","arguments":["watch"],"deadline_ms":60000}),
        );
        let frame = session.receive();
        assert_eq!(frame["payload"]["ok"], true, "{frame}");
        let found = frame["payload"]["offers"]
            .as_array()
            .unwrap()
            .iter()
            .any(|offer| offer["handle"] == handle);
        if found == present {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "chooser offer presence did not settle"
        );
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn portal_boundary_authenticates_sender_closes_and_cancels_on_name_loss() {
    let _serial = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let bus = PrivateBus::start();
    let resident = Resident::start();
    let _portal = PortalProcess::start(&resident, &bus);
    let frontend = named_bus_connection(&bus);
    wait_for_bus_name(&frontend, PORTAL_BACKEND);
    let attacker = bus_connection(&bus);
    let mut observer = resident.session();
    let accepted_handle = "/org/freedesktop/portal/desktop/request/test/accepted";
    let accepted = std::thread::spawn({
        let frontend = frontend.clone();
        move || portal_open(&frontend, accepted_handle)
    });
    wait_for_offer(&mut observer, accepted_handle, true);
    let denied_open = portal_open(
        &attacker,
        "/org/freedesktop/portal/desktop/request/test/unauthorized",
    )
    .unwrap_err();
    assert!(
        denied_open.to_string().contains("AccessDenied"),
        "{denied_open}"
    );
    let denied = portal_close(&attacker, accepted_handle).unwrap_err();
    assert!(denied.to_string().contains("AccessDenied"), "{denied}");
    let path = resident.temporary.path().join("browser-upload.txt");
    fs::write(&path, "portal upload").unwrap();
    let mut chooser = resident.session();
    chooser.send(json!({"v":1,"type":"request","id":"portal-choose","generation":1,
        "command":"chooser","arguments":["choose","--handle",accepted_handle,"--path",path],"deadline_ms":60000}));
    assert_eq!(chooser.receive()["payload"]["ok"], true);
    let (response, results) = accepted.join().unwrap().unwrap();
    assert_eq!(response, 0);
    let uris: Vec<String> = results
        .into_iter()
        .find_map(|(key, value)| (key == "uris").then(|| value.try_into().unwrap()))
        .unwrap();
    assert_eq!(
        uris,
        vec![url::Url::from_file_path(&path).unwrap().to_string()]
    );
    wait_for_offer(&mut observer, accepted_handle, false);

    let closed_handle = "/org/freedesktop/portal/desktop/request/test/closed";
    let closed = std::thread::spawn({
        let frontend = frontend.clone();
        move || portal_open(&frontend, closed_handle)
    });
    wait_for_offer(&mut observer, closed_handle, true);
    portal_close(&frontend, closed_handle).unwrap();
    let (response, results) = closed.join().unwrap().unwrap();
    assert_eq!(response, 1);
    assert!(results.is_empty());
    wait_for_offer(&mut observer, closed_handle, false);

    frontend.release_name(PORTAL_FRONTEND).unwrap();
    let frontend = named_bus_connection(&bus);
    let lost_handle = "/org/freedesktop/portal/desktop/request/test/lost";
    let lost = std::thread::spawn({
        let frontend = frontend.clone();
        move || portal_open(&frontend, lost_handle)
    });
    wait_for_offer(&mut observer, lost_handle, true);
    frontend.release_name(PORTAL_FRONTEND).unwrap();
    let (response, results) = lost.join().unwrap().unwrap();
    assert_eq!(response, 1);
    assert!(results.is_empty());
    wait_for_offer(&mut observer, lost_handle, false);
}

#[test]
fn drain_wire_quiesces_by_owner_expires_and_exits_after_views_leave() {
    let _serial = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let mut resident = Resident::start();
    let view = {
        let stream = UnixStream::connect(resident.root.join("authority.sock")).unwrap();
        stream
            .set_read_timeout(Some(Duration::from_secs(2)))
            .unwrap();
        let reader = BufReader::new(stream.try_clone().unwrap());
        let mut view = Session { stream, reader };
        view.send(json!({"v":1,"type":"hello","view":true}));
        assert_eq!(view.receive()["ok"], true);
        view
    };
    let mut owner = resident.session();
    owner.send(json!({"v":1,"type":"drain","action":"status"}));
    let status = owner.receive();
    assert_eq!(status["type"], "drain", "{status}");
    assert_eq!(status["ok"], true, "{status}");
    assert_eq!(status["payload"]["views"], 1, "{status}");
    assert_eq!(status["payload"]["pid"], resident.child.id(), "{status}");

    owner.send(json!({"v":1,"type":"drain","action":"quiesce","timeout_ms":5000}));
    let paused = owner.receive();
    assert_eq!(paused["ok"], true, "{paused}");
    assert_eq!(paused["payload"]["views"], 1, "{paused}");
    owner.send(json!({"v":1,"type":"drain","action":"exit"}));
    assert_eq!(owner.receive()["ok"], false);
    assert!(resident.child.try_wait().unwrap().is_none());

    let mut second = resident.session();
    second.send(json!({"v":1,"type":"drain","action":"quiesce","timeout_ms":5000}));
    let refused_drain = second.receive();
    assert_eq!(refused_drain["type"], "drain", "{refused_drain}");
    assert_eq!(refused_drain["ok"], false, "{refused_drain}");
    second.send(
        json!({"v":1,"type":"request","id":"drain-paused-write","generation":1,
        "command":"state-write","arguments":["--document","not-json"]}),
    );
    let refused_write = second.receive();
    assert_eq!(refused_write["ok"], false, "{refused_write}");
    assert!(
        refused_write["error"]
            .as_str()
            .is_some_and(|error| error.contains("draining")),
        "{refused_write}"
    );
    second.send(
        json!({"v":1,"type":"subscribe","id":"drain-paused-subscribe","generation":1,
        "topic":"filesystem","paths":[resident.temporary.path()]}),
    );
    let refused_subscribe = second.receive();
    assert_eq!(refused_subscribe["ok"], false, "{refused_subscribe}");
    assert!(
        refused_subscribe["error"]
            .as_str()
            .is_some_and(|error| error.contains("draining")),
        "{refused_subscribe}"
    );

    let rejected_view = UnixStream::connect(resident.root.join("authority.sock")).unwrap();
    rejected_view
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut rejected_view_reader = BufReader::new(rejected_view.try_clone().unwrap());
    let mut rejected_view = rejected_view;
    serde_json::to_writer(
        &mut rejected_view,
        &json!({"v":1,"type":"hello","view":true}),
    )
    .unwrap();
    rejected_view.write_all(b"\n").unwrap();
    let mut rejected_view_response = String::new();
    assert_eq!(
        rejected_view_reader
            .read_line(&mut rejected_view_response)
            .unwrap(),
        0
    );

    drop(owner);
    let deadline = Instant::now() + Duration::from_secs(3);
    let resumed_operation = loop {
        second.send(
            json!({"v":1,"type":"request","id":"drain-resume-write","generation":1,
            "command":"state-write","arguments":["--document","not-json"]}),
        );
        let response = second.receive();
        if response["type"] == "accepted" {
            break response["op"].as_str().unwrap().to_owned();
        }
        assert_eq!(response["ok"], false, "{response}");
        assert!(
            response["error"]
                .as_str()
                .is_some_and(|error| error.contains("draining")),
            "{response}"
        );
        assert!(
            Instant::now() < deadline,
            "drain owner EOF did not resume admission"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    let resumed_result = second.receive();
    assert_eq!(resumed_result["type"], "response");
    assert_eq!(resumed_result["op"], resumed_operation);
    assert_eq!(resumed_result["payload"]["ok"], false, "{resumed_result}");

    second.send(json!({"v":1,"type":"drain","action":"quiesce","timeout_ms":30}));
    let short_pause = second.receive();
    assert_eq!(short_pause["ok"], true, "{short_pause}");
    let deadline = Instant::now() + Duration::from_secs(3);
    let expired_operation = loop {
        second.send(
            json!({"v":1,"type":"request","id":"drain-expiry-write","generation":1,
            "command":"state-write","arguments":["--document","not-json"]}),
        );
        let response = second.receive();
        if response["type"] == "accepted" {
            break response["op"].as_str().unwrap().to_owned();
        }
        assert_eq!(response["ok"], false, "{response}");
        assert!(
            response["error"]
                .as_str()
                .is_some_and(|error| error.contains("draining")),
            "{response}"
        );
        assert!(Instant::now() < deadline, "short drain did not expire");
        std::thread::sleep(Duration::from_millis(20));
    };
    let expired_result = second.receive();
    assert_eq!(expired_result["type"], "response");
    assert_eq!(expired_result["op"], expired_operation);
    assert_eq!(expired_result["payload"]["ok"], false, "{expired_result}");

    drop(view);
    let deadline = Instant::now() + Duration::from_secs(3);
    loop {
        second.send(json!({"v":1,"type":"drain","action":"status"}));
        let status = second.receive();
        assert_eq!(status["ok"], true, "{status}");
        if status["payload"]["views"] == 0 {
            break;
        }
        assert!(Instant::now() < deadline, "view EOF did not detach");
        std::thread::sleep(Duration::from_millis(20));
    }
    second.send(json!({"v":1,"type":"drain","action":"quiesce","timeout_ms":5000}));
    let final_pause = second.receive();
    assert_eq!(final_pause["ok"], true, "{final_pause}");
    assert_eq!(final_pause["payload"]["views"], 0, "{final_pause}");
    second.send(json!({"v":1,"type":"drain","action":"exit"}));
    let exited = second.receive();
    assert_eq!(exited["type"], "drain", "{exited}");
    assert_eq!(exited["ok"], true, "{exited}");
    drop(second);

    let deadline = Instant::now() + Duration::from_secs(3);
    let status = loop {
        if let Some(status) = resident.child.try_wait().unwrap() {
            break status;
        }
        assert!(
            Instant::now() < deadline,
            "drain exit did not stop the authority"
        );
        std::thread::sleep(Duration::from_millis(20));
    };
    assert!(status.success(), "{status}");
    assert!(!resident.root.join("authority.sock").exists());
    let reacquired = Authority::acquire(&resident.root).unwrap();
    drop(reacquired);
}

#[test]
fn public_preferences_use_the_resident_authority_and_fail_closed_without_it() {
    let _serial = TEST_LOCK.lock().unwrap_or_else(|error| error.into_inner());
    let mut resident = Resident::start();
    for arguments in [
        vec![
            "preferences",
            "--trash-retention-days",
            "0",
            "--agent-management",
            "false",
            "-o",
            "json",
        ],
        vec!["preferences", "-o", "json"],
    ] {
        let output = isolated_command(resident.temporary.path(), &resident.root)
            .args(arguments)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let response: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(response["ok"], true);
        assert_eq!(response["settings"]["trashRetentionDays"], 0);
        assert_eq!(response["settings"]["agentManagement"], false);
    }
    let path = resident
        .temporary
        .path()
        .join("config/omarchy/fileblade/settings.json");
    let saved = fs::read(&path).unwrap();
    resident.child.kill().unwrap();
    resident.child.wait().unwrap();
    let output = isolated_command(resident.temporary.path(), &resident.root)
        .args(["preferences", "--agent-management", "true", "-o", "json"])
        .output()
        .unwrap();
    assert!(!output.status.success());
    assert_eq!(fs::read(&path).unwrap(), saved);
}
