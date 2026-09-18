use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output};

fn app_root(root: &Path, qs_body: &str, launch_body: &str) {
    fs::create_dir_all(root.join("app")).unwrap();
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::write(root.join("manifest.json"), "{}").unwrap();
    fs::write(root.join("Service.qml"), "").unwrap();
    fs::write(root.join("app/shell.qml"), "").unwrap();
    for (path, body) in [("bin/qs", qs_body), ("app/launch", launch_body)] {
        fs::write(root.join(path), format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(root.join(path), fs::Permissions::from_mode(0o700)).unwrap();
    }
}

fn open(root: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_fileblade"))
        .args(["native", "open"])
        .args(arguments)
        .env("HOME", root)
        .env("FILEBLADE_APP_ROOT", root)
        .env("PATH", root.join("bin"))
        .output()
        .unwrap()
}

#[test]
fn a_file_argument_opens_its_parent_and_selects_it_over_ipc() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    app_root(
        root,
        &format!(
            "printf '%s\\n' \"$*\" > {}; echo '{{\"ok\":true}}'",
            root.join("qs.argv").display()
        ),
        "exit 9",
    );
    let file = root.join("docs/note.txt");
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, "x").unwrap();
    let output = open(root, &[file.to_str().unwrap()]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        fs::read_to_string(root.join("qs.argv")).unwrap().trim(),
        format!(
            "ipc -n -p {} call -- fileblade.native open {} {}",
            root.join("app").display(),
            root.join("docs").display(),
            file.display()
        )
    );
}

#[test]
fn remote_and_missing_paths_are_refused_before_anything_starts() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    app_root(root, "exit 9", "exit 9");
    for (argument, expected) in [
        ("sftp://peer.test/files", "not a local path"),
        (&format!("{}/absent", root.display()), "No such file"),
    ] {
        let output = open(root, &[argument]);
        assert_eq!(output.status.code(), Some(1), "{output:?}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(expected),
            "{output:?}"
        );
    }
}

#[test]
fn without_a_running_view_the_app_starts_with_the_target_in_its_environment() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    app_root(
        root,
        "echo 'No running instances' >&2; exit 255",
        &format!(
            "printf 'open=%s\\nselect=%s\\n' \"$FILEBLADE_OPEN\" \"$FILEBLADE_SELECT\" > {}",
            root.join("launch.env").display()
        ),
    );
    let file = root.join("docs/note.txt");
    fs::create_dir_all(file.parent().unwrap()).unwrap();
    fs::write(&file, "x").unwrap();
    let output = open(root, &[file.to_str().unwrap()]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        fs::read_to_string(root.join("launch.env")).unwrap(),
        format!(
            "open={}\nselect={}\n",
            root.join("docs").display(),
            file.display()
        )
    );
    let output = open(root, &[root.join("docs").to_str().unwrap()]);
    assert!(output.status.success(), "{output:?}");
    assert_eq!(
        fs::read_to_string(root.join("launch.env")).unwrap(),
        format!("open={}\nselect=\n", root.join("docs").display())
    );
}
