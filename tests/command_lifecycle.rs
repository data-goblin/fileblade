use fileblade::command::CommandSpec;
use std::path::PathBuf;
use std::thread;
use std::time::{Duration, Instant};

#[test]
fn detached_children_are_reaped() {
    let pid = CommandSpec::new("/bin/sh")
        .args(["-c", "exit 0"])
        .spawn_detached()
        .expect("detached command should start");
    let process = PathBuf::from(format!("/proc/{pid}"));
    let deadline = Instant::now() + Duration::from_secs(2);
    while process.exists() && Instant::now() < deadline {
        thread::sleep(Duration::from_millis(10));
    }
    assert!(!process.exists(), "detached child remained as a zombie");
}

#[test]
fn a_detached_child_reads_its_input_and_outlives_the_call() {
    let file = std::env::temp_dir().join(format!("fileblade-detached-{}", std::process::id()));
    let _ = std::fs::remove_file(&file);
    let target = file.to_string_lossy().to_string();
    let pid = CommandSpec::new("/bin/sh")
        .args([
            "-c",
            "cat > \"$0\"; printf closed >> \"$0\"; sleep 5",
            &target,
        ])
        .stdin("clipboard payload")
        .timeout(Duration::from_secs(3))
        .spawn_detached_with_stdin(&std::sync::atomic::AtomicBool::new(false))
        .expect("detached command should start");
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        if std::fs::read_to_string(&file).unwrap_or_default() == "clipboard payloadclosed" {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    let written = std::fs::read_to_string(&file).unwrap_or_default();
    let alive = PathBuf::from(format!("/proc/{pid}")).exists();
    let _ = std::process::Command::new("/bin/kill")
        .args(["-KILL", &format!("-{pid}")])
        .status();
    let _ = std::fs::remove_file(&file);
    assert_eq!(
        written, "clipboard payloadclosed",
        "detached stdin was not delivered and closed"
    );
    assert!(alive, "the detached child was killed with the caller");
}

#[test]
fn a_daemonising_child_does_not_hold_the_runner() {
    let started = Instant::now();
    let output = CommandSpec::new("/bin/sh")
        .args(["-c", "sleep 30 & printf done; exit 0"])
        .timeout(Duration::from_secs(5))
        .run()
        .expect("command should finish once the direct child exits");
    assert!(output.status.success());
    assert_eq!(output.stdout, b"done");
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "runner waited on a background descendant for {:?}",
        started.elapsed()
    );
}

#[test]
fn a_daemonising_child_with_closed_pipes_returns_at_once() {
    let started = Instant::now();
    let output = CommandSpec::new("/bin/sh")
        .args(["-c", "sleep 30 >/dev/null 2>&1 & printf done; exit 0"])
        .timeout(Duration::from_secs(5))
        .run()
        .expect("command should finish once the direct child exits");
    assert_eq!(output.stdout, b"done");
    assert!(
        started.elapsed() < Duration::from_secs(2),
        "took {:?}",
        started.elapsed()
    );
}
