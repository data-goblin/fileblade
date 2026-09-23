use fileblade::core_modules::canonical::sha256_hex;
use std::path::{Path, PathBuf};
use std::process::Command;
use tempfile::TempDir;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
}

fn run(manifest: &Path, home: &Path) -> (bool, String) {
    let output = Command::new("sh")
        .arg(root().join("install.sh"))
        .env_clear()
        .env("PATH", std::env::var("PATH").unwrap_or_default())
        .env("HOME", home)
        .env("XDG_DATA_HOME", home.join(".local/share"))
        .env("FILEBLADE_INSTALL_MANIFEST", manifest)
        .output()
        .expect("install.sh runs");
    let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
    text.push_str(&String::from_utf8_lossy(&output.stderr));
    (output.status.success(), text)
}

fn manifest(directory: &Path, body: &str) -> PathBuf {
    let path = directory.join("latest.json");
    std::fs::write(&path, body).expect("manifest written");
    path
}

fn scratch(name: &str) -> TempDir {
    tempfile::Builder::new()
        .prefix(name)
        .tempdir_in(root().join("target"))
        .unwrap()
}

#[test]
fn the_bootstrap_refuses_a_manifest_without_this_architecture() {
    let scratch = scratch("missing-target");
    let directory = scratch.path();
    let path = manifest(
        directory,
        r#"{"schema":1,"version":"9.9.9","artifacts":{"riscv64-unknown-linux-musl":{"url":"https://example.invalid/a.tar.gz","sha256":"0000000000000000000000000000000000000000000000000000000000000000"}}}"#,
    );
    let (ok, text) = run(&path, directory);
    assert!(!ok, "{text}");
    assert!(text.contains("carries no"), "{text}");
    assert!(!directory.join(".local/bin/fileblade").exists());
}

#[test]
fn the_bootstrap_refuses_a_digest_that_is_not_sha256() {
    let scratch = scratch("short-digest");
    let directory = scratch.path();
    let path = manifest(
        directory,
        r#"{"schema":1,"version":"9.9.9","artifacts":{"x86_64-unknown-linux-musl":{"url":"https://example.invalid/a.tar.gz","sha256":"abc123"}}}"#,
    );
    let (ok, text) = run(&path, directory);
    assert!(!ok, "{text}");
    assert!(text.contains("not a SHA-256 digest"), "{text}");
}

#[test]
fn the_bootstrap_refuses_a_manifest_without_a_version() {
    let scratch = scratch("no-version");
    let directory = scratch.path();
    let path = manifest(directory, r#"{"schema":1,"artifacts":{}}"#);
    let (ok, text) = run(&path, directory);
    assert!(!ok, "{text}");
    assert!(text.contains("no version"), "{text}");
}

#[test]
fn bootstrap_rejects_mislabeled_archives_before_running_their_installer() {
    use serde_json::json;

    let scratch = scratch("mislabeled-archive");
    let directory = scratch.path();
    let payload = directory.join("payload");
    std::fs::create_dir(&payload).unwrap();
    let archive = directory.join("payload.tar.gz");
    let target = format!("{}-unknown-linux-musl", std::env::consts::ARCH);
    for (version, payload_target) in [("0.1.3", target.as_str()), ("0.2.0", "wrong-target")] {
        std::fs::write(
            payload.join("payload.json"),
            json!({"version": version, "target": payload_target}).to_string(),
        )
        .unwrap();
        assert!(
            Command::new("tar")
                .arg("-czf")
                .arg(&archive)
                .arg("-C")
                .arg(directory)
                .arg("payload")
                .status()
                .unwrap()
                .success()
        );
        let digest = sha256_hex(&std::fs::read(&archive).unwrap());
        let path = manifest(
            directory,
            &json!({"schema": 1, "version": "0.2.0", "artifacts": {
                target.clone(): {"url": archive, "sha256": digest}
            }})
            .to_string(),
        );
        let (ok, text) = run(&path, directory);
        assert!(!ok, "{text}");
        assert!(text.contains("archive version or target differs"), "{text}");
        assert!(!directory.join(".local/bin/fileblade").exists());
    }
}

#[test]
fn release_archives_refuse_to_mix_versions_without_changing_existing_assets() {
    use serde_json::{Value, json};
    use std::os::unix::fs::PermissionsExt;

    let scratch = scratch("release-archive");
    let directory = scratch.path();
    let payload = directory.join("payload");
    let output = directory.join("output");
    std::fs::create_dir(&payload).unwrap();
    std::fs::create_dir(&output).unwrap();
    let runtime: Value =
        serde_json::from_slice(&std::fs::read(root().join("packaging/runtime.json")).unwrap())
            .unwrap();
    let mut files = Vec::new();
    for relative in runtime["required"].as_array().unwrap() {
        let relative = relative.as_str().unwrap();
        let source = root().join(if relative == "bin/fileblade" {
            "fileblade-bin"
        } else {
            relative
        });
        let destination = payload.join(relative);
        std::fs::create_dir_all(destination.parent().unwrap()).unwrap();
        std::fs::copy(source, &destination).unwrap();
        files.push(json!({
            "path": relative,
            "mode": format!("{:o}", destination.metadata().unwrap().permissions().mode() & 0o777),
            "sha256": sha256_hex(&std::fs::read(destination).unwrap()),
        }));
    }
    files.sort_by(|a, b| a["path"].as_str().cmp(&b["path"].as_str()));
    let source_manifest: Value =
        serde_json::from_slice(&std::fs::read(root().join("manifest.json")).unwrap()).unwrap();
    std::fs::write(
        payload.join("payload.json"),
        json!({"schema": 1, "kind": "fileblade-native", "version": source_manifest["version"],
            "source": "0".repeat(40), "target": "x86_64-unknown-linux-musl",
            "architecture": "x86_64", "files": files})
        .to_string(),
    )
    .unwrap();
    let previous = r#"{"schema":1,"version":"0.1.3","artifacts":{"unsupported-unknown-linux-musl":{"url":"https://example.invalid/old.tar.gz","sha256":"old"}}}"#;
    std::fs::write(output.join("latest.json"), previous).unwrap();
    let result = Command::new(root().join("packaging/release-archive"))
        .arg(&payload)
        .arg(&output)
        .output()
        .unwrap();
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stderr).contains("belongs to another release"));
    assert_eq!(
        std::fs::read_to_string(output.join("latest.json")).unwrap(),
        previous
    );
    assert_eq!(std::fs::read_dir(output).unwrap().count(), 1);
}

#[test]
fn the_bootstrap_never_reads_a_local_artifact_named_by_a_remote_manifest() {
    let script = std::fs::read_to_string(root().join("install.sh")).expect("bootstrap readable");
    assert!(
        script.contains("a remote manifest may not point at a local artifact"),
        "a remote manifest must not be able to install an attacker-named local path"
    );
    assert!(
        script.contains("the manifest artifact URL is not https"),
        "remote artifacts must be fetched over https"
    );
    assert!(
        script.contains("checksum mismatch"),
        "the downloaded archive must be checked against the manifest digest"
    );
    assert!(
        script.contains("tools/native\" verify"),
        "the payload inventory must be verified before it is installed"
    );
}
