use serde_json::{Value, json};
use std::fs;
use std::io::{BufRead, BufReader};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};

struct Desktop {
    root: tempfile::TempDir,
    process: Child,
    client: Value,
    file: PathBuf,
}

impl Desktop {
    fn new(comm: &str) -> Self {
        let root = tempfile::tempdir().unwrap();
        for directory in [
            "bin",
            "data/applications",
            "data/icons",
            "config",
            "runtime",
        ] {
            fs::create_dir_all(root.path().join(directory)).unwrap();
        }
        let program = r#"#!/usr/bin/python3
import json, os, sys
from pathlib import Path
root = Path(os.environ['DESKTOP_FIXTURE'])
name = Path(sys.argv[0]).name
args = sys.argv[1:]
client = json.loads((root/'clients').read_text())[0]
if name == 'hyprctl':
    if args[:2] == ['-j', 'clients']:
        print((root/'clients').read_text())
    elif args[:2] == ['-j', 'monitors']:
        print(json.dumps([{'id':0,'x':0,'y':0,'width':1000,'height':800,'scale':1,'activeWorkspace':{'id':7}}]))
    elif args[:2] == ['-j', 'getprop']:
        print((root/'colors').read_text())
    elif args[0] == 'dispatch':
        with (root/'dispatches').open('a') as output: output.write(args[1] + '\n')
        print('ok')
    else: sys.exit(2)
elif name == 'herdr':
    key, rows = ('workspaces', [{'workspace_id':'w1','label':'fixture space','focused':True}]) if 'workspace' in args else ('panes', [{'workspace_id':'w1','pane_id':'w1:p1','focused':True}])
    print(json.dumps({'result':{key:rows}}))
elif name == 'tmux':
    print(f"{client['pid']}\t/dev/pts/10\t$1\t{client['pid']}")
else:
    print('{}')
"#;
        for name in ["hyprctl", "herdr", "tmux", "hunk"] {
            let path = root.path().join("bin").join(name);
            fs::write(&path, program).unwrap();
            fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        let mut process = Command::new("/usr/bin/python3")
            .args(["-c", "import ctypes,sys; ctypes.CDLL(None).prctl(15,sys.argv[1].encode(),0,0,0); print('ready',flush=True); sys.stdin.read()", comm])
            .env("HERDR_SOCKET_PATH", root.path().join("herdr.sock"))
            .env("HERDR_PANE_ID", "w1:stale").env("HERDR_WORKSPACE_ID", "w1")
            .stdin(Stdio::piped()).stdout(Stdio::piped()).spawn().unwrap();
        let mut ready = String::new();
        BufReader::new(process.stdout.take().unwrap())
            .read_line(&mut ready)
            .unwrap();
        assert_eq!(ready.trim(), "ready");
        let client = json!({"address":"0xabc","pid":process.id(),"class":"footclient",
            "title":"fixture space","mapped":true,"hidden":false,"at":[100,100],
            "size":[500,400],"monitor":0,"workspace":{"id":7}});
        fs::write(root.path().join("clients"), json!([client]).to_string()).unwrap();
        let file = root.path().join("quote' $(false) file.txt");
        fs::write(&file, "fixture").unwrap();
        Self {
            root,
            process,
            client,
            file,
        }
    }

    fn backend(&self, arguments: &[&str]) -> Value {
        let output = Command::new(env!("CARGO_BIN_EXE_fileblade"))
            .arg("_backend")
            .args(arguments)
            .env("HOME", self.root.path())
            .env("DESKTOP_FIXTURE", self.root.path())
            .env("XDG_CONFIG_HOME", self.root.path().join("config"))
            .env("XDG_DATA_HOME", self.root.path().join("data"))
            .env("XDG_STATE_HOME", self.root.path().join("state"))
            .env("XDG_RUNTIME_DIR", self.root.path().join("runtime"))
            .env(
                "PATH",
                format!("{}:/usr/bin:/bin", self.root.path().join("bin").display()),
            )
            .env_remove("FILEBLADE_HYPR_SOCKET")
            .env_remove("HYPRLAND_INSTANCE_SIGNATURE")
            .output()
            .unwrap();
        serde_json::from_slice(&output.stdout).unwrap_or_else(|_| panic!("{output:?}"))
    }

    fn context(&self, path: &Path) -> Value {
        self.backend(&[
            "drop-context",
            "--x",
            "200",
            "--y",
            "200",
            "--path",
            &fileblade::common::path_text(path),
        ])
    }

    fn run(&self, action: &str, placement: &str, target: &Value, path: &Path) -> Value {
        self.backend(&[
            "drop-run",
            "--action",
            action,
            "--placement",
            placement,
            "--target",
            &target.to_string(),
            "--path",
            &fileblade::common::path_text(path),
            "--dry-run",
        ])
    }
}

impl Drop for Desktop {
    fn drop(&mut self) {
        let _ = self.process.kill();
        let _ = self.process.wait();
    }
}

#[test]
fn review_tracks_selected_git_changes_and_terminal_opens_files_in_nvim() {
    let desktop = Desktop::new("bash");
    let review_enabled = |path: &Path| {
        desktop.context(path)["actions"]
            .as_array()
            .unwrap()
            .iter()
            .find(|row| row["id"] == "review")
            .unwrap()["enabled"]
            .as_bool()
            .unwrap()
    };
    assert!(!review_enabled(&desktop.file));
    let git = |args: &[&str]| {
        assert!(
            Command::new("git")
                .arg("-C")
                .arg(desktop.root.path())
                .args(args)
                .status()
                .unwrap()
                .success()
        );
    };
    git(&["init", "-q"]);
    assert!(review_enabled(&desktop.file));
    git(&["add", "--", desktop.file.to_str().unwrap()]);
    git(&[
        "-c",
        "user.name=FileBlade",
        "-c",
        "user.email=test@example.invalid",
        "commit",
        "-qm",
        "fixture",
    ]);
    assert!(!review_enabled(&desktop.file));
    assert!(review_enabled(desktop.root.path()));
    fs::write(&desktop.file, "changed").unwrap();
    assert!(review_enabled(&desktop.file));
    git(&["add", "--", desktop.file.to_str().unwrap()]);
    assert!(review_enabled(&desktop.file));
    let file = desktop.run("terminal", "", &json!({}), &desktop.file);
    assert_eq!(file["ok"], true, "{file}");
    let command = file["commands"][0].as_array().unwrap();
    assert_eq!(
        &command[command.len() - 4..],
        &[json!("-e"), json!("nvim"), json!("--"), json!(desktop.file)]
    );
    let folder = desktop.run("terminal", "", &json!({}), desktop.root.path());
    assert_eq!(folder["ok"], true, "{folder}");
    assert!(
        !folder["commands"][0]
            .as_array()
            .unwrap()
            .iter()
            .any(|arg| arg == "nvim")
    );
    git(&["reset", "--hard", "-q", "HEAD"]);
    let refused = desktop.run("review", "", &json!({}), &desktop.file);
    assert_eq!(refused["ok"], false, "{refused}");
    assert_eq!(refused["commands"], json!([]));
}

#[test]
fn wheel_discovery_and_execution_preserve_mux_destinations_and_focus() {
    for (comm, name, key) in [("herdr", "herdr", "h"), ("tmux: client", "tmux", "t")] {
        let desktop = Desktop::new(comm);
        assert!(
            Command::new("git")
                .args(["init", "-q"])
                .arg(desktop.root.path())
                .status()
                .unwrap()
                .success()
        );
        let context = desktop.context(&desktop.file);
        assert_eq!(context["ok"], true, "{context}");
        let target = &context["target"];
        let actions = context["actions"].as_array().unwrap();
        assert_eq!(
            actions
                .iter()
                .map(|row| row["label"].as_str().unwrap())
                .collect::<Vec<_>>(),
            [
                name,
                "Open in new window",
                "Open with",
                "Open in new terminal",
                "Review with hunk"
            ]
        );
        assert_eq!(actions[0]["label"], name, "{context}");
        if name == "herdr" {
            assert_eq!(target["terminal"]["pane_id"], "w1:p1");
        }
        assert_eq!(actions[0]["key"], key);
        assert_eq!(
            actions[0]["icon_mask"],
            if name == "herdr" { "dark" } else { "ink" }
        );
        assert_eq!(actions[0]["placements"].as_array().unwrap().len(), 4);
        assert!(
            actions
                .iter()
                .all(|row| row["id"] != "edit" && row["id"] != "copy-paths")
        );
        let folder = desktop.context(desktop.root.path());
        assert!(
            folder["actions"][0]["description"]
                .as_str()
                .unwrap()
                .contains("Open a shell")
        );
        for (placement, direction) in [("right", "-h"), ("down", "-v")] {
            let result = desktop.run("mux-open", placement, target, &desktop.file);
            assert_eq!(result["ok"], true, "{result}");
            let commands = result["commands"].as_array().unwrap();
            let split = commands[0].as_array().unwrap();
            assert!(split.iter().any(|part| part
                == if name == "herdr" {
                    placement
                } else {
                    direction
                }));
            assert!(split.iter().all(|part| part != "layout"));
            assert!(commands.last().unwrap().to_string().contains("0xabc"));
        }
        let pasted = desktop.backend(&[
            "drop-paste",
            "--x",
            "200",
            "--y",
            "200",
            "--path",
            desktop.file.to_str().unwrap(),
            "--dry-run",
        ]);
        assert_eq!(pasted["ok"], true, "{pasted}");
        assert!(pasted["commands"][0].to_string().contains("0xabc"));
        for action in [
            "edit",
            "term-view",
            "term-paste",
            "mux-shell",
            "mux-review",
            "mux-paste",
        ] {
            let refused = desktop.run(action, "pane", target, &desktop.file);
            assert_eq!(refused["ok"], false, "{refused}");
            assert_eq!(refused["commands"], json!([]));
        }
    }
}

#[test]
fn nvim_actions_retain_the_editor_identity_and_finish_by_focusing_its_window() {
    let desktop = Desktop::new("nvim");
    fs::write(
        desktop
            .root
            .path()
            .join(format!("runtime/nvim.{}.0", desktop.process.id())),
        "",
    )
    .unwrap();
    let context = desktop.context(&desktop.file);
    let action = context["actions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == "nvim-open")
        .unwrap();
    assert_eq!(action["desktop_id"], "nvim.desktop");
    let opened = desktop.run("nvim-open", "buffer", &context["target"], &desktop.file);
    assert_eq!(opened["ok"], true, "{opened}");
    assert_eq!(opened["commands"][0][0], "nvim");
    assert_eq!(opened["commands"][1][0], "hyprctl");
    assert!(opened["commands"][1].to_string().contains("0xabc"));
    let excluded = desktop.backend(&[
        "drop-context",
        "--x",
        "200",
        "--y",
        "200",
        "--path",
        desktop.file.to_str().unwrap(),
        "--blade-title",
        "fixture space",
    ]);
    assert_eq!(excluded["target"]["kind"], "desktop", "{excluded}");
}

#[test]
fn configured_terminal_commands_and_pasted_paths_round_trip_native_bytes() {
    use std::os::unix::ffi::OsStrExt;
    let desktop = Desktop::new("tmux: client");
    let path = desktop
        .root
        .path()
        .join(std::ffi::OsStr::from_bytes(b"odd-\xff'$(false)\n.txt"));
    fs::write(&path, "fixture").unwrap();
    let config = desktop.root.path().join("config/omarchy/fileblade");
    fs::create_dir_all(&config).unwrap();
    let literal = "$(false); 'quoted' \\ backslash\n\n";
    fs::write(config.join("settings.json"), json!({"version":1,"dropWheel":{"version":1,
        "customActions":[{"id":"custom:bytes","label":"Bytes","runMode":"terminal",
            "command":["/usr/bin/printf","%s\\0","{paths}","file:///tmp/example%20name","",literal]}]}}).to_string()).unwrap();
    fs::set_permissions(
        config.join("settings.json"),
        fs::Permissions::from_mode(0o600),
    )
    .unwrap();
    let result = desktop.run("configured", "[\"custom:bytes\"]", &json!({}), &path);
    assert_eq!(result["ok"], true, "{result}");
    let command = result["commands"][0].as_array().unwrap();
    let start = command.iter().position(|arg| arg == "exec-hex").unwrap();
    assert_eq!(command[start - 1], env!("CARGO_BIN_EXE_fileblade"));
    let output = Command::new(env!("CARGO_BIN_EXE_fileblade"))
        .args(command[start..].iter().map(|arg| arg.as_str().unwrap()))
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    let expected = [
        path.as_os_str().as_bytes(),
        b"file:///tmp/example%20name",
        b"",
        literal.as_bytes(),
    ]
    .into_iter()
    .flat_map(|bytes| bytes.iter().copied().chain([0]))
    .collect::<Vec<_>>();
    assert_eq!(output.stdout, expected);
    let pasted = desktop.backend(&[
        "drop-paste",
        "--x",
        "200",
        "--y",
        "200",
        "--path",
        &fileblade::common::path_text(&path),
        "--dry-run",
    ]);
    assert_eq!(pasted["ok"], true, "{pasted}");
    let output = Command::new("/bin/bash")
        .args([
            "-c",
            &format!("printf '%s' {}", pasted["text"].as_str().unwrap()),
        ])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert_eq!(output.stdout, path.as_os_str().as_bytes());
}

#[test]
fn border_dimming_formats_gradients_and_refuses_code_injection() {
    for (color, rendered) in [
        ("ffabcdef 0deg", "rgba(abcdefff)"),
        (
            "eeabcdef ff112233 45deg",
            "-1 rgba(abcdefee) rgba(112233ff) 45deg",
        ),
        ("\"}; os.execute('bad')", ""),
        ("ffabcdef nan deg", ""),
    ] {
        let desktop = Desktop::new("fixture");
        fs::write(
            desktop.root.path().join("colors"),
            json!({"active_border_color":color,"inactive_border_color":color}).to_string(),
        )
        .unwrap();
        let result = desktop.backend(&["dim-windows", "--state", "on"]);
        assert_eq!(result["ok"], !rendered.is_empty(), "{result}");
        let dispatches =
            fs::read_to_string(desktop.root.path().join("dispatches")).unwrap_or_default();
        if rendered.is_empty() {
            assert!(dispatches.is_empty());
        } else {
            assert!(dispatches.contains(rendered), "{dispatches}");
        }
    }
}

#[test]
fn shared_terminal_windows_require_an_unambiguous_title_and_mapped_sibling() {
    let mut desktop = Desktop::new("herdr");
    let mut sibling = desktop.client.clone();
    sibling["address"] = json!("0xdef");
    sibling["at"] = json!([700, 100]);
    for title in ["fixture space", "host: fixture space", "host: fixture", ""] {
        desktop.client["title"] = json!(title);
        fs::write(
            desktop.root.path().join("clients"),
            json!([desktop.client, sibling]).to_string(),
        )
        .unwrap();
        let context = desktop.context(&desktop.file);
        let resolved = title.ends_with("fixture space");
        assert_eq!(
            context["actions"]
                .as_array()
                .unwrap()
                .iter()
                .any(|row| row["id"] == "mux-open"),
            resolved,
            "{context}"
        );
        if !resolved {
            assert_eq!(context["target"]["terminal"]["ambiguous"], true);
            assert!(
                context["actions"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|row| row["id"] == "terminal")
            );
        }
    }
    sibling["mapped"] = json!(false);
    fs::write(
        desktop.root.path().join("clients"),
        json!([desktop.client, sibling]).to_string(),
    )
    .unwrap();
    assert_eq!(
        desktop.context(&desktop.file)["actions"][0]["id"],
        "mux-open"
    );
}

#[test]
fn desktop_icons_and_application_arguments_remain_confined_and_byte_exact() {
    use std::os::unix::ffi::OsStrExt;
    let mut desktop = Desktop::new("fixture");
    desktop.client["class"] = json!("fixture-viewer");
    fs::write(
        desktop.root.path().join("clients"),
        json!([desktop.client]).to_string(),
    )
    .unwrap();
    for (name, bytes, accepted) in [
        (
            "data/icons/logo space.png",
            b"\x89PNG\r\n\x1a\nstub".as_slice(),
            true,
        ),
        ("outside.png", b"\x89PNG\r\n\x1a\nstub", false),
        ("data/icons/vector.svg", b"<svg/>", false),
        ("data/icons/disguised.png", b"<svg/>", false),
    ] {
        let icon = desktop.root.path().join(name);
        fs::write(&icon, bytes).unwrap();
        fs::write(desktop.root.path().join("data/applications/dev.zed.Zed.desktop"),
            format!("[Desktop Entry]\nType=Application\nName=Fixture\nStartupWMClass=fixture-viewer\nExec=viewer %F\nIcon={}\n", icon.display())).unwrap();
        let context = desktop.context(&desktop.file);
        assert_eq!(
            context["target"]["app"]["icon_mask"], "luminance",
            "{context}"
        );
        let source = context["target"]["app"]["icon_source"].as_str().unwrap();
        assert_eq!(!source.is_empty(), accepted, "{context}");
        if accepted {
            assert!(source.starts_with("file://") && source.contains("logo%20space.png"));
        }
        assert_eq!(context["actions"][0]["desktop_id"], "dev.zed.Zed.desktop");
        let opened = desktop.run("app-open", "", &context["target"], &desktop.file);
        assert_eq!(opened["ok"], true, "{opened}");
        assert!(
            opened["commands"]
                .as_array()
                .unwrap()
                .last()
                .unwrap()
                .to_string()
                .contains("0xabc")
        );
    }
    for name in [b"\xff.txt".as_slice(), "�.txt".as_bytes()] {
        let path = desktop.root.path().join(std::ffi::OsStr::from_bytes(name));
        fs::write(&path, "fixture").unwrap();
        let result = desktop.backend(&[
            "drop-run",
            "--action",
            "application",
            "--desktop-id",
            "obsidian.desktop",
            "--path",
            &fileblade::common::path_text(&path),
            "--dry-run",
        ]);
        assert_eq!(result["ok"], true, "{result}");
        let expected = if name[0] == 255 {
            "%FF.txt"
        } else {
            "%EF%BF%BD.txt"
        };
        assert!(
            result["commands"][0].to_string().contains(expected),
            "{result}"
        );
    }
}

#[test]
fn absent_windows_offer_fileblade_navigation_and_failed_queries_keep_desktop_actions() {
    let mut desktop = Desktop::new("fixture");
    for class in [
        "foot",
        "footclient",
        "kitty",
        "Alacritty",
        "com.mitchellh.ghostty",
        "org.omarchy.terminal",
    ] {
        desktop.client["class"] = json!(class);
        fs::write(
            desktop.root.path().join("clients"),
            json!([desktop.client]).to_string(),
        )
        .unwrap();
        let context = desktop.context(&desktop.file);
        assert_eq!(context["target"]["kind"], "terminal", "{context}");
        assert!(
            context["actions"]
                .as_array()
                .unwrap()
                .iter()
                .all(|row| row["id"] != "mux-open")
        );
    }
    desktop.client["at"] = json!([700, 100]);
    fs::write(
        desktop.root.path().join("clients"),
        json!([desktop.client]).to_string(),
    )
    .unwrap();
    let context = desktop.context(desktop.root.path());
    assert_eq!(context["target"]["kind"], "desktop");
    assert_eq!(context["actions"][0]["label"], "Open in FileBlade");
    let opened = desktop.run("open", "", &context["target"], desktop.root.path());
    assert_eq!(opened["ok"], true, "{opened}");
    assert!(
        opened["commands"][0]
            .as_array()
            .unwrap()
            .iter()
            .any(|arg| arg == "navigate")
    );
    fs::write(desktop.root.path().join("clients"), "broken").unwrap();
    let context = desktop.context(&desktop.file);
    assert_eq!(context["ok"], true, "{context}");
    assert_eq!(context["target"]["kind"], "desktop");
    assert!(context["target_warning"].as_str().is_some());
    assert!(!context["actions"].as_array().unwrap().is_empty());
}
