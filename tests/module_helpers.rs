#[path = "common/plugin_environment.rs"]
mod plugin_environment;
use fileblade::{
    AppError,
    module_helpers::{self, Request},
};
use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::ffi::OsStrExt;
use std::os::unix::fs::PermissionsExt;
use std::process::{Command, Stdio};
use std::sync::atomic::AtomicBool;

struct Fixture {
    root: tempfile::TempDir,
}

impl Fixture {
    fn new(body: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("bin")).unwrap();
        plugin_environment::install(root.path(), "test.inventory");
        let fixture = Self { root };
        fixture.script(body);
        fixture.manifest(json!({"id":"inventory", "entry":"bin/helper", "read":["list"], "write":["apply"], "timeoutMs":1000}));
        fixture
    }
    fn script(&self, body: &str) {
        let path = self.root.path().join("bin/helper");
        fs::write(&path, format!("#!/usr/bin/env bash\nset -eu\n{body}\n")).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    fn manifest(&self, declaration: Value) {
        fs::write(self.root.path().join("manifest.json"), json!({
            "id":"test.inventory", "version":"0.1.0", "extensions":{"data-goblin.fileblade/helper":[declaration]}
        }).to_string()).unwrap();
    }
    fn request(
        &self,
        method: &str,
        arguments: &str,
        input: Option<&str>,
        write: bool,
    ) -> fileblade::AppResult<Value> {
        self.execute(&Request {
            provider: "test.inventory",
            directory: self.root.path().to_str().unwrap(),
            helper: "inventory",
            method,
            arguments,
            input,
            write,
        })
    }
    fn command(&self) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_fileblade"));
        plugin_environment::configure(&mut command, self.root.path());
        command.env("XDG_STATE_HOME", self.root.path().join("state"));
        command
    }
    fn execute(&self, request: &Request<'_>) -> fileblade::AppResult<Value> {
        let mut child = self
            .command()
            .args(["serve", "--no-recover"])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let mut input = child.stdin.take().unwrap();
        let mut output = BufReader::new(child.stdout.take().unwrap());
        writeln!(input, "{}", json!({"v":1,"type":"hello"})).unwrap();
        let mut line = String::new();
        output.read_line(&mut line).unwrap();
        let args = json!([
            "--provider",
            request.provider,
            "--plugin-dir",
            request.directory,
            "--helper",
            request.helper,
            "--method",
            request.method,
            "--arguments",
            request.arguments
        ]);
        let mut frame = json!({"v":1,"type":"request","id":"test","generation":1,"command":if request.write {"helper-write"} else {"helper-read"},"arguments":args});
        if let Some(text) = request.input {
            frame["input"] = json!(text)
        }
        writeln!(input, "{frame}").unwrap();
        line.clear();
        output.read_line(&mut line).unwrap();
        drop(input);
        assert!(child.wait().unwrap().success());
        let response: Value = serde_json::from_str(&line).unwrap();
        if response["ok"] == true {
            Ok(response["payload"].clone())
        } else {
            Err(AppError::invalid(
                response["error"].as_str().unwrap_or("helper refused"),
            ))
        }
    }
}

#[test]
fn only_declared_methods_run_in_the_correct_request_kind() {
    let fixture = Fixture::new("printf '%s\\n' '{\"ok\":true,\"items\":[]}'");
    assert_eq!(
        fixture.request("list", "[]", None, false).unwrap()["items"],
        json!([])
    );
    assert_eq!(
        fixture.request("apply", "[]", None, true).unwrap()["ok"],
        true
    );
    for (method, write) in [
        ("apply", false),
        ("list", true),
        ("--version", false),
        ("other", false),
    ] {
        assert!(fixture.request(method, "[]", None, write).is_err());
    }
    fixture
        .manifest(json!({"id":"inventory","entry":"bin/helper","read":["list"],"write":["list"]}));
    assert!(fixture.request("list", "[]", None, false).is_err());
}

#[test]
fn helper_entry_cannot_select_an_external_executable_or_symlink() {
    let fixture = Fixture::new("printf '%s\\n' '{}'");
    for entry in [
        "/usr/bin/true",
        "../outside",
        "bin/../../outside",
        "bin/helper\n",
    ] {
        fixture.manifest(json!({"id":"inventory", "entry":entry, "read":["list"]}));
        assert!(
            fixture.request("list", "[]", None, false).is_err(),
            "{entry}"
        );
    }
    std::os::unix::fs::symlink("/usr/bin/true", fixture.root.path().join("bin/link")).unwrap();
    fixture.manifest(json!({"id":"inventory", "entry":"bin/link", "read":["list"]}));
    assert!(fixture.request("list", "[]", None, false).is_err());
}

#[test]
fn input_and_arguments_are_bounded_before_any_helper_side_effect() {
    let fixture = Fixture::new("touch ran\nprintf '%s\\n' '{}'");
    assert!(
        fixture
            .request("apply", "[]", Some(&"x".repeat(65537)), true)
            .is_err()
    );
    assert!(
        fixture
            .request("apply", &json!(vec!["x"; 129]).to_string(), None, true)
            .is_err()
    );
    assert!(
        fixture
            .request("apply", "[\"\\u0000\"]", None, true)
            .is_err()
    );
    assert!(
        fixture
            .request("apply", &json!(["x".repeat(65537)]).to_string(), None, true)
            .is_err()
    );
    assert!(fixture.request("apply", "[7]", None, true).is_err());
    assert!(!fixture.root.path().join("ran").exists());
}

#[test]
fn flood_malformed_and_premature_success_responses_are_refused() {
    let fixture = Fixture::new("head -c $((3 * 1024 * 1024)) /dev/zero | tr '\\0' 'x'");
    assert!(
        fixture
            .request("list", "[]", None, false)
            .unwrap_err()
            .to_string()
            .contains("2 MiB")
    );
    fixture.script("printf '%s\\n' 'not JSON'");
    assert!(fixture.request("list", "[]", None, false).is_err());
    fixture.script("printf '%s\\n' '[]'");
    assert!(fixture.request("list", "[]", None, false).is_err());
    fixture.script("printf '%s\\n' '{\"ok\":true}'\nexit 7");
    assert!(fixture.request("list", "[]", None, false).is_err());
    fixture.script("printf '%s\\n' '{\"ok\":false,\"message\":\"refused\"}'\nexit 1");
    assert_eq!(
        fixture.request("list", "[]", None, false).unwrap()["message"],
        "refused"
    );
}

#[test]
fn native_provider_paths_and_closed_private_input_work() {
    let fixture = Fixture::new(
        "text=$(cat)\nprintf '%s\\n' \"$@\" | jq -Rs --arg input \"$text\" '{ok:true, input:$input, args:(if . == \"\" then [] else rtrimstr(\"\\n\") | split(\"\\n\") end)}'",
    );
    let link = fixture
        .root
        .path()
        .join(std::ffi::OsStr::from_bytes(b"native-\xff"));
    std::os::unix::fs::symlink(fixture.root.path(), &link).unwrap();
    let result = fixture
        .execute(&Request {
            provider: "test.inventory",
            directory: &fileblade::common::path_text(&link),
            helper: "inventory",
            method: "apply",
            arguments: "[\"--project\",\"file:///project-%FF\"]",
            input: Some("private\nmessage"),
            write: true,
        })
        .unwrap();
    assert_eq!(result["input"], "private\nmessage");
    assert_eq!(
        result["args"],
        json!(["apply", "--project", "file:///project-%FF"])
    );
}

#[test]
fn deadlines_and_cancellation_use_the_shared_native_runner() {
    let fixture = Fixture::new("sleep 10");
    fixture.manifest(
        json!({"id":"inventory", "entry":"bin/helper", "read":["list"], "timeoutMs":100}),
    );
    assert!(
        fixture
            .request("list", "[]", None, false)
            .unwrap_err()
            .to_string()
            .contains("100 ms")
    );
    let result = module_helpers::run(
        &Request {
            provider: "test.inventory",
            directory: fixture.root.path().to_str().unwrap(),
            helper: "inventory",
            method: "list",
            arguments: "[]",
            input: None,
            write: false,
        },
        &AtomicBool::new(true),
    );
    assert!(matches!(result, Err(AppError::Cancelled)));
}

#[test]
fn resident_input_is_private_and_not_accepted_for_unrelated_commands() {
    let fixture = Fixture::new(
        "text=$(cat)\nprintf '%s\\n' \"$0\" \"$@\" | jq -Rs --arg input \"$text\" '{ok:true, input:$input, argv:(rtrimstr(\"\\n\") | split(\"\\n\"))}'",
    );
    let mut server = fixture
        .command()
        .args(["serve", "--no-recover"])
        .env("XDG_STATE_HOME", fixture.root.path().join("state"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut input = server.stdin.take().unwrap();
    let mut output = BufReader::new(server.stdout.take().unwrap());
    writeln!(input, "{}", json!({"v":1,"type":"hello"})).unwrap();
    let mut line = String::new();
    output.read_line(&mut line).unwrap();
    for (id, name, arguments) in [
        (
            "helper",
            "helper-write",
            json!([
                "--provider",
                "test.inventory",
                "--plugin-dir",
                fixture.root.path(),
                "--helper",
                "inventory",
                "--method",
                "apply"
            ]),
        ),
        ("unrelated", "agents", json!([])),
    ] {
        writeln!(input, "{}", json!({"v":1,"type":"request","id":id,"generation":1,"command":name,"arguments":arguments,"input":"a-private-test-secret"})).unwrap();
        line.clear();
        output.read_line(&mut line).unwrap();
        let response: Value = serde_json::from_str(&line).unwrap();
        if id == "helper" {
            assert_eq!(
                response["payload"]["input"], "a-private-test-secret",
                "{response}"
            );
            assert!(
                !response["payload"]["argv"]
                    .to_string()
                    .contains("a-private-test-secret")
            );
        } else {
            assert_eq!(response["ok"], false, "{response}");
        }
    }
    drop(input);
    assert!(server.wait().unwrap().success());
}

#[test]
fn disabled_or_unknown_activation_never_starts_a_helper() {
    let fixture = Fixture::new("touch ran\nprintf '%s\\n' '{}'");
    for state in [r#"[{"id":"test.inventory","enabled":false}]"#, "not json"] {
        fs::write(fixture.root.path().join("enabled.json"), state).unwrap();
        assert!(
            fixture
                .request("list", "[]", None, false)
                .unwrap_err()
                .to_string()
                .contains("explicitly enabled")
        );
        assert!(fixture.request("apply", "[]", None, true).is_err());
        assert!(!fixture.root.path().join("ran").exists());
    }
}
