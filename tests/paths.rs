use std::fs;

fn scratch(name: &str) -> std::path::PathBuf {
    let root = std::env::temp_dir().join(format!("fileblade-paths-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

#[test]
fn legacy_state_directory_moves_once_and_never_over_an_existing_one() {
    let root = scratch("state");
    unsafe { std::env::set_var("XDG_STATE_HOME", &root) };
    let legacy = root.join("omarchy/filetree");
    fs::create_dir_all(&legacy).unwrap();
    fs::write(legacy.join("state.json"), "{}").unwrap();

    let current = fileblade::paths::state_dir();
    assert_eq!(current, root.join("omarchy/fileblade"));
    assert!(current.join("state.json").is_file());
    assert!(!legacy.exists());

    fs::create_dir_all(&legacy).unwrap();
    fs::write(legacy.join("state.json"), "stale").unwrap();
    assert_eq!(fileblade::paths::state_dir(), current);
    assert_eq!(
        fs::read_to_string(current.join("state.json")).unwrap(),
        "{}"
    );
    assert!(legacy.join("state.json").is_file());

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn legacy_config_symlink_is_left_alone() {
    let root = scratch("config");
    unsafe { std::env::set_var("XDG_CONFIG_HOME", &root) };
    let elsewhere = root.join("elsewhere");
    fs::create_dir_all(&elsewhere).unwrap();
    fs::create_dir_all(root.join("omarchy")).unwrap();
    std::os::unix::fs::symlink(&elsewhere, root.join("omarchy/filetree")).unwrap();

    let current = fileblade::paths::config_dir();
    assert_eq!(current, root.join("omarchy/fileblade"));
    assert!(current.symlink_metadata().is_err());
    assert!(
        root.join("omarchy/filetree")
            .symlink_metadata()
            .unwrap()
            .is_symlink()
    );

    let _ = fs::remove_dir_all(&root);
}

#[test]
fn a_replaced_binary_path_drops_the_deleted_suffix_when_the_file_exists() {
    let dir = tempfile::tempdir().unwrap();
    let binary = dir.path().join("fileblade");
    std::fs::write(&binary, b"").unwrap();
    let deleted = std::path::PathBuf::from(format!("{} (deleted)", binary.display()));
    assert_eq!(fileblade::common::replaced_binary_path(&deleted), binary);
    let missing =
        std::path::PathBuf::from(format!("{} (deleted)", dir.path().join("gone").display()));
    assert_eq!(fileblade::common::replaced_binary_path(&missing), missing);
    assert_eq!(fileblade::common::replaced_binary_path(&binary), binary);
}

#[test]
fn a_declared_app_root_must_carry_the_manifest_and_the_service() {
    let root = scratch("app-root");
    unsafe { std::env::set_var("FILEBLADE_APP_ROOT", &root) };
    assert!(fileblade::paths::app_root().is_err());

    fs::write(root.join("manifest.json"), "{}").unwrap();
    fs::write(root.join("Service.qml"), "").unwrap();
    assert_eq!(fileblade::paths::app_root().unwrap(), root);

    unsafe { std::env::set_var("FILEBLADE_APP_ROOT", "relative/root") };
    assert!(fileblade::paths::app_root().is_err());

    unsafe { std::env::remove_var("FILEBLADE_APP_ROOT") };
    if let Ok(found) = fileblade::paths::app_root() {
        assert!(found.join("manifest.json").symlink_metadata().is_ok());
        assert!(found.join("Service.qml").symlink_metadata().is_ok());
        assert!(std::env::current_exe().unwrap().starts_with(&found));
    }

    let _ = fs::remove_dir_all(&root);
}
