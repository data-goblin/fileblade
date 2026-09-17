use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::AtomicBool;

use fileblade::updates::{RepositorySpec, check, parse_specs};

fn scratch(name: &str) -> PathBuf {
    let root =
        std::env::temp_dir().join(format!("fileblade-updates-{name}-{}", std::process::id()));
    let _ = fs::remove_dir_all(&root);
    fs::create_dir_all(&root).unwrap();
    root
}

fn git(cwd: &Path, arguments: &[&str]) {
    let status = Command::new("git")
        .args(arguments)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_EDITOR", "true")
        .current_dir(cwd)
        .env("GIT_AUTHOR_NAME", "t")
        .env("GIT_AUTHOR_EMAIL", "t@example.invalid")
        .env("GIT_COMMITTER_NAME", "t")
        .env("GIT_COMMITTER_EMAIL", "t@example.invalid")
        .status()
        .unwrap();
    assert!(status.success(), "git {arguments:?}");
}

fn git_stdout(cwd: &Path, arguments: &[&str]) -> String {
    let output = Command::new("git")
        .args(arguments)
        .env("GIT_CONFIG_GLOBAL", "/dev/null")
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env("GIT_EDITOR", "true")
        .current_dir(cwd)
        .output()
        .unwrap();
    assert!(output.status.success(), "git {arguments:?}");
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

fn manifest(version: &str) -> String {
    format!(
        "{{\"schemaVersion\":1,\"id\":\"t.plugin\",\"name\":\"T\",\"version\":\"{version}\",\"kinds\":[\"service\"],\"entryPoints\":{{\"service\":\"Service.qml\"}}}}\n"
    )
}

struct Fixture {
    remote: PathBuf,
    installed: PathBuf,
    author: PathBuf,
}

fn fixture(name: &str) -> Fixture {
    let root = scratch(name);
    let remote = root.join("remote.git");
    let author = root.join("author");
    let installed = root.join("installed");
    git(&root, &["init", "-q", "--bare", "-b", "main", "remote.git"]);
    fs::create_dir_all(&author).unwrap();
    git(&author, &["init", "-q", "-b", "main"]);
    fs::write(author.join("manifest.json"), manifest("1.0.0")).unwrap();
    fs::write(author.join("Service.qml"), "import QtQuick\nItem {}\n").unwrap();
    git(&author, &["add", "."]);
    git(&author, &["commit", "-q", "-m", "first"]);
    git(
        &author,
        &["remote", "add", "origin", remote.to_str().unwrap()],
    );
    git(&author, &["push", "-q", "-u", "origin", "main"]);
    git(
        &root,
        &["clone", "-q", remote.to_str().unwrap(), "installed"],
    );
    Fixture {
        remote,
        installed,
        author,
    }
}

fn publish(fixture: &Fixture, version: &str, subject: &str) {
    fs::write(fixture.author.join("manifest.json"), manifest(version)).unwrap();
    git(&fixture.author, &["commit", "-q", "-am", subject]);
    git(&fixture.author, &["push", "-q", "origin", "main"]);
}

fn spec(id: &str, path: &Path) -> RepositorySpec {
    RepositorySpec {
        id: id.to_string(),
        path: path.to_path_buf(),
    }
}

#[test]
fn specs_parse_only_well_formed_absolute_entries() {
    let parsed = parse_specs(&[
        "data-goblin.fileblade=/tmp/a".to_string(),
        "bad id=/tmp/b".to_string(),
        "relative=tmp/c".to_string(),
        "noequals".to_string(),
    ]);
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].id, "data-goblin.fileblade");
}

#[test]
fn check_observes_remote_changes_without_downloading_objects() {
    let fixture = fixture("check");
    let cancelled = AtomicBool::new(false);
    let specs = [spec("t.plugin", &fixture.installed)];
    let clean = check(&specs, "t.plugin", &cancelled);
    assert_eq!(clean["available"], false);
    assert_eq!(clean["repositories"][0]["behind"], 0);
    assert_eq!(clean["repositories"][0]["core"], true);

    let installed_head = git_stdout(&fixture.installed, &["rev-parse", "HEAD"]);
    publish(&fixture, "1.1.0", "second");
    let behind = check(&specs, "", &cancelled);
    let row = &behind["repositories"][0];
    assert_eq!(behind["available"], true);
    assert!(row["behind"].is_null());
    assert_eq!(row["comparison_known"], false);
    assert_eq!(row["ahead"], 0);
    assert_eq!(row["dirty"], false);
    assert_eq!(row["updatable"], true);
    assert_eq!(row["current_version"], "1.0.0");
    assert_eq!(row["upstream_version"], "");
    assert_eq!(row["subjects"], serde_json::json!([]));
    assert_eq!(
        git_stdout(&fixture.installed, &["rev-parse", "@{u}"]),
        installed_head
    );
    let remote_head = git_stdout(&fixture.author, &["rev-parse", "HEAD"]);
    assert!(
        !Command::new("git")
            .args(["cat-file", "-e", &remote_head])
            .current_dir(&fixture.installed)
            .status()
            .unwrap()
            .success()
    );
    assert_eq!(row["core"], false);
    assert_eq!(
        git_stdout(&fixture.installed, &["rev-parse", "HEAD"]),
        installed_head
    );
    assert!(
        fs::read_to_string(fixture.installed.join("manifest.json"))
            .unwrap()
            .contains("1.0.0"),
        "checking must not update the working tree"
    );

    fs::write(
        fixture.installed.join("Service.qml"),
        "import QtQuick\nItem { id: x }\n",
    )
    .unwrap();
    let dirty = check(&specs, "", &cancelled);
    assert_eq!(dirty["repositories"][0]["dirty"], true);
    assert_eq!(dirty["repositories"][0]["updatable"], false);
    assert_eq!(dirty["available"], false);
    let _ = fs::remove_dir_all(fixture.remote.parent().unwrap());
}

#[test]
fn check_refuses_non_checkouts_and_missing_upstreams() {
    let root = scratch("plain");
    let plain = root.join("plain");
    fs::create_dir_all(&plain).unwrap();
    let cancelled = AtomicBool::new(false);
    let report = check(&[spec("t.plain", &plain)], "", &cancelled);
    assert_eq!(report["repositories"][0]["ok"], false);
    assert_eq!(report["repositories"][0]["error"], "not a git checkout");

    let local = root.join("local");
    fs::create_dir_all(&local).unwrap();
    git(&local, &["init", "-q", "-b", "main"]);
    fs::write(local.join("a"), "a").unwrap();
    git(&local, &["add", "."]);
    git(&local, &["commit", "-q", "-m", "a"]);
    let report = check(&[spec("t.local", &local)], "", &cancelled);
    assert_eq!(report["repositories"][0]["error"], "no upstream branch");
    let _ = fs::remove_dir_all(&root);
}

#[test]
fn backend_exposes_update_check_but_not_update_apply() {
    assert!(fileblade::backend::parse(["fileblade", "update-check"]).is_ok());
    assert!(fileblade::backend::parse(["fileblade", "update-apply"]).is_err());
}

#[test]
fn a_detached_pin_can_check_and_use_the_omarchy_fast_forward_update_path() {
    let fixture = fixture("detached");
    git(&fixture.installed, &["checkout", "--detach", "-q"]);
    let specs = [spec("t.plugin", &fixture.installed)];
    let cancelled = AtomicBool::new(false);
    assert_eq!(check(&specs, "", &cancelled)["available"], false);
    publish(&fixture, "1.1.0", "next release");
    let report = check(&specs, "", &cancelled);
    assert_eq!(report["repositories"][0]["updatable"], true, "{report}");
    git(&fixture.installed, &["fetch", "--quiet", "origin", "HEAD"]);
    git(&fixture.installed, &["merge", "--ff-only", "FETCH_HEAD"]);
    assert_eq!(
        git_stdout(&fixture.installed, &["rev-parse", "HEAD"]),
        git_stdout(&fixture.author, &["rev-parse", "HEAD"])
    );
    assert_eq!(check(&specs, "", &cancelled)["available"], false);
    fs::remove_dir_all(fixture.remote.parent().unwrap()).unwrap();
}

fn publish_tag(fixture: &Fixture, name: &str, annotated: bool) {
    if annotated {
        git(&fixture.author, &["tag", "-a", name, "-m", "release"]);
    } else {
        git(&fixture.author, &["tag", name]);
    }
    git(&fixture.author, &["push", "-q", "origin", "--tags"]);
}

fn checked_row(fixture: &Fixture) -> serde_json::Value {
    check(
        &[spec("t.plugin", &fixture.installed)],
        "",
        &AtomicBool::new(false),
    )["repositories"][0]
        .clone()
}

fn assert_no_remote_objects(fixture: &Fixture) {
    let remote_head = git_stdout(&fixture.author, &["rev-parse", "HEAD"]);
    assert!(
        !Command::new("git")
            .args(["cat-file", "-e", &remote_head])
            .current_dir(&fixture.installed)
            .output()
            .unwrap()
            .status
            .success()
    );
    assert_eq!(git_stdout(&fixture.installed, &["tag"]), "");
    assert_eq!(
        git_stdout(&fixture.installed, &["rev-parse", "HEAD"]),
        git_stdout(&fixture.installed, &["rev-parse", "@{u}"])
    );
}

#[test]
fn release_tags_name_remote_versions_without_fetching_lightweight_or_annotated_objects() {
    for annotated in [false, true] {
        let fixture = fixture(if annotated {
            "annotated"
        } else {
            "lightweight"
        });
        publish(&fixture, "1.9.0", "earlier release");
        publish_tag(&fixture, "v1.9.0", false);
        publish(&fixture, "1.10.0", "current release");
        publish_tag(&fixture, "v1.10.0", annotated);
        publish_tag(&fixture, "v01.99.0", false);
        let row = checked_row(&fixture);
        assert_eq!(row["updatable"], true, "{row}");
        assert_eq!(row["upstream_version"], "1.10.0");
        assert_eq!(row["version_change"], "newer");
        assert_no_remote_objects(&fixture);
        git(&fixture.installed, &["checkout", "--detach", "-q"]);
        assert_eq!(checked_row(&fixture)["upstream_version"], "1.10.0");
        fs::remove_dir_all(fixture.remote.parent().unwrap()).unwrap();
    }
}

#[test]
fn an_untagged_tip_or_a_newer_tag_on_another_branch_does_not_borrow_a_release_version() {
    let fixture = fixture("untagged");
    publish(&fixture, "1.1.0", "release");
    publish_tag(&fixture, "v1.1.0", true);
    publish(&fixture, "1.2.0", "unreleased change");
    let row = checked_row(&fixture);
    assert_eq!(row["updatable"], true);
    assert_eq!(row["upstream_version"], "");
    publish_tag(&fixture, "v1.2.0", false);
    git(&fixture.author, &["checkout", "-q", "-b", "next"]);
    fs::write(fixture.author.join("manifest.json"), manifest("2.0.0")).unwrap();
    git(&fixture.author, &["commit", "-q", "-am", "next release"]);
    publish_tag(&fixture, "v2.0.0", true);
    let row = checked_row(&fixture);
    assert_eq!(row["updatable"], true);
    assert_eq!(row["upstream_version"], "");
    assert_no_remote_objects(&fixture);
    fs::remove_dir_all(fixture.remote.parent().unwrap()).unwrap();
}

#[test]
fn a_local_manifest_takes_precedence_over_tags_and_invalid_versions_stay_unknown() {
    let fixture = fixture("local-version");
    publish(&fixture, "1.2.0", "next release");
    publish_tag(&fixture, "v9.9.9", false);
    git(&fixture.installed, &["fetch", "-q", "origin"]);
    assert_eq!(checked_row(&fixture)["upstream_version"], "1.2.0");
    for version in [
        "",
        "01.2.0",
        "1.2",
        "1.2.3\nunknown",
        &format!("1.2.0-{}", "a".repeat(64)),
    ] {
        fs::write(
            fixture.author.join("manifest.json"),
            serde_json::json!({"version": version}).to_string(),
        )
        .unwrap();
        git(&fixture.author, &["commit", "-q", "-am", "invalid version"]);
        git(&fixture.author, &["push", "-q", "origin", "main"]);
        git(&fixture.installed, &["fetch", "-q", "origin"]);
        assert_eq!(checked_row(&fixture)["upstream_version"], "");
    }
    fs::remove_dir_all(fixture.remote.parent().unwrap()).unwrap();
}

#[test]
fn version_precedence_distinguishes_prerelease_same_build_metadata_and_older_changes() {
    for (version, expected) in [
        ("1.0.0-beta.2", "older"),
        ("1.0.0+build.2", "same"),
        ("0.9.9", "older"),
        ("1.0.1-beta.2", "newer"),
    ] {
        let fixture = fixture(&format!("precedence-{expected}-{version}"));
        publish(&fixture, version, "remote change");
        publish_tag(&fixture, &format!("v{version}"), false);
        let row = checked_row(&fixture);
        assert_eq!(row["upstream_version"], version);
        assert_eq!(row["version_change"], expected);
        assert_no_remote_objects(&fixture);
        fs::remove_dir_all(fixture.remote.parent().unwrap()).unwrap();
    }
}

#[test]
fn remote_ref_count_and_output_limits_refuse_incomplete_release_lists() {
    for oversized in [false, true] {
        let fixture = fixture(if oversized { "ref-bytes" } else { "ref-count" });
        publish(&fixture, "1.1.0", "next release");
        let head = git_stdout(&fixture.author, &["rev-parse", "HEAD"]);
        let tags: String = (0..512)
            .map(|index| {
                let suffix = if oversized {
                    format!("-{}.{}", "a".repeat(100), index)
                } else {
                    format!("-{index}")
                };
                format!("{head} refs/tags/v1.1.0{suffix}\n")
            })
            .collect();
        fs::write(fixture.remote.join("packed-refs"), tags).unwrap();
        let row = checked_row(&fixture);
        assert_eq!(row["ok"], false, "{row}");
        assert_eq!(row["updatable"], false);
        assert_eq!(
            row["error"],
            if oversized {
                "remote check failed: command exceeded its output limit"
            } else {
                "remote reference count exceeded"
            }
        );
        assert_no_remote_objects(&fixture);
        fs::remove_dir_all(fixture.remote.parent().unwrap()).unwrap();
    }
}
