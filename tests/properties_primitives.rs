use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::thread;
use std::time::{Duration, Instant};

struct Server {
    child: Child,
    input: Option<ChildStdin>,
    output: BufReader<ChildStdout>,
    serial: u32,
}

impl Server {
    fn start(root: &Path) -> Self {
        fs::create_dir_all(root.join("bin")).unwrap();
        let mut child = Command::new(env!("CARGO_BIN_EXE_fileblade"))
            .args(["serve", "--no-recover"])
            .env("HOME", root)
            .env("XDG_STATE_HOME", root.join("state"))
            .env("XDG_CONFIG_HOME", root.join("config"))
            .env("CAPTURE", root.join("capture"))
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", root.join("bin").display()),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        let input = child.stdin.take();
        let output = BufReader::new(child.stdout.take().unwrap());
        let mut server = Self {
            child,
            input,
            output,
            serial: 0,
        };
        server.send(json!({"v":1,"type":"hello"}));
        server.read();
        server
    }

    fn send(&mut self, frame: Value) {
        writeln!(self.input.as_mut().unwrap(), "{frame}").unwrap();
    }

    fn read(&mut self) -> Value {
        let mut line = String::new();
        self.output.read_line(&mut line).unwrap();
        serde_json::from_str(&line).unwrap()
    }

    fn request(&mut self, command: &str, arguments: Value, input: Option<&str>) -> Value {
        self.serial += 1;
        let id = format!("properties-{}", self.serial);
        let mut frame = json!({"v":1,"type":"request","id":id,"generation":1,"command":command,"arguments":arguments});
        if let Some(text) = input {
            frame["input"] = json!(text);
        }
        self.send(frame);
        loop {
            let response = self.read();
            if response["id"] == id && response["type"] != "progress" {
                return response;
            }
        }
    }
}

impl Drop for Server {
    fn drop(&mut self) {
        drop(self.input.take());
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            if self.child.try_wait().unwrap().is_some() {
                return;
            }
            thread::sleep(Duration::from_millis(20));
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn tool(root: &Path, name: &str, body: &str) {
    let path = root.join("bin").join(name);
    fs::create_dir_all(root.join("bin")).unwrap();
    fs::write(&path, format!("#!/bin/sh\n{body}\n")).unwrap();
    fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
}

fn wait_for(path: &Path) -> String {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(text) = fs::read_to_string(path)
            && !text.is_empty()
        {
            return text;
        }
        assert!(
            Instant::now() < deadline,
            "{} was never written",
            path.display()
        );
        thread::sleep(Duration::from_millis(20));
    }
}

fn payload(response: &Value) -> &Value {
    if response.get("payload").is_some() {
        &response["payload"]
    } else {
        response
    }
}

#[test]
fn copied_code_reaches_the_clipboard_as_private_text_not_arguments() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    tool(
        root,
        "wl-copy",
        "printf '%s\\n' \"$*\" > \"$CAPTURE.args\"\n/bin/cat > \"$CAPTURE\"\nexec /bin/sleep 600",
    );
    let mut server = Server::start(root);
    let code = "SELECT *\n  FROM sales -- 'quoted' $HOME";
    let copied = server.request("clipboard-plain", json!([]), Some(code));
    assert_eq!(payload(&copied)["ok"], true, "{copied}");
    assert_eq!(payload(&copied)["bytes"], code.len());
    assert_eq!(wait_for(&root.join("capture")), code);
    assert_eq!(
        wait_for(&root.join("capture.args")).trim(),
        "--foreground --type text/plain;charset=utf-8"
    );

    let empty = server.request("clipboard-plain", json!([]), None);
    assert_eq!(payload(&empty)["ok"], false, "{empty}");
    assert!(
        payload(&empty)["error"]
            .as_str()
            .unwrap_or("")
            .contains("no text")
    );
    let oversized = "x".repeat(64 * 1024 + 1);
    let refused = server.request("clipboard-plain", json!([]), Some(&oversized));
    assert_eq!(refused["ok"], false, "{refused}");
}

#[test]
fn links_open_through_the_desktop_handler_and_only_for_web_addresses() {
    let temporary = tempfile::tempdir().unwrap();
    let root = temporary.path();
    tool(
        root,
        "gio",
        "printf '%s\\n' \"$@\" > \"$CAPTURE.tmp\"\n/bin/mv \"$CAPTURE.tmp\" \"$CAPTURE\"",
    );
    let mut server = Server::start(root);
    let link = "https://app.fabric.microsoft.com/groups/abc?experience=fabric-developer";
    let opened = server.request("launch", json!(["--path", link, "--mode", "url"]), None);
    assert_eq!(payload(&opened)["ok"], true, "{opened}");
    assert_eq!(payload(&opened)["mode"], "url");
    assert_eq!(
        wait_for(&root.join("capture")),
        format!("open\n--\n{link}\n")
    );

    for refused in [
        "javascript:alert(1)",
        "file:///etc/passwd",
        "/etc/passwd",
        "https://exa mple.com",
        "https://example.com/\nsecond",
        "https://",
    ] {
        fs::remove_file(root.join("capture")).ok();
        let response = server.request("launch", json!(["--path", refused, "--mode", "url"]), None);
        assert_eq!(payload(&response)["ok"], false, "{refused}: {response}");
        thread::sleep(Duration::from_millis(100));
        assert!(
            !root.join("capture").exists(),
            "{refused} reached the desktop handler"
        );
    }
    let long = format!("https://example.com/{}", "a".repeat(2048));
    let response = server.request("launch", json!(["--path", long, "--mode", "url"]), None);
    assert_eq!(payload(&response)["ok"], false, "{response}");
}
