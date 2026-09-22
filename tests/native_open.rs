use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Command, Output};

fn app_root(root: &Path, qs_body: &str, launch_body: &str) {
    fs::create_dir_all(root.join("app")).unwrap();
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::write(root.join("manifest.json"), "{}").unwrap();
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

#[test]
fn native_launch_waits_for_plugin_discovery_before_starting_its_writer() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    app_root(root, "exit 9", "exit 9");
    fs::write(root.join("app/launch"), include_str!("../app/launch")).unwrap();
    for (name, body) in [
        (
            "qs",
            r#"if [ "$1" = ipc ]; then
  count=$(cat "$HOME/probes" 2>/dev/null || echo 0)
  count=$((count + 1))
  echo "$count" > "$HOME/probes"
  case "$count" in
    1) exit 1 ;;
    2) echo 'Not ready to accept queries yet' ;;
    *) echo '[]' ;;
  esac
else
  echo native-view
fi"#,
        ),
        (
            "fileblade",
            r#"case "$*" in
  'serve --native-probe') test -f "$HOME/authority" ;;
  'serve --native-authority --max-concurrency 16') cp "$HOME/probes" "$HOME/authority" ;;
  *) exit 0 ;;
esac"#,
        ),
        ("hyprctl", "exit 0"),
    ] {
        let path = root.join("bin").join(name);
        fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    let output = Command::new(root.join("app/launch"))
        .env("HOME", root)
        .env("XDG_STATE_HOME", root.join("state"))
        .env("OMARCHY_PATH", root.join("omarchy"))
        .env("PATH", format!("{}:/usr/bin", root.join("bin").display()))
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert_eq!(fs::read_to_string(root.join("authority")).unwrap(), "3\n");
    assert_eq!(String::from_utf8_lossy(&output.stdout), "native-view\n");
}
