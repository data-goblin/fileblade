use semver::{BuildMetadata, Version};
use std::fs;
use std::path::Path;
use std::process::Command;

fn released_version(text: &str) -> Option<Version> {
    Version::parse(text).ok().map(|mut version| {
        version.build = BuildMetadata::EMPTY;
        version
    })
}

#[test]
fn working_version_is_above_every_released_version() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let working = released_version(env!("CARGO_PKG_VERSION")).unwrap();
    let changelog = fs::read_to_string(root.join("CHANGELOG.md")).unwrap();
    let headings: Vec<_> = changelog
        .lines()
        .filter_map(|line| line.strip_prefix("## "))
        .map(str::trim)
        .collect();
    assert_eq!(
        headings
            .first()
            .map(|h| h.trim_end_matches(" (unreleased)")),
        Some(working.to_string().as_str()),
        "the newest CHANGELOG.md heading must name the working version"
    );
    let mut released: Vec<Version> = headings[1..]
        .iter()
        .filter(|heading| !heading.ends_with("(unreleased)"))
        .filter_map(|heading| released_version(heading))
        .collect();
    assert!(
        !released.is_empty(),
        "CHANGELOG.md lists no released version"
    );
    if let Ok(output) = Command::new("git")
        .args(["-C", root.to_str().unwrap(), "tag", "--list", "v*"])
        .output()
        && output.status.success()
    {
        released.extend(
            String::from_utf8_lossy(&output.stdout)
                .split_whitespace()
                .filter_map(|tag| released_version(tag.strip_prefix('v')?)),
        );
    }
    let newest = released.iter().max().unwrap();
    assert!(
        working > *newest,
        "the working version {working} is not above the released {newest}"
    );
}

#[test]
fn manifest_and_crate_versions_agree() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join("manifest.json")).unwrap()).unwrap();
    let manifest_version = manifest["version"].as_str().unwrap_or_default();
    assert_eq!(
        manifest_version,
        env!("CARGO_PKG_VERSION"),
        "manifest.json and Cargo.toml versions differ; updates.rs treats a mismatch as a stale backend"
    );
}

#[test]
fn provenance_actions_are_pinned_to_full_commits() {
    let workflow = fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join(".github/workflows/bundle-provenance.yml"),
    )
    .unwrap();
    let actions: Vec<_> = workflow
        .lines()
        .filter_map(|line| line.trim().strip_prefix("uses: "))
        .collect();
    assert!(
        !actions.is_empty(),
        "provenance workflow has no pinned actions"
    );
    for action in actions {
        let (repository, commit) = action.split_once('@').expect("action has no commit pin");
        assert!(repository.starts_with("actions/"));
        assert_eq!(commit.len(), 40, "action must use a full commit: {action}");
        assert!(commit.bytes().all(|byte| byte.is_ascii_hexdigit()));
    }
}

#[test]
fn marketplace_release_metadata_and_bundled_install_are_documented() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let manifest: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join("manifest.json")).unwrap()).unwrap();
    assert_eq!(manifest["name"], "FileBlade");
    assert_eq!(
        manifest["repository"],
        "https://github.com/data-goblin/fileblade"
    );
    assert_eq!(manifest["license"], "MIT");
    assert_eq!(manifest["minOmarchyVersion"], "4.0.2");

    let security = fs::read_to_string(root.join("SECURITY.md")).unwrap();
    for required in [
        "Omarchy Quattro 4.0.2 or newer",
        "## Dependencies and previews",
        "`gtk-launch`",
        "`xdg-terminal-exec`",
        "git ls-remote",
        "sends no telemetry",
        "deliberately preserves your layout",
        "Eligible media previews load automatically on selection",
        "automatically renders a size-constrained inline preview out of process",
        "`fileblade _backend thumbnail-render`",
        "The shell only ever hands Qt that PNG",
        "type, link, byte, and target-size gates limit exposure",
    ] {
        assert!(
            security.contains(required),
            "SECURITY.md is missing: {required}"
        );
    }
    assert!(!security.contains("Load preview"));
    assert!(!security.contains("Qt decodes the selected image inside the shell"));
    assert!(!security.contains("never decoded merely by selecting"));
    assert_eq!(
        security.matches("`git ls-remote`").count(),
        1,
        "SECURITY.md must describe the optional update check exactly once"
    );

    let preview = fs::read(root.join("preview.png")).unwrap();
    assert!(preview.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert!(preview.len() <= 50 * 1024 * 1024);
    assert!(preview.len() >= 24, "preview.png has no PNG dimensions");
    assert_eq!(
        u32::from_be_bytes(preview[16..20].try_into().unwrap()),
        1024
    );
    assert_eq!(u32::from_be_bytes(preview[20..24].try_into().unwrap()), 576);
    let social = fs::read(root.join("assets/fileblade-twitter-preview.png")).unwrap();
    assert!(social.starts_with(b"\x89PNG\r\n\x1a\n"));
    assert_eq!(u32::from_be_bytes(social[16..20].try_into().unwrap()), 1200);
    assert_eq!(
        u32::from_be_bytes(social[20..24].try_into().unwrap()),
        1200,
        "the social preview stays a 1200x1200 PNG while the marketplace preview is the overview still"
    );
    let preview_source =
        fs::read_to_string(root.join("assets/fileblade-twitter-preview.svg")).unwrap();
    assert!(preview_source.contains("managing files in Omarchy"));
    assert!(!preview_source.contains("Omachy"));

    let bindings = fs::read_to_string(root.join("examples/fileblade-bindings.lua")).unwrap();
    assert!(bindings.contains("toggleBladeFocus left"));
    assert!(bindings.contains("toggleBladeFocus right"));
    assert!(bindings.contains("|| hyprctl dispatch"));

    let ignore = fs::read_to_string(root.join(".gitignore")).unwrap();
    for local_demo_output in [
        "/assets/demos/*.gif",
        "/assets/demos/*.mp4",
        "/assets/demos/*.take/",
    ] {
        assert!(
            ignore.contains(local_demo_output),
            "release snapshot must ignore local demo output: {local_demo_output}"
        );
    }
    assert!(
        fs::read(root.join("fileblade-bin"))
            .unwrap()
            .starts_with(b"\x7fELF")
    );
    assert!(
        fs::read_to_string(root.join("tests/run"))
            .unwrap()
            .contains("tools/bundle verify")
    );

    let demo_session = fs::read_to_string(root.join("demos/session.sh")).unwrap();
    assert!(!demo_session.contains("XDG_RUNTIME_DIR:-/tmp"));
    assert!(demo_session.contains("umask 077"));
    assert!(demo_session.contains("[[ ! -L $BASE ]]"));
    assert!(demo_session.contains("demo base is not owned by the current user"));

    let properties_demo = fs::read_to_string(root.join("demos/readme-02-properties.toml")).unwrap();
    assert!(properties_demo.contains("Supported image previews appear as soon as you select them"));
    assert!(!properties_demo.contains("load_preview"));
    assert!(!properties_demo.contains("Load preview"));
}
