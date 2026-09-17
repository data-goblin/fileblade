use super::sql::{self, Bound, Sql, literal};
use super::store::{self, Environment, MCP_AGENTS, SKILL_AGENTS, Session, identity};
use serde_json::{Map, Value, json};
use std::collections::{BTreeMap, HashMap};
use std::sync::OnceLock;

pub const SCHEMA_VERSION: i64 = 1;
pub const MAX_OBSERVED: usize = 64;
pub const WINDOW_DAYS: i64 = 160 * 7;
pub const UNAVAILABLE: &str = "usage store unavailable";
const MCP_EVENTS: &str = "(kind IN ('tool', 'resource', 'resource-list') OR (agent = 'claude' AND kind = 'command' AND name GLOB 'mcp__?*__?*'))";

fn definition_agent(agent: &str) -> Option<&'static str> {
    match agent {
        "claude" => Some("claude"),
        "codex" => Some("codex"),
        "opencode" => Some("opencode"),
        "github-copilot-cli" => Some("copilot"),
        _ => None,
    }
}

pub fn sanitize(name: &str) -> String {
    static NON_WORD: OnceLock<regex::Regex> = OnceLock::new();
    static RUNS: OnceLock<regex::Regex> = OnceLock::new();
    let non_word =
        NON_WORD.get_or_init(|| regex::Regex::new(r"[^a-zA-Z0-9_-]").expect("valid pattern"));
    let value = non_word.replace_all(name, "_").to_string();
    if !name.starts_with("claude.ai ") {
        return value;
    }
    let runs = RUNS.get_or_init(|| regex::Regex::new(r"_+").expect("valid pattern"));
    runs.replace_all(&value, "_").trim_matches('_').to_string()
}

fn field(item: &Value, key: &str) -> String {
    match item.get(key) {
        Some(Value::String(value)) => value.clone(),
        Some(Value::Null) | None => String::new(),
        Some(Value::Bool(false)) => String::new(),
        Some(other) => other.to_string(),
    }
}

pub fn skill_names(item: &Value) -> Vec<String> {
    let name = field(item, "name");
    let source = field(item, "source");
    let mut names = vec![name.clone()];
    if let Some(rest) = source.strip_prefix("plugin:") {
        let plugin = rest.split('@').next().unwrap_or("");
        let qualified = format!("{plugin}:{name}");
        if qualified != name {
            names.push(qualified);
        }
    }
    names
}

fn counts(agent: i64, user: i64, scheduled: i64, failed: i64) -> Map<String, Value> {
    let mut values = Map::new();
    values.insert("uses".to_string(), json!(agent + user));
    values.insert("usesAgent".to_string(), json!(agent));
    values.insert("usesUser".to_string(), json!(user));
    values.insert("usesScheduled".to_string(), json!(scheduled));
    values.insert("failed".to_string(), json!(failed));
    values
}

fn attach(item: &mut Value, values: &Map<String, Value>) {
    let Some(entries) = item.as_object_mut() else {
        return;
    };
    for (key, value) in values {
        entries.insert(key.clone(), value.clone());
    }
    let metrics = entries
        .entry("metrics".to_string())
        .or_insert_with(|| Value::Object(Map::new()));
    if !metrics.is_object() {
        *metrics = Value::Object(Map::new());
    }
    if let Some(metrics) = metrics.as_object_mut() {
        for (key, value) in values {
            metrics.insert(key.clone(), value.clone());
        }
    }
}

fn agent_list(agents: &[&str]) -> String {
    serde_json::to_string(agents).unwrap_or_else(|_| "[]".to_string())
}

fn sources(database: &Sql, agents: &[&str]) -> sql::Result<i64> {
    let statement = sql::bind(
        "SELECT count(*) FROM source WHERE agent IN (SELECT value FROM json_each(?))",
        &[Bound::Text(agent_list(agents))],
    )?;
    Ok(database
        .query_one(&statement)?
        .map_or(0, |row| row.integer(0)))
}

type Tallies = HashMap<String, [i64; 4]>;

fn skill_tallies(database: &Sql, where_clause: &str, parameters: &[Bound]) -> sql::Result<Tallies> {
    let statement = sql::bind(
        &format!(
            "SELECT kind, name, origin, count(*), sum(failed) FROM event WHERE kind IN ('skill', 'command') \
             {where_clause} GROUP BY kind, name, origin"
        ),
        parameters,
    )?;
    let mut tallies: Tallies = HashMap::new();
    for row in database.query(&statement)? {
        let (kind, name, origin, total, failed) = (
            row.text(0),
            row.text(1),
            row.text(2),
            row.integer(3),
            row.integer(4),
        );
        let tally = tallies.entry(name).or_insert([0; 4]);
        let slot = if kind == "skill" {
            0
        } else if origin == "user" {
            1
        } else {
            2
        };
        tally[slot] += total;
        if kind == "skill" {
            tally[3] += failed;
        }
    }
    Ok(tallies)
}

fn item_counts(item: &Value, tallies: &Tallies) -> Map<String, Value> {
    let mut total = [0i64; 4];
    let mut seen: Vec<String> = Vec::new();
    for name in skill_names(item) {
        if seen.contains(&name) {
            continue;
        }
        let tally = tallies.get(&name).copied().unwrap_or([0; 4]);
        for slot in 0..4 {
            total[slot] += tally[slot];
        }
        seen.push(name);
    }
    counts(total[0], total[1], total[2], total[3])
}

fn unavailable_extra() -> Map<String, Value> {
    let mut document = Map::new();
    document.insert("usageError".to_string(), Value::from(UNAVAILABLE));
    document
}

pub fn attach_skills(environment: &Environment, items: &mut [Value]) -> Map<String, Value> {
    match attached_skills(environment, items) {
        Ok(document) => document,
        Err(_) => unavailable_extra(),
    }
}

fn attached_skills(
    environment: &Environment,
    items: &mut [Value],
) -> sql::Result<Map<String, Value>> {
    let session = store::session(environment, true)?;
    let tallies = skill_tallies(&session.database, "", &[])?;
    for item in items.iter_mut() {
        let values = item_counts(item, &tallies);
        attach(item, &values);
    }
    let transcripts = sources(&session.database, &SKILL_AGENTS)?;
    let mut document = Map::new();
    document.insert("usageTranscripts".to_string(), json!(transcripts));
    document.insert("usageUnreadable".to_string(), json!(session.unreadable));
    document.insert("usageIngestPending".to_string(), json!(session.pending));
    Ok(document)
}

pub fn skill_counts(environment: &Environment, items: &[Value]) -> Value {
    match counted_skills(environment, items) {
        Ok(document) => document,
        Err(_) => json!({"ok": false, "schemaVersion": SCHEMA_VERSION, "error": UNAVAILABLE}),
    }
}

fn counted_skills(environment: &Environment, items: &[Value]) -> sql::Result<Value> {
    let session = store::session(environment, true)?;
    let tallies = skill_tallies(&session.database, "", &[])?;
    let mut counted = Map::new();
    for item in items {
        let id = field(item, "id");
        if id.is_empty() {
            continue;
        }
        counted.insert(id, Value::Object(item_counts(item, &tallies)));
    }
    let transcripts = sources(&session.database, &SKILL_AGENTS)?;
    Ok(json!({
        "ok": true,
        "schemaVersion": SCHEMA_VERSION,
        "counts": Value::Object(counted),
        "usageTranscripts": transcripts,
        "usageUnreadable": session.unreadable,
        "usageIngestPending": session.pending,
    }))
}

fn valid_day(day: &str) -> bool {
    day.len() == 10
        && day.as_bytes()[4] == b'-'
        && day.as_bytes()[7] == b'-'
        && day
            .bytes()
            .enumerate()
            .all(|(index, byte)| index == 4 || index == 7 || byte.is_ascii_digit())
}

pub fn skill_day(environment: &Environment, items: &[Value], day: &str) -> Value {
    if !valid_day(day) {
        return json!({"ok": false, "schemaVersion": SCHEMA_VERSION, "error": "day must be YYYY-MM-DD"});
    }
    match day_of_skills(environment, items, day) {
        Ok(document) => document,
        Err(_) => json!({"ok": false, "schemaVersion": SCHEMA_VERSION, "error": UNAVAILABLE}),
    }
}

fn day_of_skills(environment: &Environment, items: &[Value], day: &str) -> sql::Result<Value> {
    let session = store::session(environment, false)?;
    let tallies = skill_tallies(
        &session.database,
        "AND at >= CAST(strftime('%s', ?, 'utc') AS INTEGER) * 1000 \
         AND at < CAST(strftime('%s', ?, '+1 day', 'utc') AS INTEGER) * 1000",
        &[Bound::from(day), Bound::from(day)],
    )?;
    let mut used: Vec<(i64, String, Value)> = Vec::new();
    for item in items {
        let values = item_counts(item, &tallies);
        let uses = values.get("uses").and_then(Value::as_i64).unwrap_or(0);
        let scheduled = values
            .get("usesScheduled")
            .and_then(Value::as_i64)
            .unwrap_or(0);
        if uses <= 0 && scheduled <= 0 {
            continue;
        }
        let name = field(item, "name");
        let mut row = Map::new();
        row.insert("id".to_string(), Value::from(field(item, "id")));
        row.insert("name".to_string(), Value::from(name.clone()));
        for (key, value) in &values {
            row.insert(key.clone(), value.clone());
        }
        used.push((uses, name, Value::Object(row)));
    }
    used.sort_by(|left, right| right.0.cmp(&left.0).then_with(|| left.1.cmp(&right.1)));
    let rows: Vec<Value> = used.into_iter().map(|(_, _, row)| row).collect();
    Ok(json!({
        "ok": true,
        "schemaVersion": SCHEMA_VERSION,
        "day": day,
        "items": rows,
    }))
}

fn opencode_owner(name: &str, prefixes: &[String]) -> (String, String) {
    for prefix in prefixes {
        let head = format!("{prefix}_");
        if name.starts_with(&head) && name.len() > head.len() {
            return (prefix.clone(), name[head.len()..].to_string());
        }
    }
    (String::new(), name.to_string())
}

fn mcp_server(item: &Value) -> Option<(String, String)> {
    let name = field(item, "name");
    let agent = definition_agent(&field(item, "agent"))?;
    if agent == "opencode" {
        return Some((agent.to_string(), sanitize(&name)));
    }
    if agent != "claude" {
        return Some((agent.to_string(), name));
    }
    if field(item, "scope") != "plugin" {
        return Some((agent.to_string(), sanitize(&name)));
    }
    let plugin = match item.get("source") {
        Some(source @ Value::Object(_)) => field(source, "plugin"),
        _ => String::new(),
    };
    (!plugin.is_empty()).then(|| {
        (
            agent.to_string(),
            format!("plugin_{}_{}", sanitize(&plugin), sanitize(&name)),
        )
    })
}

#[derive(Default, Clone)]
struct Observation {
    agent: i64,
    user: i64,
    scheduled: i64,
    failed: i64,
    last: String,
}

pub fn attach_mcp(environment: &Environment, definitions: &mut [Value]) -> Map<String, Value> {
    match attached_mcp(environment, definitions) {
        Ok(document) => document,
        Err(_) => unavailable_extra(),
    }
}

fn attached_mcp(
    environment: &Environment,
    definitions: &mut [Value],
) -> sql::Result<Map<String, Value>> {
    let session = store::session(environment, true)?;
    let keys: Vec<Option<(String, String)>> = definitions.iter().map(mcp_server).collect();
    let mut prefixes: Vec<String> = keys
        .iter()
        .flatten()
        .filter(|(agent, _)| agent == "opencode")
        .map(|(_, server)| server.clone())
        .collect();
    prefixes.sort();
    prefixes.dedup();
    prefixes.sort_by(|left, right| right.len().cmp(&left.len()).then_with(|| left.cmp(right)));
    let mut servers: HashMap<(String, String), BTreeMap<(String, String), Observation>> =
        HashMap::new();
    {
        let statement = format!(
            "SELECT agent, server, kind, name, origin, count(*), sum(failed), \
             date(max(at) / 1000, 'unixepoch', 'localtime') FROM event WHERE {MCP_EVENTS} \
             GROUP BY agent, server, kind, name, origin"
        );
        for row in session.database.query(&statement)? {
            let (agent, mut server, mut kind, mut name, origin, total, failed, last) = (
                row.text(0),
                row.text(1),
                row.text(2),
                row.text(3),
                row.text(4),
                row.integer(5),
                row.integer(6),
                row.text(7),
            );
            if kind == "command" {
                let mut parts = name.splitn(3, "__");
                parts.next();
                server = parts.next().unwrap_or("").to_string();
                name = parts.next().unwrap_or("").to_string();
                kind = "prompt".to_string();
            }
            if agent == "claude" {
                server = sanitize(&server);
            } else if agent == "opencode" && server.is_empty() {
                let (owner, rest) = opencode_owner(&name, &prefixes);
                server = owner;
                name = rest;
            }
            let entry = servers
                .entry((agent, server))
                .or_default()
                .entry((kind.clone(), name))
                .or_default();
            if kind != "prompt" {
                entry.agent += total;
            } else if origin == "user" {
                entry.user += total;
            } else {
                entry.scheduled += total;
            }
            entry.failed += failed;
            if last > entry.last {
                entry.last = last;
            }
        }
    }
    let mut owners: HashMap<&(String, String), usize> = HashMap::new();
    for key in keys.iter().flatten() {
        *owners.entry(key).or_insert(0) += 1;
    }
    let mut ambiguous = 0i64;
    for (item, key) in definitions.iter_mut().zip(keys.iter()) {
        let mut observed = key
            .as_ref()
            .and_then(|key| servers.get(key))
            .cloned()
            .unwrap_or_default();
        if let Some(key) = key
            && owners.get(key).copied().unwrap_or(0) > 1
        {
            ambiguous += 1;
            if let Some(entries) = item.as_object_mut() {
                entries.insert("usageAmbiguous".to_string(), Value::Bool(true));
            }
            observed.clear();
        }
        let totals = observed.values().fold([0i64; 4], |mut total, entry| {
            total[0] += entry.agent;
            total[1] += entry.user;
            total[2] += entry.scheduled;
            total[3] += entry.failed;
            total
        });
        let values = counts(totals[0], totals[1], totals[2], totals[3]);
        attach(item, &values);
        let mut ranked: Vec<((String, String), Observation)> = observed.into_iter().collect();
        ranked.sort_by(|left, right| {
            let left_uses = -(left.1.agent + left.1.user);
            let right_uses = -(right.1.agent + right.1.user);
            left_uses
                .cmp(&right_uses)
                .then_with(|| left.0.1.cmp(&right.0.1))
                .then_with(|| left.0.0.cmp(&right.0.0))
        });
        let rows: Vec<Value> = ranked
            .into_iter()
            .take(MAX_OBSERVED)
            .map(|((kind, name), entry)| {
                json!({
                    "kind": kind,
                    "name": name,
                    "uses": entry.agent + entry.user,
                    "failed": entry.failed,
                    "lastUsed": entry.last,
                })
            })
            .collect();
        if let Some(entries) = item.as_object_mut() {
            entries.insert("observed".to_string(), Value::Array(rows));
        }
    }
    let transcripts = sources(&session.database, &MCP_AGENTS)?;
    let mut document = Map::new();
    document.insert("usageTranscripts".to_string(), json!(transcripts));
    document.insert("usageUnreadable".to_string(), json!(session.unreadable));
    document.insert("usageIngestPending".to_string(), json!(session.pending));
    document.insert("usageAmbiguous".to_string(), json!(ambiguous));
    Ok(document)
}

fn history(
    environment: &Environment,
    kind: &str,
    agents: &[&str],
    where_clause: &str,
    parameters: &[Bound],
    ingest: bool,
) -> Value {
    match tallied_history(environment, kind, agents, where_clause, parameters, ingest) {
        Ok(document) => document,
        Err(_) => {
            json!({"ok": false, "schemaVersion": SCHEMA_VERSION, "kind": kind, "error": UNAVAILABLE})
        }
    }
}

fn tallied_history(
    environment: &Environment,
    kind: &str,
    agents: &[&str],
    where_clause: &str,
    parameters: &[Bound],
    ingest: bool,
) -> sql::Result<Value> {
    let session: Session = store::session(environment, ingest)?;
    let coverage = sql::bind(
        "SELECT date(min(first_at) / 1000, 'unixepoch', 'localtime'), date('now', 'localtime') FROM coverage \
         WHERE agent IN (SELECT value FROM json_each(?))",
        &[Bound::Text(agent_list(agents))],
    )?;
    let (start, until): (Option<String>, Option<String>) = session
        .database
        .query_one(&coverage)?
        .map_or((None, None), |row| {
            (row.optional_text(0), row.optional_text(1))
        });
    let statement = sql::bind(
        &format!(
            "SELECT date(at / 1000, 'unixepoch', 'localtime') AS day, sum(kind <> 'command' OR origin = 'user'), \
             sum(kind <> 'command'), sum(kind = 'command' AND origin = 'user'), \
             sum(kind = 'command' AND origin = 'scheduled'), sum(failed) \
             FROM event WHERE {where_clause} AND day > date('now', 'localtime', '-{WINDOW_DAYS} days') \
             AND day <= date('now', 'localtime') GROUP BY day ORDER BY day"
        ),
        parameters,
    )?;
    let days: Vec<Value> = session
        .database
        .query(&statement)?
        .iter()
        .map(|row| {
            json!([
                row.text(0),
                row.integer(1),
                row.integer(2),
                row.integer(3),
                row.integer(4),
                row.integer(5),
            ])
        })
        .collect();
    Ok(json!({
        "ok": true,
        "schemaVersion": SCHEMA_VERSION,
        "kind": kind,
        "coverageStart": start,
        "until": until,
        "ingestPending": session.pending,
        "days": days,
    }))
}

pub fn skill_usage(environment: &Environment, items: &[Value], scoped: bool) -> Value {
    let mut names: Vec<String> = items.iter().flat_map(skill_names).collect();
    names.sort();
    names.dedup();
    let encoded = serde_json::to_string(&names).unwrap_or_else(|_| "[]".to_string());
    if scoped {
        return history(
            environment,
            "skill",
            &SKILL_AGENTS,
            "kind IN ('skill', 'command') AND name IN (SELECT value FROM json_each(?))",
            &[Bound::Text(encoded)],
            false,
        );
    }
    history(
        environment,
        "skill",
        &SKILL_AGENTS,
        "(kind = 'skill' OR (kind = 'command' AND name IN (SELECT value FROM json_each(?))))",
        &[Bound::Text(encoded)],
        true,
    )
}

pub fn mcp_usage(environment: &Environment) -> Value {
    history(environment, "mcp", &MCP_AGENTS, MCP_EVENTS, &[], true)
}

pub fn forget(environment: &Environment, before: Option<&str>) -> Value {
    match forgotten(environment, before) {
        Ok(document) => document,
        Err(_) => {
            json!({"ok": false, "schemaVersion": SCHEMA_VERSION, "removed": 0, "error": UNAVAILABLE})
        }
    }
}

fn forgotten(environment: &Environment, before: Option<&str>) -> sql::Result<Value> {
    let session = store::session(environment, false)?;
    let database = &session.database;
    let (cutoff, prelude, removal) = match before {
        Some(before) => {
            let statement = sql::bind(
                "SELECT CAST(strftime('%s', ?, 'utc') AS INTEGER) * 1000",
                &[Bound::from(before)],
            )?;
            let cutoff = database
                .query_one(&statement)?
                .and_then(|row| row.optional_integer(0))
                .ok_or_else(|| sql::Error::new("usage cutoff is not a day"))?;
            let value = literal(&Bound::Integer(cutoff));
            (
                cutoff,
                format!(
                    "DELETE FROM failure WHERE at < {value};\n\
                     UPDATE coverage SET first_at = max(first_at, {value});\n"
                ),
                format!("DELETE FROM event WHERE at < {value};\n"),
            )
        }
        None => {
            let cutoff = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|elapsed| elapsed.as_millis() as i64)
                .unwrap_or(0)
                + 1;
            let statement = sql::bind(
                "SELECT agent, call FROM event WHERE at >= ? UNION \
                 SELECT agent, call FROM failure WHERE at >= ?",
                &[Bound::Integer(cutoff), Bound::Integer(cutoff)],
            )?;
            let mut prelude = String::new();
            for row in database.query(&statement)? {
                prelude.push_str(&format!(
                    "INSERT OR IGNORE INTO forgotten VALUES ({});\n",
                    literal(&Bound::Blob(identity(&row.text(0), &row.text(1))))
                ));
            }
            prelude.push_str(
                "DELETE FROM failure;\nDELETE FROM coverage;\n\
                 UPDATE source SET project = NULL;\nDELETE FROM project;\n",
            );
            (cutoff, prelude, "DELETE FROM event;\n".to_string())
        }
    };
    let script = format!(
        "PRAGMA secure_delete = ON;\nBEGIN IMMEDIATE;\n{prelude}\
         INSERT INTO retention VALUES (1, {}) ON CONFLICT (id) DO UPDATE SET before = max(before, excluded.before);\n\
         {removal}SELECT changes();\nCOMMIT;\n",
        literal(&Bound::Integer(cutoff))
    );
    let removed = database.query_one(&script)?.map_or(0, |row| row.integer(0));
    Ok(json!({"ok": true, "schemaVersion": SCHEMA_VERSION, "removed": removed}))
}
