#[path = "common/plugin_environment.rs"]
mod plugin_environment;
use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};

struct Fixture {
    root: tempfile::TempDir,
}

#[test]
fn pruning_reports_a_full_scan_budget_at_a_module_boundary() {
    let fixture = Fixture::new();
    let base = fixture.root.path().join("data/fileblade/bin");
    fs::create_dir_all(&base).unwrap();
    fs::set_permissions(&base, fs::Permissions::from_mode(0o700)).unwrap();
    for module_index in 0..21 {
        let module = format!("module-{module_index}");
        let directory = base.join(&module);
        fs::create_dir(&directory).unwrap();
        fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
        let manifest = json!({"schemaVersion":1,"module":module,"id":"item",
            "name":"item","kind":"hook","scope":"user","detail":"",
            "path":"","realpath":"","deletedAt":"2099-01-01T00:00:00Z",
            "deletedAtEpoch":4070908800_i64,"items":[]})
        .to_string();
        for entry_index in 0..500 {
            let entry = directory.join(format!("entry-{entry_index}"));
            fs::create_dir(&entry).unwrap();
            fs::set_permissions(&entry, fs::Permissions::from_mode(0o700)).unwrap();
            let path = entry.join("manifest.json");
            fs::write(&path, &manifest).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o600)).unwrap();
        }
    }
    let result = fixture.run(&["trash-prune".into(), "--days".into(), "7".into()]);
    assert_eq!(result["satellites"]["examined"], 10_000, "{result}");
    assert_eq!(result["satellites"]["truncated"], true, "{result}");
    assert_eq!(result["satellites"]["completed"], 0, "{result}");
}
impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("bin")).unwrap();
        plugin_environment::install(root.path(), "test.recovery");
        fs::write(root.path().join("manifest.json"), json!({"id":"test.recovery", "extensions":{"data-goblin.fileblade/helper":[{
            "id":"inventory", "entry":"bin/helper", "read":[], "write":["prepare-remove","remove-prepared","restore","discard"], "timeoutMs":500
        }]}}).to_string()).unwrap();
        let script = root.path().join("bin/helper");
        fs::write(&script, r##"#!/usr/bin/python3
import json, os, sys, time
from pathlib import Path
root = Path.cwd()
source = root / 'source'
mode = (root / 'mode').read_text() if (root / 'mode').exists() else ''
command = sys.argv[1]
if command == 'prepare-remove':
    payload = {'body': source.read_text(), 'source':str(source)}
    print(json.dumps({'ok':True,'schemaVersion':1,'payload':payload,'recordId':sys.argv[sys.argv.index('--transaction-id')+1]}))
elif command == 'remove-prepared':
    payload = json.load(sys.stdin)
    manifests = list((Path(os.environ['XDG_DATA_HOME'])/'fileblade/bin/hooks').glob('*/manifest.json'))
    assert len(manifests) == 1
    saved = json.loads(manifests[0].read_text())
    assert saved['payload'] == payload and saved['restoreHelper']['provider'] == 'test.recovery'
    assert manifests[0].stat().st_mode & 0o777 == 0o600
    assert manifests[0].parent.stat().st_mode & 0o777 == 0o700
    if mode == 'before': os._exit(8)
    source.unlink()
    if mode == 'after': os._exit(9)
    if mode == 'sleep':
        (root/'started').touch()
        time.sleep(10)
    print('{"ok":true,"schemaVersion":1}')
elif command == 'discard':
    if '--payload-stdin' in sys.argv: json.load(sys.stdin)
    (root/'discarded').touch()
    print('{"ok":true,"schemaVersion":1}')
else:
    payload = json.load(sys.stdin)
    if source.exists() and source.read_text() != payload['body']:
        print('{"ok":false,"schemaVersion":1,"message":"source changed"}')
        sys.exit(1)
    source.write_text(payload['body'])
    if mode == 'restore-after': os._exit(10)
    print('{"ok":true,"schemaVersion":1}')
"##).unwrap();
        fs::set_permissions(script, fs::Permissions::from_mode(0o700)).unwrap();
        fs::write(root.path().join("source"), "private fixture payload").unwrap();
        Self { root }
    }
    fn route(&self) -> String {
        json!({"provider":"test.recovery", "directory":self.root.path(), "helper":"inventory"})
            .to_string()
    }
    fn args(&self, item: Value) -> Vec<String> {
        vec![
            "bin-remove".into(),
            "--module".into(),
            "hooks".into(),
            "--item".into(),
            item.to_string(),
            "--helper-route".into(),
            self.route(),
        ]
    }
    fn command(&self) -> Command {
        let mut command = Command::new(
            std::env::var_os("FILEBLADE_BINARY")
                .unwrap_or_else(|| env!("CARGO_BIN_EXE_fileblade").into()),
        );
        plugin_environment::configure(&mut command, self.root.path());
        command
            .env_remove("FILEBLADE_APP_ROOT")
            .env("XDG_DATA_HOME", self.root.path().join("data"))
            .env("XDG_STATE_HOME", self.root.path().join("state"));
        if self.root.path().join("python").is_dir() {
            command.env("FILEBLADE_APP_ROOT", self.root.path());
        }
        command
    }
    fn run(&self, args: &[String]) -> Value {
        let output = self.command().arg("_backend").args(args).output().unwrap();
        serde_json::from_slice(
            output
                .stdout
                .split(|byte| *byte == b'\n')
                .rfind(|line| !line.is_empty())
                .unwrap_or_default(),
        )
        .unwrap_or_else(|_| panic!("{}", String::from_utf8_lossy(&output.stderr)))
    }
    fn restore(&self, id: &str) -> Value {
        self.run(&[
            "bin-restore".into(),
            "--module".into(),
            "hooks".into(),
            "--id".into(),
            id.into(),
        ])
    }
    fn entries(&self) -> Vec<Value> {
        self.run(&["bin-list".into(), "--module".into(), "hooks".into()])["items"]
            .as_array()
            .unwrap()
            .clone()
    }
    fn historical_record(
        &self,
        module: &str,
        provider: &str,
        helper: &str,
    ) -> (String, std::path::PathBuf, Value) {
        let helpers = self.root.path().join("python/bin");
        fs::create_dir_all(&helpers).unwrap();
        for module in ["hooks", "mcp"] {
            let target = helpers.join(format!("agent-{module}ctl"));
            fs::copy(self.root.path().join("bin/helper"), &target).unwrap();
            fs::set_permissions(target, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let payload =
            json!({"body":"private fixture payload", "source":self.root.path().join("source")});
        let stored = self.run(&[
            "bin-put".into(),
            "--module".into(),
            module.into(),
            "--item".into(),
            json!({"id":"historical", "payload":payload}).to_string(),
        ]);
        assert_eq!(stored["ok"], true, "{stored}");
        let id = stored["entry"].as_str().unwrap().to_owned();
        let path = self
            .root
            .path()
            .join("data/fileblade/bin")
            .join(module)
            .join(id.strip_prefix("bin:").unwrap())
            .join("manifest.json");
        let mut manifest: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        manifest["restoreHelper"] =
            json!({"provider":provider,"helper":helper,"directory":"/retired/checkout"});
        manifest["helperRecordId"] = json!("0123456789abcdef0123456789abcdef");
        fs::write(&path, serde_json::to_vec(&manifest).unwrap()).unwrap();
        fs::remove_file(self.root.path().join("source")).unwrap();
        (id, path, manifest)
    }

    fn restore_route(&self, module: &str, id: &str, provider: &str, helper: &str) -> Value {
        self.run(&[
            "bin-restore".into(),
            "--module".into(),
            module.into(),
            "--id".into(),
            id.into(),
            "--helper-route".into(),
            json!({"provider":provider,"helper":helper,"directory":""}).to_string(),
        ])
    }

    fn mode(&self, mode: &str) {
        fs::write(self.root.path().join("mode"), mode).unwrap();
    }
}

#[test]
fn complete_record_precedes_removal_and_restores_without_a_view() {
    let f = Fixture::new();
    let removed = f.run(&f.args(json!({"id":"selected","name":"Selected","position":2})));
    assert_eq!(removed["ok"], true, "{removed}");
    assert!(removed["payload"].is_null());
    assert!(!f.root.path().join("source").exists());
    let rows = f.run(&["trash-list".into()]);
    assert_eq!(rows["entries"][0]["requires_module_restore"], false);
    assert_eq!(f.restore(removed["entry"].as_str().unwrap())["ok"], true);
    assert_eq!(
        fs::read_to_string(f.root.path().join("source")).unwrap(),
        "private fixture payload"
    );
    assert!(f.entries().is_empty());
    let audit =
        fs::read_to_string(f.root.path().join("state/omarchy/fileblade/audit.jsonl")).unwrap();
    assert!(!audit.contains("private fixture payload"));
}

#[test]
fn crashes_before_and_after_removal_keep_recovery_and_retry_is_idempotent() {
    for mode in ["before", "after", "sleep"] {
        let f = Fixture::new();
        f.mode(mode);
        let removed = f.run(&f.args(json!({"id":"selected"})));
        assert_eq!(removed["ok"], false, "{mode}: {removed}");
        assert_eq!(f.entries().len(), 1);
        assert_eq!(f.root.path().join("source").exists(), mode == "before");
        f.mode("restore-after");
        let id = removed["entry"].as_str().unwrap();
        assert_eq!(f.restore(id)["ok"], false);
        assert_eq!(f.entries().len(), 1);
        f.mode("");
        assert_eq!(f.restore(id)["ok"], true);
        assert!(f.entries().is_empty());
    }
}

#[test]
fn wrapper_and_utf8_escape_bounds_are_checked_before_removal() {
    for body in ["x".repeat(65_300), "\"".repeat(33_000), "🦀".repeat(17_000)] {
        let f = Fixture::new();
        fs::write(f.root.path().join("source"), &body).unwrap();
        let removed = f.run(&f.args(json!({"id":"selected", "detail":"metadata".repeat(40)})));
        assert_eq!(removed["ok"], false, "{removed}");
        assert_eq!(
            fs::read_to_string(f.root.path().join("source")).unwrap(),
            body
        );
        assert!(f.entries().is_empty());
    }
}

#[test]
fn restore_conflicts_or_unavailable_providers_keep_records() {
    let f = Fixture::new();
    let removed = f.run(&f.args(json!({"id":"selected"})));
    let id = removed["entry"].as_str().unwrap();
    fs::write(f.root.path().join("source"), "new contents").unwrap();
    assert_eq!(f.restore(id)["ok"], false);
    fs::remove_file(f.root.path().join("source")).unwrap();
    fs::rename(
        f.root.path().join("manifest.json"),
        f.root.path().join("disabled-manifest.json"),
    )
    .unwrap();
    assert_eq!(f.restore(id)["ok"], false);
    assert_eq!(f.entries().len(), 1);
}

#[test]
fn a_live_transaction_cannot_be_purged_or_pruned_and_cancellation_keeps_recovery() {
    let f = Fixture::new();
    f.mode("sleep");
    let mut server = f
        .command()
        .args(["serve", "--no-recover"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = server.stdin.take().unwrap();
    let mut output = BufReader::new(server.stdout.take().unwrap());
    writeln!(input, "{}", json!({"v":1,"type":"hello"})).unwrap();
    let mut hello = String::new();
    output.read_line(&mut hello).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&hello).unwrap()["type"],
        "hello"
    );
    let args = f.args(json!({"id":"selected"}));
    writeln!(input, "{}", json!({"v":1,"type":"request","id":"remove","generation":1,"command":args[0],"arguments":args[1..]})).unwrap();
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    while !f.root.path().join("started").exists() {
        assert!(std::time::Instant::now() < deadline);
        std::thread::sleep(std::time::Duration::from_millis(5));
    }
    let entries = f.entries();
    let id = entries[0]["id"].as_str().unwrap();
    for args in [
        vec!["bin-purge", "--module", "hooks", "--id", id],
        vec!["trash-empty"],
        vec!["trash-prune", "--days", "1"],
    ] {
        let result = f.run(&args.iter().map(|arg| arg.to_string()).collect::<Vec<_>>());
        assert_eq!(result["ok"], false, "{result}");
    }
    writeln!(
        input,
        "{}",
        json!({"v":1,"type":"cancel","id":"remove","generation":1})
    )
    .unwrap();
    let mut line = String::new();
    loop {
        line.clear();
        assert_ne!(output.read_line(&mut line).unwrap(), 0);
        let frame: Value = serde_json::from_str(&line).unwrap();
        if frame["type"] == "response" {
            break;
        }
    }
    drop(input);
    server.wait().unwrap();
    assert_eq!(f.entries().len(), 1);
    f.mode("");
    assert_eq!(f.restore(id)["ok"], true);
}

#[test]
fn historical_mcp_aliases_reach_the_in_process_core_and_keep_saved_evidence() {
    for provider in [
        "data-goblin.fileblade-mcp".to_string(),
        "kurt.agent-mcp".to_string(),
    ] {
        let f = Fixture::new();
        let (id, path, saved) = f.historical_record("mcp", &provider, "inventory");
        f.mode("restore-after");
        let refused = f.restore_route("mcp", &id, "fileblade.core.mcp", "inventory");
        assert_eq!(refused["ok"], false, "{refused}");
        assert!(
            !f.root.path().join("source").exists(),
            "the retired mcp helper must not run"
        );
        let retained: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(retained, saved);
    }
}

#[test]
fn historical_hooks_aliases_reach_the_in_process_core_and_keep_saved_evidence() {
    for provider in [
        "data-goblin.fileblade-hooks".to_string(),
        "kurt.agent-hooks".to_string(),
    ] {
        let f = Fixture::new();
        let (id, path, saved) = f.historical_record("hooks", &provider, "inventory");
        f.mode("restore-after");
        let refused = f.restore_route("hooks", &id, "fileblade.core.hooks", "inventory");
        assert_eq!(refused["ok"], false, "{refused}");
        assert!(
            !f.root.path().join("source").exists(),
            "the retired hooks helper must not run"
        );
        let retained: Value = serde_json::from_slice(&fs::read(&path).unwrap()).unwrap();
        assert_eq!(retained, saved);
    }
}

#[test]
fn unrelated_or_unknown_recovery_routes_refuse_before_changing_saved_evidence() {
    for (saved_provider, saved_helper, requested_provider, requested_helper) in [
        (
            "data-goblin.fileblade-hooks",
            "inventory",
            "fileblade.core.mcp",
            "inventory",
        ),
        (
            "data-goblin.fileblade-hooks",
            "inventory",
            "fileblade.core.hooks",
            "other",
        ),
        ("custom.original", "inventory", "custom.other", "inventory"),
        ("custom.original", "original", "custom.original", "other"),
        (
            "custom.original",
            "inventory",
            "fileblade.core.hooks",
            "inventory",
        ),
    ] {
        let f = Fixture::new();
        let (id, path, _) = f.historical_record("hooks", saved_provider, saved_helper);
        let before = fs::read(&path).unwrap();
        let refused = f.restore_route("hooks", &id, requested_provider, requested_helper);
        assert_eq!(refused["ok"], false, "{refused}");
        assert!(refused.to_string().contains("does not match"), "{refused}");
        assert_eq!(fs::read(&path).unwrap(), before);
        assert!(!f.root.path().join("source").exists());
        assert!(!f.root.path().join("discarded").exists());
    }
}
