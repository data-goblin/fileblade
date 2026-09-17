#[path = "common/usage_fixtures.rs"]
mod usage_fixtures;

use chrono::{DateTime, Duration, TimeZone, Utc};
use serde_json::{Value, json};
use usage_fixtures::*;

const BASELINE: &str = "tests/golden/python-baseline/usage";
const IDENTITIES: [&str; 9] = [
    "6a8087c3e250d97a3c2ef1b3213026b01b7bdbca291f5ec0d4bbdf7b8a9fc1bb",
    "c8ee0f54dfc35e823b734a9257d3eea617f53d751ea181ef1d3c90a7654c882f",
    "bef9fc97e47ccaa8befc0a8e8c147978a2cce737d9fb7e1497a27d88ce84ce32",
    "ffdf67967bfe502b203c4a43be7934af4072115135024950b6b5271f381669d1",
    "675d54c882065a6db66429694e2a4c3f8edea6d42b89a888f204fffa9efe1823",
    "d8ab63a1b59725ee59540632da38318b800201039284efaebf57af2c2d22bc64",
    "8703b21c0fb19b2cc590720455e84adc26986d2bc071d4695c927bb9458f3d8a",
    "f449cadb2729eeacfd6f8820d71f7e61c25d353cd756b2d1a09a3d843123f909",
    "3ada0a6e82341d193084a1596e08468e0bd989b3040384321ca727ac3cf1887b",
];

fn frozen_day() -> DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 9, 10, 12, 0, 0)
        .single()
        .expect("frozen day")
}

fn baseline(name: &str) -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join(BASELINE)
        .join(name)
}

fn build_transcript(fixture: &Fixture) {
    let mut records = Vec::new();
    for (index, offset) in [0i64, 1, 2].into_iter().enumerate() {
        let at = frozen_day() + Duration::days(offset);
        records.push(skill_call(at, &format!("t{index}0"), "pdf"));
        records.push(called(
            at + Duration::minutes(1),
            &format!("t{index}1"),
            "mcp__fileblade__list",
            json!({}),
        ));
        records.push(typed(
            at + Duration::minutes(2),
            &format!("u{index}"),
            &["review"],
            None,
        ));
    }
    fixture.transcript("session.jsonl", &records);
}

fn items() -> Vec<Value> {
    vec![
        stub("skill-pdf", "pdf", "user"),
        stub("skill-review", "review", "user"),
    ]
}

fn expected() -> Value {
    let text = std::fs::read_to_string(baseline("queries.json")).expect("golden queries");
    let today = Utc::now().date_naive().format("%Y-%m-%d").to_string();
    serde_json::from_str(&text.replace("{TODAY}", &today)).expect("golden document")
}

fn answers(fixture: &Fixture) -> Value {
    let skills = items();
    let environment = fixture.environment();
    json!({
        "skillCounts": fileblade::core_modules::usage::query::skill_counts(&environment, &skills),
        "skillUsage": fileblade::core_modules::usage::query::skill_usage(&environment, &skills, false),
        "mcpUsage": fileblade::core_modules::usage::query::mcp_usage(&environment),
    })
}

#[test]
fn the_python_written_store_answers_identically() {
    use_utc();
    let fixture = Fixture::new();
    build_transcript(&fixture);
    let directory = fixture.store.parent().expect("store directory");
    std::fs::create_dir_all(directory).expect("store directory");
    std::fs::copy(baseline("agent-usage.sqlite3"), &fixture.store).expect("baseline store");
    assert_eq!(answers(&fixture), expected());
}

#[test]
fn a_fresh_ingest_reproduces_the_recorded_rows_and_identities() {
    use_utc();
    let fixture = Fixture::new();
    build_transcript(&fixture);
    assert_eq!(answers(&fixture), expected());

    let dump: Value = serde_json::from_str(
        &std::fs::read_to_string(baseline("store-dump.json")).expect("golden dump"),
    )
    .expect("golden dump document");
    let connection = fixture.open_store();
    let version: i64 = connection
        .query_row("PRAGMA user_version", [], |row| row.get(0))
        .expect("user version");
    assert_eq!(json!(version), dump["userVersion"]);
    let mut statement = connection
        .prepare(
            "SELECT agent, call, at, kind, origin, server, name, subagent, failed FROM event \
             ORDER BY agent, call",
        )
        .expect("event query");
    let rows: Vec<Value> = statement
        .query_map([], |row| {
            Ok(json!([
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
                row.get::<_, String>(4)?,
                row.get::<_, String>(5)?,
                row.get::<_, String>(6)?,
                row.get::<_, i64>(7)?,
                row.get::<_, i64>(8)?,
            ]))
        })
        .expect("event rows")
        .collect::<rusqlite::Result<Vec<Value>>>()
        .expect("event rows");
    assert_eq!(Value::Array(rows.clone()), dump["events"]);

    let mut statement = connection
        .prepare("SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")
        .expect("table query");
    let tables: Vec<String> = statement
        .query_map([], |row| row.get(0))
        .expect("tables")
        .collect::<rusqlite::Result<Vec<String>>>()
        .expect("tables");
    let mut recorded: Vec<String> = dump["tables"]
        .as_object()
        .expect("tables")
        .keys()
        .cloned()
        .collect();
    recorded.sort();
    assert_eq!(tables, recorded);
    for table in &tables {
        let mut statement = connection
            .prepare(&format!("PRAGMA table_info({table})"))
            .expect("table info");
        let columns: Vec<String> = statement
            .query_map([], |row| row.get(1))
            .expect("columns")
            .collect::<rusqlite::Result<Vec<String>>>()
            .expect("columns");
        assert_eq!(
            Value::from(columns),
            dump["tables"][table]["columns"],
            "{table} columns"
        );
    }

    let digests: Vec<String> = rows
        .iter()
        .map(|row| {
            let agent = row[0].as_str().unwrap_or("");
            let call = row[1].as_str().unwrap_or("");
            fileblade::core_modules::usage::store::identity(agent, call)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        })
        .collect();
    assert_eq!(digests, IDENTITIES.map(str::to_string).to_vec());
}
