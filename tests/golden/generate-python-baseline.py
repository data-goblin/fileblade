#!/usr/bin/env python3
from __future__ import annotations

import argparse
import importlib.util
import json
import os
import shutil
import sqlite3
import subprocess
import sys
from datetime import datetime, timedelta, timezone
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
PYTHON = REPO / "python"
DEFAULT_ROOT = Path("/tmp/fileblade-python-baseline")
DEFAULT_OUT = REPO / "tests" / "golden" / "python-baseline"
FROZEN_EPOCH = 1758000000
FROZEN_DAY = datetime(2026, 9, 10, 12, 0, 0, tzinfo=timezone.utc)
TRANSACTION_ID = "0123456789abcdef0123456789abcdef"


def fixed_times(root: Path) -> None:
    for path in sorted(root.rglob("*")) + [root]:
        try:
            os.utime(path, (FROZEN_EPOCH, FROZEN_EPOCH), follow_symlinks=False)
        except (OSError, NotImplementedError):
            continue


def environment(home: Path, state: Path) -> dict[str, str]:
    return {
        "PATH": "/usr/bin:/bin",
        "HOME": str(home),
        "TZ": "UTC",
        "LC_ALL": "C.UTF-8",
        "XDG_CONFIG_HOME": str(home / ".config"),
        "XDG_STATE_HOME": str(state),
        "XDG_CACHE_HOME": str(home / ".cache"),
        "XDG_DATA_HOME": str(home / ".local/share"),
        "CODEX_HOME": str(home / ".codex"),
        "CLAUDE_CONFIG_DIR": str(home / ".claude"),
        "PYTHONPATH": str(PYTHON),
        "PYTHONDONTWRITEBYTECODE": "1",
    }


def run(command: list[str], env: dict[str, str], stdin: str | None = None) -> dict:
    result = subprocess.run(command, env=env, input=stdin, text=True, timeout=120,
                            stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if not result.stdout.strip():
        raise SystemExit(f"{command[1] if len(command) > 1 else command[0]} produced nothing: {result.stderr}")
    return json.loads(result.stdout)


def normalized(value, root: Path):
    if isinstance(value, str):
        return value.replace(str(root), "{ROOT}")
    if isinstance(value, list):
        return [normalized(item, root) for item in value]
    if isinstance(value, dict):
        return {normalized(key, root): normalized(item, root) for key, item in value.items()}
    return value


def write_json(path: Path, document) -> None:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(document, indent=2, sort_keys=False, ensure_ascii=False) + "\n", encoding="utf-8")


def rows_of(document: dict) -> list[dict]:
    return document.get("definitions") or document.get("items") or []


def loaded(name: str, path: Path):
    for entry in (str(PYTHON), str(path.parent)):
        if entry not in sys.path:
            sys.path.insert(0, entry)
    specification = importlib.util.spec_from_file_location(name, path)
    module = importlib.util.module_from_spec(specification)
    sys.modules[name] = module
    specification.loader.exec_module(module)
    return module


def build_skills(home: Path, project: Path) -> None:
    fixtures = loaded("baseline_skills_fixtures", REPO / "tests/core_modules/skills/fixtures.py")

    vault = home / "vault" / "skills"
    fixtures.write_skill(home / ".claude" / "skills", "claude-user", "A user skill")
    fixtures.write_skill(home / ".agents" / "skills", "agents-user", "A shared skill")
    fixtures.write_skill(home / ".pi" / "agent" / "skills", "pi-user", "A pi skill")
    fixtures.write_skill(home / ".copilot" / "skills", "copilot-user", "A copilot skill")
    fixtures.write_skill(home / ".config" / "opencode" / "skills", "opencode-user", "An opencode skill")
    fixtures.write_skill(home / ".gemini" / "antigravity-cli" / "skills", "antigravity-user", "An antigravity skill")
    fixtures.write_skill(project / ".claude" / "skills", "project-only", "A project skill")
    native = project / ".claude" / "skills" / os.fsdecode(b"native-\xff")
    native.mkdir(parents=True, exist_ok=True)
    (native / "SKILL.md").write_text(
        fixtures.SKILL.format(name="native-byte", description="A native-byte skill"), encoding="utf-8")
    descriptor = fixtures.write_skill(vault, "linked", "A linked skill")
    fixtures.link_skill(home / ".claude" / "skills", "linked", descriptor.parent)
    fixtures.link_skill(home / ".agents" / "skills", "linked", descriptor.parent)


def build_memory(home: Path, project: Path) -> None:
    fixtures = loaded("baseline_memory_fixtures", REPO / "tests/core_modules/memory/fixtures.py")
    fixtures.build_home(home)
    fixtures.build_project(project)
    fixtures.build_audit_fixtures(home, project)


def build_hooks(home: Path, project: Path) -> None:
    hook_fixtures = loaded("baseline_hooks_fixtures", REPO / "tests/core_modules/hooks/fixtures.py")
    hook_fixtures.build_all(home, project)


def build_mcp(home: Path, project: Path) -> None:
    test_inventory = loaded("baseline_mcp_inventory", REPO / "tests/core_modules/mcp/test_inventory.py")
    case = test_inventory.InventoryCase("test_schema_sources_precedence_and_secret_redaction")
    case.home = home
    case.project = project
    case.config = home / ".config"
    case.etc = home / "etc"
    project.mkdir(parents=True, exist_ok=True)
    case.populate()


BUILDERS = {"skills": build_skills, "memory": build_memory, "hooks": build_hooks, "mcp": build_mcp}


def list_arguments(module: str, home: Path, project: Path) -> list[str]:
    arguments = ["list", "--project", str(project), "--json"]
    if module != "mcp":
        arguments += ["--home", str(home)]
    return arguments


def module_baseline(module: str, root: Path, out: Path) -> dict:
    home = root / module / "home"
    project = root / module / "work" / "project"
    state = root / module / "state"
    project.mkdir(parents=True, exist_ok=True)
    state.mkdir(parents=True, exist_ok=True)
    BUILDERS[module](home, project)
    fixed_times(root / module)
    env = environment(home, state)
    control = str(REPO / "python" / "bin" / f"agent-{module}ctl")
    document = run([control, *list_arguments(module, home, project)], env)
    write_json(out / module / "list.json", normalized(document, root))
    rows = rows_of(document)
    summary = {"rows": len(rows)}
    if module in ("hooks", "mcp"):
        summary.update(removal_baseline(module, control, env, root, out, project, home, rows))
    return summary


def removal_baseline(module: str, control: str, env: dict[str, str], root: Path, out: Path,
                     project: Path, home: Path, rows: list[dict]) -> dict:
    for row in rows:
        arguments = ["prepare-remove", "--project", str(project), "--id", row["id"], "--json",
                     "--transaction-id", TRANSACTION_ID]
        if module != "mcp":
            arguments += ["--home", str(home)]
        prepared = run([control, *arguments], env)
        if not prepared.get("ok"):
            continue
        write_json(out / module / "prepare-remove.json", normalized(prepared, root))
        applied = run([control, *[argument.replace("prepare-remove", "remove-prepared") for argument in arguments],
                       "--payload-stdin"], env, stdin=json.dumps(prepared) + "\n")
        write_json(out / module / "remove-prepared.json", normalized(applied, root))
        store = Path(env["XDG_STATE_HOME"]) / "fileblade" / f"{module}-recovery"
        records = sorted(store.glob("*.json"))
        if not records:
            raise SystemExit(f"{module}: removal wrote no recovery record")
        record = json.loads(records[0].read_text(encoding="utf-8"))
        record["createdAt"] = "{CREATED_AT}"
        write_json(out / module / "recovery-record.json", normalized(record, root))
        return {"removedId": row["id"], "recoveryRecords": len(records)}
    raise SystemExit(f"{module}: no row could be prepared for removal")


def transcript(home: Path) -> None:
    def iso(moment: datetime) -> str:
        return moment.astimezone(timezone.utc).isoformat(timespec="milliseconds").replace("+00:00", "Z")

    def envelope(kind: str, uuid: str, at: datetime, content) -> dict:
        return {"parentUuid": None, "isSidechain": False, "userType": "external", "cwd": "/work/project",
                "sessionId": "0f1e2d3c", "version": "2.1.258", "gitBranch": "main", "type": kind,
                "uuid": uuid, "timestamp": iso(at), "message": {"role": kind, "content": content}}

    def called(at: datetime, identity: str, name: str, **arguments) -> dict:
        part = {"type": "tool_use", "id": identity, "name": name, "input": arguments,
                "caller": {"type": "direct"}}
        return envelope("assistant", "a-" + identity, at, [part])

    def typed(at: datetime, uuid: str, name: str) -> dict:
        content = (f"<command-message>{name}</command-message>\n"
                   f"<command-name>/{name}</command-name>\n<command-args></command-args>")
        return envelope("user", uuid, at, content)

    records = []
    for index, day in enumerate((0, 1, 2)):
        at = FROZEN_DAY + timedelta(days=day)
        records.append(called(at, f"t{index}0", "Skill", skill="pdf"))
        records.append(called(at + timedelta(minutes=1), f"t{index}1", "mcp__fileblade__list"))
        records.append(typed(at + timedelta(minutes=2), f"u{index}", "review"))
    path = home / ".claude" / "projects" / "-work-project" / "session.jsonl"
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text("".join(json.dumps(record, separators=(",", ":")) + "\n" for record in records),
                    encoding="utf-8")


USAGE_PROGRAM = """
import json
import sys
from agent_usage import query

items = [{"id": "skill-pdf", "name": "pdf", "source": "user"},
         {"id": "skill-review", "name": "review", "source": "user"}]
document = {
    "skillCounts": query.skill_counts(items),
    "skillUsage": query.skill_usage(items),
    "mcpUsage": query.mcp_usage(),
}
sys.stdout.write(json.dumps(document))
"""


def dump_store(path: Path) -> dict:
    connection = sqlite3.connect(f"file:{path}?mode=ro", uri=True)
    try:
        version = connection.execute("PRAGMA user_version").fetchone()[0]
        tables = [name for (name,) in connection.execute(
            "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name")]
        document = {"userVersion": version, "tables": {}}
        for table in tables:
            columns = [row[1] for row in connection.execute(f"PRAGMA table_info({table})")]
            count = connection.execute(f"SELECT count(*) FROM {table}").fetchone()[0]
            document["tables"][table] = {"columns": columns, "rowCount": count}
        events = connection.execute(
            "SELECT agent, call, at, kind, origin, server, name, subagent, failed FROM event ORDER BY agent, call").fetchall()
        document["events"] = [list(row) for row in events]
        return document
    finally:
        connection.close()


def usage_baseline(root: Path, out: Path) -> dict:
    home = root / "usage" / "home"
    state = root / "usage" / "state"
    state.mkdir(parents=True, exist_ok=True)
    transcript(home)
    fixed_times(root / "usage")
    env = environment(home, state)
    result = subprocess.run([sys.executable, "-B", "-c", USAGE_PROGRAM], env=env, text=True,
                            timeout=120, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if result.returncode != 0:
        raise SystemExit(f"usage queries failed: {result.stderr}")
    document = json.loads(result.stdout)
    today = datetime.now().astimezone().date().isoformat()
    document = normalized(document, root)
    document = json.loads(json.dumps(document).replace(today, "{TODAY}"))
    write_json(out / "usage" / "queries.json", document)
    store = state / "omarchy" / "fileblade" / "agent-usage.sqlite3"
    if not store.exists():
        raise SystemExit("the usage store was not written")
    shutil.copy2(store, out / "usage" / "agent-usage.sqlite3")
    dump = dump_store(store)
    write_json(out / "usage" / "store-dump.json", dump)
    return {"events": len(dump["events"])}


def main() -> int:
    parser = argparse.ArgumentParser(description="Generate the frozen Python behaviour fixtures")
    parser.add_argument("--root", type=Path, default=DEFAULT_ROOT,
                        help="sandbox the fixtures are built in; row ids hash these paths")
    parser.add_argument("--out", type=Path, default=DEFAULT_OUT, help="directory the fixtures are written to")
    parser.add_argument("--modules", default="", help="comma separated subset to regenerate; the rest keep their recorded manifest entry")
    arguments = parser.parse_args()
    selected = [name for name in arguments.modules.split(",") if name]
    root = arguments.root.resolve()
    out = arguments.out.resolve()
    if root.exists():
        shutil.rmtree(root)
    root.mkdir(parents=True)
    previous = {}
    if (out / "manifest.json").exists():
        previous = json.loads((out / "manifest.json").read_text(encoding="utf-8")).get("modules", {})
    manifest = {"frozen": "2026-09-17", "root": str(root), "modules": {}}
    for module in list(BUILDERS) + ["usage"]:
        if selected and module not in selected:
            if module in previous:
                manifest["modules"][module] = previous[module]
            continue
        if module == "usage":
            manifest["modules"][module] = usage_baseline(root, out)
        else:
            manifest["modules"][module] = module_baseline(module, root, out)
    write_json(out / "manifest.json", manifest)
    print(json.dumps(manifest, indent=2))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
