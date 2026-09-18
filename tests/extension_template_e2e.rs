use fileblade::extension_template::{self, Request};
use serde_json::Value;
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

fn fileblade() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fileblade"))
}

fn request(id: &str) -> Request {
    Request {
        id: id.to_string(),
        author: Some("Jane Doe".to_string()),
        ..Request::default()
    }
}

fn rendered(files: &[extension_template::Rendered], path: &str) -> extension_template::Rendered {
    files
        .iter()
        .find(|file| file.path == path)
        .unwrap_or_else(|| panic!("{path} is rendered"))
        .clone()
}

fn text(file: &extension_template::Rendered) -> String {
    String::from_utf8(file.contents.clone()).unwrap()
}

#[test]
fn defaults_follow_the_plugin_id() {
    let scaffold = extension_template::scaffold(&request("acme.fileblade-weather-radar")).unwrap();
    assert_eq!(scaffold.publisher, "acme");
    assert_eq!(scaffold.short, "fileblade-weather-radar");
    assert_eq!(scaffold.module, "weather-radar");
    assert_eq!(scaffold.name, "Weather Radar");
    assert_eq!(scaffold.author, "Jane Doe");
    assert_eq!(
        scaffold.description,
        "Adds a Weather Radar blade to FileBlade."
    );
    assert_eq!(
        scaffold.repository,
        "https://github.com/acme/fileblade-weather-radar.git"
    );
    assert!(scaffold.year >= 2026);

    let plain = extension_template::scaffold(&request("acme.clock")).unwrap();
    assert_eq!(plain.module, "clock");
    assert_eq!(plain.name, "Clock");

    let mut explicit = request("acme.fileblade-weather");
    explicit.name = Some("Weather Radar".to_string());
    explicit.module = Some("radar".to_string());
    explicit.description = Some("Rain, soon.".to_string());
    explicit.repository = Some("https://example.org/acme/weather.git".to_string());
    let explicit = extension_template::scaffold(&explicit).unwrap();
    assert_eq!(explicit.module, "radar");
    assert_eq!(explicit.name, "Weather Radar");
    assert_eq!(explicit.description, "Rain, soon.");
    assert_eq!(explicit.repository, "https://example.org/acme/weather.git");
}

#[test]
fn invalid_input_is_refused() {
    for id in [
        "Weather",
        "acme",
        "acme.",
        ".weather",
        "acme.fileblade.weather",
        "acme.wea ther",
        "Acme.weather",
        "acme/weather",
    ] {
        assert!(
            extension_template::scaffold(&request(id)).is_err(),
            "{id} must be refused"
        );
    }
    let mut quoted = request("acme.weather");
    quoted.name = Some("Say \"hi\"".to_string());
    assert!(extension_template::scaffold(&quoted).is_err());
    let mut braces = request("acme.weather");
    braces.description = Some("{{PLUGIN_ID}}".to_string());
    assert!(extension_template::scaffold(&braces).is_err());
    let mut module = request("acme.weather");
    module.module = Some("-bad".to_string());
    assert!(extension_template::scaffold(&module).is_err());
    let mut long = request("acme.weather");
    long.description = Some("x".repeat(161));
    assert!(extension_template::scaffold(&long).is_err());
    let mut repository = request("acme.weather");
    repository.repository = Some("https://example.org/a b".to_string());
    assert!(extension_template::scaffold(&repository).is_err());
    let mut empty = request("acme.weather");
    empty.author = Some("   ".to_string());
    assert!(extension_template::scaffold(&empty).is_err());
}

#[test]
fn rendered_files_carry_no_placeholders_and_match_the_manifest_contract() {
    let scaffold = extension_template::scaffold(&request("acme.fileblade-weather")).unwrap();
    let files = extension_template::render(&scaffold).unwrap();
    for expected in [
        "manifest.json",
        "Service.qml",
        "Provider.qml",
        "HostGuard.qml",
        "HostGuard.js",
        "bin/fileblade-host-status",
        "blades/Module.qml",
        "README.md",
        "ARCHITECTURE.md",
        "docs/agent-guidelines.md",
        "LICENSE",
        ".gitignore",
        "docs/agent-written/README.md",
        "tests/run",
        "tests/test_contract.py",
        "tests/tst_host_guard.qml",
        "tests/tst_module.qml",
        "tests/imports/qs/Commons/qmldir",
        "tests/imports/qs/Commons/Style.qml",
        "tests/imports/qs/Commons/Color.qml",
        "tests/imports/qs/Commons/Util.qml",
        "scripts/fileblade-extension-image.py",
        "assets/fileblade-logo.png",
    ] {
        rendered(&files, expected);
    }
    for file in &files {
        if let Ok(text) = std::str::from_utf8(&file.contents) {
            assert!(!text.contains("{{"), "placeholder left in {}", file.path);
            assert!(!text.contains("}}"), "placeholder left in {}", file.path);
        }
    }
    let manifest: Value =
        serde_json::from_slice(&rendered(&files, "manifest.json").contents).unwrap();
    assert_eq!(manifest["schemaVersion"], 1);
    assert_eq!(manifest["id"], "acme.fileblade-weather");
    assert_eq!(manifest["name"], "FileBlade Weather");
    assert_eq!(manifest["version"], "0.1.0");
    assert_eq!(manifest["author"], "Jane Doe");
    assert_eq!(manifest["entryPoints"]["service"], "Service.qml");
    let module = &manifest["extensions"]["data-goblin.fileblade/blade"][0];
    assert_eq!(module["id"], "weather");
    assert_eq!(module["name"], "Weather");
    assert_eq!(module["entry"], "blades/Module.qml");
    assert_eq!(module["hostContract"], 2);
    assert_eq!(module["settings"]["defaults"]["caption"], "");
    assert_eq!(module["settings"]["schema"][0]["key"], "caption");

    assert!(
        rendered(&files, "assets/fileblade-logo.png")
            .contents
            .starts_with(b"\x89PNG")
    );
    let script = rendered(&files, "scripts/fileblade-extension-image.py");
    assert!(script.executable);
    assert!(text(&script).starts_with("#!/usr/bin/env python3\n"));
    assert!(rendered(&files, "tests/run").executable);
    assert!(!rendered(&files, "manifest.json").executable);

    let readme = text(&rendered(&files, "README.md"));
    assert!(readme.contains("**FileBlade Weather**"));
    assert!(readme.contains(
        "omarchy plugin add https://github.com/acme/fileblade-weather.git --yes --enable"
    ));
    assert!(readme.contains("fileblade blade add right acme.fileblade-weather/weather"));
    assert!(readme.contains("assets/fileblade-extension-logo.svg"));
    let license = text(&rendered(&files, "LICENSE"));
    assert!(license.contains(&format!("Copyright (c) {} Jane Doe", scaffold.year)));
    let module_qml = text(&rendered(&files, "blades/Module.qml"));
    assert!(module_qml.contains("readonly property string title: \"Weather\""));
    assert!(module_qml.contains("textFormat: Text.PlainText"));
    assert!(!module_qml.contains("Process {"));
    let guard = text(&rendered(&files, "HostGuard.js"));
    assert!(guard.contains("var HOST_ID = \"data-goblin.fileblade\""));
}

#[test]
fn the_cli_writes_the_template_and_refuses_a_non_empty_directory() {
    let temporary = tempfile::tempdir().unwrap();
    let target = temporary.path().join("weather");
    let output = fileblade()
        .args(["extension", "template", "acme.fileblade-weather"])
        .arg(&target)
        .args(["--author", "Jane Doe", "--output", "json"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(document["ok"], true);
    assert_eq!(document["id"], "acme.fileblade-weather");
    assert_eq!(document["module"], "acme.fileblade-weather/weather");
    assert_eq!(document["directory"], target.to_str().unwrap());
    assert!(document["files"].as_array().unwrap().len() >= 20);
    assert!(document["next"].as_array().unwrap().iter().any(|step| {
        step.as_str()
            .unwrap()
            .contains("fileblade blade add right acme.fileblade-weather/weather")
    }));
    assert!(target.join("manifest.json").is_file());
    assert!(target.join("assets/fileblade-logo.png").is_file());
    assert!(target.join(".gitignore").is_file());
    assert!(target.join("tests/imports/qs/Commons/qmldir").is_file());
    let executable = fs::metadata(target.join("tests/run"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(executable & 0o777, 0o755);
    let plain = fs::metadata(target.join("manifest.json"))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(plain & 0o777, 0o644);

    let contract = Command::new("python3")
        .args(["-B", "tests/test_contract.py"])
        .current_dir(&target)
        .output()
        .unwrap();
    assert!(
        contract.status.success(),
        "generated extension contract failed:\n{}\n{}",
        String::from_utf8_lossy(&contract.stdout),
        String::from_utf8_lossy(&contract.stderr)
    );

    let again = fileblade()
        .args(["extension", "template", "acme.fileblade-weather"])
        .arg(&target)
        .output()
        .unwrap();
    assert!(!again.status.success());
    assert!(String::from_utf8_lossy(&again.stderr).contains("not empty"));
    assert_eq!(
        fs::read_dir(&target).unwrap().count(),
        fs::read_dir(&target).unwrap().count()
    );

    fs::write(target.join("README.md"), "mine\n").unwrap();
    fs::write(target.join("notes.txt"), "keep\n").unwrap();
    let forced = fileblade()
        .args(["extension", "template", "acme.fileblade-weather"])
        .arg(&target)
        .args(["--author", "Jane Doe", "--force"])
        .output()
        .unwrap();
    assert!(forced.status.success());
    assert_ne!(
        fs::read_to_string(target.join("README.md")).unwrap(),
        "mine\n"
    );
    assert_eq!(
        fs::read_to_string(target.join("notes.txt")).unwrap(),
        "keep\n"
    );

    let file = temporary.path().join("file");
    fs::write(&file, "x").unwrap();
    let onto_file = fileblade()
        .args(["extension", "template", "acme.fileblade-weather"])
        .arg(&file)
        .output()
        .unwrap();
    assert!(!onto_file.status.success());

    let output = fileblade()
        .current_dir(temporary.path())
        .args(["extension", "template", "acme.clock", "--author", "Jane"])
        .output()
        .unwrap();
    assert!(output.status.success());
    assert!(temporary.path().join("acme.clock/manifest.json").is_file());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.starts_with("Created acme.clock (Clock) in "));
    assert!(stdout.contains("fileblade blade add right acme.clock/clock"));

    let bad = fileblade()
        .current_dir(temporary.path())
        .args(["extension", "template", "Clock"])
        .output()
        .unwrap();
    assert!(!bad.status.success());
    assert!(String::from_utf8_lossy(&bad.stderr).contains("publisher.name"));
    assert!(!temporary.path().join("Clock").exists());
}

#[test]
fn the_bundled_image_script_writes_the_banner() {
    if Command::new("python3").arg("--version").output().is_err() {
        eprintln!("python3 is not installed; banner check skipped");
        return;
    }
    let temporary = tempfile::tempdir().unwrap();
    let target = temporary.path().join("weather");
    let scaffolded = fileblade()
        .args(["extension", "template", "acme.fileblade-weather"])
        .arg(&target)
        .args(["--author", "Jane Doe"])
        .output()
        .unwrap();
    assert!(scaffolded.status.success());
    let output = Command::new("python3")
        .arg(target.join("scripts/fileblade-extension-image.py"))
        .arg("--manifest")
        .arg(target.join("manifest.json"))
        .arg("--out")
        .arg(target.join("assets"))
        .env("PYTHONDONTWRITEBYTECODE", "1")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let svg = fs::read_to_string(target.join("assets/fileblade-extension-logo.svg")).unwrap();
    assert!(svg.starts_with("<svg xmlns=\"http://www.w3.org/2000/svg\" viewBox=\"0 0 960 272\""));
    assert!(svg.contains("aria-label=\"FileBlade weather extension\""));
    assert!(svg.contains("id=\"fileblade-extension-logo-mask\""));
    assert!(svg.contains("translate(410.39,243.20) scale(0.4)"));
    assert!(svg.contains("fill=\"#e0af68\""));
    assert!(svg.trim_end().ends_with("</svg>"));
    assert!(!Path::new(&target).join("scripts/__pycache__").exists());
}

#[test]
fn generated_extensions_ship_no_automatic_agent_instruction_path() {
    let scaffold = extension_template::scaffold(&request("acme.fileblade-weather")).unwrap();
    let files = extension_template::render(&scaffold).unwrap();
    for file in &files {
        let root_entry = file.path.split('/').next().unwrap_or(file.path);
        assert!(
            !matches!(
                root_entry,
                "AGENTS.md"
                    | "CLAUDE.md"
                    | "GEMINI.md"
                    | ".cursorrules"
                    | ".clinerules"
                    | ".claude"
                    | ".codex"
                    | ".agents"
            ),
            "{} is published with the extension, where coding agents read it on their own",
            file.path
        );
    }
    rendered(&files, "docs/agent-guidelines.md");
}

#[test]
fn generated_host_check_recognizes_native_fileblade_without_a_legacy_plugin() {
    let temporary = tempfile::tempdir().unwrap();
    let target = temporary.path().join("extension");
    let output = fileblade()
        .args(["extension", "template", "acme.fileblade-weather"])
        .arg(&target)
        .args(["--author", "Jane Doe"])
        .output()
        .unwrap();
    assert!(output.status.success());
    let bin = temporary.path().join("bin");
    fs::create_dir(&bin).unwrap();
    let fixture = r#"#!/usr/bin/python3
import json
import os
import sys
from pathlib import Path
name = Path(sys.argv[0]).name
if name == "omarchy":
    assert sys.argv[1:] == ["plugin", "list", "--json"]
    print(json.dumps([{"id": "acme.fileblade-weather", "name": "Weather", "enabled": True}]))
elif sys.argv[1:] == ["native", "roles", "status", "--json"]:
    print(json.dumps({"schema": 1, "action": "roles_status", "error": ""}))
else:
    assert sys.argv[1:] == ["native", "ipc", "--", "fileblade.native", "status"]
    print(json.dumps({"loaded": os.environ["NATIVE_READY"] == "1"}))
"#;
    for name in ["fileblade", "omarchy"] {
        let path = bin.join(name);
        fs::write(&path, fixture).unwrap();
        fs::set_permissions(path, fs::Permissions::from_mode(0o700)).unwrap();
    }
    for (ready, expected) in [("1", "ready"), ("0", "starting"), ("missing", "missing")] {
        if ready == "missing" {
            fs::remove_file(bin.join("fileblade")).unwrap();
        }
        let output = Command::new("/usr/bin/python3")
            .arg(target.join("bin/fileblade-host-status"))
            .env_clear()
            .env("PATH", &bin)
            .env("HOME", temporary.path())
            .env("NATIVE_READY", ready)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let status: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(status["state"], expected, "{status}");
        assert_eq!(status["plugins"][0]["id"], "acme.fileblade-weather");
    }
}
