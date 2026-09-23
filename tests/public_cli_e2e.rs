use serde_json::{Value, json};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use tempfile::TempDir;

struct CliHarness {
    temporary: TempDir,
    fake_bin: PathBuf,
    omarchy: PathBuf,
    argv_log: PathBuf,
    calls_log: PathBuf,
    data_home: PathBuf,
}

impl CliHarness {
    fn new() -> Self {
        let temporary = tempfile::tempdir().unwrap();
        let root = temporary.path();
        let fake_bin = root.join("bin");
        let omarchy = root.join("omarchy");
        let shell = omarchy.join("shell");
        let home = root.join("home");
        let config_home = root.join("config");
        let state_home = root.join("state");
        let data_home = root.join("data");
        let runtime = root.join("runtime");
        for directory in [
            &fake_bin,
            &shell,
            &home,
            &config_home,
            &state_home,
            &data_home,
            &runtime,
        ] {
            fs::create_dir_all(directory).unwrap();
        }
        fs::write(shell.join("shell.qml"), "import QtQuick\nItem {}\n").unwrap();
        let qs = fake_bin.join("qs");
        fs::write(
            &qs,
            r##"#!/bin/sh
set -eu
: > "$FILEBLADE_TEST_ARGV"
for argument do
  printf '%s\n' "$argument" >> "$FILEBLADE_TEST_ARGV"
done
printf '%s\t%s\n' "$7" "$8" >> "$FILEBLADE_TEST_CALLS"
printf '%s\n' "$FILEBLADE_TEST_RESPONSE"
"##,
        )
        .unwrap();
        let mut permissions = fs::metadata(&qs).unwrap().permissions();
        permissions.set_mode(0o700);
        fs::set_permissions(&qs, permissions).unwrap();
        let argv_log = root.join("qs.argv");
        let calls_log = root.join("qs.calls");
        Self {
            temporary,
            fake_bin,
            omarchy,
            argv_log,
            calls_log,
            data_home,
        }
    }

    fn run(&self, arguments: &[&str], response: &str) -> Output {
        self.run_in(self.temporary.path(), arguments, response)
    }

    fn run_in(&self, directory: &Path, arguments: &[&str], response: &str) -> Output {
        let root = self.temporary.path();
        let mut command = Command::new(env!("CARGO_BIN_EXE_fileblade"));
        command
            .args(arguments)
            .current_dir(directory)
            .env_clear()
            .env("PATH", &self.fake_bin)
            .env("HOME", root.join("home"))
            .env("XDG_CONFIG_HOME", root.join("config"))
            .env("XDG_STATE_HOME", root.join("state"))
            .env("XDG_DATA_HOME", &self.data_home)
            .env("XDG_RUNTIME_DIR", root.join("runtime"))
            .env("OMARCHY_PATH", &self.omarchy)
            .env("WAYLAND_DISPLAY", "wayland-test")
            .env("LC_ALL", "C")
            .env("FILEBLADE_TEST_ARGV", &self.argv_log)
            .env("FILEBLADE_TEST_CALLS", &self.calls_log)
            .env("FILEBLADE_TEST_RESPONSE", response);
        command.output().unwrap()
    }

    fn recorded_arguments(&self) -> Vec<String> {
        fs::read_to_string(&self.argv_log)
            .unwrap()
            .lines()
            .map(ToOwned::to_owned)
            .collect()
    }

    fn shell_config(&self) -> String {
        self.omarchy.join("shell").to_string_lossy().into_owned()
    }

    fn recorded_calls(&self) -> Vec<String> {
        fs::read_to_string(&self.calls_log)
            .unwrap()
            .lines()
            .map(ToOwned::to_owned)
            .collect()
    }
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn assert_empty(directory: &Path) {
    assert_eq!(fs::read_dir(directory).unwrap().count(), 0);
}

#[test]
fn navigate_preserves_virtual_trash_resource_in_exact_quickshell_argv() {
    let harness = CliHarness::new();
    let output = harness.run(&["navigate", "trash:///"], "queued");
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(output.stdout, b"queued\n");
    assert_eq!(
        harness.recorded_arguments(),
        [
            "ipc".to_string(),
            "-n".to_string(),
            "-p".to_string(),
            harness.shell_config(),
            "call".to_string(),
            "--".to_string(),
            "data-goblin.fileblade.control".to_string(),
            "navigate".to_string(),
            "trash:///".to_string(),
        ]
    );
    assert_empty(&harness.data_home);
}

#[test]
fn json_output_decodes_the_direct_ipc_document() {
    let harness = CliHarness::new();
    let response = json!({
        "open": true,
        "rootPath": "trash:///",
        "selectedPaths": ["/tmp/example"],
    });
    let output = harness.run(&["--output", "json", "status"], &response.to_string());
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(output.stderr.is_empty());
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap(),
        response
    );
    assert_eq!(
        harness.recorded_arguments(),
        [
            "ipc".to_string(),
            "-n".to_string(),
            "-p".to_string(),
            harness.shell_config(),
            "call".to_string(),
            "--".to_string(),
            "data-goblin.fileblade".to_string(),
            "status".to_string(),
        ]
    );
    assert_empty(&harness.data_home);
}

#[test]
fn doctor_json_failure_keeps_each_stream_machine_parseable() {
    let harness = CliHarness::new();
    fs::remove_file(harness.fake_bin.join("qs")).unwrap();
    let output = harness.run(&["--output", "json", "doctor"], "");
    assert_eq!(output.status.code(), Some(1));
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    let error: Value = serde_json::from_slice(&output.stderr).unwrap();
    assert_eq!(report["ok"], false);
    assert_eq!(report["backend_version"], env!("CARGO_PKG_VERSION"));
    assert_eq!(report["serve"]["ok"], true, "{report}");
    assert_eq!(report["shell"]["ok"], false, "{report}");
    assert_eq!(
        error,
        json!({"ok": false, "error": "fileblade is not healthy"})
    );
    assert_empty(&harness.data_home);
}

#[test]
fn permanent_trash_commands_require_confirmation_before_dispatch() {
    let harness = CliHarness::new();
    for arguments in [
        &["trash-delete", "--id", "entry-id"][..],
        &["trash-empty"][..],
    ] {
        let output = harness.run(arguments, "must not run");
        assert_eq!(output.status.code(), Some(2));
        assert!(output.stdout.is_empty());
        assert!(stderr(&output).contains("--yes"), "{}", stderr(&output));
        assert!(!harness.argv_log.exists());
        assert_empty(&harness.data_home);
    }
}

#[test]
fn live_trash_dispatches_directly_to_the_control_target() {
    let harness = CliHarness::new();
    let output = harness.run(&["trash", "--yes"], "op-approved");
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(output.stdout, b"op-approved\n");
    assert_eq!(
        harness.recorded_calls(),
        ["data-goblin.fileblade.control\ttrashSelection"]
    );
    assert_eq!(
        harness.recorded_arguments(),
        [
            "ipc".to_string(),
            "-n".to_string(),
            "-p".to_string(),
            harness.shell_config(),
            "call".to_string(),
            "--".to_string(),
            "data-goblin.fileblade.control".to_string(),
            "trashSelection".to_string(),
        ]
    );
    assert_empty(&harness.data_home);
}

#[test]
fn live_rename_dispatches_directly_to_the_control_target() {
    let harness = CliHarness::new();
    let output = harness.run(&["rename", "after.txt"], "op-renamed");
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(output.stdout, b"op-renamed\n");
    assert_eq!(
        harness.recorded_calls(),
        ["data-goblin.fileblade.control\trenameSelection"]
    );
    assert_eq!(
        harness.recorded_arguments(),
        [
            "ipc".to_string(),
            "-n".to_string(),
            "-p".to_string(),
            harness.shell_config(),
            "call".to_string(),
            "--".to_string(),
            "data-goblin.fileblade.control".to_string(),
            "renameSelection".to_string(),
            "after.txt".to_string(),
        ]
    );
    assert_empty(&harness.data_home);
}

#[test]
fn shell_scripts_parse_and_pick_prints_accepted_paths() {
    let harness = CliHarness::new();
    for shell in ["bash", "zsh"] {
        let output = harness.run(&["shell", shell], "");
        assert!(output.status.success(), "{}", stderr(&output));
        let script = String::from_utf8(output.stdout).unwrap();
        assert!(script.contains("fileblade-file-widget"));
        assert!(script.contains("fileblade pick --mode folder"));
        if let Ok(check) = Command::new(shell)
            .arg("-n")
            .arg("-c")
            .arg(&script)
            .output()
        {
            assert!(
                check.status.success(),
                "{shell} -n: {}",
                String::from_utf8_lossy(&check.stderr)
            );
        }
    }
    let accepted = r#"{"status":"accepted","mode":"open","paths":["/tmp/a b.txt","/tmp/c.md"]}"#;
    let output = harness.run(&["pick", "--multiple", "--shell-quote"], accepted);
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "'/tmp/a b.txt' /tmp/c.md"
    );
    let output = harness.run(&["--output", "json", "pick"], accepted);
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(document["paths"][1], "/tmp/c.md");
    let cancelled = r#"{"status":"cancelled","mode":"open","paths":[]}"#;
    let output = harness.run(&["pick"], cancelled);
    assert!(!output.status.success());
    assert!(stderr(&output).contains("picker cancelled"));
    let output = harness.run(&["root"], r#"{"rootPath":"/home/example/git","open":true}"#);
    assert_eq!(
        String::from_utf8(output.stdout).unwrap().trim(),
        "/home/example/git"
    );
}

#[test]
fn every_wrapped_control_verb_dials_its_ipc_method() {
    let cases: [(&[&str], &[&str]); 18] = [
        (&["search-deep", "on"], &["setSearchDeep", "true"]),
        (
            &["git-details", "modified", "new"],
            &["setGitStatusDetails", "modified,new"],
        ),
        (
            &["git-summary", "branch", "worktree", "untracked"],
            &["setGitSummaryFields", "branch,worktree,untracked"],
        ),
        (&["git-summary"], &["setGitSummaryFields", ""]),
        (&["cycle-metric"], &["cyclePriorityProperty"]),
        (
            &["set-folder-color", "/tmp/a", "red"],
            &["setFolderColor", "/tmp/a", "red"],
        ),
        (
            &["clear-folder-color", "/tmp/a"],
            &["clearFolderColor", "/tmp/a"],
        ),
        (&["color-scope", "row"], &["setFolderColorScope", "row"]),
        (
            &["toggle-favorite", "/tmp/a"],
            &["toggleFavorite", "/tmp/a"],
        ),
        (
            &["properties", "placement", "below"],
            &["setPlacement", "below"],
        ),
        (&["properties", "focus"], &["focusProperties"]),
        (&["settings"], &["toggleSettings"]),
        (&["toggle-path", "/tmp/a"], &["togglePath", "/tmp/a"]),
        (
            &["blade", "slot-module", "left", "0", "notes"],
            &["setSlotModule", "left", "0", "notes"],
        ),
        (
            &["blade", "tab-set", "left", "0", "1"],
            &["setBladeTab", "left", "0", "1"],
        ),
        (
            &["blade", "tab-cycle", "right", "1", "-1"],
            &["cycleBladeTab", "right", "1", "-1"],
        ),
        (
            &["blade", "tab-remove", "left", "0", "2"],
            &["removeBladeTab", "left", "0", "2"],
        ),
        (
            &["blade", "tab-into", "left", "0", "right", "1", "--tab", "2"],
            &["tabBladeSlot", "left", "0", "right", "1", "2", ""],
        ),
    ];
    for (arguments, expected) in cases {
        let harness = CliHarness::new();
        let output = harness.run(arguments, "ok");
        assert!(
            output.status.success(),
            "{arguments:?}: {}",
            stderr(&output)
        );
        let recorded = harness.recorded_arguments();
        let call = &recorded[6..];
        assert_eq!(
            call[0], "data-goblin.fileblade.control",
            "{arguments:?}: {recorded:?}"
        );
        assert_eq!(&call[1..], expected, "{arguments:?}: {recorded:?}");
        if arguments.first() == Some(&"blade") {
            for response in ["invalid-module", "invalid-slot", "invalid-tab", "no-screen"] {
                let rejected = harness.run(arguments, response);
                assert!(!rejected.status.success(), "{arguments:?}: {response}");
                assert!(stderr(&rejected).contains(response));
                assert!(rejected.stdout.is_empty());
                assert_eq!(harness.recorded_arguments(), recorded);
            }
        }
    }
}

#[test]
fn modules_text_output_groups_rows_under_category_headers() {
    let harness = CliHarness::new();
    let response = json!({
        "count": 4,
        "modules": [
            { "id": "files", "name": "Files", "glyph": "", "description": "The file tree", "category": "Module", "source": "builtin", "singleton": true, "entry": "file:///plugin/modules/files/Module.qml", "settings": { "keys": [] }, "placed": { "edge": "left", "index": 0, "tab": 0 } },
            { "id": "notes", "name": "Notes", "glyph": "", "description": "", "category": "Module", "source": "builtin", "singleton": true, "entry": "file:///plugin/modules/notes/Module.qml", "settings": { "keys": [] }, "placed": null },
            { "id": "agents", "name": "Agents", "glyph": "", "description": "Agent\u{7} strip", "category": "Agents\u{7}", "source": "user", "singleton": true, "entry": "file:///home/u/.config/omarchy/fileblade/modules/agents/Module.qml", "settings": { "keys": [] }, "placed": false },
            { "id": "data-goblin.blade-example/clock", "name": "Clock", "glyph": "", "description": "A live clock", "category": "Plugin", "source": "plugin:data-goblin.blade-example", "singleton": false, "entry": "file:///plugins/example/blades/Clock.qml", "settings": { "keys": ["format"] }, "placed": null }
        ]
    });
    let output = harness.run(&["modules"], &response.to_string());
    assert!(output.status.success(), "{}", stderr(&output));
    let text = String::from_utf8(output.stdout).unwrap();
    assert_eq!(
        text.lines().collect::<Vec<_>>(),
        [
            "MODULE",
            "  files  Files  (placed)  The file tree",
            "  notes  Notes",
            "AGENTS",
            "  agents  Agents  Agent strip",
            "PLUGIN",
            "  data-goblin.blade-example/clock  Clock  A live clock",
        ]
    );
    let recorded = harness.recorded_arguments();
    assert_eq!(&recorded[6..], ["data-goblin.fileblade", "bladeModules"]);
    let output = harness.run(&["--output", "json", "modules"], &response.to_string());
    assert!(output.status.success(), "{}", stderr(&output));
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(document, response);
    assert_eq!(document["modules"][3]["settings"]["keys"][0], "format");
    assert_eq!(document["modules"][0]["placed"]["edge"], "left");
}

#[test]
fn module_dirs_creates_both_paths_in_process_without_the_shell() {
    let harness = CliHarness::new();
    fs::remove_file(harness.fake_bin.join("qs")).unwrap();
    let output = harness.run(&["--output", "json", "module-dirs", "notes"], "");
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(output.stderr.is_empty());
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    let root = harness.temporary.path();
    let state_dir = root.join("state/omarchy/fileblade/modules/notes");
    let config_dir = root.join("config/omarchy/fileblade/config/notes");
    assert_eq!(document["ok"], true);
    assert_eq!(document["name"], "notes");
    assert_eq!(document["state_dir"], state_dir.to_string_lossy().as_ref());
    assert_eq!(
        document["config_dir"],
        config_dir.to_string_lossy().as_ref()
    );
    for directory in [&state_dir, &config_dir] {
        assert!(directory.is_dir(), "{}", directory.display());
        assert_eq!(
            fs::metadata(directory).unwrap().permissions().mode() & 0o777,
            0o700
        );
    }
    assert!(!harness.argv_log.exists(), "module-dirs never dials qs");

    let text = harness.run(&["module-dirs", "data-goblin.blade-example/clock"], "");
    assert!(text.status.success(), "{}", stderr(&text));
    let lines: Vec<String> = String::from_utf8_lossy(&text.stdout)
        .lines()
        .map(ToOwned::to_owned)
        .collect();
    assert_eq!(
        lines,
        [
            root.join("state/omarchy/fileblade/modules/data-goblin.blade-example+clock")
                .to_string_lossy()
                .into_owned(),
            root.join("config/omarchy/fileblade/config/data-goblin.blade-example+clock")
                .to_string_lossy()
                .into_owned(),
        ]
    );

    let refused = harness.run(&["--output", "json", "module-dirs", "a/b/c"], "");
    assert_eq!(refused.status.code(), Some(1));
    let error: Value = serde_json::from_slice(&refused.stderr).unwrap();
    assert_eq!(error["ok"], false);
    assert!(!root.join("state/omarchy/fileblade/modules/a").exists());
    assert_empty(&harness.data_home);
}

fn action_listing() -> Value {
    json!({
        "ok": true,
        "actions": [
            {
                "key": "kurt.tools/dump",
                "id": "dump",
                "source": "plugin",
                "plugin": "kurt.tools",
                "pluginRoot": "/tmp/kurt.tools",
                "title": "Dump env",
                "glyph": "\u{f0476}",
                "description": "Writes the environment",
                "contexts": ["dir", "selection"],
                "confirm": false,
                "detach": false,
                "output": "notice",
                "timeout": 30,
                "program": "scripts/dump-env"
            },
            {
                "key": "user/wipe",
                "id": "wipe",
                "source": "user",
                "plugin": "",
                "pluginRoot": "/tmp/actions",
                "title": "Wipe",
                "glyph": "\u{f0476}",
                "description": "",
                "contexts": ["selection"],
                "confirm": true,
                "detach": false,
                "output": "notice",
                "timeout": 60,
                "program": "rm"
            }
        ]
    })
}

#[test]
fn actions_are_listed_over_the_read_target_and_grouped_by_provider() {
    let harness = CliHarness::new();
    let output = harness.run(&["actions"], &action_listing().to_string());
    assert!(output.status.success(), "{}", stderr(&output));
    let lines: Vec<String> = String::from_utf8_lossy(&output.stdout)
        .lines()
        .map(ToOwned::to_owned)
        .collect();
    assert_eq!(lines[0], "KURT.TOOLS");
    assert!(lines[1].contains("kurt.tools/dump"), "{lines:?}");
    assert!(lines[1].contains("[dir,selection]"), "{lines:?}");
    assert!(lines[1].contains("Dump env"), "{lines:?}");
    assert_eq!(lines[2], "USER");
    assert!(lines[3].contains("user/wipe"), "{lines:?}");
    let arguments = harness.recorded_arguments();
    assert_eq!(arguments[6], "data-goblin.fileblade");
    assert_eq!(arguments[7], "actions");
    assert_empty(&harness.data_home);
}

#[test]
fn action_run_dials_the_control_target_then_polls_the_result() {
    let harness = CliHarness::new();
    let mut response = action_listing();
    response["request_id"] = json!("act-1");
    response["status"] = json!("done");
    response["result"] = json!({
        "ok": true,
        "exit_code": 0,
        "elapsed_ms": 12,
        "stdout_tail": "wrote last.json\n",
        "timed_out": false,
        "detached": false
    });
    let output = harness.run(
        &["action", "dump", "/tmp/one", "--wait", "600"],
        &response.to_string(),
    );
    assert!(output.status.success(), "{}", stderr(&output));
    let text = String::from_utf8_lossy(&output.stdout);
    assert!(text.contains("Dump env: exit 0 in 12 ms"), "{text}");
    assert!(text.contains("wrote last.json"), "{text}");
    assert_eq!(
        harness.recorded_calls(),
        [
            "data-goblin.fileblade\tactions",
            "data-goblin.fileblade.control\trunAction",
            "data-goblin.fileblade\tactionResult",
        ]
    );
    assert_empty(&harness.data_home);
}

#[test]
fn action_run_sends_the_json_paths_and_the_confirmation_flag() {
    let harness = CliHarness::new();
    let mut response = action_listing();
    response["ok"] = json!(false);
    response["error"] = json!("plugin kurt.tools is disabled");
    let output = harness.run(
        &["action", "kurt.tools/dump", "/tmp/a", "/tmp/b", "--yes"],
        &response.to_string(),
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("plugin kurt.tools is disabled"),
        "{}",
        stderr(&output)
    );
    let arguments = harness.recorded_arguments();
    assert_eq!(arguments[6], "data-goblin.fileblade.control");
    assert_eq!(arguments[7], "runAction");
    assert_eq!(arguments[8], "kurt.tools/dump");
    assert_eq!(arguments[9], "base64:WyIvdG1wL2EiLCIvdG1wL2IiXQ==");
    assert_eq!(arguments[10], "true");
    assert_empty(&harness.data_home);
}

#[test]
fn an_action_without_paths_keeps_its_empty_array_argument() {
    let harness = CliHarness::new();
    let mut response = action_listing();
    response["actions"].as_array_mut().unwrap().push(json!({
        "key": "user/marker",
        "id": "marker",
        "source": "user",
        "plugin": "",
        "pluginRoot": "/tmp/actions",
        "title": "Marker",
        "contexts": ["none"],
        "confirm": false,
        "timeout": 10
    }));
    response["ok"] = json!(false);
    response["error"] = json!("stop after transport");
    let output = harness.run(&["action", "user/marker"], &response.to_string());
    assert_eq!(output.status.code(), Some(1));
    let arguments = harness.recorded_arguments();
    assert_eq!(arguments[7], "runAction");
    assert_eq!(arguments[8], "user/marker");
    assert_eq!(arguments[9], "base64:W10=");
    assert_eq!(arguments[10], "false");
}

#[test]
fn a_confirming_action_refuses_to_run_before_it_reaches_the_control_target() {
    let harness = CliHarness::new();
    let output = harness.run(
        &["action", "user/wipe", "/tmp/a"],
        &action_listing().to_string(),
    );
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("--yes"), "{}", stderr(&output));
    assert_eq!(harness.recorded_calls(), ["data-goblin.fileblade\tactions"]);
    assert_empty(&harness.data_home);
}

#[test]
fn an_unknown_action_key_never_reaches_the_control_target() {
    let harness = CliHarness::new();
    let output = harness.run(&["action", "nope"], &action_listing().to_string());
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("no script action nope"),
        "{}",
        stderr(&output)
    );
    assert_eq!(harness.recorded_calls(), ["data-goblin.fileblade\tactions"]);
    assert_empty(&harness.data_home);
}

#[test]
fn action_cli_refuses_invalid_waits_and_paths_before_the_control_target() {
    let harness = CliHarness::new();
    let wait = harness.run(
        &["action", "dump", "/tmp/a", "--wait", "906"],
        &action_listing().to_string(),
    );
    assert_eq!(wait.status.code(), Some(1));
    assert!(
        stderr(&wait).contains("between 1 and 905"),
        "{}",
        stderr(&wait)
    );
    assert_eq!(harness.recorded_calls(), ["data-goblin.fileblade\tactions"]);

    let harness = CliHarness::new();
    let empty = harness.run(&["action", "dump", ""], &action_listing().to_string());
    assert_eq!(empty.status.code(), Some(1));
    assert!(stderr(&empty).contains("every path must be given"));
    assert_eq!(harness.recorded_calls(), ["data-goblin.fileblade\tactions"]);

    let harness = CliHarness::new();
    let mut owned = vec!["action".to_string(), "dump".to_string()];
    owned.extend((0..257).map(|index| format!("/tmp/{index}")));
    let borrowed: Vec<&str> = owned.iter().map(String::as_str).collect();
    let many = harness.run(&borrowed, &action_listing().to_string());
    assert_eq!(many.status.code(), Some(1));
    assert!(stderr(&many).contains("selection too large (256)"));
    assert_eq!(harness.recorded_calls(), ["data-goblin.fileblade\tactions"]);

    let harness = CliHarness::new();
    let long = "x".repeat(65536);
    let large = harness.run(&["action", "dump", &long], &action_listing().to_string());
    assert_eq!(large.status.code(), Some(1));
    assert!(stderr(&large).contains("too large for shell IPC"));
    assert_eq!(harness.recorded_calls(), ["data-goblin.fileblade\tactions"]);
}

#[test]
fn branches_open_close_and_list_the_repository_of_the_current_directory() {
    let harness = CliHarness::new();
    for (arguments, expected) in [
        (&["branches"][..], &["openBranches"][..]),
        (&["branches", "close"], &["closeBranches"]),
    ] {
        let output = harness.run(arguments, "ok");
        assert!(output.status.success(), "{}", stderr(&output));
        let recorded = harness.recorded_arguments();
        assert_eq!(&recorded[6..7], ["data-goblin.fileblade.control"]);
        assert_eq!(&recorded[7..], expected, "{arguments:?}: {recorded:?}");
    }
    let git = which_git();
    std::os::unix::fs::symlink(&git, harness.fake_bin.join("git")).unwrap();
    let repo = harness.temporary.path().join("repo");
    fs::create_dir_all(&repo).unwrap();
    let run_git = |arguments: &[&str]| {
        let output = Command::new(&git)
            .args([
                "-c",
                "user.name=Fixture",
                "-c",
                "user.email=fixture@example.com",
            ])
            .args([
                "-c",
                "commit.gpgsign=false",
                "-c",
                "init.defaultBranch=main",
            ])
            .args(arguments)
            .current_dir(&repo)
            .env("HOME", harness.temporary.path().join("home"))
            .env("GIT_CONFIG_NOSYSTEM", "1")
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "git {arguments:?}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    };
    run_git(&["init"]);
    fs::write(repo.join("README"), "one\n").unwrap();
    run_git(&["add", "README"]);
    run_git(&["commit", "-m", "Initial commit"]);
    run_git(&["branch", "scratch"]);
    fs::write(repo.join("README"), "two\n").unwrap();
    let output = harness.run_in(&repo, &["branches", "list"], "unused");
    assert!(output.status.success(), "{}", stderr(&output));
    let text = String::from_utf8_lossy(&output.stdout);
    let repo_text = repo.to_str().unwrap();
    assert_eq!(
        text,
        format!(
            "* main local 1 changed just now\n  scratch local no upstream just now\n{repo_text} main 1 changed\n"
        )
    );
    let output = harness.run_in(&repo, &["-o", "json", "branches", "list"], "unused");
    assert!(output.status.success(), "{}", stderr(&output));
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(document["ok"], true);
    assert_eq!(document["root"], repo_text);
    assert_eq!(document["current"], "main");
    assert_eq!(document["branches"][0]["name"], "main");
    assert_eq!(document["branches"][0]["kind"], "local");
    assert_eq!(document["branches"][0]["current"], true);
    assert_eq!(document["branches"][1]["name"], "scratch");
    assert_eq!(document["worktrees"][0]["path"], repo_text);
    assert_eq!(document["worktrees"][0]["unstaged"], 1);
    assert_eq!(document["worktrees"][0]["dirty"], true);
    assert_eq!(document["worktrees"][0]["main"], true);
    assert!(
        !harness
            .recorded_arguments()
            .contains(&"git-places".to_string())
    );
    let output = harness.run(&["branches", "list"], "unused");
    assert!(!output.status.success());
    assert!(
        stderr(&output).contains("no repository here"),
        "{}",
        stderr(&output)
    );
}

fn which_git() -> PathBuf {
    std::env::var_os("PATH")
        .into_iter()
        .flat_map(|paths| std::env::split_paths(&paths).collect::<Vec<_>>())
        .map(|directory| directory.join("git"))
        .find(|candidate| candidate.is_file())
        .expect("git on PATH")
}

#[test]
fn doctor_reports_the_live_root_path() {
    let harness = CliHarness::new();
    let output = harness.run(
        &["doctor", "-o", "json"],
        r#"{"open":true,"rootPath":"/work/project"}"#,
    );
    let report: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["shell"]["ok"], true);
    assert_eq!(report["shell"]["root"], "/work/project");
}
