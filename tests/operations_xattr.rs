use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::Command;
use xattr::FileExt;

fn backend(root: &Path, args: &[&str]) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_fileblade"))
        .args(["--output", "json", "_backend"])
        .args(args)
        .env("XDG_DATA_HOME", root.join("data"))
        .env("XDG_STATE_HOME", root.join("state"))
        .env("FILEBLADE_JOURNAL", root.join("journal.json"))
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    serde_json::from_str(
        String::from_utf8_lossy(&output.stdout)
            .lines()
            .last()
            .unwrap(),
    )
    .unwrap()
}

#[test]
fn copy_preserves_extended_attributes_larger_than_one_page() {
    let parent = Path::new(env!("CARGO_MANIFEST_DIR")).join("target/operation-fixtures");
    fs::create_dir_all(&parent).unwrap();
    let root = tempfile::tempdir_in(parent).unwrap();
    let source = root.path().join("source");
    let target = root.path().join("target");
    fs::create_dir(&source).unwrap();
    fs::create_dir(&target).unwrap();
    let file = source.join("tagged");
    fs::write(&file, "payload").unwrap();
    let small = vec![b's'; 16];
    let large: Vec<u8> = (0..8192).map(|index| (index % 251) as u8).collect();
    let handle = fs::File::open(&file).unwrap();
    handle.set_xattr("user.small", &small).unwrap();
    handle.set_xattr("user.large", &large).unwrap();
    let result = backend(
        root.path(),
        &[
            "copy",
            "--source",
            file.to_str().unwrap(),
            "--destination",
            target.to_str().unwrap(),
        ],
    );
    assert_eq!(result["ok"], true, "{result}");
    let copied = fs::File::open(target.join("tagged")).unwrap();
    assert_eq!(copied.get_xattr("user.small").unwrap(), Some(small));
    assert_eq!(copied.get_xattr("user.large").unwrap(), Some(large));
}
