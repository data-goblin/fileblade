#[path = "common/usage_fixtures.rs"]
mod usage_fixtures;

use chrono::{Duration, Utc};
use fileblade::core_modules::usage::sql::Sql;
use serde_json::{Value, json};
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use usage_fixtures::*;

fn count_of(database: &Sql, statement: &str) -> i64 {
    database
        .query_one(statement)
        .expect("count")
        .map_or(0, |row| row.integer(0))
}

fn snapshot_reader(store: &Path) -> Child {
    let mut child = Command::new("sqlite3")
        .args(["-batch", "-bail"])
        .arg(store)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("snapshot reader");
    let stdin = child.stdin.as_mut().expect("snapshot stdin");
    stdin
        .write_all(b"BEGIN;\nSELECT count(*) FROM event;\n")
        .expect("snapshot statements");
    stdin.flush().expect("snapshot flush");
    let mut answer = String::new();
    BufReader::new(child.stdout.as_mut().expect("snapshot stdout"))
        .read_line(&mut answer)
        .expect("snapshot answer");
    child
}

fn alpha() -> Value {
    stub("skill-alpha", "alpha", "user")
}

fn beta() -> Value {
    stub("skill-beta", "beta", "user")
}

#[test]
fn skill_rows_separate_agent_typed_and_scheduled_uses() {
    use_utc();
    let fixture = Fixture::new();
    let day = fixture.day;
    let review = stub("skill-review", "review", "plugin:toolkit@market");
    let gamma = stub("skill-gamma", "gamma", "project");
    let first = skill_call(day, "toolu_s1", "alpha");
    fixture.transcript(
        "s1.jsonl",
        &[
            opening(),
            first.clone(),
            called(
                day,
                "toolu_s2",
                "Skill",
                json!({"skill": "alpha", "args": "--fast"}),
            ),
            failed(day, "toolu_s2"),
            typed(day, "u1", &["alpha"], None),
            typed(day, "u2", &["alpha"], Some("a36c8f4f")),
            typed(day, "u3", &["clear"], None),
            typed(day, "u4", &["model"], None),
            typed(day, "u5", &["gamma", "toolkit:review"], None),
            skill_call(day, "toolu_s3", "toolkit:review"),
        ],
    );
    fixture.transcript(
        "s1/subagents/workflows/wf_1/agent-a1.jsonl",
        &[called_from(
            day,
            "toolu_s4",
            "Skill",
            json!({"skill": "alpha"}),
            true,
        )],
    );
    let copy = fixture.transcript(
        "s2.jsonl",
        &[
            opening(),
            first,
            called(
                day,
                "toolu_s2",
                "Skill",
                json!({"skill": "alpha", "args": "--fast"}),
            ),
        ],
    );
    filetime_epoch(&copy);

    let items = vec![alpha(), beta(), gamma.clone(), review.clone()];
    assert_eq!(
        fixture.uses(&items, "skill-alpha"),
        counts_of([4, 3, 1, 1, 1])
    );
    assert_eq!(
        fixture.uses(&items, "skill-beta"),
        counts_of([0, 0, 0, 0, 0])
    );
    assert_eq!(
        fixture.uses(&items, "skill-gamma"),
        counts_of([1, 0, 1, 0, 0])
    );
    assert_eq!(
        fixture.uses(&items, "skill-review"),
        counts_of([2, 1, 1, 0, 0])
    );
    let document = fixture.counts(&items);
    assert_eq!(document["usageTranscripts"], json!(3));
    assert_eq!(document["usageUnreadable"], json!(0));
    assert_eq!(document["usageIngestPending"], json!(false));

    let history = fixture.history(&items);
    assert_eq!(history["ok"], json!(true));
    assert_eq!(history["schemaVersion"], json!(1));
    assert_eq!(history["kind"], json!("skill"));
    assert_eq!(history["coverageStart"], json!(fixture.date(day)));
    assert_eq!(history["ingestPending"], json!(false));
    assert_eq!(
        history["until"],
        json!(Utc::now().date_naive().format("%Y-%m-%d").to_string())
    );
    assert_eq!(history["days"], json!([[fixture.date(day), 7, 4, 3, 1, 1]]));
}

fn filetime_epoch(path: &std::path::Path) {
    let epoch = std::fs::FileTimes::new()
        .set_accessed(std::time::UNIX_EPOCH)
        .set_modified(std::time::UNIX_EPOCH);
    let file = std::fs::OpenOptions::new()
        .write(true)
        .open(path)
        .expect("transcript handle");
    file.set_times(epoch).expect("frozen times");
}

#[test]
fn usage_day_lists_the_skills_used_on_one_local_day() {
    use_utc();
    let fixture = Fixture::new();
    let day = fixture.day;
    let other = day + Duration::days(1);
    fixture.transcript(
        "s1.jsonl",
        &[
            opening(),
            skill_call(day, "toolu_d1", "alpha"),
            typed(day, "u1", &["alpha"], None),
            skill_call(other, "toolu_d2", "beta"),
        ],
    );
    let items = vec![alpha(), beta()];
    fixture.counts(&items);
    let environment = fixture.environment();
    let first =
        fileblade::core_modules::usage::query::skill_day(&environment, &items, &fixture.date(day));
    assert_eq!(first["ok"], json!(true));
    assert_eq!(first["day"], json!(fixture.date(day)));
    assert_eq!(
        first["items"]
            .as_array()
            .expect("items")
            .iter()
            .map(|row| (
                row["name"].clone(),
                row["uses"].clone(),
                row["usesAgent"].clone(),
                row["usesUser"].clone()
            ))
            .collect::<Vec<_>>(),
        vec![(json!("alpha"), json!(2), json!(1), json!(1))]
    );
    let later = fileblade::core_modules::usage::query::skill_day(
        &environment,
        &items,
        &fixture.date(other),
    );
    assert_eq!(later["items"][0]["name"], json!("beta"));
    let refused =
        fileblade::core_modules::usage::query::skill_day(&environment, &items, "yesterday");
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(refused["error"], json!("day must be YYYY-MM-DD"));
}

#[test]
fn usage_counts_answers_for_stubs_without_discovery() {
    use_utc();
    let fixture = Fixture::new();
    let day = fixture.day;
    fixture.transcript(
        "s1.jsonl",
        &[
            opening(),
            skill_call(day, "toolu_c1", "alpha"),
            typed(day, "u1", &["alpha"], None),
        ],
    );
    let items = vec![alpha(), stub("ghost", "ghost", "")];
    let document = fixture.counts(&items);
    assert_eq!(document["ok"], json!(true));
    assert_eq!(
        fixture.uses(&items, "skill-alpha"),
        counts_of([2, 1, 1, 0, 0])
    );
    assert_eq!(document["counts"]["ghost"]["uses"], json!(0));
}

#[test]
fn selected_skill_history_and_day_filter_reuse_loaded_items() {
    use_utc();
    let fixture = Fixture::new();
    let day = fixture.day;
    fixture.transcript(
        "selection.jsonl",
        &[
            opening(),
            skill_call(day, "a", "alpha"),
            skill_call(day, "b", "beta"),
            typed(day, "u", &["alpha"], None),
            skill_call(day, "p", "tools:alpha"),
        ],
    );
    let items = vec![alpha(), beta()];
    fixture.counts(&items);
    let environment = fixture.environment();
    let history = fileblade::core_modules::usage::query::skill_usage(
        &environment,
        std::slice::from_ref(&alpha()),
        true,
    );
    assert_eq!(history["days"], json!([[fixture.date(day), 2, 1, 1, 0, 0]]));
    let plugin = fileblade::core_modules::usage::query::skill_usage(
        &environment,
        &[stub("skill-alpha", "alpha", "plugin:tools@market")],
        true,
    );
    assert_eq!(plugin["days"], json!([[fixture.date(day), 3, 2, 1, 0, 0]]));
    let scoped_day =
        fileblade::core_modules::usage::query::skill_day(&environment, &items, &fixture.date(day));
    let named: Vec<(String, i64)> = scoped_day["items"]
        .as_array()
        .expect("items")
        .iter()
        .map(|row| {
            (
                row["name"].as_str().unwrap_or("").to_string(),
                row["uses"].as_i64().unwrap_or(0),
            )
        })
        .collect();
    assert_eq!(
        named,
        vec![("alpha".to_string(), 2), ("beta".to_string(), 1)]
    );
    let empty = fileblade::core_modules::usage::query::skill_usage(&environment, &[], true);
    assert_eq!(empty["days"], json!([]));
}

#[test]
fn a_complete_transcript_is_never_reopened() {
    use_utc();
    if unsafe { libc::geteuid() } == 0 {
        return;
    }
    let fixture = Fixture::new();
    let day = fixture.day;
    let path = fixture.transcript(
        "s1.jsonl",
        &[opening(), skill_call(day, "toolu_r1", "alpha")],
    );
    let items = vec![alpha()];
    let before = fixture.counts(&items);
    assert_eq!(before["usageUnreadable"], json!(0));
    assert_eq!(before["counts"]["skill-alpha"]["uses"], json!(1));
    set_mode(&path, 0);
    let after = fixture.counts(&items);
    assert_eq!(after["usageUnreadable"], json!(0));
    assert_eq!(after["counts"]["skill-alpha"]["uses"], json!(1));
    set_mode(&path, 0o600);
    assert_eq!(
        fixture.history(&items)["days"],
        json!([[fixture.date(day), 1, 1, 0, 0, 0]])
    );
}

fn set_mode(path: &std::path::Path, mode: u32) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(mode)).expect("mode");
}

#[test]
fn an_unreadable_transcript_is_reported() {
    use_utc();
    if unsafe { libc::geteuid() } == 0 {
        return;
    }
    let fixture = Fixture::new();
    let day = fixture.day;
    let blocked = fixture.transcript("s1.jsonl", &[skill_call(day, "toolu_u1", "alpha")]);
    set_mode(&blocked, 0);
    let items = vec![alpha()];
    let document = fixture.counts(&items);
    assert_eq!(document["usageUnreadable"], json!(1));
    assert_eq!(document["counts"]["skill-alpha"]["uses"], json!(0));
    set_mode(&blocked, 0o600);
}

#[test]
fn an_appended_transcript_is_read_from_where_the_last_read_stopped() {
    use_utc();
    let fixture = Fixture::new();
    let day = fixture.day;
    let items = vec![alpha()];
    let path = fixture.transcript(
        "s1.jsonl",
        &[opening(), skill_call(day, "toolu_a1", "alpha")],
    );
    assert_eq!(
        fixture.counts(&items)["counts"]["skill-alpha"]["usesAgent"],
        json!(1)
    );
    let body = std::fs::read(&path).expect("body");
    let rewritten = String::from_utf8(body)
        .expect("utf-8")
        .replace("toolu_a1", "toolu_b1");
    std::fs::write(&path, rewritten).expect("rewrite");
    fixture.append("s1.jsonl", &[skill_call(day, "toolu_a2", "alpha")]);
    assert_eq!(
        fixture.counts(&items)["counts"]["skill-alpha"]["usesAgent"],
        json!(2)
    );
}

#[test]
fn a_replaced_transcript_is_not_counted_twice() {
    use_utc();
    let fixture = Fixture::new();
    let day = fixture.day;
    let items = vec![alpha()];
    let records: Vec<Value> = (0..3)
        .map(|index| skill_call(day, &format!("toolu_r{index}"), "alpha"))
        .collect();
    let path = fixture.transcript("s1.jsonl", &records[..2]);
    assert_eq!(
        fixture.counts(&items)["counts"]["skill-alpha"]["usesAgent"],
        json!(2)
    );
    let replacement = fixture.transcript("s1.jsonl.new", &records);
    std::fs::rename(replacement, &path).expect("replace");
    assert_eq!(
        fixture.counts(&items)["counts"]["skill-alpha"]["usesAgent"],
        json!(3)
    );
}

#[test]
fn a_half_written_last_line_waits_for_its_newline() {
    use_utc();
    use std::io::Write;
    let fixture = Fixture::new();
    let day = fixture.day;
    let items = vec![alpha()];
    let second = lines(&[skill_call(day, "toolu_h2", "alpha")]);
    let path = fixture.transcript("s1.jsonl", &[skill_call(day, "toolu_h1", "alpha")]);
    let mut handle = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("append handle");
    handle
        .write_all(&second.as_bytes()[..40])
        .expect("partial line");
    drop(handle);
    assert_eq!(
        fixture.counts(&items)["counts"]["skill-alpha"]["usesAgent"],
        json!(1)
    );
    let mut handle = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("append handle");
    handle
        .write_all(&second.as_bytes()[40..])
        .expect("rest of line");
    drop(handle);
    assert_eq!(
        fixture.counts(&items)["counts"]["skill-alpha"]["usesAgent"],
        json!(2)
    );
}

#[test]
fn a_deleted_transcript_keeps_its_uses_and_the_coverage_start() {
    use_utc();
    let fixture = Fixture::new();
    let day = fixture.day;
    let oldest = day - Duration::days(10);
    let items = vec![alpha()];
    let old = fixture.transcript(
        "old.jsonl",
        &[opening(), skill_call(oldest, "toolu_o1", "alpha")],
    );
    fixture.transcript(
        "new.jsonl",
        &[opening(), skill_call(day, "toolu_n1", "alpha")],
    );
    let document = fixture.counts(&items);
    assert_eq!(document["counts"]["skill-alpha"]["uses"], json!(2));
    assert_eq!(document["usageTranscripts"], json!(2));
    assert_eq!(
        fixture.history(&items)["coverageStart"],
        json!(fixture.date(oldest))
    );
    std::fs::remove_file(old).expect("remove");
    fixture.append("new.jsonl", &[skill_call(day, "toolu_n2", "alpha")]);
    let document = fixture.counts(&items);
    assert_eq!(document["counts"]["skill-alpha"]["uses"], json!(3));
    assert_eq!(document["usageTranscripts"], json!(1));
    let history = fixture.history(&items);
    assert_eq!(history["coverageStart"], json!(fixture.date(oldest)));
    assert_eq!(
        history["days"]
            .as_array()
            .expect("days")
            .iter()
            .map(|day| day[0].clone())
            .collect::<Vec<Value>>(),
        vec![json!(fixture.date(oldest)), json!(fixture.date(day))]
    );
}

#[test]
fn forget_removes_history_before_a_day_or_all_of_it() {
    use_utc();
    let fixture = Fixture::new();
    let day = fixture.day;
    let early = day - Duration::days(5);
    let items = vec![alpha()];
    fixture.transcript(
        "s1.jsonl",
        &[
            skill_call(early, "toolu_f1", "alpha"),
            skill_call(day, "toolu_f2", "alpha"),
        ],
    );
    assert_eq!(
        fixture.counts(&items)["counts"]["skill-alpha"]["uses"],
        json!(2)
    );
    let cutoff = fixture.date(day - Duration::days(1));
    assert_eq!(
        fixture.forget(Some(&cutoff)),
        json!({"ok": true, "schemaVersion": 1, "removed": 1})
    );
    let history = fixture.history(&items);
    assert_eq!(history["coverageStart"], json!(cutoff));
    assert_eq!(history["days"], json!([[fixture.date(day), 1, 1, 0, 0, 0]]));
    assert_eq!(
        fixture.counts(&items)["counts"]["skill-alpha"]["uses"],
        json!(1)
    );
    assert_eq!(
        fixture.forget(None),
        json!({"ok": true, "schemaVersion": 1, "removed": 1})
    );
    assert_eq!(
        fixture.counts(&items)["counts"]["skill-alpha"]["uses"],
        json!(0)
    );
    let history = fixture.history(&items);
    assert_eq!(history["coverageStart"], Value::Null);
    assert_eq!(history["days"], json!([]));
    let database = fixture.open_store();
    let events = count_of(&database, "SELECT count(*) FROM event");
    let projects = count_of(&database, "SELECT count(*) FROM project");
    assert_eq!((events, projects), (0, 0));
}

#[test]
fn forgotten_history_stays_forgotten_after_replacement_and_truncation() {
    use_utc();
    let fixture = Fixture::new();
    let day = fixture.day;
    let items = vec![alpha()];
    let old = skill_call(day - Duration::days(5), "old", "alpha");
    let recent = skill_call(day, "recent", "alpha");
    let path = fixture.transcript("session.jsonl", &[old.clone(), recent.clone()]);
    fixture.counts(&items);
    fixture.forget(Some(&fixture.date(day)));
    let replacement = fixture.transcript("replacement", &[old.clone(), recent.clone()]);
    std::fs::rename(replacement, &path).expect("replace");
    assert_eq!(
        fixture.counts(&items)["counts"]["skill-alpha"]["uses"],
        json!(1)
    );
    fixture.forget(None);
    std::fs::write(&path, lines(&[old])).expect("truncate");
    assert_eq!(fixture.history(&items)["days"], json!([]));
    fixture.transcript("unread.jsonl", &[recent]);
    assert_eq!(fixture.history(&items)["days"], json!([]));
    let future = Utc::now() + Duration::seconds(1);
    fixture.append("session.jsonl", &[skill_call(future, "new", "alpha")]);
    assert_eq!(
        fixture.counts(&items)["counts"]["skill-alpha"]["uses"],
        json!(1)
    );
}

#[test]
fn forgetting_a_future_dated_event_does_not_block_new_uses() {
    use_utc();
    let fixture = Fixture::new();
    let items = vec![alpha()];
    let future = Utc::now() + Duration::days(365);
    let path = fixture.transcript("session.jsonl", &[skill_call(future, "bad-clock", "alpha")]);
    fixture.counts(&items);
    fixture.forget(None);
    let replacement =
        fixture.transcript("replacement", &[skill_call(future, "bad-clock", "alpha")]);
    std::fs::rename(replacement, &path).expect("replace");
    assert_eq!(fixture.history(&items)["coverageStart"], Value::Null);
    let fresh = Utc::now() + Duration::seconds(1);
    fixture.append("session.jsonl", &[skill_call(fresh, "new", "alpha")]);
    assert_eq!(
        fixture.counts(&items)["counts"]["skill-alpha"]["uses"],
        json!(1)
    );
}

#[test]
fn failure_records_accept_standard_json_spacing() {
    use_utc();
    use std::io::Write;
    let fixture = Fixture::new();
    let day = fixture.day;
    let items = vec![alpha()];
    let path = fixture.transcript("session.jsonl", &[skill_call(day, "failed", "alpha")]);
    let mut handle = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("append handle");
    let spaced = serde_json::to_string_pretty(&failed(day, "failed"))
        .expect("record")
        .replace('\n', " ");
    writeln!(handle, "{spaced}").expect("failure record");
    drop(handle);
    assert_eq!(
        fixture.counts(&items)["counts"]["skill-alpha"]["failed"],
        json!(1)
    );
}

#[test]
fn failures_survive_newest_first_ingest_across_transcripts() {
    use_utc();
    let fixture = Fixture::new();
    let day = fixture.day;
    let items = vec![alpha()];
    let older = fixture.transcript("original.jsonl", &[skill_call(day, "resumed", "alpha")]);
    filetime_epoch(&older);
    fixture.transcript("resumed.jsonl", &[failed(day, "resumed")]);
    assert_eq!(
        fixture.counts(&items)["counts"]["skill-alpha"]["failed"],
        json!(1)
    );
    fixture.append("resumed.jsonl", &[failed(day, "pending")]);
    fixture.counts(&items);
    fixture.append("original.jsonl", &[skill_call(day, "pending", "alpha")]);
    assert_eq!(
        fixture.counts(&items)["counts"]["skill-alpha"]["failed"],
        json!(2)
    );
}

#[test]
fn oversized_records_do_not_hide_the_next_event() {
    use_utc();
    use std::io::Write;
    let fixture = Fixture::new();
    let day = fixture.day;
    let path = fixture.transcript(
        "large-record.jsonl",
        &[called(day, "first", "mcp__docs__search", json!({}))],
    );
    let mut handle = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("append handle");
    handle
        .write_all(&vec![b'x'; 12 * 1024 * 1024])
        .expect("filler");
    handle.write_all(b"\n").expect("filler newline");
    handle
        .write_all(lines(&[called(day, "last", "mcp__docs__search", json!({}))]).as_bytes())
        .expect("last record");
    drop(handle);
    assert_eq!(
        fixture.mcp_history()["days"],
        json!([[fixture.date(day), 2, 2, 0, 0, 0]])
    );
}

#[test]
fn malformed_deep_record_does_not_block_later_uses() {
    use_utc();
    use std::io::Write;
    let fixture = Fixture::new();
    let day = fixture.day;
    let path = fixture.transcript(
        "session.jsonl",
        &[called(day, "first", "mcp__docs__search", json!({}))],
    );
    let mut handle = std::fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .expect("append handle");
    let deep = format!(
        "{{\"mcp__nested\":{}0{}}}\n",
        "[".repeat(2000),
        "]".repeat(2000)
    );
    handle.write_all(deep.as_bytes()).expect("deep record");
    handle
        .write_all(lines(&[called(day, "last", "mcp__docs__search", json!({}))]).as_bytes())
        .expect("last record");
    drop(handle);
    assert_eq!(
        fixture.mcp_history()["days"],
        json!([[fixture.date(day), 2, 2, 0, 0, 0]])
    );
}

#[test]
fn schema_upgrade_preserves_history_from_deleted_transcripts() {
    use_utc();
    let fixture = Fixture::new();
    let day = fixture.day;
    let path = fixture.transcript(
        "session.jsonl",
        &[called(day, "preserved", "mcp__docs__search", json!({}))],
    );
    let expected = fixture.mcp_history()["days"].clone();
    std::fs::remove_file(path).expect("remove");
    fixture
        .open_store()
        .execute(
            "DROP TABLE IF EXISTS retention; DROP TABLE IF EXISTS failure; \
             DROP TABLE IF EXISTS forgotten; PRAGMA user_version = 1;",
        )
        .expect("downgrade");
    assert_eq!(fixture.mcp_history()["days"], expected);
}

#[test]
fn unknown_schema_is_refused_without_erasing_history() {
    use_utc();
    let fixture = Fixture::new();
    let day = fixture.day;
    fixture.transcript(
        "session.jsonl",
        &[called(day, "preserved", "mcp__docs__search", json!({}))],
    );
    fixture.mcp_history();
    fixture
        .open_store()
        .execute("PRAGMA user_version = 99")
        .expect("unknown schema");
    let refused = fixture.mcp_history();
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(refused["error"], json!("usage store unavailable"));
    let events = count_of(&fixture.open_store(), "SELECT count(*) FROM event");
    assert_eq!(events, 1);
}

#[test]
fn full_forget_keeps_old_codex_project_paths_out_of_replayed_chunks() {
    use_utc();
    let fixture = Fixture::new();
    let sessions = fixture.home.join(".codex/sessions");
    std::fs::create_dir_all(&sessions).expect("sessions");
    let path = sessions.join("session.jsonl");
    let mut body = lines(&[session_meta(fixture.day)]);
    body.push_str(&"x".repeat(12 * 1024 * 1024));
    body.push('\n');
    std::fs::write(&path, &body).expect("rollout");
    fixture.mcp_history();
    fixture.forget(None);
    let replacement = sessions.join("replacement");
    std::fs::write(&replacement, &body).expect("replacement");
    std::fs::rename(replacement, &path).expect("replace");
    assert_eq!(fixture.mcp_history()["days"], json!([]));
    let projects = count_of(&fixture.open_store(), "SELECT count(*) FROM project");
    assert_eq!(projects, 0);
}

#[test]
fn the_store_is_private_and_the_old_cache_is_removed() {
    use_utc();
    let fixture = Fixture::new();
    let cache = fixture.cache.join("omarchy/fileblade");
    std::fs::create_dir_all(&cache).expect("cache");
    for name in ["agent-usage.json", "agent-usage.tmp"] {
        std::fs::write(cache.join(name), "{\"schema\": 1, \"files\": {}}").expect("cache file");
    }
    fixture.counts(&[alpha()]);
    assert_eq!(
        std::fs::read_dir(&cache)
            .expect("cache listing")
            .flatten()
            .count(),
        0
    );
    assert_eq!(mode_of(fixture.store.parent().expect("parent")), 0o700);
    assert_eq!(mode_of(&fixture.store), 0o600);
    assert_eq!(
        mode_of(&fixture.store.with_file_name("agent-usage.sqlite3.lock")),
        0o600
    );
    assert!(!fixture.home.join(".local/state").exists());
}

#[test]
fn forget_commits_while_another_connection_keeps_a_read_snapshot() {
    use_utc();
    let fixture = Fixture::new();
    let day = fixture.day;
    fixture.transcript(
        "session.jsonl",
        &[called(day, "private", "mcp__docs__private_name", json!({}))],
    );
    fixture.mcp_history();
    let mut reader = snapshot_reader(&fixture.store);
    assert_eq!(
        fixture.forget(None),
        json!({"ok": true, "schemaVersion": 1, "removed": 1})
    );
    drop(reader.stdin.take());
    reader.wait().expect("snapshot reader");
    assert_eq!(fixture.mcp_history()["days"], json!([]));
    let bytes = std::fs::read(&fixture.store).expect("store bytes");
    assert!(
        !bytes
            .windows(b"private_name".len())
            .any(|window| window == b"private_name")
    );
}

#[test]
fn concurrent_readers_ingest_once_and_agree() {
    use_utc();
    let fixture = Fixture::new();
    let day = fixture.day;
    assert_eq!(fixture.mcp_history()["days"], json!([]));
    fixture
        .open_store()
        .execute(
            "CREATE TABLE ingest_log (path TEXT NOT NULL);\
             CREATE TRIGGER source_inserted AFTER INSERT ON source BEGIN INSERT INTO ingest_log VALUES (NEW.path); END;\
             CREATE TRIGGER source_updated AFTER UPDATE ON source BEGIN INSERT INTO ingest_log VALUES (NEW.path); END;",
        )
        .expect("ingest log");
    for session in 0..24 {
        let records: Vec<Value> = std::iter::once(opening())
            .chain((0..40).map(|call| {
                called(
                    day,
                    &format!("toolu_{session}_{call}"),
                    &format!("mcp__server{call}__tool"),
                    json!({}),
                )
            }))
            .collect();
        fixture.transcript(&format!("s{session}.jsonl"), &records);
    }
    let answers: Vec<Value> = std::thread::scope(|scope| {
        let handles: Vec<_> = (0..4)
            .map(|_| {
                let environment = fixture.environment();
                scope.spawn(move || fileblade::core_modules::usage::query::mcp_usage(&environment))
            })
            .collect();
        handles
            .into_iter()
            .map(|handle| handle.join().expect("reader"))
            .collect()
    });
    for answer in &answers {
        assert_eq!(answer, &answers[0]);
    }
    assert_eq!(answers[0]["ingestPending"], json!(false));
    assert_eq!(
        answers[0]["days"],
        json!([[fixture.date(day), 960, 960, 0, 0, 0]])
    );
    let row = fixture
        .open_store()
        .query_one("SELECT count(DISTINCT path), count(*) FROM ingest_log")
        .expect("ingest log rows")
        .expect("ingest log row");
    let writes: (i64, i64) = (row.integer(0), row.integer(1));
    assert_eq!(writes, (24, 24));
}

#[test]
fn a_nul_byte_in_a_record_does_not_break_the_store() {
    use_utc();
    let fixture = Fixture::new();
    let day = fixture.day;
    fixture.transcript(
        "nul.jsonl",
        &[
            opening(),
            called(
                day,
                "toolu_n\u{0}1",
                "Skill",
                json!({"skill": "alpha\u{0}"}),
            ),
            skill_call(day, "toolu_n2", "alpha"),
        ],
    );

    let items = vec![alpha(), beta()];
    let document = fixture.counts(&items);
    assert_eq!(document["ok"], json!(true));
    assert_eq!(document["counts"]["skill-alpha"]["uses"], json!(2));
    let row = fixture
        .open_store()
        .query_one("SELECT count(*) FROM event")
        .expect("event rows")
        .expect("event row");
    assert_eq!(row.integer(0), 2);
}
