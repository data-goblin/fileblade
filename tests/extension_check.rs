use serde_json::{Value, json};
use std::fs;
use std::path::Path;
use std::process::Command;

fn fileblade() -> Command {
    Command::new(env!("CARGO_BIN_EXE_fileblade"))
}

fn scaffold(directory: &Path) {
    let output = fileblade()
        .args(["extension", "template", "acme.fileblade-weather"])
        .arg(directory)
        .args(["--author", "Jane Doe"])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn check(directory: &Path) -> std::process::Output {
    fileblade()
        .args(["extension", "check", "."])
        .current_dir(directory)
        .output()
        .unwrap()
}

fn manifest(directory: &Path) -> Value {
    serde_json::from_slice(&fs::read(directory.join("manifest.json")).unwrap()).unwrap()
}

fn write_manifest(directory: &Path, document: &Value) {
    fs::write(
        directory.join("manifest.json"),
        serde_json::to_vec_pretty(document).unwrap(),
    )
    .unwrap();
}

#[test]
fn a_scaffolded_extension_passes_and_reports_its_checks() {
    let temporary = tempfile::tempdir().unwrap();
    let target = temporary.path().join("weather");
    scaffold(&target);
    let output = fileblade()
        .args(["--output", "json", "extension", "check"])
        .arg(&target)
        .output()
        .unwrap();
    assert!(output.status.success());
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(document["ok"], true);
    assert_eq!(document["checks"].as_array().unwrap().len(), 5);
    let text = check(&target);
    assert!(String::from_utf8_lossy(&text.stdout).contains("contract: 5 checks ok"));
}

#[test]
fn a_broken_extension_is_refused_with_the_failing_rule() {
    let temporary = tempfile::tempdir().unwrap();
    let target = temporary.path().join("weather");
    scaffold(&target);

    type Case = (&'static str, fn(&mut Value), &'static str);
    let cases: [Case; 6] = [
        (
            "version",
            |document| document["version"] = json!("1.0.0"),
            "stay at 0.1.0 until the first release",
        ),
        (
            "id",
            |document| document["id"] = json!("Acme.Weather"),
            "id must be publisher.name in lowercase",
        ),
        (
            "service",
            |document| document["entryPoints"]["service"] = json!("Missing.qml"),
            "the service entry point must exist",
        ),
        (
            "entry",
            |document| {
                document["extensions"]["data-goblin.fileblade/blade"][0]["entry"] =
                    json!("../outside/Module.qml")
            },
            "entry must be a safe relative path",
        ),
        (
            "setting type",
            |document| {
                document["extensions"]["data-goblin.fileblade/blade"][0]["settings"]["schema"][0]
                    ["type"] = json!("colour")
            },
            "unknown setting type colour",
        ),
        (
            "default",
            |document| {
                document["extensions"]["data-goblin.fileblade/blade"][0]["settings"]["defaults"]["stray"] =
                    json!(1)
            },
            "default stray has no schema row",
        ),
    ];
    let original = manifest(&target);
    for (name, mutate, message) in cases {
        let mut document = original.clone();
        mutate(&mut document);
        write_manifest(&target, &document);
        let output = check(&target);
        assert!(!output.status.success(), "{name} must be refused");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(message),
            "{name}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    write_manifest(&target, &original);
    assert!(check(&target).status.success());

    fs::remove_file(target.join("assets/fileblade-logo.png")).unwrap();
    let output = check(&target);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("needs assets/fileblade-logo.png"));

    fs::write(
        target.join("HostGuard.js"),
        "var HOST_ID = \"acme.other\"\n",
    )
    .unwrap();
    let output = check(&target);
    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("must name the FileBlade host"));
}
