use fileblade::module_helpers::{CoreRoute, canonical_route};
use serde_json::{Value, json};
use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Command;

struct Fixture {
    root: tempfile::TempDir,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let path = root.path();
        fs::create_dir_all(path.join("python/bin")).unwrap();
        fs::write(path.join("manifest.json"), "{}").unwrap();
        for name in ["skills", "memory", "hooks", "mcp"] {
            let helper = path.join(format!("python/bin/agent-{name}ctl"));
            fs::write(&helper, "#!/usr/bin/python3\nimport json,sys\nprint(json.dumps({'ok':True,'args':sys.argv[1:]}))\n").unwrap();
            fs::set_permissions(helper, fs::Permissions::from_mode(0o700)).unwrap();
        }
        fs::create_dir(path.join("scripts")).unwrap();
        let registry = path.join("scripts/omarchy");
        fs::write(&registry, "#!/usr/bin/python3\nimport os,pathlib\npathlib.Path(os.environ['FILEBLADE_APP_ROOT'],'registry-called').touch()\nraise SystemExit(1)\n").unwrap();
        fs::set_permissions(registry, fs::Permissions::from_mode(0o700)).unwrap();
        Self { root }
    }

    fn run(
        &self,
        provider: &str,
        directory: &str,
        helper: &str,
        method: &str,
        write: bool,
    ) -> Value {
        let output = Command::new(
            std::env::var_os("FILEBLADE_BINARY")
                .unwrap_or_else(|| env!("CARGO_BIN_EXE_fileblade").into()),
        )
        .args([
            "_backend",
            if write { "helper-write" } else { "helper-read" },
            "--provider",
            provider,
            "--plugin-dir",
            directory,
            "--helper",
            helper,
            "--method",
            method,
            "--arguments",
            "[]",
        ])
        .env("HOME", self.root.path())
        .env("FILEBLADE_APP_ROOT", self.root.path())
        .env("XDG_STATE_HOME", self.root.path().join("state"))
        .env("XDG_CONFIG_HOME", self.root.path().join("config"))
        .env(
            "PATH",
            format!("{}:/usr/bin", self.root.path().join("scripts").display()),
        )
        .output()
        .unwrap();
        serde_json::from_slice(&output.stdout)
            .unwrap_or_else(|_| json!({"ok":false,"error":String::from_utf8_lossy(&output.stderr)}))
    }
}

#[test]
fn canonical_routes_match_only_the_exact_owned_identities_and_helper() {
    for (module, route) in [
        ("skills", CoreRoute::Skills),
        ("memory", CoreRoute::Memory),
        ("hooks", CoreRoute::Hooks),
        ("mcp", CoreRoute::Mcp),
    ] {
        for provider in [
            format!("fileblade.core.{module}"),
            format!("data-goblin.fileblade-{module}"),
            format!("kurt.agent-{module}"),
        ] {
            assert_eq!(canonical_route(&provider, "inventory"), Some(route));
            assert_eq!(canonical_route(&provider, "other"), None);
            assert_eq!(
                canonical_route(&format!("{provider}/{module}"), "inventory"),
                None
            );
        }
    }
    for provider in [
        "kurt.goblin-images",
        "acme.skills",
        "fileblade.core.other",
        "skills",
    ] {
        assert_eq!(canonical_route(provider, "inventory"), None);
    }
}

fn rejected(result: &Value) -> bool {
    result["ok"] == false
        && result["error"]
            .as_str()
            .is_some_and(|error| error.contains("core helper"))
}

const IN_PROCESS: [&str; 3] = ["skills", "memory", "hooks"];

#[test]
fn core_helpers_never_query_the_registry_and_cannot_be_retargeted() {
    let fixture = Fixture::new();
    for module in ["skills", "memory", "hooks", "mcp"] {
        let provider = format!("fileblade.core.{module}");
        let result = fixture.run(&provider, "", "inventory", "list", false);
        assert_eq!(result["ok"], true, "{result}");
        if IN_PROCESS.contains(&module) {
            assert_eq!(result["args"], Value::Null, "{result}");
            assert_eq!(result["schemaVersion"], json!(1), "{result}");
        } else {
            assert_eq!(result["args"], json!(["list"]));
        }
        assert!(rejected(&fixture.run(
            &provider,
            "/retired/checkout",
            "inventory",
            "list",
            false
        )));
        assert!(rejected(
            &fixture.run(&provider, "", "other", "list", false)
        ));
        assert!(rejected(&fixture.run(
            &provider,
            "",
            "inventory",
            "apply",
            false
        )));
        assert!(rejected(&fixture.run(
            &provider,
            "",
            "inventory",
            "list",
            true
        )));
        for legacy in [
            format!("data-goblin.fileblade-{module}"),
            format!("kurt.agent-{module}"),
        ] {
            assert_eq!(
                fixture.run(&legacy, "/retired/checkout", "inventory", "list", false)["ok"],
                true
            );
        }
    }
    assert!(!fixture.root.path().join("registry-called").exists());
}

#[test]
fn skills_and_memory_writes_need_no_separate_consent() {
    let fixture = Fixture::new();
    for provider in [
        "fileblade.core.skills",
        "fileblade.core.memory",
        "data-goblin.fileblade-skills",
        "kurt.agent-memory",
    ] {
        let result = fixture.run(provider, "", "inventory", "apply", true);
        assert!(!rejected(&result), "{result}");
    }
}

#[test]
fn recovery_label_and_usage_methods_are_limited_to_their_owned_modules() {
    let fixture = Fixture::new();
    for module in ["hooks", "mcp"] {
        let provider = format!("fileblade.core.{module}");
        assert!(!rejected(&fixture.run(
            &provider,
            "",
            "inventory",
            "recovery-list",
            false
        )));
        for method in ["prepare-remove", "remove-prepared", "restore", "discard"] {
            assert!(!rejected(&fixture.run(
                &provider,
                "",
                "inventory",
                method,
                true
            )));
            assert!(rejected(&fixture.run(
                &provider,
                "",
                "inventory",
                method,
                false
            )));
        }
        assert_eq!(
            !rejected(&fixture.run(&provider, "", "inventory", "label", true)),
            module == "hooks"
        );
    }
    for module in ["skills", "memory", "hooks", "mcp"] {
        let provider = format!("fileblade.core.{module}");
        assert_eq!(
            !rejected(&fixture.run(&provider, "", "inventory", "usage", false)),
            matches!(module, "skills" | "mcp")
        );
        assert_eq!(
            !rejected(&fixture.run(&provider, "", "inventory", "usage-forget", true)),
            module == "mcp"
        );
        for (method, write) in [("usage", true), ("usage-forget", false)] {
            assert!(rejected(&fixture.run(
                &provider,
                "",
                "inventory",
                method,
                write
            )));
        }
    }
    assert!(rejected(&fixture.run(
        "fileblade.core.skills",
        "",
        "inventory",
        "recovery-list",
        false
    )));
}
