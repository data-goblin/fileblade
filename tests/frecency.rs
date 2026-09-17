use fileblade::frecency;
use std::fs;

fn paths(payload: &serde_json::Value) -> Vec<String> {
    payload["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["path"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn visits_rank_by_decayed_score_follow_renames_and_import_recent_bookmarks() {
    let temporary = tempfile::Builder::new()
        .prefix("fileblade-recent-")
        .tempdir()
        .unwrap();
    let root = temporary.path();
    unsafe {
        std::env::set_var("XDG_STATE_HOME", root.join("state"));
        std::env::set_var("XDG_DATA_HOME", root.join("data"));
    }
    for name in ["a.md", "b.md", "gone.md", "other.txt"] {
        fs::write(root.join(name), "x").unwrap();
    }
    let a = root.join("a.md").to_str().unwrap().to_string();
    let b = root.join("b.md").to_str().unwrap().to_string();
    let gone = root.join("gone.md").to_str().unwrap().to_string();
    assert_eq!(
        frecency::visit(&[b.clone(), a.clone(), gone.clone()])["ok"],
        true
    );
    assert_eq!(frecency::visit(std::slice::from_ref(&a))["ok"], true);
    fs::remove_file(root.join("gone.md")).unwrap();
    let listed = frecency::list(10, "");
    assert_eq!(paths(&listed), [a.clone(), b.clone()], "{listed}");
    assert!(listed["entries"][0]["frecency"].as_f64().unwrap() > 1.9);
    let moved = root.join("moved.md").to_str().unwrap().to_string();
    fs::rename(root.join("a.md"), root.join("moved.md")).unwrap();
    frecency::remap_from(&serde_json::json!({
        "operation": "rename",
        "mappings": [{"source": a, "destination": moved}]
    }));
    assert_eq!(paths(&frecency::list(10, ""))[0], moved);
    fs::create_dir_all(root.join("data")).unwrap();
    let other = root.join("other.txt");
    fs::write(
        root.join("data/recently-used.xbel"),
        format!(
            "<xbel><bookmark href=\"file://{}\" added=\"2026-09-01T10:00:00Z\" visited=\"2026-09-01T10:00:00Z\"></bookmark></xbel>",
            other.display()
        ),
    )
    .unwrap();
    let with_recent = frecency::list(10, "other");
    assert_eq!(paths(&with_recent), [other.to_str().unwrap().to_string()]);
    assert_eq!(with_recent["entries"][0]["name_spans"], "0-5");
    assert!(frecency::scores().contains_key(&moved));
}
