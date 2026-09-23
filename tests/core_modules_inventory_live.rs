use serde_json::{Value, json};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::Path;
use std::process::Command;

fn request(home: &Path, module: &str, method: &str, arguments: Value) -> Value {
    let output = Command::new(env!("CARGO_BIN_EXE_fileblade"))
        .args([
            "_backend",
            "helper-read",
            "--provider",
            &format!("fileblade.core.{module}"),
            "--plugin-dir",
            "",
            "--helper",
            "inventory",
            "--method",
            method,
            "--arguments",
            &arguments.to_string(),
        ])
        .env("FILEBLADE_APP_ROOT", env!("CARGO_MANIFEST_DIR"))
        .env("HOME", home)
        .env("XDG_STATE_HOME", home.join("state"))
        .env("XDG_CACHE_HOME", home.join("cache"))
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_DATA_HOME", home.join("data"))
        .env("CLAUDE_CONFIG_DIR", home.join(".claude"))
        .env("CODEX_HOME", home.join(".codex"))
        .env("COPILOT_HOME", home.join(".copilot"))
        .env("GEMINI_HOME", home.join(".gemini"))
        .env("PI_HOME", home.join(".pi"))
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let response: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(response["ok"], true, "{response}");
    response
}

#[test]
fn inventory_is_available_before_usage_and_counts_follow_appended_transcripts() {
    let fixture = tempfile::tempdir().unwrap();
    let home = fixture.path();
    fs::create_dir_all(home.join(".claude/skills/alpha")).unwrap();
    fs::create_dir_all(home.join(".claude/projects/test")).unwrap();
    let codex_month = home.join(
        chrono::Utc::now()
            .format(".codex/sessions/%Y/%m")
            .to_string(),
    );
    fs::create_dir_all(&codex_month).unwrap();
    fs::write(
        home.join(".claude/skills/alpha/SKILL.md"),
        "---\nname: alpha\ndescription: Alpha skill\n---\n",
    )
    .unwrap();
    fs::write(
        home.join(".claude.json"),
        json!({"mcpServers": {"docs": {"command": "never-execute-this"}}}).to_string(),
    )
    .unwrap();
    let skill = request(
        home,
        "skills",
        "list",
        json!(["--json", "--no-usage", "--scope", "user"]),
    );
    let mcp = request(
        home,
        "mcp",
        "list",
        json!(["--json", "--no-usage", "--scope", "user", "--watch"]),
    );
    assert!(
        skill["usageWatchPaths"]
            .as_array()
            .unwrap()
            .contains(&json!(codex_month))
    );
    let skill = skill["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["name"] == "alpha")
        .unwrap();
    let mcp = mcp["definitions"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["name"] == "docs")
        .unwrap();
    assert!(
        !home
            .join("state/omarchy/fileblade/agent-usage.sqlite3")
            .exists()
    );
    let mut transcript = OpenOptions::new()
        .create(true)
        .append(true)
        .open(home.join(".claude/projects/test/session.jsonl"))
        .unwrap();
    for total in 1..=2 {
        for (name, input) in [
            ("Skill", json!({"skill": "alpha"})),
            ("mcp__docs__search", json!({})),
        ] {
            writeln!(transcript, "{}", json!({
                "type": "assistant", "timestamp": chrono::Utc::now().to_rfc3339(),
                "message": {"content": [{"type": "tool_use", "id": format!("{name}-{total}"), "name": name, "input": input}]}
            })).unwrap();
        }
        transcript.flush().unwrap();
        let skills = request(
            home,
            "skills",
            "usage-counts",
            json!([
                "--json",
                "--items",
                json!([{"id": skill["id"], "name": "alpha", "source": "user"}]).to_string()
            ]),
        );
        let mcps = request(home, "mcp", "usage-counts", json!(["--json"]));
        assert_eq!(
            skills["counts"][skill["id"].as_str().unwrap()]["uses"],
            total
        );
        assert_eq!(mcps["counts"][mcp["id"].as_str().unwrap()]["uses"], total);
        assert_eq!(
            mcps["counts"][mcp["id"].as_str().unwrap()]["observed"][0]["name"],
            "search"
        );
        assert_eq!(
            mcps["counts"][mcp["id"].as_str().unwrap()]["usageAmbiguous"],
            false
        );
        for module in ["skills", "mcp"] {
            let history = request(home, module, "usage", json!(["--json", "--no-ingest"]));
            assert_eq!(history["days"][0][1], total);
        }
    }
}
