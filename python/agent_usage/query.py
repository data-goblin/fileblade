from __future__ import annotations

import json
import re
import sqlite3
import time
from collections import Counter
from typing import Any

from .store import MCP_AGENTS, SKILL_AGENTS, identity, session

SCHEMA_VERSION = 1
MAX_OBSERVED = 64
WINDOW_DAYS = 160 * 7
UNAVAILABLE = "usage store unavailable"
MCP_EVENTS = "(kind IN ('tool', 'resource', 'resource-list') OR (agent = 'claude' AND kind = 'command' AND name GLOB 'mcp__?*__?*'))"
MCP_DEFINITION_AGENTS = {"claude": "claude", "codex": "codex", "opencode": "opencode", "github-copilot-cli": "copilot"}


def sanitize(name: str) -> str:
    value = re.sub(r"[^a-zA-Z0-9_-]", "_", name)
    return re.sub(r"_+", "_", value).strip("_") if name.startswith("claude.ai ") else value


def skill_names(item: dict[str, Any]) -> set[str]:
    name = str(item.get("name") or "")
    source = str(item.get("source") or "")
    return {name, source[len("plugin:"):].split("@", 1)[0] + ":" + name} if source.startswith("plugin:") else {name}


def mcp_server(item: dict[str, Any]) -> tuple[str, str] | None:
    name = str(item.get("name") or "")
    agent = MCP_DEFINITION_AGENTS.get(str(item.get("agent") or ""))
    if agent is None:
        return None
    if agent == "opencode":
        return agent, sanitize(name)
    if agent != "claude":
        return agent, name
    if item.get("scope") != "plugin":
        return agent, sanitize(name)
    plugin = str((item.get("source") or {}).get("plugin") or "")
    return (agent, f"plugin_{sanitize(plugin)}_{sanitize(name)}") if plugin else None


def counts(agent: int, user: int, scheduled: int, failed: int) -> dict[str, int]:
    return {"uses": agent + user, "usesAgent": agent, "usesUser": user, "usesScheduled": scheduled, "failed": failed}


def attach(item: dict[str, Any], values: dict[str, int]) -> None:
    item.update(values)
    item.setdefault("metrics", {}).update(values)


def sources(connection: sqlite3.Connection, agents: tuple[str, ...]) -> int:
    return connection.execute("SELECT count(*) FROM source WHERE agent IN (SELECT value FROM json_each(?))",
                              (json.dumps(agents),)).fetchone()[0]


def skill_tallies(connection: sqlite3.Connection, where: str = "", parameters: tuple[Any, ...] = ()) -> dict[str, list[int]]:
    tallies: dict[str, list[int]] = {}
    for kind, name, origin, total, failed in connection.execute(
            "SELECT kind, name, origin, count(*), sum(failed) FROM event WHERE kind IN ('skill', 'command') "
            f"{where} GROUP BY kind, name, origin", parameters):
        tally = tallies.setdefault(name, [0, 0, 0, 0])
        tally[0 if kind == "skill" else 1 if origin == "user" else 2] += total
        tally[3] += failed if kind == "skill" else 0
    return tallies


def item_counts(item: dict[str, Any], tallies: dict[str, list[int]]) -> dict[str, int]:
    return counts(*(sum(column) for column in zip(*(tallies.get(name, [0, 0, 0, 0]) for name in skill_names(item)))))


def attach_skills(items: list[dict[str, Any]]) -> dict[str, Any]:
    try:
        with session() as (connection, (pending, unreadable)):
            tallies = skill_tallies(connection)
            for item in items:
                attach(item, item_counts(item, tallies))
            return {"usageTranscripts": sources(connection, SKILL_AGENTS), "usageUnreadable": unreadable, "usageIngestPending": pending}
    except (OSError, sqlite3.Error):
        return {"usageError": UNAVAILABLE}


def skill_counts(items: list[dict[str, Any]]) -> dict[str, Any]:
    try:
        with session() as (connection, (pending, unreadable)):
            tallies = skill_tallies(connection)
            return {"ok": True, "schemaVersion": SCHEMA_VERSION,
                    "counts": {str(item.get("id") or ""): item_counts(item, tallies) for item in items if item.get("id")},
                    "usageTranscripts": sources(connection, SKILL_AGENTS), "usageUnreadable": unreadable, "usageIngestPending": pending}
    except (OSError, sqlite3.Error):
        return {"ok": False, "schemaVersion": SCHEMA_VERSION, "error": UNAVAILABLE}


def skill_day(items: list[dict[str, Any]], day: str) -> dict[str, Any]:
    if not re.fullmatch(r"\d{4}-\d{2}-\d{2}", day):
        return {"ok": False, "schemaVersion": SCHEMA_VERSION, "error": "day must be YYYY-MM-DD"}
    try:
        with session(ingest_history=False) as (connection, _):
            tallies = skill_tallies(connection,
                                    "AND at >= CAST(strftime('%s', ?, 'utc') AS INTEGER) * 1000 "
                                    "AND at < CAST(strftime('%s', ?, '+1 day', 'utc') AS INTEGER) * 1000", (day, day))
            used = []
            for item in items:
                values = item_counts(item, tallies)
                if values["uses"] > 0 or values["usesScheduled"] > 0:
                    used.append({"id": str(item.get("id") or ""), "name": str(item.get("name") or ""), **values})
            used.sort(key=lambda entry: (-entry["uses"], entry["name"]))
            return {"ok": True, "schemaVersion": SCHEMA_VERSION, "day": day, "items": used}
    except (OSError, sqlite3.Error):
        return {"ok": False, "schemaVersion": SCHEMA_VERSION, "error": UNAVAILABLE}


def opencode_owner(name: str, prefixes: list[str]) -> tuple[str, str]:
    for prefix in prefixes:
        if name.startswith(prefix + "_") and len(name) > len(prefix) + 1:
            return prefix, name[len(prefix) + 1:]
    return "", name


def attach_mcp(definitions: list[dict[str, Any]]) -> dict[str, Any]:
    try:
        with session() as (connection, (pending, unreadable)):
            keys = [mcp_server(item) for item in definitions]
            prefixes = sorted({server for key in keys if key and key[0] == "opencode" for server in [key[1]]}, key=len, reverse=True)
            servers: dict[tuple[str, str], dict[tuple[str, str], list[Any]]] = {}
            for agent, server, kind, name, origin, total, failed, last in connection.execute(
                    "SELECT agent, server, kind, name, origin, count(*), sum(failed), "
                    f"date(max(at) / 1000, 'unixepoch', 'localtime') FROM event WHERE {MCP_EVENTS} "
                    "GROUP BY agent, server, kind, name, origin"):
                if kind == "command":
                    _, server, name = name.split("__", 2)
                    kind = "prompt"
                if agent == "claude":
                    server = sanitize(server)
                elif agent == "opencode" and server == "":
                    server, name = opencode_owner(name, prefixes)
                entry = servers.setdefault((agent, server), {}).setdefault((kind, name), [0, 0, 0, 0, ""])
                entry[0 if kind != "prompt" else 1 if origin == "user" else 2] += total
                entry[3] += failed
                entry[4] = max(entry[4], last)
            owners = Counter(key for key in keys if key)
            ambiguous = 0
            for item, key in zip(definitions, keys):
                observed = servers.get(key, {}) if key else {}
                if key and owners[key] > 1:
                    ambiguous += 1
                    item["usageAmbiguous"] = True
                    observed = {}
                attach(item, counts(*(sum(entry[slot] for entry in observed.values()) for slot in range(4))))
                ranked = sorted(observed.items(), key=lambda pair: (-pair[1][0] - pair[1][1], pair[0][1], pair[0][0]))
                item["observed"] = [{"kind": kind, "name": name, "uses": entry[0] + entry[1], "failed": entry[3], "lastUsed": entry[4]}
                                    for (kind, name), entry in ranked[:MAX_OBSERVED]]
            return {"usageTranscripts": sources(connection, MCP_AGENTS), "usageUnreadable": unreadable,
                    "usageIngestPending": pending, "usageAmbiguous": ambiguous}
    except (OSError, sqlite3.Error):
        return {"usageError": UNAVAILABLE}


def history(kind: str, agents: tuple[str, ...], where: str, parameters: tuple[Any, ...] = (), *, ingest: bool = True) -> dict[str, Any]:
    try:
        with session(ingest_history=ingest) as (connection, (pending, _)):
            start, until = connection.execute(
                "SELECT date(min(first_at) / 1000, 'unixepoch', 'localtime'), date('now', 'localtime') FROM coverage "
                "WHERE agent IN (SELECT value FROM json_each(?))", (json.dumps(agents),)).fetchone()
            days = connection.execute(
                "SELECT date(at / 1000, 'unixepoch', 'localtime') AS day, sum(kind <> 'command' OR origin = 'user'), "
                "sum(kind <> 'command'), sum(kind = 'command' AND origin = 'user'), "
                "sum(kind = 'command' AND origin = 'scheduled'), sum(failed) "
                f"FROM event WHERE {where} AND day > date('now', 'localtime', '-{WINDOW_DAYS} days') "
                "AND day <= date('now', 'localtime') GROUP BY day ORDER BY day", parameters).fetchall()
            return {"ok": True, "schemaVersion": SCHEMA_VERSION, "kind": kind, "coverageStart": start, "until": until,
                    "ingestPending": pending, "days": [list(day) for day in days]}
    except (OSError, sqlite3.Error):
        return {"ok": False, "schemaVersion": SCHEMA_VERSION, "kind": kind, "error": UNAVAILABLE}


def skill_usage(items: list[dict[str, Any]], *, scoped: bool = False) -> dict[str, Any]:
    names = sorted({name for item in items for name in skill_names(item)})
    if scoped:
        return history("skill", SKILL_AGENTS,
                       "kind IN ('skill', 'command') AND name IN (SELECT value FROM json_each(?))", (json.dumps(names),), ingest=False)
    return history("skill", SKILL_AGENTS,
                   "(kind = 'skill' OR (kind = 'command' AND name IN (SELECT value FROM json_each(?))))", (json.dumps(names),))


def mcp_usage() -> dict[str, Any]:
    return history("mcp", MCP_AGENTS, MCP_EVENTS)


def forget(before: str | None) -> dict[str, Any]:
    try:
        with session(ingest_history=False) as (connection, _):
            connection.execute("PRAGMA secure_delete = ON")
            with connection:
                connection.execute("BEGIN IMMEDIATE")
                if before:
                    cutoff = connection.execute("SELECT CAST(strftime('%s', ?, 'utc') AS INTEGER) * 1000", (before,)).fetchone()[0]
                    removed = connection.execute("DELETE FROM event WHERE at < ?", (cutoff,)).rowcount
                    connection.execute("DELETE FROM failure WHERE at < ?", (cutoff,))
                    connection.execute("UPDATE coverage SET first_at = max(first_at, ?)", (cutoff,))
                else:
                    cutoff = time.time_ns() // 1_000_000 + 1
                    future = connection.execute("SELECT agent, call FROM event WHERE at >= ? UNION "
                                                "SELECT agent, call FROM failure WHERE at >= ?", (cutoff, cutoff)).fetchall()
                    connection.executemany("INSERT OR IGNORE INTO forgotten VALUES (?)", [(identity(*row),) for row in future])
                    removed = connection.execute("DELETE FROM event").rowcount
                    for statement in ("DELETE FROM failure", "DELETE FROM coverage", "UPDATE source SET project = NULL", "DELETE FROM project"):
                        connection.execute(statement)
                connection.execute("INSERT INTO retention VALUES (1, ?) ON CONFLICT (id) DO UPDATE "
                                   "SET before = max(before, excluded.before)", (cutoff,))
            return {"ok": True, "schemaVersion": SCHEMA_VERSION, "removed": removed}
    except (OSError, sqlite3.Error):
        return {"ok": False, "schemaVersion": SCHEMA_VERSION, "removed": 0, "error": UNAVAILABLE}
