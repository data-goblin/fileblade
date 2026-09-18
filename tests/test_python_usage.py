import json
import os
import shutil
import sqlite3
import stat
import subprocess
import tempfile
import time
import unittest
from contextlib import closing
from datetime import datetime, timedelta, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
DAY = timedelta(days=1)
EAST = "<+14>-14"


def iso(moment):
    return moment.astimezone(timezone.utc).isoformat(timespec="milliseconds").replace("+00:00", "Z")


def lines(*records):
    return "".join(json.dumps(record, separators=(",", ":")) + "\n" for record in records)


def envelope(kind, uuid, at, content, sidechain=False):
    return {"parentUuid": None, "isSidechain": sidechain, "userType": "external", "cwd": "/work/project",
            "sessionId": "0f1e2d3c", "version": "2.1.258", "gitBranch": "main", "type": kind, "uuid": uuid,
            "timestamp": iso(at), "message": {"role": kind, "content": content}}


def called(at, identity, name, sidechain=False, **arguments):
    part = {"type": "tool_use", "id": identity, "name": name, "input": arguments, "caller": {"type": "direct"}}
    return envelope("assistant", "a-" + identity, at, [part], sidechain)


def failed(at, identity):
    return envelope("user", "r-" + identity, at, [{"tool_use_id": identity, "type": "tool_result", "content": "boom", "is_error": True}])


def typed(at, uuid, *names, scheduled=None):
    content = "\n".join(f"<command-message>{name}</command-message>\n<command-name>/{name}</command-name>\n<command-args></command-args>"
                        for name in names)
    record = envelope("user", uuid, at, content)
    if scheduled:
        record["scheduledTaskId"] = scheduled
    return record


def opening():
    return {"type": "permission-mode", "permissionMode": "default", "sessionId": "0f1e2d3c"}


def session_meta(at):
    return {"timestamp": iso(at), "type": "session_meta",
            "payload": {"id": "c0de", "timestamp": iso(at), "cwd": "/work/project", "originator": "codex_cli_rs", "cli_version": "0.60.0"}}


def codex_call(at, identity, server, tool, status="completed"):
    item = {"type": "McpToolCall", "id": identity, "server": server, "tool": tool, "arguments": {"query": "x"},
            "readOnlyHint": True, "status": status, "result": {"content": []}, "duration": {"secs": 0, "nanos": 5}}
    return {"timestamp": iso(at), "type": "event_msg",
            "payload": {"type": "item_completed", "thread_id": "t", "turn_id": "u", "item": item}}


def mode(path):
    return stat.S_IMODE(path.stat().st_mode)


def uses(row):
    return {key: row[key] for key in ("uses", "usesAgent", "usesUser", "usesScheduled", "failed")}


class UsageHistory(unittest.TestCase):
    def setUp(self):
        temporary = tempfile.TemporaryDirectory(prefix="fileblade-usage-")
        self.addCleanup(temporary.cleanup)
        self.base = Path(temporary.name)
        self.home = self.base / "home"
        self.project = self.base / "project"
        (self.project / ".git").mkdir(parents=True)
        self.claude = self.home / ".claude"
        self.transcripts = self.claude / "projects" / "-work-project"
        self.transcripts.mkdir(parents=True)
        self.store = self.base / "state" / "omarchy" / "fileblade" / "agent-usage.sqlite3"
        self.day = (datetime.now(timezone.utc) - 3 * DAY).replace(hour=11, minute=0, second=0, microsecond=0)
        self.env = {
            "PATH": os.environ.get("PATH", "/usr/bin:/bin"),
            "HOME": str(self.home),
            "XDG_STATE_HOME": str(self.base / "state"),
            "XDG_CACHE_HOME": str(self.base / "cache"),
            "XDG_CONFIG_HOME": str(self.home / ".config"),
            "XDG_DATA_HOME": str(self.home / ".local" / "share"),
            "CLAUDE_CONFIG_DIR": str(self.claude),
            "CODEX_HOME": str(self.home / ".codex"),
            "TZ": "UTC0",
            "PYTHONDONTWRITEBYTECODE": "1",
        }

    def command(self, helper, *arguments, zone="UTC0"):
        return [str(ROOT / "python" / "bin" / f"agent-{helper}ctl"), *arguments], dict(self.env, TZ=zone)

    def helper(self, helper, *arguments, zone="UTC0"):
        command, env = self.command(helper, *arguments, zone=zone)
        process = subprocess.run(command, env=env, capture_output=True, text=True, timeout=60, check=False)
        self.assertEqual(process.returncode, 0, process.stderr + process.stdout)
        return json.loads(process.stdout)

    def skills(self, *arguments):
        return self.helper("skills", "list", "--json", "--project", str(self.project), *arguments)

    def skill_usage(self, zone="UTC0"):
        return self.helper("skills", "usage", "--json", "--project", str(self.project), zone=zone)

    def mcp(self):
        return self.helper("mcp", "list", "--json", "--project", str(self.project))

    def transcript(self, name, *records, mode="w"):
        path = self.transcripts / name
        path.parent.mkdir(parents=True, exist_ok=True)
        with path.open(mode) as handle:
            handle.write(lines(*records))
        return path

    def skill(self, directory, name):
        descriptor = directory / name / "SKILL.md"
        descriptor.parent.mkdir(parents=True)
        descriptor.write_text(f"---\nname: {name}\ndescription: fixture skill\n---\n")

    def plugin(self):
        install = self.claude / "plugins" / "cache" / "market" / "toolkit" / "1.0.0"
        self.skill(install / "skills", "review")
        (install / ".mcp.json").write_text(json.dumps({"mcpServers": {"docs": {"command": "docs-server"}}}))
        registry = {"version": 2, "plugins": {"toolkit@market": [{"scope": "user", "installPath": str(install), "version": "1.0.0"}]}}
        (self.claude / "plugins" / "installed_plugins.json").write_text(json.dumps(registry))

    def row(self, rows, name, **fields):
        found = [row for row in rows if row["name"] == name and all(row.get(key) == value for key, value in fields.items())]
        self.assertEqual(len(found), 1, [(row["name"], row.get("scope"), row.get("agent")) for row in rows])
        return found[0]

    def date(self, moment, zone=timezone.utc):
        return moment.astimezone(zone).date().isoformat()

    def test_skill_rows_separate_agent_typed_and_scheduled_uses(self):
        self.skill(self.claude / "skills", "alpha")
        self.skill(self.claude / "skills", "beta")
        self.skill(self.project / ".claude" / "skills", "gamma")
        self.plugin()
        first = called(self.day, "toolu_s1", "Skill", skill="alpha")
        self.transcript("s1.jsonl", opening(), first,
                        called(self.day, "toolu_s2", "Skill", skill="alpha", args="--fast"), failed(self.day, "toolu_s2"),
                        typed(self.day, "u1", "alpha"), typed(self.day, "u2", "alpha", scheduled="a36c8f4f"),
                        typed(self.day, "u3", "clear"), typed(self.day, "u4", "model"),
                        typed(self.day, "u5", "gamma", "toolkit:review"),
                        called(self.day, "toolu_s3", "Skill", skill="toolkit:review"))
        self.transcript("s1/subagents/workflows/wf_1/agent-a1.jsonl", called(self.day, "toolu_s4", "Skill", sidechain=True, skill="alpha"))
        copy = self.transcript("s2.jsonl", opening(), first, called(self.day, "toolu_s2", "Skill", skill="alpha", args="--fast"))
        os.utime(copy, (0, 0))
        document = self.skills()
        rows = document["items"]
        self.assertEqual(uses(self.row(rows, "alpha")), {"uses": 4, "usesAgent": 3, "usesUser": 1, "usesScheduled": 1, "failed": 1})
        self.assertEqual(uses(self.row(rows, "alpha")["metrics"]), uses(self.row(rows, "alpha")))
        self.assertEqual(self.row(rows, "beta")["uses"], 0)
        self.assertEqual(uses(self.row(rows, "gamma")), {"uses": 1, "usesAgent": 0, "usesUser": 1, "usesScheduled": 0, "failed": 0})
        self.assertEqual(uses(self.row(rows, "review", source="plugin:toolkit@market")),
                         {"uses": 2, "usesAgent": 1, "usesUser": 1, "usesScheduled": 0, "failed": 0})
        self.assertFalse({"clear", "model"} & {row["name"] for row in rows})
        self.assertEqual((document["usageTranscripts"], document["usageUnreadable"], document["usageIngestPending"]), (3, 0, False))
        history = self.skill_usage()
        self.assertEqual({key: history[key] for key in ("ok", "schemaVersion", "kind", "coverageStart", "ingestPending")},
                         {"ok": True, "schemaVersion": 1, "kind": "skill", "coverageStart": self.date(self.day), "ingestPending": False})
        self.assertEqual(history["until"], datetime.now(timezone.utc).date().isoformat())
        self.assertEqual(history["days"], [[self.date(self.day), 7, 4, 3, 1, 1]])

    def test_usage_day_lists_the_skills_used_on_one_local_day(self):
        self.skill(self.claude / "skills", "alpha")
        self.skill(self.claude / "skills", "beta")
        other = self.day + DAY
        self.transcript("s1.jsonl", opening(), called(self.day, "toolu_d1", "Skill", skill="alpha"),
                        typed(self.day, "u1", "alpha"), called(other, "toolu_d2", "Skill", skill="beta"))
        self.skills()
        day = self.helper("skills", "usage-day", "--json", "--project", str(self.project), "--day", self.date(self.day))
        self.assertEqual((day["ok"], day["day"]), (True, self.date(self.day)))
        self.assertEqual([(entry["name"], entry["uses"], entry["usesAgent"], entry["usesUser"]) for entry in day["items"]],
                         [("alpha", 2, 1, 1)])
        later = self.helper("skills", "usage-day", "--json", "--project", str(self.project), "--day", self.date(other))
        self.assertEqual([entry["name"] for entry in later["items"]], ["beta"])
        command, env = self.command("skills", "usage-day", "--json", "--project", str(self.project), "--day", "yesterday")
        process = subprocess.run(command, env=env, capture_output=True, text=True, timeout=60, check=False)
        self.assertEqual(process.returncode, 1, process.stdout)

    def test_usage_counts_answers_for_stubs_without_discovery(self):
        self.skill(self.claude / "skills", "alpha")
        self.transcript("s1.jsonl", opening(), called(self.day, "toolu_c1", "Skill", skill="alpha"), typed(self.day, "u1", "alpha"))
        rows = self.skills()["items"]
        alpha = self.row(rows, "alpha")
        stubs = json.dumps([{"id": alpha["id"], "name": "alpha", "source": alpha.get("source", "")}, {"id": "ghost", "name": "ghost", "source": ""}])
        document = self.helper("skills", "usage-counts", "--json", "--project", str(self.project), "--items", stubs)
        self.assertEqual(document["ok"], True)
        self.assertEqual(uses(document["counts"][alpha["id"]]), {"uses": 2, "usesAgent": 1, "usesUser": 1, "usesScheduled": 0, "failed": 0})
        self.assertEqual(document["counts"]["ghost"]["uses"], 0)

    def test_selected_skill_history_and_day_filter_reuse_loaded_items(self):
        self.skill(self.claude / "skills", "alpha")
        self.skill(self.claude / "skills", "beta")
        self.transcript("selection.jsonl", opening(), called(self.day, "a", "Skill", skill="alpha"),
                        called(self.day, "b", "Skill", skill="beta"), typed(self.day, "u", "alpha"),
                        called(self.day, "p", "Skill", skill="tools:alpha"))
        rows = self.skills()["items"]
        alpha = self.row(rows, "alpha")
        stubs = [{key: row.get(key, "") for key in ("id", "name", "source")} for row in rows]
        shutil.rmtree(self.claude / "skills")
        history = self.helper("skills", "usage", "--json", "--items", json.dumps([stubs[rows.index(alpha)]]))
        self.assertEqual(history["days"], [[self.date(self.day), 2, 1, 1, 0, 0]])
        plugin = self.helper("skills", "usage", "--json", "--items", json.dumps([dict(stubs[rows.index(alpha)], source="plugin:tools@market")]))
        self.assertEqual(plugin["days"], [[self.date(self.day), 3, 2, 1, 0, 0]])
        day = self.helper("skills", "usage-day", "--json", "--day", self.date(self.day), "--items", json.dumps(stubs))
        self.assertEqual({row["name"]: row["uses"] for row in day["items"]}, {"alpha": 2, "beta": 1})
        empty = self.helper("skills", "usage", "--json", "--items", "[]")
        self.assertEqual(empty["days"], [])

    def test_day_filter_uses_local_midnight_across_daylight_saving(self):
        from zoneinfo import ZoneInfo
        zone = ZoneInfo("Europe/Brussels")
        start = datetime(2026, 3, 29, tzinfo=zone).astimezone(timezone.utc)
        end = datetime(2026, 3, 30, tzinfo=zone).astimezone(timezone.utc)
        self.skill(self.claude / "skills", "alpha")
        self.transcript("dst.jsonl", opening(), called(start - timedelta(milliseconds=1), "before", "Skill", skill="alpha"),
                        called(start, "start", "Skill", skill="alpha"),
                        called(end - timedelta(milliseconds=1), "last", "Skill", skill="alpha"),
                        called(end, "after", "Skill", skill="alpha"))
        rows = self.skills()["items"]
        day = self.helper("skills", "usage-day", "--json", "--day", "2026-03-29", "--items", json.dumps(rows), zone="Europe/Brussels")
        self.assertEqual([(row["name"], row["uses"]) for row in day["items"]], [("alpha", 2)])

    @unittest.skipIf(os.geteuid() == 0, "root can read a mode 000 file")
    def test_a_complete_transcript_is_never_reopened(self):
        self.skill(self.claude / "skills", "alpha")
        path = self.transcript("s1.jsonl", opening(), called(self.day, "toolu_r1", "Skill", skill="alpha"))
        before = self.skills()
        self.assertEqual((before["usageUnreadable"], self.row(before["items"], "alpha")["uses"]), (0, 1))
        os.chmod(path, 0)
        self.addCleanup(os.chmod, path, stat.S_IRUSR | stat.S_IWUSR)
        after = self.skills()
        self.assertEqual((after["usageUnreadable"], self.row(after["items"], "alpha")["uses"]), (0, 1))
        history = self.skill_usage()
        self.assertEqual(history["days"], [[self.date(self.day), 1, 1, 0, 0, 0]])

    def test_mcp_rows_follow_the_server_names_agents_record(self):
        self.plugin()
        servers = {name: {"command": "server"} for name in ("my.server", "twin.a", "twin_a", "quiet")}
        (self.home / ".claude.json").write_text(json.dumps({"mcpServers": servers}))
        (self.home / ".codex").mkdir(parents=True)
        (self.home / ".codex" / "config.toml").write_text(
            '[mcp_servers.docs]\ncommand = "docs"\n\n[mcp_servers.openaiDeveloperDocs]\nurl = "https://developers.openai.com/mcp"\n')
        later = self.day + timedelta(hours=1)
        self.transcript("s1.jsonl", opening(),
                        called(self.day, "toolu_m1", "mcp__my_server__lookup", query="a"),
                        called(later, "toolu_m2", "mcp__my_server__lookup", query="b"), failed(later, "toolu_m2"),
                        called(self.day, "toolu_m3", "ListMcpResourcesTool", server="my.server"),
                        called(self.day, "toolu_m4", "ReadMcpResourceTool", server="my.server", uri="probe://notes/alpha?version=2#top"),
                        typed(self.day, "p1", "mcp__my_server__greet"), typed(self.day, "p2", "mcp__my_server__greet", scheduled="b1"),
                        called(self.day, "toolu_m5", "mcp__twin_a__ping"),
                        called(self.day, "toolu_m6", "mcp__plugin_toolkit_docs__search", query="c"))
        rollout = self.home / ".codex" / "sessions" / "2026" / "09" / "14" / "rollout-2026-09-14T10-00-00-c0de.jsonl"
        rollout.parent.mkdir(parents=True)
        rollout.write_text(lines(session_meta(self.day), codex_call(self.day, "exec-1", "docs", "search"),
                                 codex_call(self.day, "exec-2", "openaiDeveloperDocs", "search_openai_docs"),
                                 codex_call(self.day, "exec-3", "openaiDeveloperDocs", "fetch_openai_doc", status="failed")))
        document = self.mcp()
        rows = document["definitions"]
        date = self.date(self.day)
        mine = self.row(rows, "my.server", agent="claude")
        self.assertEqual(uses(mine), {"uses": 5, "usesAgent": 4, "usesUser": 1, "usesScheduled": 1, "failed": 1})
        self.assertEqual(uses(mine["metrics"]), uses(mine))
        self.assertEqual(mine["observed"], [
            {"kind": "tool", "name": "lookup", "uses": 2, "failed": 1, "lastUsed": date},
            {"kind": "resource-list", "name": "", "uses": 1, "failed": 0, "lastUsed": date},
            {"kind": "prompt", "name": "greet", "uses": 1, "failed": 0, "lastUsed": date},
            {"kind": "resource", "name": "probe://notes/alpha", "uses": 1, "failed": 0, "lastUsed": date},
        ])
        for twin in ("twin.a", "twin_a"):
            row = self.row(rows, twin, agent="claude")
            self.assertEqual((row["uses"], row["usageAmbiguous"], row["observed"]), (0, True, []))
        self.assertEqual(document["usageAmbiguous"], 2)
        quiet = self.row(rows, "quiet", agent="claude")
        self.assertEqual((quiet["uses"], quiet["observed"], "usageAmbiguous" in quiet), (0, [], False))
        plugin = self.row(rows, "docs", agent="claude", scope="plugin")
        self.assertEqual((plugin["uses"], [entry["name"] for entry in plugin["observed"]]), (1, ["search"]))
        self.assertEqual(self.row(rows, "docs", agent="codex")["uses"], 1)
        openai = self.row(rows, "openaiDeveloperDocs", agent="codex")
        self.assertEqual(uses(openai), {"uses": 2, "usesAgent": 2, "usesUser": 0, "usesScheduled": 0, "failed": 1})
        self.assertEqual((document["usageTranscripts"], document["usageIngestPending"]), (2, False))
        history = self.helper("mcp", "usage", "--json")
        self.assertEqual((history["kind"], history["coverageStart"], history["days"]), ("mcp", date, [[date, 10, 9, 1, 1, 2]]))

    def codex_rollout(self, *records):
        rollout = self.home / ".codex" / "sessions" / "2026" / "09" / "14" / "rollout-2026-09-14T10-00-00-c0de.jsonl"
        rollout.parent.mkdir(parents=True, exist_ok=True)
        rollout.write_text(lines(session_meta(self.day), *records))
        return rollout

    def codex_explicit(self, at, identity, name):
        body = f"<skill>\n<name>{name}</name>\n<path>/home/x/.agents/skills/{name}/SKILL.md</path>\nbody\n</skill>\n"
        return {"timestamp": iso(at), "type": "response_item",
                "payload": {"type": "message", "role": "user", "id": identity, "content": [{"type": "input_text", "text": body}],
                            "internal_chat_message_metadata_passthrough": {"turn_id": "turn-1", "create_time": 1.0,
                                                                          "content_item_kinds": ["skills.selected_skill_instructions"]}}}

    def codex_read(self, at, identity, turn, path):
        item = {"type": "CommandExecution", "id": identity, "command": ["/usr/bin/bash", "-lc", f"sed -n '1,240p' {path}"],
                "cwd": "/work/project", "status": "completed", "exit_code": 0,
                "parsed_cmd": [{"type": "read", "cmd": f"sed -n '1,240p' {path}", "name": "SKILL.md", "path": path}]}
        return {"timestamp": iso(at), "type": "event_msg", "payload": {"type": "item_completed", "thread_id": "t", "turn_id": turn, "item": item}}

    def codex_legacy_exec(self, at, call, command):
        return {"timestamp": iso(at), "type": "response_item",
                "payload": {"type": "custom_tool_call", "name": "exec", "call_id": call, "id": "ct_" + call, "input": command}}

    def opencode_stable(self, name, parts):
        directory = self.home / ".local" / "share" / "opencode"
        directory.mkdir(parents=True, exist_ok=True)
        with closing(sqlite3.connect(directory / name)) as connection:
            connection.executescript(
                "CREATE TABLE session (id TEXT PRIMARY KEY, project_id TEXT, directory TEXT, title TEXT, version TEXT, "
                "time_created INTEGER, time_updated INTEGER);"
                "CREATE TABLE message (id TEXT PRIMARY KEY, session_id TEXT, time_created INTEGER, time_updated INTEGER, data TEXT);"
                "CREATE TABLE part (id TEXT PRIMARY KEY, message_id TEXT, session_id TEXT, time_created INTEGER, time_updated INTEGER, data TEXT);")
            at = int(self.day.timestamp() * 1000)
            connection.execute("INSERT INTO session VALUES ('ses_1', 'p', '/work/project', 't', '1.18.30', ?, ?)", (at, at))
            for index, (call, tool, status, arguments) in enumerate(parts):
                data = {"type": "tool", "callID": call, "tool": tool,
                        "state": {"status": status, "input": arguments, "time": {"start": at + index, "end": at + index + 1}}}
                connection.execute("INSERT INTO part VALUES (?, 'msg_1', 'ses_1', ?, ?, ?)", (f"prt_{index}", at, at + index, json.dumps(data)))
            connection.commit()

    def opencode_beta(self, name, items):
        directory = self.home / ".local" / "share" / "opencode"
        directory.mkdir(parents=True, exist_ok=True)
        with closing(sqlite3.connect(directory / name)) as connection:
            connection.executescript(
                "CREATE TABLE session_v2 (id TEXT PRIMARY KEY, directory TEXT, version TEXT, time_created INTEGER, time_updated INTEGER);"
                "CREATE TABLE session_message (id TEXT PRIMARY KEY, session_id TEXT, type TEXT, seq INTEGER, time_created INTEGER, "
                "time_updated INTEGER, data TEXT);")
            at = int(self.day.timestamp() * 1000)
            connection.execute("INSERT INTO session_v2 VALUES ('ses_2', '/work/project', '0.0.0-beta', ?, ?)", (at, at))
            content = [{"type": "tool", "id": call, "name": tool, "state": {"status": status, "input": arguments},
                        "time": {"created": at, "completed": at + 1}} for call, tool, status, arguments in items]
            connection.execute("INSERT INTO session_message VALUES ('msg_2', 'ses_2', 'assistant', 1, ?, ?, ?)",
                               (at, at, json.dumps({"content": content, "time": {"created": at}})))
            connection.commit()

    def copilot_events(self, *events):
        path = self.home / ".copilot" / "session-state" / "31cbc57f" / "events.jsonl"
        path.parent.mkdir(parents=True, exist_ok=True)
        records = [{"id": f"e{index}", "timestamp": iso(self.day), "parentId": None, "type": kind, "data": data}
                   for index, (kind, data) in enumerate(events)]
        path.write_text(lines(*records))
        return path

    def antigravity_files(self, slash, skill_path):
        root = self.home / ".gemini" / "antigravity-cli"
        logs = root / "brain" / "9c1d" / ".system_generated" / "logs"
        logs.mkdir(parents=True)
        at = int(self.day.timestamp() * 1000)
        (root / "history.jsonl").write_text(lines(
            {"timestamp": at, "workspace": "/work/project", "display": "/usage", "type": "slash_command"},
            {"timestamp": at + 1, "workspace": "/work/project", "display": slash, "type": "slash_command", "conversationId": "9c1d"},
            {"timestamp": at + 2, "workspace": "/work/project", "display": "plain prompt"}))
        (logs / "transcript_full.jsonl").write_text(lines(
            {"step_index": 0, "type": "USER_INPUT", "source": "USER_EXPLICIT", "status": "DONE", "created_at": self.day.strftime("%Y-%m-%dT%H:%M:%SZ"), "content": "x"},
            {"step_index": 1, "type": "GENERIC", "source": "MODEL", "status": "DONE", "created_at": self.day.strftime("%Y-%m-%dT%H:%M:%SZ"),
             "tool_calls": [{"name": "view_file", "args": {"AbsolutePath": skill_path}}, {"name": "list_dir", "args": {"DirectoryPath": "/work"}}]}))

    def pi_session(self, *entries):
        path = self.home / ".pi" / "agent" / "sessions" / "--work-project--" / "2026-09-14T10-00-00_s1.jsonl"
        path.parent.mkdir(parents=True)
        header = {"type": "session", "version": 3, "id": "s1", "timestamp": iso(self.day), "cwd": "/work/project"}
        path.write_text(lines(header, *entries))
        return path

    def test_every_supported_agent_counts_skill_uses(self):
        for name in ("alpha", "beta", "gamma"):
            self.skill(self.claude / "skills", name)
        alpha = "/home/x/.agents/skills/alpha/SKILL.md"
        self.codex_rollout(self.codex_explicit(self.day, "msg_1", "alpha"),
                           self.codex_read(self.day, "cmd_1", "turn-1", alpha), self.codex_read(self.day, "cmd_2", "turn-1", alpha),
                           self.codex_legacy_exec(self.day, "call_9", "cat /home/x/.agents/skills/beta/SKILL.md | head"),
                           codex_call(self.day, "exec-1", "docs", "search"))
        self.opencode_stable("opencode.db", [("call_a", "skill", "completed", {"name": "alpha"}),
                                             ("call_b", "skill", "error", {"name": "alpha"}),
                                             ("call_c", "skill", "running", {"name": "alpha"}),
                                             ("call_d", "read", "completed", {"filePath": "/x"})])
        self.opencode_beta("opencode-beta.db", [("call_e", "skill", "completed", {"name": "gamma"}), ("call_f", "bash", "completed", {})])
        self.copilot_events(("session.start", {"sessionId": "31cbc57f", "context": {"cwd": "/work/project"}}),
                            ("skill.invoked", {"name": "alpha", "path": "/x/alpha/SKILL.md", "trigger": "user-invoked", "content": "body"}),
                            ("skill.invoked", {"name": "alpha", "path": "/x/alpha/SKILL.md", "trigger": "agent-invoked", "content": "body"}),
                            ("skill.invoked", {"name": "beta", "path": "/x/beta/SKILL.md", "trigger": "context-load", "content": "body"}),
                            ("tool.execution_start", {"toolCallId": "tc-1", "toolName": "docs-search", "mcpServerName": "docs", "mcpToolName": "search"}),
                            ("tool.execution_complete", {"toolCallId": "tc-1", "success": False}))
        self.antigravity_files("/alpha do it", "/home/x/.agents/skills/beta/SKILL.md")
        self.pi_session({"type": "message", "id": "m1", "parentId": None, "timestamp": iso(self.day),
                         "message": {"role": "assistant", "content": [{"type": "toolCall", "id": "tc_pi", "name": "read",
                                                                       "arguments": {"path": "/home/x/.pi/agent/skills/gamma/SKILL.md"}}]}},
                        {"type": "message", "id": "m2", "parentId": "m1", "timestamp": iso(self.day),
                         "message": {"role": "toolResult", "toolCallId": "tc_pi", "toolName": "read", "content": [], "isError": True}})
        document = self.skills()
        rows = document["items"]
        self.assertEqual(uses(self.row(rows, "alpha")), {"uses": 7, "usesAgent": 4, "usesUser": 3, "usesScheduled": 0, "failed": 1})
        self.assertEqual(uses(self.row(rows, "beta")), {"uses": 2, "usesAgent": 2, "usesUser": 0, "usesScheduled": 0, "failed": 0})
        self.assertEqual(uses(self.row(rows, "gamma")), {"uses": 2, "usesAgent": 2, "usesUser": 0, "usesScheduled": 0, "failed": 1})
        self.assertEqual((document["usageTranscripts"], document["usageUnreadable"], document["usageIngestPending"]), (7, 0, False))
        self.assertIn(str(self.transcripts.parent), document["usageWatchPaths"])
        self.assertIn(str(self.home / ".codex" / "sessions"), document["usageWatchPaths"])
        history = self.skill_usage()
        self.assertEqual(history["coverageStart"], self.date(self.day))
        self.assertEqual(history["days"], [[self.date(self.day), 11, 8, 3, 0, 2]])
        with closing(sqlite3.connect(self.store)) as connection:
            agents = dict(connection.execute("SELECT agent, count(*) FROM event WHERE kind IN ('skill', 'command') GROUP BY agent"))
            self.assertEqual(agents, {"codex": 3, "opencode": 3, "copilot": 2, "antigravity": 3, "pi": 1})
            projects = {row[0] for row in connection.execute("SELECT DISTINCT project.path FROM event JOIN project ON project.id = event.project")}
            self.assertEqual(projects, {"/work/project"})
            self.assertEqual(connection.execute("SELECT count(*) FROM failure").fetchone()[0], 0)

    def test_an_opencode_call_that_finishes_later_is_counted_once_it_completes(self):
        self.skill(self.claude / "skills", "alpha")
        self.opencode_stable("opencode.db", [("call_a", "skill", "running", {"name": "alpha"})])
        self.assertEqual(self.row(self.skills()["items"], "alpha")["usesAgent"], 0)
        at = int(self.day.timestamp() * 1000)
        with closing(sqlite3.connect(self.home / ".local" / "share" / "opencode" / "opencode.db")) as connection:
            data = {"type": "tool", "callID": "call_a", "tool": "skill", "state": {"status": "completed", "input": {"name": "alpha"},
                                                                                     "time": {"start": at, "end": at + 5}}}
            connection.execute("UPDATE part SET data = ?, time_updated = ? WHERE id = 'prt_0'", (json.dumps(data), at + 5000))
            connection.commit()
        self.assertEqual(self.row(self.skills()["items"], "alpha")["usesAgent"], 1)
        self.assertEqual(self.row(self.skills()["items"], "alpha")["usesAgent"], 1)

    def test_other_agents_mcp_calls_reach_their_definitions(self):
        (self.home / ".copilot").mkdir(parents=True, exist_ok=True)
        (self.home / ".copilot" / "mcp-config.json").write_text(json.dumps({"mcpServers": {"docs": {"type": "local", "command": "docs"}}}))
        (self.home / ".config" / "opencode").mkdir(parents=True, exist_ok=True)
        (self.home / ".config" / "opencode" / "opencode.json").write_text(json.dumps({"mcp": {"my.docs": {"type": "local", "command": ["docs"]}}}))
        self.copilot_events(("session.start", {"sessionId": "31cbc57f", "context": {"cwd": "/work/project"}}),
                            ("tool.execution_start", {"toolCallId": "tc-1", "toolName": "docs-search", "mcpServerName": "docs", "mcpToolName": "search"}),
                            ("tool.execution_complete", {"toolCallId": "tc-1", "success": False}),
                            ("tool.execution_start", {"toolCallId": "tc-2", "toolName": "bash", "arguments": {}}),
                            ("tool.execution_complete", {"toolCallId": "tc-2", "success": True}))
        self.opencode_stable("opencode.db", [("call_a", "my_docs_search", "completed", {"query": "x"}),
                                             ("call_b", "my_docs_fetch_page", "error", {"query": "y"}),
                                             ("call_c", "read", "completed", {"filePath": "/x"})])
        rows = self.mcp()["definitions"]
        copilot = self.row(rows, "docs", agent="github-copilot-cli")
        self.assertEqual(uses(copilot), {"uses": 1, "usesAgent": 1, "usesUser": 0, "usesScheduled": 0, "failed": 1})
        self.assertEqual([entry["name"] for entry in copilot["observed"]], ["search"])
        opencode = self.row(rows, "my.docs", agent="opencode")
        self.assertEqual(uses(opencode), {"uses": 2, "usesAgent": 2, "usesUser": 0, "usesScheduled": 0, "failed": 1})
        self.assertEqual(sorted(entry["name"] for entry in opencode["observed"]), ["fetch_page", "search"])

    def test_an_appended_transcript_is_read_from_where_the_last_read_stopped(self):
        self.skill(self.claude / "skills", "alpha")
        path = self.transcript("s1.jsonl", opening(), called(self.day, "toolu_a1", "Skill", skill="alpha"))
        self.assertEqual(self.row(self.skills()["items"], "alpha")["usesAgent"], 1)
        rewritten = path.read_bytes().replace(b"toolu_a1", b"toolu_b1")
        with path.open("r+b") as handle:
            handle.write(rewritten)
        self.transcript("s1.jsonl", called(self.day, "toolu_a2", "Skill", skill="alpha"), mode="a")
        self.assertEqual(self.row(self.skills()["items"], "alpha")["usesAgent"], 2)

    def test_a_replaced_transcript_is_not_counted_twice(self):
        self.skill(self.claude / "skills", "alpha")
        records = [called(self.day, f"toolu_r{index}", "Skill", skill="alpha") for index in range(3)]
        path = self.transcript("s1.jsonl", *records[:2])
        self.assertEqual(self.row(self.skills()["items"], "alpha")["usesAgent"], 2)
        replacement = self.transcript("s1.jsonl.new", *records)
        os.replace(replacement, path)
        self.assertEqual(self.row(self.skills()["items"], "alpha")["usesAgent"], 3)

    def test_a_half_written_last_line_waits_for_its_newline(self):
        self.skill(self.claude / "skills", "alpha")
        second = lines(called(self.day, "toolu_h2", "Skill", skill="alpha"))
        path = self.transcript("s1.jsonl", called(self.day, "toolu_h1", "Skill", skill="alpha"))
        with path.open("a") as handle:
            handle.write(second[:40])
        self.assertEqual(self.row(self.skills()["items"], "alpha")["usesAgent"], 1)
        with path.open("a") as handle:
            handle.write(second[40:])
        self.assertEqual(self.row(self.skills()["items"], "alpha")["usesAgent"], 2)

    def test_a_deleted_transcript_keeps_its_uses_and_the_coverage_start(self):
        self.skill(self.claude / "skills", "alpha")
        oldest = self.day - 10 * DAY
        old = self.transcript("old.jsonl", opening(), called(oldest, "toolu_o1", "Skill", skill="alpha"))
        self.transcript("new.jsonl", opening(), called(self.day, "toolu_n1", "Skill", skill="alpha"))
        document = self.skills()
        self.assertEqual((self.row(document["items"], "alpha")["uses"], document["usageTranscripts"]), (2, 2))
        self.assertEqual(self.skill_usage()["coverageStart"], self.date(oldest))
        old.unlink()
        self.transcript("new.jsonl", called(self.day, "toolu_n2", "Skill", skill="alpha"), mode="a")
        document = self.skills()
        self.assertEqual((self.row(document["items"], "alpha")["uses"], document["usageTranscripts"]), (3, 1))
        history = self.skill_usage()
        self.assertEqual((history["coverageStart"], [day[0] for day in history["days"]]), (self.date(oldest), [self.date(oldest), self.date(self.day)]))

    def test_a_typed_command_counts_whichever_lane_reads_it_first(self):
        self.skill(self.claude / "skills", "pdf")
        self.skill(self.project / ".claude" / "skills", "gamma")
        self.transcript("s1.jsonl", typed(self.day, "u1", "pdf"))
        self.mcp()
        self.transcript("s1.jsonl", typed(self.day, "u2", "pdf"), mode="a")
        project = self.skills("--scope", "project")
        self.assertEqual([row["name"] for row in project["items"]], ["gamma"])
        self.assertEqual(self.row(self.skills("--scope", "user")["items"], "pdf")["usesUser"], 2)

    def test_days_are_local_dates(self):
        self.skill(self.claude / "skills", "alpha")
        self.transcript("s1.jsonl", called(self.day, "toolu_t1", "Skill", skill="alpha"))
        utc = self.skill_usage()
        east = self.skill_usage(zone=EAST)
        zone = timezone(timedelta(hours=14))
        self.assertNotEqual(self.date(self.day), self.date(self.day, zone))
        self.assertEqual(utc["days"], [[self.date(self.day), 1, 1, 0, 0, 0]])
        self.assertEqual(east["days"], [[self.date(self.day, zone), 1, 1, 0, 0, 0]])
        self.assertEqual((east["coverageStart"], east["until"]), (self.date(self.day, zone), datetime.now(zone).date().isoformat()))

    def test_concurrent_helpers_ingest_once_and_agree(self):
        self.assertEqual(self.helper("mcp", "usage", "--json")["days"], [])
        with closing(sqlite3.connect(self.store)) as connection:
            connection.executescript("""
                CREATE TABLE ingest_log (path TEXT NOT NULL);
                CREATE TRIGGER source_inserted AFTER INSERT ON source BEGIN INSERT INTO ingest_log VALUES (NEW.path); END;
                CREATE TRIGGER source_updated AFTER UPDATE ON source BEGIN INSERT INTO ingest_log VALUES (NEW.path); END;
            """)
        for session in range(24):
            self.transcript(f"s{session}.jsonl", opening(), *(called(self.day, f"toolu_{session}_{call}", f"mcp__server{call}__tool")
                                                              for call in range(40)))
        command, env = self.command("mcp", "usage", "--json")
        processes = [subprocess.Popen(command, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE) for _ in range(4)]
        answers = [process.communicate(timeout=60) for process in processes]
        self.assertEqual([process.returncode for process in processes], [0, 0, 0, 0], [error for _, error in answers])
        self.assertEqual(len({output for output, _ in answers}), 1)
        history = json.loads(answers[0][0])
        self.assertEqual((history["ingestPending"], history["days"]), (False, [[self.date(self.day), 960, 960, 0, 0, 0]]))
        with closing(sqlite3.connect(self.store)) as connection:
            writes = connection.execute("SELECT count(DISTINCT path), count(*) FROM ingest_log").fetchone()
        self.assertEqual(writes, (24, 24))

    def test_forget_removes_history_before_a_day_or_all_of_it(self):
        self.skill(self.claude / "skills", "alpha")
        early = self.day - 5 * DAY
        self.transcript("s1.jsonl", called(early, "toolu_f1", "Skill", skill="alpha"), called(self.day, "toolu_f2", "Skill", skill="alpha"))
        self.assertEqual(self.row(self.skills()["items"], "alpha")["uses"], 2)
        cutoff = self.date(self.day - DAY)
        self.assertEqual(self.helper("mcp", "usage-forget", "--json", "--before", cutoff), {"ok": True, "schemaVersion": 1, "removed": 1})
        history = self.skill_usage()
        self.assertEqual((history["coverageStart"], history["days"]), (cutoff, [[self.date(self.day), 1, 1, 0, 0, 0]]))
        self.assertEqual(self.row(self.skills()["items"], "alpha")["uses"], 1)
        self.assertEqual(self.helper("mcp", "usage-forget", "--json"), {"ok": True, "schemaVersion": 1, "removed": 1})
        self.assertEqual(self.row(self.skills()["items"], "alpha")["uses"], 0)
        history = self.skill_usage()
        self.assertEqual((history["coverageStart"], history["days"]), (None, []))
        with closing(sqlite3.connect(self.store)) as connection:
            self.assertEqual(connection.execute("SELECT count(*) FROM event UNION ALL SELECT count(*) FROM project").fetchall(), [(0,), (0,)])

    def test_forgotten_history_stays_forgotten_after_replacement_and_truncation(self):
        self.skill(self.claude / "skills", "alpha")
        old = called(self.day - 5 * DAY, "old", "Skill", skill="alpha")
        recent = called(self.day, "recent", "Skill", skill="alpha")
        path = self.transcript("session.jsonl", old, recent)
        self.skills()
        self.helper("mcp", "usage-forget", "--json", "--before", self.date(self.day))
        os.replace(self.transcript("replacement", old, recent), path)
        self.assertEqual(self.row(self.skills()["items"], "alpha")["uses"], 1)
        self.helper("mcp", "usage-forget", "--json")
        path.write_text(lines(old))
        self.assertEqual(self.skill_usage()["days"], [])
        self.transcript("unread.jsonl", recent)
        self.assertEqual(self.skill_usage()["days"], [])
        future = datetime.now(timezone.utc) + timedelta(seconds=1)
        self.transcript("session.jsonl", called(future, "new", "Skill", skill="alpha"), mode="a")
        self.assertEqual(self.row(self.skills()["items"], "alpha")["uses"], 1)

    def test_failure_records_accept_standard_json_spacing(self):
        self.skill(self.claude / "skills", "alpha")
        path = self.transcript("session.jsonl", called(self.day, "failed", "Skill", skill="alpha"))
        with path.open("a") as handle:
            handle.write(json.dumps(failed(self.day, "failed")) + "\n")
        self.assertEqual(self.row(self.skills()["items"], "alpha")["failed"], 1)

    def test_failures_survive_newest_first_ingest_across_transcripts(self):
        self.skill(self.claude / "skills", "alpha")
        older = self.transcript("original.jsonl", called(self.day, "resumed", "Skill", skill="alpha"))
        os.utime(older, (0, 0))
        self.transcript("resumed.jsonl", failed(self.day, "resumed"))
        self.assertEqual(self.row(self.skills()["items"], "alpha")["failed"], 1)
        self.transcript("resumed.jsonl", failed(self.day, "pending"), mode="a")
        self.skills()
        self.transcript("original.jsonl", called(self.day, "pending", "Skill", skill="alpha"), mode="a")
        self.assertEqual(self.row(self.skills()["items"], "alpha")["failed"], 2)

    def test_plugin_usage_uses_registry_identity_outside_the_cache_layout(self):
        install = self.claude / "plugins" / "local-toolkit"
        install.mkdir(parents=True)
        (install / ".mcp.json").write_text(json.dumps({"mcpServers": {"docs": {"command": "server"}}}))
        (install.parent / "installed_plugins.json").write_text(json.dumps({
            "version": 2, "plugins": {"toolkit@market": [{"scope": "user", "installPath": str(install)}]}}))
        self.transcript("session.jsonl", called(self.day, "plugin", "mcp__plugin_toolkit_docs__search"))
        self.assertEqual(self.row(self.mcp()["definitions"], "docs", agent="claude")["uses"], 1)

    def test_ambiguity_is_consistent_across_inventory_lanes(self):
        config = json.dumps({"mcpServers": {"docs": {"command": "server"}}})
        (self.home / ".claude.json").write_text(config)
        (self.project / ".mcp.json").write_text(config)
        self.transcript("session.jsonl", called(self.day, "ambiguous", "mcp__docs__search"))
        for scope in ("all", "user", "project"):
            document = self.helper("mcp", "list", "--json", "--project", str(self.project), "--scope", scope)
            for row in document["definitions"]:
                if row["agent"] == "claude" and row["name"] == "docs":
                    self.assertEqual((row["uses"], row.get("usageAmbiguous")), (0, True), scope)

    def test_a_killed_large_ingest_resumes_committed_progress_without_duplicates(self):
        self.helper("mcp", "usage", "--json")
        path = self.transcripts / "large.jsonl"
        total = 20000
        with path.open("w") as handle:
            for index in range(total):
                handle.write(lines(called(self.day, f"large-{index}", "mcp__docs__search", padding="x" * 1024)))
        command, env = self.command("mcp", "usage", "--json")
        process = subprocess.Popen(command, env=env, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
        try:
            deadline = time.monotonic() + 7
            offset = 0
            with closing(sqlite3.connect(self.store)) as connection:
                while process.poll() is None and time.monotonic() < deadline:
                    row = connection.execute("SELECT offset FROM source WHERE path = ?", (str(path),)).fetchone()
                    offset = row[0] if row else 0
                    if 0 < offset < path.stat().st_size:
                        break
                    time.sleep(0.005)
            self.assertTrue(0 < offset < path.stat().st_size, "large files must commit progress before completion")
        finally:
            if process.poll() is None:
                process.kill()
            process.communicate(timeout=10)
        for _ in range(10):
            result = self.helper("mcp", "usage", "--json")
            if not result["ingestPending"]:
                break
        self.assertFalse(result["ingestPending"])
        self.assertEqual(result["days"], [[self.date(self.day), total, total, 0, 0, 0]])

    def test_oversized_records_do_not_hide_the_next_event(self):
        path = self.transcript("large-record.jsonl", called(self.day, "first", "mcp__docs__search"))
        with path.open("ab") as handle:
            handle.write(b"x" * (12 * 1024 * 1024) + b"\n")
            handle.write(lines(called(self.day, "last", "mcp__docs__search")).encode())
        result = self.helper("mcp", "usage", "--json")
        self.assertEqual(result["days"], [[self.date(self.day), 2, 2, 0, 0, 0]])

    def test_forget_commits_while_another_connection_keeps_a_read_snapshot(self):
        self.transcript("session.jsonl", called(self.day, "private", "mcp__docs__private_name"))
        self.helper("mcp", "usage", "--json")
        with closing(sqlite3.connect(self.store)) as connection:
            connection.execute("BEGIN")
            connection.execute("SELECT * FROM event").fetchall()
            self.assertEqual(self.helper("mcp", "usage-forget", "--json"),
                             {"ok": True, "schemaVersion": 1, "removed": 1})
        self.assertEqual(self.helper("mcp", "usage", "--json")["days"], [])
        self.assertNotIn(b"private_name", self.store.read_bytes())

    def test_schema_upgrade_preserves_history_from_deleted_transcripts(self):
        path = self.transcript("session.jsonl", called(self.day, "preserved", "mcp__docs__search"))
        expected = self.helper("mcp", "usage", "--json")["days"]
        path.unlink()
        with closing(sqlite3.connect(self.store)) as connection:
            connection.executescript("DROP TABLE IF EXISTS retention; DROP TABLE IF EXISTS failure; "
                                     "DROP TABLE IF EXISTS forgotten; PRAGMA user_version = 1;")
        self.assertEqual(self.helper("mcp", "usage", "--json")["days"], expected)

    def test_unknown_schema_is_refused_without_erasing_history(self):
        self.transcript("session.jsonl", called(self.day, "preserved", "mcp__docs__search"))
        self.helper("mcp", "usage", "--json")
        with closing(sqlite3.connect(self.store)) as connection:
            connection.execute("PRAGMA user_version = 99")
        command, env = self.command("mcp", "usage", "--json")
        result = subprocess.run(command, env=env, capture_output=True, text=True, timeout=8)
        self.assertEqual(result.returncode, 1)
        self.assertFalse(json.loads(result.stdout)["ok"])
        with closing(sqlite3.connect(self.store)) as connection:
            self.assertEqual(connection.execute("SELECT count(*) FROM event").fetchone()[0], 1)

    def test_full_forget_keeps_old_codex_project_paths_out_of_replayed_chunks(self):
        sessions = self.home / ".codex" / "sessions"
        sessions.mkdir(parents=True)
        path = sessions / "session.jsonl"
        path.write_text(lines(session_meta(self.day)) + "x" * (12 * 1024 * 1024) + "\n")
        self.helper("mcp", "usage", "--json")
        self.helper("mcp", "usage-forget", "--json")
        replacement = sessions / "replacement"
        replacement.write_bytes(path.read_bytes())
        os.replace(replacement, path)
        self.assertEqual(self.helper("mcp", "usage", "--json")["days"], [])
        with closing(sqlite3.connect(self.store)) as connection:
            self.assertEqual(connection.execute("SELECT count(*) FROM project").fetchone()[0], 0)

    def test_malformed_deep_record_does_not_block_later_uses(self):
        path = self.transcript("session.jsonl", called(self.day, "first", "mcp__docs__search"))
        with path.open("a") as handle:
            handle.write('{"mcp__nested":' + '[' * 2000 + '0' + ']' * 2000 + '}\n')
            handle.write(lines(called(self.day, "last", "mcp__docs__search")))
        self.assertEqual(self.helper("mcp", "usage", "--json")["days"], [[self.date(self.day), 2, 2, 0, 0, 0]])

    def test_forgetting_a_future_dated_event_does_not_block_new_uses(self):
        self.skill(self.claude / "skills", "alpha")
        future = datetime.now(timezone.utc) + 365 * DAY
        path = self.transcript("session.jsonl", called(future, "bad-clock", "Skill", skill="alpha"))
        self.skills()
        self.helper("mcp", "usage-forget", "--json")
        os.replace(self.transcript("replacement", called(future, "bad-clock", "Skill", skill="alpha")), path)
        self.assertEqual(self.skill_usage()["coverageStart"], None)
        new = datetime.now(timezone.utc) + timedelta(seconds=1)
        self.transcript("session.jsonl", called(new, "new", "Skill", skill="alpha"), mode="a")
        self.assertEqual(self.row(self.skills()["items"], "alpha")["uses"], 1)

    def test_the_store_is_private_and_the_old_cache_is_removed(self):
        cache = self.base / "cache" / "omarchy" / "fileblade"
        cache.mkdir(parents=True)
        for name in ("agent-usage.json", "agent-usage.tmp"):
            (cache / name).write_text('{"schema": 1, "files": {}}')
        self.skills()
        self.assertEqual(sorted(path.name for path in cache.iterdir()), [])
        self.assertEqual(mode(self.store.parent), 0o700)
        self.assertEqual((mode(self.store), mode(self.store.with_name("agent-usage.sqlite3.lock"))), (0o600, 0o600))
        self.assertFalse((self.home / ".local" / "state").exists())

    @unittest.skipIf(os.geteuid() == 0, "root reads every file")
    def test_an_unreadable_transcript_is_reported(self):
        self.skill(self.claude / "skills", "alpha")
        blocked = self.transcript("s1.jsonl", called(self.day, "toolu_u1", "Skill", skill="alpha"))
        blocked.chmod(0)
        self.addCleanup(blocked.chmod, 0o600)
        document = self.skills()
        self.assertEqual((document["usageUnreadable"], self.row(document["items"], "alpha")["uses"]), (1, 0))


if __name__ == "__main__":
    unittest.main()
