use serde_json::{Value, json};
use std::fs;
use std::os::unix::fs::symlink;
use std::process::Command;

#[test]
fn native_discovery_without_authority_cannot_create_activation_receipts() {
    let binary = std::env::var_os("FILEBLADE_BINARY")
        .unwrap_or_else(|| env!("CARGO_BIN_EXE_fileblade").into());
    for selection in ["explicit", "receipt", "mismatched", "missing"] {
        let receipt_selected = selection != "explicit";
        let root = tempfile::tempdir().unwrap();
        let config = root.path().join("config");
        let state = root.path().join("state");
        let data = root.path().join("data");
        let provider = config.join("fileblade/extensions/test.goblins");
        fs::create_dir_all(&provider).unwrap();
        fs::write(provider.join("Module.qml"), "import QtQuick\nItem {}\n").unwrap();
        fs::write(
            provider.join("manifest.json"),
            json!({"id":"test.goblins","name":"Fixture","version":"0.1.0",
                "extensions":{"data-goblin.fileblade/blade":[{"id":"goblins","entry":"Module.qml"}]}})
                .to_string(),
        )
        .unwrap();
        let app = root.path().join("app");
        fs::create_dir_all(&app).unwrap();
        fs::write(app.join("manifest.json"), "{}").unwrap();
        fs::create_dir_all(app.join("app")).unwrap();
        fs::write(app.join("app/shell.qml"), "").unwrap();
        let executable = if receipt_selected {
            let installation = data.join("fileblade/installation");
            let payload = "a".repeat(64);
            let version = installation.join("versions").join(&payload);
            let generation = installation.join("generations/generation.fixture");
            fs::create_dir_all(&version).unwrap();
            fs::create_dir_all(&generation).unwrap();
            fs::write(
                generation.join("receipt.json"),
                json!({"schema":1,"owner":"direct","installation":installation,"payload":if selection == "mismatched" {"b".repeat(64)} else {payload.clone()}})
                    .to_string(),
            )
            .unwrap();
            symlink(
                format!("../../versions/{payload}"),
                generation.join("runtime"),
            )
            .unwrap();
            symlink(
                "generations/generation.fixture",
                installation.join("active"),
            )
            .unwrap();
            let executable = version.join("fileblade-bin");
            fs::copy(&binary, &executable).unwrap();
            executable
        } else {
            binary.clone().into()
        };
        if selection == "missing" {
            fs::remove_file(
                data.join("fileblade/installation/generations/generation.fixture/receipt.json"),
            )
            .unwrap();
        }
        let mut command = Command::new(executable);
        command
            .args(["_backend", "plugin-catalog"])
            .env("HOME", root.path())
            .env("XDG_CONFIG_HOME", &config)
            .env("XDG_STATE_HOME", &state)
            .env("XDG_DATA_HOME", &data)
            .env_remove("FILEBLADE_NATIVE_STATE_ROOT")
            .env_remove("FILEBLADE_APP_ROOT");
        if !receipt_selected {
            command.env("FILEBLADE_APP_ROOT", &app).env(
                "FILEBLADE_NATIVE_STATE_ROOT",
                state.join("omarchy/fileblade"),
            );
        }
        let output = command.output().unwrap();
        assert!(output.status.success(), "{output:?}");
        let catalog: Value = serde_json::from_slice(&output.stdout).unwrap();
        if matches!(selection, "missing" | "mismatched") {
            assert_eq!(catalog["providers"], json!([]), "{catalog}");
            assert!(!catalog["diagnostics"].as_array().unwrap().is_empty());
        } else {
            assert_eq!(
                catalog["providers"].as_array().unwrap().len(),
                1,
                "{catalog}"
            );
            assert_eq!(catalog["providers"][0]["id"], "test.goblins");
            assert_eq!(catalog["providers"][0]["enabled"], false, "{catalog}");
        }
        assert_eq!(catalog["activation"], "unknown", "{catalog}");
        assert!(!config.join("omarchy").exists());
        assert!(!state.exists());
    }
}
