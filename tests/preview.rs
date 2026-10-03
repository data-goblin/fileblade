use fileblade::preview;
use std::fs;
use std::sync::atomic::AtomicBool;
use tempfile::tempdir;

#[test]
fn preview_returns_lines_marks_binaries_and_colours_code_when_bat_exists() {
    let temporary = tempdir().unwrap();
    let root = temporary.path();
    let source = root.join("main.rs");
    fs::write(&source, "fn main() {\n    let answer = 42;\n}\n").unwrap();
    fs::write(root.join("blob.bin"), b"\x00\x01\x02binary").unwrap();
    let cancelled = AtomicBool::new(false);
    let text = preview::preview(source.to_str().unwrap(), 10, &cancelled);
    assert_eq!(text["ok"], true, "{text}");
    assert_eq!(text["binary"], false);
    let lines = text["lines"].as_array().unwrap();
    assert_eq!(lines.len(), 3);
    let first: String = lines[0]
        .as_array()
        .unwrap()
        .iter()
        .map(|run| run["text"].as_str().unwrap())
        .collect();
    assert_eq!(first, "fn main() {");
    if text["backend"] == "bat" {
        assert!(
            lines
                .iter()
                .flat_map(|line| line.as_array().unwrap())
                .any(|run| run["color"].as_i64().unwrap() >= 0)
        );
    }
    let binary = preview::preview(root.join("blob.bin").to_str().unwrap(), 10, &cancelled);
    assert_eq!(binary["binary"], true);
    assert!(binary["lines"].as_array().unwrap().is_empty());
    let limited = preview::preview(source.to_str().unwrap(), 1, &cancelled);
    assert_eq!(limited["truncated"], true);
    assert_eq!(limited["lines"].as_array().unwrap().len(), 1);
}

#[test]
fn a_file_reached_through_a_link_explains_the_refusal_without_an_error_code() {
    let temporary = tempdir().unwrap();
    let root = temporary.path();
    fs::create_dir(root.join("real")).unwrap();
    fs::write(root.join("real/note.txt"), "hello\n").unwrap();
    fs::write(root.join("real/picture.png"), b"\x89PNG\r\n\x1a\n").unwrap();
    std::os::unix::fs::symlink("real", root.join("linked")).unwrap();
    std::os::unix::fs::symlink(".", root.join("cycle")).unwrap();
    let cancelled = AtomicBool::new(false);
    for folder in ["linked", "cycle/real"] {
        let text = root.join(folder).join("note.txt");
        let image = root.join(folder).join("picture.png");
        for response in [
            preview::preview(text.to_str().unwrap(), 10, &cancelled),
            fileblade::thumbnail::thumbnail(image.to_str().unwrap(), "key", 64, 64, &cancelled),
        ] {
            assert_eq!(response["ok"], false, "{response}");
            let error = response["error"].as_str().unwrap();
            assert!(error.contains("reached through a link"), "{error}");
            assert!(!error.contains("os error"), "{error}");
        }
    }
    let direct = preview::preview(root.join("real/note.txt").to_str().unwrap(), 10, &cancelled);
    assert_eq!(direct["ok"], true, "{direct}");
}
