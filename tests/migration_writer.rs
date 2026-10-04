use fileblade::migration::{Roots, legacy_writer};
use std::fs::{self, File, OpenOptions};
use std::os::fd::AsRawFd;
use std::os::unix::fs::{MetadataExt, OpenOptionsExt, PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

struct Fixture {
    root: TempDir,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        for path in ["bin", "desktop/omarchy", "omarchy/shell"] {
            fs::create_dir_all(root.path().join(path)).unwrap();
        }
        fs::write(root.path().join("omarchy/shell/shell.qml"), "").unwrap();
        fs::write(
            root.path().join("desktop/omarchy/shell.json"),
            r#"{"plugins":[],"disabledPlugins":[]}"#,
        )
        .unwrap();
        for (name, script) in [
            (
                "omarchy",
                "#!/bin/sh\nprintf '%s\\n' \"$MIGRATION_ACTIVATION\"\n",
            ),
            (
                "qs",
                "#!/bin/sh\nprintf '%s\\n' \"$MIGRATION_IPC\"\nexit \"$MIGRATION_EXIT\"\n",
            ),
        ] {
            executable(&root.path().join("bin").join(name), script);
        }
        Self { root }
    }

    fn command(&self, expected: &str) -> Command {
        let mut command = Command::new(std::env::current_exe().unwrap());
        command
            .args(["--exact", "probe_worker", "--nocapture"])
            .env("MIGRATION_FIXTURE", self.root.path())
            .env("MIGRATION_EXPECTED", expected)
            .env("PATH", self.root.path().join("bin"))
            .env("HOME", self.root.path())
            .env("XDG_CONFIG_HOME", self.root.path().join("desktop"))
            .env("OMARCHY_PATH", self.root.path().join("omarchy"))
            .env("MIGRATION_ACTIVATION", "[]")
            .env("MIGRATION_IPC", "Target not found.")
            .env("MIGRATION_EXIT", "0");
        command
    }

    fn lock(&self) -> File {
        fs::create_dir(self.root.path().join("state")).unwrap();
        OpenOptions::new()
            .read(true)
            .write(true)
            .create_new(true)
            .mode(0o600)
            .open(self.root.path().join("state/journal.json.lock"))
            .unwrap()
    }
}

fn executable(path: &Path, script: &str) {
    fs::write(path, script).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}

fn run(command: &mut Command) {
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn probe_worker() {
    let Some(path) = std::env::var_os("MIGRATION_FIXTURE") else {
        return;
    };
    let path = PathBuf::from(path);
    let result = legacy_writer(&Roots {
        config: path.join("desktop/omarchy/fileblade"),
        state: path.join("state"),
        recovery: path.join("recovery"),
    });
    let result = format!("{result:?}");
    println!("{result}");
    assert!(result.starts_with(&std::env::var("MIGRATION_EXPECTED").unwrap()));
}

#[test]
fn absent_and_stopped_require_both_shell_and_ipc_evidence() {
    let fixture = Fixture::new();
    run(&mut fixture.command("Absent"));
    run(fixture
        .command("Absent")
        .env("XDG_CONFIG_HOME", fixture.root.path().join("native-config")));
    assert!(!fixture.root.path().join("state").exists());
    fs::create_dir(fixture.root.path().join("state")).unwrap();
    run(&mut fixture.command("Stopped"));
    assert_eq!(
        fs::read_dir(fixture.root.path().join("state"))
            .unwrap()
            .count(),
        0
    );
    run(fixture
        .command("Unknown")
        .env("MIGRATION_ACTIVATION", "invalid"));
    run(fixture
        .command("Unknown")
        .env("MIGRATION_IPC", "Function not found."));
    run(fixture.command("Unknown").env("MIGRATION_EXIT", "1"));
    fs::remove_file(fixture.root.path().join("desktop/omarchy/shell.json")).unwrap();
    run(&mut fixture.command("Unknown"));
}

#[test]
fn hidden_core_and_companion_activation_is_never_stopped() {
    let fixture = Fixture::new();
    for id in [
        "data-goblin.fileblade",
        "data-goblin.fileblade-skills",
        "kurt.agent-hooks",
    ] {
        let activation = format!(r#"[{{"id":"{id}","enabled":true}}]"#);
        run(fixture
            .command("Active")
            .env("MIGRATION_ACTIVATION", activation));
    }
    run(fixture
        .command("Active")
        .env("MIGRATION_ACTIVATION", "invalid")
        .env("MIGRATION_IPC", r#"{"visible":false}"#));
    run(fixture.command("Absent").env(
        "MIGRATION_ACTIVATION",
        r#"[{"id":"unrelated.clock","enabled":true} ]"#,
    ));
}

#[test]
fn actual_legacy_flock_is_detected_before_unavailable_shell_tools() {
    let fixture = Fixture::new();
    let lock = fixture.lock();
    assert_eq!(
        unsafe { libc::flock(lock.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) },
        0
    );
    run(fixture.command("Active").env("PATH", "/nonexistent"));
    drop(lock);
    run(&mut fixture.command("Stopped"));
}

#[test]
fn ofd_writer_is_detected_and_probe_does_not_modify_evidence() {
    let fixture = Fixture::new();
    let lock = fixture.lock();
    let path = fixture.root.path().join("state/journal.json.lock");
    fs::write(&path, b"diagnostic evidence").unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o644)).unwrap();
    let before = fs::metadata(&path).unwrap();
    let mut range: libc::flock = unsafe { std::mem::zeroed() };
    range.l_type = libc::F_WRLCK as _;
    range.l_whence = libc::SEEK_SET as _;
    assert_eq!(
        unsafe { libc::fcntl(lock.as_raw_fd(), libc::F_OFD_SETLK, &range) },
        0
    );
    run(fixture.command("Active").env("PATH", "/nonexistent"));
    drop(lock);
    run(&mut fixture.command("Stopped"));
    let after = fs::metadata(&path).unwrap();
    assert_eq!(
        (before.mode(), before.ino(), before.mtime(), before.ctime()),
        (after.mode(), after.ino(), after.mtime(), after.ctime())
    );
    assert_eq!(fs::read(&path).unwrap(), b"diagnostic evidence");
}

#[test]
fn unsafe_and_replaced_storage_never_proves_stopped() {
    let fixture = Fixture::new();
    drop(fixture.lock());
    let path = fixture.root.path().join("state/journal.json.lock");
    fs::remove_file(&path).unwrap();
    symlink(
        fixture.root.path().join("desktop/omarchy/shell.json"),
        &path,
    )
    .unwrap();
    run(&mut fixture.command("Unknown"));
    fs::remove_file(&path).unwrap();
    executable(
        &fixture.root.path().join("bin/qs"),
        "#!/bin/sh\n/bin/mv \"$MIGRATION_FIXTURE/state\" \"$MIGRATION_FIXTURE/state-old\"\n/bin/mkdir \"$MIGRATION_FIXTURE/state\"\nprintf 'Target not found.\\n'\n",
    );
    run(&mut fixture.command("Unknown"));
    assert!(fixture.root.path().join("state-old").is_dir());
}

#[test]
fn conflicting_activation_and_bounded_ipc_fail_closed() {
    let fixture = Fixture::new();
    fs::write(
        fixture.root.path().join("desktop/omarchy/shell.json"),
        r#"{"plugins":[{"id":"data-goblin.fileblade"}]}"#,
    )
    .unwrap();
    run(&mut fixture.command("Unknown"));
    executable(
        &fixture.root.path().join("bin/qs"),
        "#!/bin/sh\n/bin/sleep 10\n",
    );
    let start = std::time::Instant::now();
    run(&mut fixture.command("Unknown"));
    assert!(start.elapsed() < std::time::Duration::from_secs(6));
}

#[test]
fn shell_plugin_rows_accept_the_entry_forms_omarchy_reads() {
    let fixture = Fixture::new();
    fs::create_dir(fixture.root.path().join("state")).unwrap();
    let shell = fixture.root.path().join("desktop/omarchy/shell.json");
    let malformed = r#"Unknown { reason: "legacy shell activation configuration is malformed" }"#;
    let disagree = r#"Unknown { reason: "legacy CLI and shell activation evidence disagree" }"#;
    for (config, expected) in [
        (
            r#"{"plugins":[{"id":"kurt.notifications"},"kurt.calendar"],"disabledPlugins":["kurt.elgato"]}"#,
            "Stopped",
        ),
        (r#"{"plugins":["data-goblin.fileblade"]}"#, "Stopped"),
        (
            r#"{"plugins":["kurt.calendar"],"bar":{"layout":{"center":["data-goblin.fileblade"]}}}"#,
            disagree,
        ),
        (
            r#"{"plugins":[{"id":"data-goblin.fileblade"},"kurt.calendar"]}"#,
            disagree,
        ),
        (r#"{"plugins":[""]}"#, malformed),
        (r#"{"plugins":[7]}"#, malformed),
        (r#"{"plugins":[null]}"#, malformed),
        (r#"{"plugins":[["kurt.calendar"]]}"#, malformed),
        (r#"{"plugins":[{"name":"kurt.calendar"}]}"#, malformed),
        (r#"{"plugins":[{"id":7}]}"#, malformed),
    ] {
        fs::write(&shell, config).unwrap();
        run(&mut fixture.command(expected));
    }
}
