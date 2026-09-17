#[path = "common/usage_fixtures.rs"]
mod usage_fixtures;

use chrono::{DateTime, Duration, Utc};
use fileblade::core_modules::usage::sql::{Bound, Sql, bind};
use serde_json::{Value, json};
use std::path::Path;
use usage_fixtures::*;

fn rollout(fixture: &Fixture, records: &[Value]) -> std::path::PathBuf {
    let directory = fixture.home.join(".codex/sessions/2026/09/14");
    std::fs::create_dir_all(&directory).expect("rollout directory");
    let path = directory.join("rollout-2026-09-14T10-00-00-c0de.jsonl");
    let mut body = lines(&[session_meta(fixture.day)]);
    body.push_str(&lines(records));
    std::fs::write(&path, body).expect("rollout body");
    path
}

fn codex_explicit(at: DateTime<Utc>, identity: &str, name: &str) -> Value {
    let body = format!(
        "<skill>\n<name>{name}</name>\n<path>/home/x/.agents/skills/{name}/SKILL.md</path>\nbody\n</skill>\n"
    );
    json!({
        "timestamp": iso(at),
        "type": "response_item",
        "payload": {"type": "message", "role": "user", "id": identity,
                    "content": [{"type": "input_text", "text": body}],
                    "internal_chat_message_metadata_passthrough": {
                        "turn_id": "turn-1", "create_time": 1.0,
                        "content_item_kinds": ["skills.selected_skill_instructions"]}},
    })
}

fn codex_read(at: DateTime<Utc>, identity: &str, turn: &str, path: &str) -> Value {
    json!({
        "timestamp": iso(at),
        "type": "event_msg",
        "payload": {"type": "item_completed", "thread_id": "t", "turn_id": turn,
                    "item": {"type": "CommandExecution", "id": identity,
                             "command": ["/usr/bin/bash", "-lc", format!("sed -n '1,240p' {path}")],
                             "cwd": "/work/project", "status": "completed", "exit_code": 0,
                             "parsed_cmd": [{"type": "read", "cmd": format!("sed -n '1,240p' {path}"),
                                             "name": "SKILL.md", "path": path}]}},
    })
}

fn codex_legacy_exec(at: DateTime<Utc>, call: &str, command: &str) -> Value {
    json!({
        "timestamp": iso(at),
        "type": "response_item",
        "payload": {"type": "custom_tool_call", "name": "exec", "call_id": call,
                    "id": format!("ct_{call}"), "input": command},
    })
}

fn opencode_directory(fixture: &Fixture) -> std::path::PathBuf {
    let directory = fixture.home.join(".local/share/opencode");
    std::fs::create_dir_all(&directory).expect("opencode directory");
    directory
}

fn opencode_stable(fixture: &Fixture, name: &str, parts: &[(&str, &str, &str, Value)]) {
    let path = opencode_directory(fixture).join(name);
    let database = Sql::open(&path).expect("opencode store");
    let at = fixture.day.timestamp_millis();
    let mut script = String::from(
        "CREATE TABLE session (id TEXT PRIMARY KEY, project_id TEXT, directory TEXT, title TEXT, version TEXT, \
         time_created INTEGER, time_updated INTEGER);\n\
         CREATE TABLE message (id TEXT PRIMARY KEY, session_id TEXT, time_created INTEGER, time_updated INTEGER, data TEXT);\n\
         CREATE TABLE part (id TEXT PRIMARY KEY, message_id TEXT, session_id TEXT, time_created INTEGER, time_updated INTEGER, data TEXT);\n",
    );
    script.push_str(
        &bind(
            "INSERT INTO session VALUES ('ses_1', 'p', '/work/project', 't', '1.18.30', ?, ?);",
            &[Bound::Integer(at), Bound::Integer(at)],
        )
        .expect("session row"),
    );
    script.push('\n');
    for (index, (call, tool, status, arguments)) in parts.iter().enumerate() {
        let data = json!({"type": "tool", "callID": call, "tool": tool,
                          "state": {"status": status, "input": arguments,
                                    "time": {"start": at + index as i64, "end": at + index as i64 + 1}}});
        script.push_str(
            &bind(
                "INSERT INTO part VALUES (?, 'msg_1', 'ses_1', ?, ?, ?);",
                &[
                    Bound::Text(format!("prt_{index}")),
                    Bound::Integer(at),
                    Bound::Integer(at + index as i64),
                    Bound::Text(serde_json::to_string(&data).expect("part data")),
                ],
            )
            .expect("part row"),
        );
        script.push('\n');
    }
    database.execute(&script).expect("opencode schema");
}

fn opencode_beta(fixture: &Fixture, name: &str, items: &[(&str, &str, &str, Value)]) {
    let path = opencode_directory(fixture).join(name);
    let database = Sql::open(&path).expect("opencode store");
    let at = fixture.day.timestamp_millis();
    let mut script = String::from(
        "CREATE TABLE session_v2 (id TEXT PRIMARY KEY, directory TEXT, version TEXT, time_created INTEGER, time_updated INTEGER);\n\
         CREATE TABLE session_message (id TEXT PRIMARY KEY, session_id TEXT, type TEXT, seq INTEGER, time_created INTEGER, \
         time_updated INTEGER, data TEXT);\n",
    );
    script.push_str(
        &bind(
            "INSERT INTO session_v2 VALUES ('ses_2', '/work/project', '0.0.0-beta', ?, ?);",
            &[Bound::Integer(at), Bound::Integer(at)],
        )
        .expect("session row"),
    );
    script.push('\n');
    let content: Vec<Value> = items
        .iter()
        .map(|(call, tool, status, arguments)| {
            json!({"type": "tool", "id": call, "name": tool,
                   "state": {"status": status, "input": arguments},
                   "time": {"created": at, "completed": at + 1}})
        })
        .collect();
    script.push_str(
        &bind(
            "INSERT INTO session_message VALUES ('msg_2', 'ses_2', 'assistant', 1, ?, ?, ?);",
            &[
                Bound::Integer(at),
                Bound::Integer(at),
                Bound::Text(
                    serde_json::to_string(&json!({"content": content, "time": {"created": at}}))
                        .expect("message data"),
                ),
            ],
        )
        .expect("message row"),
    );
    script.push('\n');
    database.execute(&script).expect("opencode schema");
}

fn copilot_events(fixture: &Fixture, events: &[(&str, Value)]) -> std::path::PathBuf {
    let path = fixture
        .home
        .join(".copilot/session-state/31cbc57f/events.jsonl");
    std::fs::create_dir_all(path.parent().expect("parent")).expect("copilot directory");
    let records: Vec<Value> = events
        .iter()
        .enumerate()
        .map(|(index, (kind, data))| {
            json!({"id": format!("e{index}"), "timestamp": iso(fixture.day),
                   "parentId": Value::Null, "type": kind, "data": data})
        })
        .collect();
    std::fs::write(&path, lines(&records)).expect("copilot events");
    path
}

fn antigravity_files(fixture: &Fixture, slash: &str, skill_path: &str) {
    let root = fixture.home.join(".gemini/antigravity-cli");
    let logs = root.join("brain/9c1d/.system_generated/logs");
    std::fs::create_dir_all(&logs).expect("antigravity directories");
    let at = fixture.day.timestamp_millis();
    std::fs::write(
        root.join("history.jsonl"),
        lines(&[
            json!({"timestamp": at, "workspace": "/work/project", "display": "/usage", "type": "slash_command"}),
            json!({"timestamp": at + 1, "workspace": "/work/project", "display": slash,
                   "type": "slash_command", "conversationId": "9c1d"}),
            json!({"timestamp": at + 2, "workspace": "/work/project", "display": "plain prompt"}),
        ]),
    )
    .expect("history");
    let stamp = fixture.day.format("%Y-%m-%dT%H:%M:%SZ").to_string();
    std::fs::write(
        logs.join("transcript_full.jsonl"),
        lines(&[
            json!({"step_index": 0, "type": "USER_INPUT", "source": "USER_EXPLICIT", "status": "DONE",
                   "created_at": stamp, "content": "x"}),
            json!({"step_index": 1, "type": "GENERIC", "source": "MODEL", "status": "DONE", "created_at": stamp,
                   "tool_calls": [{"name": "view_file", "args": {"AbsolutePath": skill_path}},
                                  {"name": "list_dir", "args": {"DirectoryPath": "/work"}}]}),
        ]),
    )
    .expect("transcript");
}

fn pi_session(fixture: &Fixture, entries: &[Value]) -> std::path::PathBuf {
    let path = fixture
        .home
        .join(".pi/agent/sessions/--work-project--/2026-09-14T10-00-00_s1.jsonl");
    std::fs::create_dir_all(path.parent().expect("parent")).expect("pi directory");
    let header = json!({"type": "session", "version": 3, "id": "s1",
                        "timestamp": iso(fixture.day), "cwd": "/work/project"});
    let mut records = vec![header];
    records.extend(entries.iter().cloned());
    std::fs::write(&path, lines(&records)).expect("pi session");
    path
}

fn row<'a>(rows: &'a [Value], name: &str, agent: &str) -> &'a Value {
    let found: Vec<&Value> = rows
        .iter()
        .filter(|row| row["name"] == json!(name) && row["agent"] == json!(agent))
        .collect();
    assert_eq!(found.len(), 1, "one row for {name} on {agent}");
    found[0]
}

fn uses_of(row: &Value) -> serde_json::Map<String, Value> {
    let mut values = serde_json::Map::new();
    for key in ["uses", "usesAgent", "usesUser", "usesScheduled", "failed"] {
        values.insert(key.to_string(), row[key].clone());
    }
    values
}

#[test]
fn every_supported_agent_counts_skill_uses() {
    use_utc();
    let fixture = Fixture::new();
    let day = fixture.day;
    let alpha_path = "/home/x/.agents/skills/alpha/SKILL.md";
    rollout(
        &fixture,
        &[
            codex_explicit(day, "msg_1", "alpha"),
            codex_read(day, "cmd_1", "turn-1", alpha_path),
            codex_read(day, "cmd_2", "turn-1", alpha_path),
            codex_legacy_exec(
                day,
                "call_9",
                "cat /home/x/.agents/skills/beta/SKILL.md | head",
            ),
            codex_call(day, "exec-1", "docs", "search", "completed"),
        ],
    );
    opencode_stable(
        &fixture,
        "opencode.db",
        &[
            ("call_a", "skill", "completed", json!({"name": "alpha"})),
            ("call_b", "skill", "error", json!({"name": "alpha"})),
            ("call_c", "skill", "running", json!({"name": "alpha"})),
            ("call_d", "read", "completed", json!({"filePath": "/x"})),
        ],
    );
    opencode_beta(
        &fixture,
        "opencode-beta.db",
        &[
            ("call_e", "skill", "completed", json!({"name": "gamma"})),
            ("call_f", "bash", "completed", json!({})),
        ],
    );
    copilot_events(
        &fixture,
        &[
            (
                "session.start",
                json!({"sessionId": "31cbc57f", "context": {"cwd": "/work/project"}}),
            ),
            (
                "skill.invoked",
                json!({"name": "alpha", "path": "/x/alpha/SKILL.md", "trigger": "user-invoked", "content": "body"}),
            ),
            (
                "skill.invoked",
                json!({"name": "alpha", "path": "/x/alpha/SKILL.md", "trigger": "agent-invoked", "content": "body"}),
            ),
            (
                "skill.invoked",
                json!({"name": "beta", "path": "/x/beta/SKILL.md", "trigger": "context-load", "content": "body"}),
            ),
            (
                "tool.execution_start",
                json!({"toolCallId": "tc-1", "toolName": "docs-search", "mcpServerName": "docs", "mcpToolName": "search"}),
            ),
            (
                "tool.execution_complete",
                json!({"toolCallId": "tc-1", "success": false}),
            ),
        ],
    );
    antigravity_files(
        &fixture,
        "/alpha do it",
        "/home/x/.agents/skills/beta/SKILL.md",
    );
    pi_session(
        &fixture,
        &[
            json!({"type": "message", "id": "m1", "parentId": Value::Null, "timestamp": iso(day),
                   "message": {"role": "assistant", "content": [{"type": "toolCall", "id": "tc_pi", "name": "read",
                               "arguments": {"path": "/home/x/.pi/agent/skills/gamma/SKILL.md"}}]}}),
            json!({"type": "message", "id": "m2", "parentId": "m1", "timestamp": iso(day),
                   "message": {"role": "toolResult", "toolCallId": "tc_pi", "toolName": "read",
                               "content": [], "isError": true}}),
        ],
    );

    let items = vec![
        stub("skill-alpha", "alpha", "user"),
        stub("skill-beta", "beta", "user"),
        stub("skill-gamma", "gamma", "user"),
    ];
    assert_eq!(
        fixture.uses(&items, "skill-alpha"),
        counts_of([7, 4, 3, 0, 1])
    );
    assert_eq!(
        fixture.uses(&items, "skill-beta"),
        counts_of([2, 2, 0, 0, 0])
    );
    assert_eq!(
        fixture.uses(&items, "skill-gamma"),
        counts_of([2, 2, 0, 0, 1])
    );
    let document = fixture.counts(&items);
    assert_eq!(document["usageTranscripts"], json!(7));
    assert_eq!(document["usageUnreadable"], json!(0));
    assert_eq!(document["usageIngestPending"], json!(false));

    let watched = fileblade::core_modules::usage::watch_paths(&fixture.environment());
    let watched: Vec<&str> = watched.iter().filter_map(Value::as_str).collect();
    for wanted in [
        fixture.transcripts.parent().expect("projects"),
        &fixture.home.join(".codex/sessions"),
    ] {
        let resolved = std::fs::canonicalize(wanted).expect("resolved watch root");
        assert!(
            watched.contains(&fileblade::common::path_text(&resolved).as_str()),
            "{resolved:?} is watched"
        );
    }

    let history = fixture.history(&items);
    assert_eq!(history["coverageStart"], json!(fixture.date(day)));
    assert_eq!(
        history["days"],
        json!([[fixture.date(day), 11, 8, 3, 0, 2]])
    );

    let database = fixture.open_store();
    let agents: Vec<(String, i64)> = database
        .query(
            "SELECT agent, count(*) FROM event WHERE kind IN ('skill', 'command') GROUP BY agent",
        )
        .expect("agent tally")
        .iter()
        .map(|row| (row.text(0), row.integer(1)))
        .collect();
    assert_eq!(
        agents
            .into_iter()
            .filter(|(agent, _)| agent != "claude")
            .collect::<Vec<(String, i64)>>(),
        vec![
            ("antigravity".to_string(), 3),
            ("codex".to_string(), 3),
            ("copilot".to_string(), 2),
            ("opencode".to_string(), 3),
            ("pi".to_string(), 1),
        ]
    );
    let projects = database
        .query_one(
            "SELECT count(DISTINCT project.path) FROM event JOIN project ON project.id = event.project",
        )
        .expect("projects")
        .map_or(0, |row| row.integer(0));
    assert_eq!(projects, 1);
    let failures = database
        .query_one("SELECT count(*) FROM failure")
        .expect("failures")
        .map_or(0, |row| row.integer(0));
    assert_eq!(failures, 0);
}

#[test]
fn an_opencode_call_that_finishes_later_is_counted_once_it_completes() {
    use_utc();
    let fixture = Fixture::new();
    opencode_stable(
        &fixture,
        "opencode.db",
        &[("call_a", "skill", "running", json!({"name": "alpha"}))],
    );
    let items = vec![stub("skill-alpha", "alpha", "user")];
    assert_eq!(
        fixture.counts(&items)["counts"]["skill-alpha"]["usesAgent"],
        json!(0)
    );
    let at = fixture.day.timestamp_millis();
    let path = fixture.home.join(".local/share/opencode/opencode.db");
    let database = Sql::open(&path).expect("opencode store");
    let data = json!({"type": "tool", "callID": "call_a", "tool": "skill",
                      "state": {"status": "completed", "input": {"name": "alpha"},
                                "time": {"start": at, "end": at + 5}}});
    let statement = bind(
        "UPDATE part SET data = ?, time_updated = ? WHERE id = 'prt_0';",
        &[
            Bound::Text(serde_json::to_string(&data).expect("data")),
            Bound::Integer(at + 5000),
        ],
    )
    .expect("late completion");
    database.execute(&statement).expect("late completion");
    assert_eq!(
        fixture.counts(&items)["counts"]["skill-alpha"]["usesAgent"],
        json!(1)
    );
    assert_eq!(
        fixture.counts(&items)["counts"]["skill-alpha"]["usesAgent"],
        json!(1)
    );
}

#[test]
fn mcp_rows_follow_the_server_names_agents_record() {
    use_utc();
    let fixture = Fixture::new();
    let day = fixture.day;
    let later = day + Duration::hours(1);
    fixture.transcript(
        "s1.jsonl",
        &[
            opening(),
            called(
                day,
                "toolu_m1",
                "mcp__my_server__lookup",
                json!({"query": "a"}),
            ),
            called(
                later,
                "toolu_m2",
                "mcp__my_server__lookup",
                json!({"query": "b"}),
            ),
            failed(later, "toolu_m2"),
            called(
                day,
                "toolu_m3",
                "ListMcpResourcesTool",
                json!({"server": "my.server"}),
            ),
            called(
                day,
                "toolu_m4",
                "ReadMcpResourceTool",
                json!({"server": "my.server", "uri": "probe://notes/alpha?version=2#top"}),
            ),
            typed(day, "p1", &["mcp__my_server__greet"], None),
            typed(day, "p2", &["mcp__my_server__greet"], Some("b1")),
            called(day, "toolu_m5", "mcp__twin_a__ping", json!({})),
            called(
                day,
                "toolu_m6",
                "mcp__plugin_toolkit_docs__search",
                json!({"query": "c"}),
            ),
        ],
    );
    rollout(
        &fixture,
        &[
            codex_call(day, "exec-1", "docs", "search", "completed"),
            codex_call(
                day,
                "exec-2",
                "openaiDeveloperDocs",
                "search_openai_docs",
                "completed",
            ),
            codex_call(
                day,
                "exec-3",
                "openaiDeveloperDocs",
                "fetch_openai_doc",
                "failed",
            ),
        ],
    );
    let mut definitions = vec![
        definition("d1", "my.server", "claude"),
        definition("d2", "twin.a", "claude"),
        definition("d3", "twin_a", "claude"),
        definition("d4", "quiet", "claude"),
        json!({"id": "d5", "name": "docs", "agent": "claude", "scope": "plugin",
               "source": {"plugin": "toolkit"}}),
        definition("d6", "docs", "codex"),
        definition("d7", "openaiDeveloperDocs", "codex"),
    ];
    let extra = fixture.attach_mcp(&mut definitions);
    let date = fixture.date(day);
    let mine = &definitions[0];
    assert_eq!(uses_of(mine), counts_of([5, 4, 1, 1, 1]));
    assert_eq!(uses_of(&mine["metrics"]), uses_of(mine));
    assert_eq!(
        mine["observed"],
        json!([
            {"kind": "tool", "name": "lookup", "uses": 2, "failed": 1, "lastUsed": date},
            {"kind": "resource-list", "name": "", "uses": 1, "failed": 0, "lastUsed": date},
            {"kind": "prompt", "name": "greet", "uses": 1, "failed": 0, "lastUsed": date},
            {"kind": "resource", "name": "probe://notes/alpha", "uses": 1, "failed": 0, "lastUsed": date},
        ])
    );
    for twin in [&definitions[1], &definitions[2]] {
        assert_eq!(twin["uses"], json!(0));
        assert_eq!(twin["usageAmbiguous"], json!(true));
        assert_eq!(twin["observed"], json!([]));
    }
    assert_eq!(extra["usageAmbiguous"], json!(2));
    let quiet = &definitions[3];
    assert_eq!(quiet["uses"], json!(0));
    assert_eq!(quiet["observed"], json!([]));
    assert_eq!(quiet.get("usageAmbiguous"), None);
    let plugin = &definitions[4];
    assert_eq!(plugin["uses"], json!(1));
    assert_eq!(plugin["observed"][0]["name"], json!("search"));
    assert_eq!(definitions[5]["uses"], json!(1));
    assert_eq!(uses_of(&definitions[6]), counts_of([2, 2, 0, 0, 1]));
    assert_eq!(extra["usageTranscripts"], json!(2));
    assert_eq!(extra["usageIngestPending"], json!(false));
    let history = fixture.mcp_history();
    assert_eq!(history["kind"], json!("mcp"));
    assert_eq!(history["coverageStart"], json!(date));
    assert_eq!(history["days"], json!([[date, 10, 9, 1, 1, 2]]));
}

#[test]
fn other_agents_mcp_calls_reach_their_definitions() {
    use_utc();
    let fixture = Fixture::new();
    copilot_events(
        &fixture,
        &[
            (
                "session.start",
                json!({"sessionId": "31cbc57f", "context": {"cwd": "/work/project"}}),
            ),
            (
                "tool.execution_start",
                json!({"toolCallId": "tc-1", "toolName": "docs-search", "mcpServerName": "docs", "mcpToolName": "search"}),
            ),
            (
                "tool.execution_complete",
                json!({"toolCallId": "tc-1", "success": false}),
            ),
            (
                "tool.execution_start",
                json!({"toolCallId": "tc-2", "toolName": "bash", "arguments": {}}),
            ),
            (
                "tool.execution_complete",
                json!({"toolCallId": "tc-2", "success": true}),
            ),
        ],
    );
    opencode_stable(
        &fixture,
        "opencode.db",
        &[
            (
                "call_a",
                "my_docs_search",
                "completed",
                json!({"query": "x"}),
            ),
            (
                "call_b",
                "my_docs_fetch_page",
                "error",
                json!({"query": "y"}),
            ),
            ("call_c", "read", "completed", json!({"filePath": "/x"})),
        ],
    );
    let mut definitions = vec![
        definition("d1", "docs", "github-copilot-cli"),
        definition("d2", "my.docs", "opencode"),
    ];
    fixture.attach_mcp(&mut definitions);
    let rows = definitions.clone();
    let copilot = row(&rows, "docs", "github-copilot-cli");
    assert_eq!(uses_of(copilot), counts_of([1, 1, 0, 0, 1]));
    assert_eq!(copilot["observed"][0]["name"], json!("search"));
    let opencode = row(&rows, "my.docs", "opencode");
    assert_eq!(uses_of(opencode), counts_of([2, 2, 0, 0, 1]));
    let mut names: Vec<String> = opencode["observed"]
        .as_array()
        .expect("observed")
        .iter()
        .map(|entry| entry["name"].as_str().unwrap_or("").to_string())
        .collect();
    names.sort();
    assert_eq!(names, vec!["fetch_page".to_string(), "search".to_string()]);
}

#[test]
fn a_typed_command_counts_whichever_lane_reads_it_first() {
    use_utc();
    let fixture = Fixture::new();
    let day = fixture.day;
    fixture.transcript("s1.jsonl", &[typed(day, "u1", &["pdf"], None)]);
    fixture.mcp_history();
    fixture.append("s1.jsonl", &[typed(day, "u2", &["pdf"], None)]);
    let items = vec![stub("skill-pdf", "pdf", "user")];
    assert_eq!(
        fixture.counts(&items)["counts"]["skill-pdf"]["usesUser"],
        json!(2)
    );
}

#[test]
fn a_long_transcript_commits_progress_and_finishes_across_chunks() {
    use_utc();
    let fixture = Fixture::new();
    let day = fixture.day;
    let total = 20000;
    let path = fixture.transcripts.join("large.jsonl");
    let padding = "x".repeat(1024);
    let mut body = String::new();
    for index in 0..total {
        body.push_str(&lines(&[called(
            day,
            &format!("large-{index}"),
            "mcp__docs__search",
            json!({"padding": padding}),
        )]));
    }
    std::fs::write(&path, body).expect("large transcript");
    let mut result = fixture.mcp_history();
    assert!(offset_of(&fixture, &path) > 0);
    for _ in 0..20 {
        if result["ingestPending"] == json!(false) {
            break;
        }
        result = fixture.mcp_history();
    }
    assert_eq!(result["ingestPending"], json!(false));
    assert_eq!(
        result["days"],
        json!([[fixture.date(day), total, total, 0, 0, 0]])
    );
}

fn offset_of(fixture: &Fixture, path: &Path) -> i64 {
    let statement = bind(
        "SELECT offset FROM source WHERE path = ?",
        &[Bound::Text(path.to_string_lossy().to_string())],
    )
    .expect("offset statement");
    fixture
        .open_store()
        .query_one(&statement)
        .ok()
        .flatten()
        .map_or(0, |row| row.integer(0))
}
