use serde_json::{Value, json};
use std::fs;
use std::os::unix::fs::{PermissionsExt, symlink};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};
use tempfile::TempDir;

const ROLES: [&str; 5] = ["autostart", "bindings", "chooser", "folder", "reveal"];
const MIMEAPPS: &str = "[Added Associations]\ntext/plain=nvim.desktop;\n\n[Default Applications]\ninode/directory=nautilus.desktop\ntext/plain=nvim.desktop\n";
const PORTALS: &str = "[preferred]\ndefault=gtk\n";
const BINDINGS: &str = "o.bind(\"SUPER + Q\", \"Quit\", \"hl.dsp.exit()\")\n";

struct Home {
    dir: TempDir,
}

impl Home {
    fn new() -> Self {
        Self::with_prefix("roles-")
    }

    fn with_prefix(prefix: &str) -> Self {
        let dir = tempfile::Builder::new().prefix(prefix).tempdir().unwrap();
        let root = dir.path();
        let installation = root.join("data/fileblade/installation");
        fs::create_dir_all(installation.join("active")).unwrap();
        fs::create_dir_all(root.join(".local/bin")).unwrap();
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::create_dir_all(root.join("config")).unwrap();
        symlink(
            env!("CARGO_MANIFEST_DIR"),
            installation.join("active/runtime"),
        )
        .unwrap();
        fs::write(installation.join("launcher"), "#!/bin/sh\n").unwrap();
        symlink(
            installation.join("launcher"),
            root.join(".local/bin/fileblade"),
        )
        .unwrap();
        let hyprctl = root.join("bin/hyprctl");
        fs::write(
            &hyprctl,
            "#!/bin/sh\necho \"$@\" >> \"$HOME/hyprctl.log\"\n",
        )
        .unwrap();
        fs::set_permissions(&hyprctl, fs::Permissions::from_mode(0o700)).unwrap();
        Self { dir }
    }

    fn path(&self, relative: &str) -> PathBuf {
        self.dir.path().join(relative)
    }

    fn launcher(&self) -> String {
        self.path(".local/bin/fileblade").display().to_string()
    }

    fn command(&self) -> Command {
        let root = self.dir.path();
        let mut command = Command::new(env!("CARGO_BIN_EXE_fileblade"));
        command
            .env_clear()
            .env("HOME", root)
            .env("XDG_CONFIG_HOME", root.join("config"))
            .env("XDG_DATA_HOME", root.join("data"))
            .env("XDG_STATE_HOME", root.join("state"))
            .env("FILEBLADE_APP_ROOT", env!("CARGO_MANIFEST_DIR"))
            .env("PATH", root.join("bin"))
            .env("HYPRLAND_INSTANCE_SIGNATURE", "test")
            .env(
                "DBUS_SESSION_BUS_ADDRESS",
                format!("unix:path={}", root.join("nobus").display()),
            );
        command
    }

    fn roles(&self, arguments: &[&str]) -> (Value, i32) {
        let output = self
            .command()
            .args(["native", "roles"])
            .args(arguments)
            .output()
            .unwrap();
        let document: Value = serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
            panic!(
                "stdout is not json: {}{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            )
        });
        (document, output.status.code().unwrap())
    }

    fn receipt(&self) -> PathBuf {
        self.path("config/omarchy/fileblade/desktop-roles.json")
    }

    fn read(&self, relative: &str) -> Option<String> {
        fs::read_to_string(self.path(relative)).ok()
    }

    fn seed(&self) {
        fs::create_dir_all(self.path("config/xdg-desktop-portal")).unwrap();
        fs::create_dir_all(self.path("config/hypr")).unwrap();
        fs::write(self.path("config/mimeapps.list"), MIMEAPPS).unwrap();
        fs::write(self.path("config/xdg-desktop-portal/portals.conf"), PORTALS).unwrap();
        fs::write(self.path("config/hypr/bindings.lua"), BINDINGS).unwrap();
    }
}

fn already_off() -> Value {
    let roles: serde_json::Map<String, Value> = ROLES
        .iter()
        .map(|role| {
            (
                role.to_string(),
                json!({"status": "already_off", "conflict": "", "remaining_owned_entries": [], "error": ""}),
            )
        })
        .collect();
    json!({"schema": 1, "action": "roles_disable", "status": "complete", "error": "",
        "remaining_owned_entries": [], "roles": roles})
}

fn installer_filter() -> String {
    let script =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("packaging/install.sh"))
            .unwrap();
    let start = script
        .find("jq -e -s --arg operation \"$operation\" '")
        .unwrap();
    let body = &script[start + "jq -e -s --arg operation \"$operation\" '".len()..];
    body[..body.find("' <<< \"$result\"").unwrap()].to_string()
}

fn expected_files(home: &Home, role: &str) -> Vec<(String, String)> {
    let launcher = format!("\"{}\"", home.launcher());
    match role {
        "folder" => vec![
            ("data/applications/fileblade.desktop".into(), format!("[Desktop Entry]\nType=Application\nName=FileBlade\nIcon=fileblade\nExec=/usr/bin/env -- {launcher} native open %U\nMimeType=inode/directory;\nNoDisplay=true\nCategories=System;FileTools;\n")),
            ("config/mimeapps.list".into(), MIMEAPPS.replace("nautilus.desktop", "fileblade.desktop")),
        ],
        "reveal" => vec![(
            "data/dbus-1/services/org.freedesktop.FileManager1.service".into(),
            format!("[D-BUS Service]\nName=org.freedesktop.FileManager1\nExec={launcher} native filemanager1\n"),
        )],
        "chooser" => vec![
            ("data/xdg-desktop-portal/portals/fileblade.portal".into(), "[portal]\nDBusName=org.freedesktop.impl.portal.desktop.fileblade\nInterfaces=org.freedesktop.impl.portal.FileChooser\nUseIn=Hyprland\n".into()),
            ("data/dbus-1/services/org.freedesktop.impl.portal.desktop.fileblade.service".into(), format!("[D-BUS Service]\nName=org.freedesktop.impl.portal.desktop.fileblade\nExec={launcher} native portal\n")),
            ("config/xdg-desktop-portal/portals.conf".into(), format!("{PORTALS}org.freedesktop.impl.portal.FileChooser=fileblade\n")),
        ],
        "bindings" => vec![
            ("config/hypr/fileblade-bindings.lua".into(), fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("examples/fileblade-bindings.lua")).unwrap()),
            ("config/hypr/bindings.lua".into(), format!("{BINDINGS}dofile(os.getenv(\"HOME\") .. \"/.config/hypr/fileblade-bindings.lua\") -- fileblade desktop role\n")),
        ],
        "autostart" => vec![(
            "config/autostart/fileblade.desktop".into(),
            format!("[Desktop Entry]\nType=Application\nName=FileBlade\nExec=/usr/bin/env -- {launcher}\nX-GNOME-Autostart-enabled=true\n"),
        )],
        _ => unreachable!(),
    }
}

#[test]
fn desktop_roles_launch_from_a_home_with_reserved_characters() {
    let home = Home::with_prefix("roles space '\"\\$`%-");
    let recorder = home.path("data/fileblade/installation/launcher");
    fs::write(
        &recorder,
        "#!/bin/sh\nprintf '%s\\0' \"$0\" \"$@\" > \"$HOME/launched\"\n",
    )
    .unwrap();
    fs::set_permissions(recorder, fs::Permissions::from_mode(0o700)).unwrap();
    for (role, entry, arguments) in [
        ("autostart", "config/autostart/fileblade.desktop", vec![]),
        (
            "folder",
            "data/applications/fileblade.desktop",
            vec!["native", "open"],
        ),
        (
            "chooser",
            "org.freedesktop.impl.portal.desktop.fileblade",
            vec!["native", "portal"],
        ),
        (
            "reveal",
            "org.freedesktop.FileManager1",
            vec!["native", "filemanager1"],
        ),
    ] {
        let (result, code) = home.roles(&["enable", "--role", role, "--json"]);
        assert_eq!(code, 0, "{result}");
        let mut command = Command::new("dbus-run-session");
        command
            .arg("--")
            .env("HOME", home.dir.path())
            .env("XDG_DATA_HOME", home.path("data"));
        if entry.ends_with(".desktop") {
            command.arg("gio").arg("launch").arg(home.path(entry));
        } else {
            command.args([
                "gdbus",
                "call",
                "--session",
                "--dest",
                entry,
                "--object-path",
                "/",
                "--method",
                "org.freedesktop.DBus.Peer.Ping",
                "--timeout",
                "1",
            ]);
        }
        let output = command.output().unwrap();
        let deadline = Instant::now() + Duration::from_secs(2);
        while !home.path("launched").exists() && Instant::now() < deadline {
            std::thread::sleep(Duration::from_millis(10));
        }
        let launched = fs::read(home.path("launched")).unwrap_or_else(|error| {
            panic!(
                "{role}: {error}: {}",
                String::from_utf8_lossy(&output.stderr)
            )
        });
        let mut expected = vec![home.launcher()];
        expected.extend(arguments.into_iter().map(str::to_string));
        assert_eq!(
            launched,
            format!("{}\0", expected.join("\0")).as_bytes(),
            "{role}"
        );
        fs::remove_file(home.path("launched")).unwrap();
    }
}

#[test]
fn fresh_home_disable_all_matches_the_installer_contract() {
    let home = Home::new();
    let (document, code) = home.roles(&["disable", "--all", "--json"]);
    assert_eq!(code, 0);
    assert_eq!(document, already_off());
    assert!(!home.receipt().exists());
    if Path::new("/usr/bin/jq").exists() {
        let status = Command::new("/usr/bin/jq")
            .args([
                "-e",
                "-s",
                "--arg",
                "operation",
                "roles_disable",
                &installer_filter(),
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .and_then(|mut child| {
                use std::io::Write;
                child
                    .stdin
                    .take()
                    .unwrap()
                    .write_all(document.to_string().as_bytes())?;
                child.wait()
            })
            .unwrap();
        assert!(
            status.success(),
            "installer jq filter rejected the document"
        );
    }
    let (text_mode, code) = home.roles(&["status"]);
    assert_eq!(code, 0);
    assert_eq!(text_mode["action"], "roles_status");
    assert_eq!(text_mode["roles"]["folder"]["enabled"], false);
    assert_eq!(text_mode["roles"]["folder"]["launcher"], home.launcher());
    let help = home
        .command()
        .args(["native", "roles", "--help"])
        .output()
        .unwrap();
    assert!(help.status.success());
}

#[test]
fn every_role_writes_its_entries_and_disable_restores_the_prior_bytes() {
    let home = Home::new();
    home.seed();
    let seeded: Vec<(String, Option<String>)> = [
        "config/mimeapps.list",
        "config/xdg-desktop-portal/portals.conf",
        "config/hypr/bindings.lua",
    ]
    .iter()
    .map(|path| (path.to_string(), home.read(path)))
    .collect();
    for role in ROLES {
        let (document, code) = home.roles(&["enable", "--role", role, "--json"]);
        assert_eq!(code, 0, "{document}");
        assert_eq!(document["status"], "complete", "{document}");
        assert_eq!(document["roles"][role]["status"], "enabled", "{document}");
        assert_eq!(document["roles"][role]["error"], "", "{document}");
        for (path, content) in expected_files(&home, role) {
            assert_eq!(
                home.read(&path).as_deref(),
                Some(content.as_str()),
                "{role}: {path}"
            );
        }
        let mode = fs::metadata(home.receipt()).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
        let receipt: Value = serde_json::from_str(
            &home
                .read("config/omarchy/fileblade/desktop-roles.json")
                .unwrap(),
        )
        .unwrap();
        assert_eq!(receipt["schema"], 1);
        assert_eq!(receipt["roles"][role]["enabled"], true);
        assert_eq!(receipt["roles"][role]["intent"], Value::Null);
        assert_eq!(
            receipt["roles"][role]["entries"].as_array().unwrap().len(),
            expected_files(&home, role).len()
        );
        let (again, code) = home.roles(&["enable", "--role", role, "--json"]);
        assert_eq!(code, 0);
        assert_eq!(again["status"], "already_on", "{again}");
        let (status, _) = home.roles(&["status", "--json"]);
        assert_eq!(status["roles"][role]["enabled"], true);
    }
    assert_eq!(home.read("hyprctl.log").as_deref(), Some("reload\n"));
    for role in ROLES {
        let (document, code) = home.roles(&["disable", "--role", role, "--json"]);
        assert_eq!(code, 0, "{document}");
        assert_eq!(document["roles"][role]["status"], "restored", "{document}");
        assert_eq!(document["roles"][role]["error"], "", "{document}");
        for (path, _) in expected_files(&home, role) {
            if !seeded.iter().any(|(seed, _)| *seed == path) {
                assert!(!home.path(&path).exists(), "{role}: {path} should be gone");
            }
        }
    }
    for (path, content) in &seeded {
        assert_eq!(home.read(path), *content, "{path}");
    }
    assert_eq!(
        home.read("hyprctl.log").as_deref(),
        Some("reload\nreload\n")
    );
    let (document, code) = home.roles(&["disable", "--all", "--json"]);
    assert_eq!(code, 0);
    assert_eq!(document, already_off());
}

#[test]
fn a_newer_user_choice_is_preserved_and_reported() {
    let home = Home::new();
    home.seed();
    home.roles(&["enable", "--role", "folder", "--json"]);
    home.roles(&["enable", "--role", "autostart", "--json"]);
    let changed = MIMEAPPS.replace("nautilus.desktop", "thunar.desktop");
    fs::write(home.path("config/mimeapps.list"), &changed).unwrap();
    fs::write(
        home.path("config/autostart/fileblade.desktop"),
        "[Desktop Entry]\nType=Application\nName=Mine\nExec=other\n",
    )
    .unwrap();
    let (document, code) = home.roles(&["disable", "--all", "--json"]);
    assert_eq!(code, 0, "{document}");
    assert_eq!(document["status"], "complete");
    assert_eq!(document["roles"]["folder"]["status"], "preserved_newer");
    assert_eq!(document["roles"]["autostart"]["status"], "preserved_newer");
    assert_eq!(
        home.read("config/mimeapps.list").as_deref(),
        Some(changed.as_str())
    );
    assert!(!home.path("data/applications/fileblade.desktop").exists());
    assert_eq!(
        home.read("config/autostart/fileblade.desktop").as_deref(),
        Some("[Desktop Entry]\nType=Application\nName=Mine\nExec=other\n")
    );
    let (status, _) = home.roles(&["status", "--json"]);
    assert_eq!(status["roles"]["folder"]["enabled"], false);
    assert_eq!(status["roles"]["folder"]["entries"], json!([]));
}

#[test]
fn a_corrupt_receipt_refuses_every_command_and_is_never_rewritten() {
    let home = Home::new();
    fs::create_dir_all(home.path("config/omarchy/fileblade")).unwrap();
    fs::write(home.receipt(), "{\"schema\": 7, \"roles\": {}}").unwrap();
    fs::set_permissions(home.receipt(), fs::Permissions::from_mode(0o600)).unwrap();
    for arguments in [
        vec!["status", "--json"],
        vec!["enable", "--role", "folder", "--json"],
        vec!["disable", "--role", "folder", "--json"],
        vec!["disable", "--all", "--json"],
    ] {
        let (document, code) = home.roles(&arguments);
        assert_eq!(code, 1, "{document}");
        assert_eq!(document["status"], "refused");
        assert!(
            document["error"].as_str().unwrap().contains("schema 7"),
            "{document}"
        );
    }
    assert_eq!(
        home.read("config/omarchy/fileblade/desktop-roles.json")
            .as_deref(),
        Some("{\"schema\": 7, \"roles\": {}}")
    );
    assert!(!home.path("data/applications/fileblade.desktop").exists());
}

#[test]
fn enable_without_a_stable_launcher_is_refused() {
    if Path::new("/usr/bin/fileblade").exists() {
        return;
    }
    let home = Home::new();
    fs::remove_file(home.path(".local/bin/fileblade")).unwrap();
    let (document, code) = home.roles(&["enable", "--role", "autostart", "--json"]);
    assert_eq!(code, 1);
    assert_eq!(document["status"], "refused");
    assert!(
        document["error"]
            .as_str()
            .unwrap()
            .contains("no stable launcher"),
        "{document}"
    );
    assert!(!home.path("config/autostart").exists());
    assert!(!home.receipt().exists());
    let (status, _) = home.roles(&["status", "--json"]);
    assert_eq!(status["roles"]["autostart"]["launcher"], Value::Null);
}

#[test]
fn native_state_root_routes_through_a_lease_or_the_live_authority() {
    let home = Home::new();
    let root = home.path("state/omarchy/fileblade");
    fs::create_dir_all(&root).unwrap();
    let native = |arguments: &[&str]| {
        let output = home
            .command()
            .env("FILEBLADE_NATIVE_STATE_ROOT", &root)
            .args(["native", "roles"])
            .args(arguments)
            .output()
            .unwrap();
        let document: Value = serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|_| panic!("{}", String::from_utf8_lossy(&output.stderr)));
        (document, output.status.code().unwrap())
    };
    let (document, code) = native(&["enable", "--role", "autostart", "--json"]);
    assert_eq!(code, 0, "{document}");
    assert_eq!(document["roles"]["autostart"]["status"], "enabled");
    assert!(home.path("config/autostart/fileblade.desktop").exists());
    let (document, code) = native(&["disable", "--all", "--json"]);
    assert_eq!(code, 0, "{document}");
    assert_eq!(document["roles"]["autostart"]["status"], "restored");
    let mut authority = home
        .command()
        .env("FILEBLADE_NATIVE_STATE_ROOT", &root)
        .env("FILEBLADE_SPIKE_HOME", home.dir.path())
        .args([
            "serve",
            "--native-authority",
            "--no-recover",
            "--native-isolated",
        ])
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(10);
    while fileblade::lease::transport::probe(&root).is_err() {
        assert!(authority.try_wait().unwrap().is_none(), "authority exited");
        assert!(Instant::now() < deadline, "authority did not become ready");
        std::thread::sleep(Duration::from_millis(20));
    }
    let (document, code) = native(&["enable", "--role", "folder", "--json"]);
    assert_eq!(code, 0, "{document}");
    assert_eq!(document["roles"]["folder"]["status"], "enabled");
    assert!(home.path("data/applications/fileblade.desktop").exists());
    let (status, _) = native(&["status", "--json"]);
    assert_eq!(status["roles"]["folder"]["enabled"], true);
    let (document, code) = native(&["disable", "--all", "--json"]);
    assert_eq!(code, 0, "{document}");
    assert_eq!(document["status"], "complete");
    assert_eq!(document["roles"]["folder"]["status"], "restored");
    assert!(!home.path("data/applications/fileblade.desktop").exists());
    assert!(authority.try_wait().unwrap().is_none(), "authority exited");
    authority.kill().unwrap();
    authority.wait().unwrap();
}
