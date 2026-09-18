use serde_json::{Value, json};
use std::fs;
use std::process::Command;

#[test]
fn standalone_bin_put_locks_recovery_with_app_root_override() {
    for app_ready in [false, true] {
        let root = tempfile::tempdir().unwrap();
        if app_ready {
            fs::write(root.path().join("manifest.json"), "{}").unwrap();
            fs::create_dir_all(root.path().join("app")).unwrap();
            fs::write(root.path().join("app/shell.qml"), "").unwrap();
        }
        let data = root.path().join("data");
        let output = Command::new(
            std::env::var_os("FILEBLADE_BINARY")
                .unwrap_or_else(|| env!("CARGO_BIN_EXE_fileblade").into()),
        )
        .args([
            "_backend",
            "bin-put",
            "--module",
            "hooks",
            "--item",
            &json!({"id":"fixture","payload":{"body":"recovery"}}).to_string(),
        ])
        .env("HOME", root.path())
        .env("FILEBLADE_APP_ROOT", root.path())
        .env("XDG_CONFIG_HOME", root.path().join("config"))
        .env("XDG_DATA_HOME", &data)
        .env("XDG_STATE_HOME", root.path().join("state"))
        .env_remove("FILEBLADE_NATIVE_STATE_ROOT")
        .output()
        .unwrap();
        assert!(output.status.success(), "{output:?}");
        let response: Value = serde_json::from_slice(
            output
                .stdout
                .split(|byte| *byte == b'\n')
                .rfind(|line| !line.is_empty())
                .unwrap_or_default(),
        )
        .unwrap_or_else(|_| panic!("{}", String::from_utf8_lossy(&output.stderr)));
        assert_eq!(response["ok"], true, "{response}");
        let entry = response["entry"].as_str().unwrap();
        let record = data
            .join("fileblade/bin/hooks")
            .join(entry.strip_prefix("bin:").unwrap())
            .join("manifest.json");
        assert!(record.is_file());
        assert!(data.join("fileblade/bin/.mutation.lock").is_file());
        let saved: Value = serde_json::from_slice(&fs::read(record).unwrap()).unwrap();
        assert_eq!(saved["payload"], json!({"body":"recovery"}));
    }
}
