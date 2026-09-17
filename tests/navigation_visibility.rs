use serde_json::Value;
use std::fs;
use std::path::Path;
use std::process::Command;

const TAG: &str = "Signature: 8a477f597d28d172789f06886806bc55\n# compiler cache\n";

fn write(root: &Path, name: &str, text: &str) {
    let path = root.join(name);
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    fs::write(path, text).unwrap();
}

fn backend(home: &Path, arguments: &[&str], hidden: bool) -> Value {
    let mut command = Command::new(env!("CARGO_BIN_EXE_fileblade"));
    command
        .env("HOME", home)
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_STATE_HOME", home.join("state"))
        .env("XDG_CACHE_HOME", home.join("cache"))
        .env("XDG_DATA_HOME", home.join("data"))
        .env("_ZO_DATA_DIR", home.join("zoxide"))
        .arg("_backend")
        .args(arguments);
    if hidden {
        command.arg("--show-hidden");
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(value["ok"], true, "{value}");
    value
}

fn paths(value: &Value) -> Vec<String> {
    let value = if value["results"].is_array() {
        &value["results"][0]
    } else {
        value
    };
    value["entries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|row| row["path"].as_str().unwrap().to_owned())
        .collect()
}

#[test]
fn cache_and_private_paths_obey_hidden_in_every_filesystem_result_source() {
    let temporary = tempfile::Builder::new()
        .prefix("fileblade-visibility-")
        .tempdir()
        .unwrap();
    let home = temporary.path();
    let root = home.to_str().unwrap();
    let visible = "git/fileblade/sqsp-cli";
    let hidden = [
        "verification/target/debug/incremental/s-sqsp-temp",
        ".hidden/sqsp-secret",
        "config/omarchy/fileblade/sqsp-state",
        "state/omarchy/fileblade/sqsp-recovery",
        "state/fileblade/mcp-recovery/sqsp-record",
        "cache/fileblade/sqsp-thumbnail",
    ];
    write(home, "verification/target/CACHEDIR.TAG", TAG);
    for name in std::iter::once(visible).chain(hidden.iter().copied()) {
        write(home, &format!("{name}/note.txt"), "sqsp-content");
        backend(
            home,
            &[
                "frecency-visit",
                "--path",
                home.join(name).to_str().unwrap(),
            ],
            false,
        );
    }
    fs::create_dir_all(home.join("zoxide")).unwrap();
    for name in [visible, hidden[0], hidden[1]] {
        assert!(
            Command::new("zoxide")
                .env("_ZO_DATA_DIR", home.join("zoxide"))
                .args(["add", home.join(name).to_str().unwrap()])
                .status()
                .unwrap()
                .success()
        );
    }
    let list = home.join("list.json");
    let document = serde_json::json!({"base": root, "paths": std::iter::once(visible).chain(hidden.iter().copied()).map(|name| home.join(name)).collect::<Vec<_>>()});
    fileblade::secure::write_private_atomic(&list, &serde_json::to_vec(&document).unwrap())
        .unwrap();
    for show_hidden in [false, true] {
        for arguments in [
            vec!["quicknav", "--root", root, "--query", "sqsp"],
            vec![
                "search",
                "--root",
                root,
                "--query",
                "sqsp type:dir",
                "--fresh",
                "--no-git",
            ],
            vec![
                "search",
                "--root",
                root,
                "--query",
                "sqsp",
                "--list",
                list.to_str().unwrap(),
                "--no-git",
            ],
            vec!["frecency-list", "--query", "sqsp"],
        ] {
            let value = backend(home, &arguments, show_hidden);
            let rows = paths(&value);
            assert!(
                rows.contains(&home.join(visible).to_str().unwrap().to_string()),
                "{arguments:?}: {value}"
            );
            for name in hidden {
                assert_eq!(
                    rows.contains(&home.join(name).to_str().unwrap().to_string()),
                    show_hidden,
                    "{arguments:?}: {name}: {value}"
                );
            }
            if arguments[0] == "quicknav" && !show_hidden {
                assert_eq!(rows[0], home.join(visible).to_str().unwrap(), "{value}");
            }
        }
        let content = backend(
            home,
            &[
                "search",
                "--root",
                root,
                "--query",
                "content:sqsp-content",
                "--no-git",
            ],
            show_hidden,
        );
        let rows = paths(&content);
        for name in hidden {
            assert_eq!(
                rows.contains(
                    &home
                        .join(name)
                        .join("note.txt")
                        .to_str()
                        .unwrap()
                        .to_string()
                ),
                show_hidden,
                "{content}"
            );
        }
        for parent in ["verification", "config/omarchy", "state/omarchy", "cache"] {
            let path = home.join(parent);
            let path = path.to_str().unwrap();
            for command in ["children-batch", "children-window"] {
                let value = backend(home, &[command, "--path", path], show_hidden);
                assert_eq!(
                    !paths(&value).is_empty(),
                    show_hidden,
                    "{command}: {parent}: {value}"
                );
            }
        }
    }
    let excluded = backend(
        home,
        &[
            "quicknav",
            "--root",
            root,
            "--query",
            "sqsp",
            "--exclude",
            home.join(visible).to_str().unwrap(),
        ],
        false,
    );
    assert!(paths(&excluded).is_empty(), "{excluded}");
}

#[test]
fn only_a_regular_valid_cache_marker_hides_a_directory() {
    let temporary = tempfile::Builder::new()
        .prefix("fileblade-markers-")
        .tempdir()
        .unwrap();
    let home = temporary.path();
    write(home, "valid/CACHEDIR.TAG", TAG);
    write(home, "invalid/CACHEDIR.TAG", "not a cache marker");
    fs::create_dir_all(home.join("symlink")).unwrap();
    std::os::unix::fs::symlink(
        home.join("valid/CACHEDIR.TAG"),
        home.join("symlink/CACHEDIR.TAG"),
    )
    .unwrap();
    fs::create_dir_all(home.join("directory/CACHEDIR.TAG")).unwrap();
    let rows = paths(&backend(
        home,
        &["children-batch", "--path", home.to_str().unwrap()],
        false,
    ));
    assert_eq!(rows.len(), 3, "{rows:?}");
    assert!(!rows.contains(&home.join("valid").to_str().unwrap().to_string()));
}
