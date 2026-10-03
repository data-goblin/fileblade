use std::{fs, os::unix::fs::PermissionsExt, path::Path, process::Command};
use tempfile::tempdir;

fn executable(path: &Path, body: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, body).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o755)).unwrap();
}

#[test]
fn launcher_prefers_the_bundle_and_honors_an_explicit_development_override() {
    let temporary = tempdir().unwrap();
    let root = temporary.path();
    let launcher = root.join("fileblade");
    executable(&launcher, include_str!("../fileblade"));
    executable(
        &root.join("fileblade-bin"),
        "#!/bin/sh\nprintf 'bundle\\n'\n",
    );
    executable(
        &root.join("target/release/fileblade"),
        "#!/bin/sh\nprintf 'development\\n'\n",
    );
    executable(
        &root.join("commands/uname"),
        "#!/bin/sh\nprintf 'x86_64\\n'\n",
    );
    let path = format!("{}:/usr/bin:/bin", root.join("commands").display());
    let output = Command::new(&launcher)
        .env_remove("FILEBLADE_BINARY")
        .env("PATH", &path)
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"bundle\n");

    let override_binary = root.join("custom backend");
    executable(&override_binary, "#!/bin/sh\nprintf '<%s>\\n' \"$@\"\n");
    let output = Command::new(&launcher)
        .env("PATH", &path)
        .env("FILEBLADE_BINARY", override_binary)
        .args(["one argument", "--flag"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"<one argument>\n<--flag>\n");
}

#[test]
fn another_architecture_skips_the_bundle_and_runs_a_backend_built_there() {
    let temporary = tempdir().unwrap();
    let root = temporary.path();
    executable(&root.join("fileblade"), include_str!("../fileblade"));
    executable(&root.join("fileblade-bin"), "#!/bin/sh\nexit 99\n");
    let built = root.join("target/release/fileblade");
    executable(&built, "#!/bin/sh\nprintf 'built here\\n'\n");
    executable(
        &root.join("commands/uname"),
        "#!/bin/sh\nprintf 'aarch64\\n'\n",
    );
    let launch = || {
        Command::new(root.join("fileblade"))
            .env_remove("FILEBLADE_BINARY")
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", root.join("commands").display()),
            )
            .output()
            .unwrap()
    };
    let output = launch();
    assert!(output.status.success());
    assert_eq!(output.stdout, b"built here\n");

    fs::remove_file(&built).unwrap();
    let output = launch();
    assert_eq!(output.status.code(), Some(126));
    assert!(output.stdout.is_empty());
    let refusal = String::from_utf8_lossy(&output.stderr);
    assert!(refusal.contains("x86-64 only"), "{refusal}");
    assert!(refusal.contains("this machine is aarch64"), "{refusal}");
    assert!(refusal.contains("FILEBLADE_BINARY"), "{refusal}");
}
