#!/usr/bin/env bash
set -euo pipefail
: "${OVM:?set OVM to the dedicated test VM harness}"
python3 - "$OVM" <<'PY'
import base64
import json
import os
from pathlib import Path
import shlex
import subprocess
import sys
import time

ovm = sys.argv[1]
runtime = os.environ.get("FILEBLADE_VM_SOURCE", "/home/omarchy/fileblade-inventory-perf")
fixture = "/home/omarchy/fileblade-inventory-fixture"

def run(*args):
    return subprocess.check_output([ovm, *args], text=True, timeout=20).strip()

def guest(command):
    return run("ssh", command)

def ipc(target, method, *args):
    return guest(shlex.join(["qs", "-n", "-p", runtime + "/app", "ipc", "call", target, method, *args]))

def control(method, *args):
    return ipc("data-goblin.fileblade.control", method, *args)

def inventory():
    return json.loads(ipc("fileblade.qualification", "inventories"))

def wait_for(predicate, label):
    deadline = time.monotonic() + 15
    while True:
        state = inventory()
        if predicate(state):
            return state
        assert time.monotonic() < deadline, (label, state, json.loads(ipc("fileblade.qualification", "status")))
        time.sleep(0.025)

def rows(state, module):
    return state.get(module, {}).get("rows", [])

def uses(state, module):
    return next((row.get("uses") for row in rows(state, module) if row["name"] == "performance-alpha"), None)

def activity_total(state, module):
    return sum(day[1] for day in state[module]["activity"]["days"])

def slots(edge, module):
    document = [{"id": "inventory-perf-" + edge, "modules": [{"module": module}]}]
    encoded = base64.b64encode(json.dumps(document).encode()).decode()
    reply = json.loads(control("setBladeSlots", edge, "base64:" + encoded))
    assert reply["slots"][0]["modules"][0]["module"] == module, reply

def write(path, text, append=False):
    encoded = base64.b64encode(text.encode()).decode()
    script = "from pathlib import Path; import base64; p=Path(" + repr(path) + "); p.parent.mkdir(parents=True,exist_ok=True); p.open(" + repr("a" if append else "w") + ").write(base64.b64decode(" + repr(encoded) + ").decode())"
    return int(guest(shlex.join(["python3", "-c", script + "; import time; print(time.time_ns() // 1000000)"])))

original = json.loads(ipc("data-goblin.fileblade", "blades"))["blades"]
original_root = json.loads(ipc("data-goblin.fileblade", "status"))["rootPath"]
results = {}
try:
    write(fixture + "/.claude/skills/performance-alpha/SKILL.md", "---\nname: performance-alpha\ndescription: Inventory performance fixture\n---\n")
    write(fixture + "/.mcp.json", json.dumps({"mcpServers": {"performance-alpha": {"command": "not-executed"}}}))
    control("closeBlade", "left")
    control("closeBlade", "right")
    control("setRoot", fixture)
    slots("left", "skills")
    slots("right", "mcp")
    start = time.monotonic()
    control("openBlade", "left")
    control("openBlade", "right")
    wait_for(lambda state: all(any(row["name"] == "performance-alpha" for row in rows(state, module)) for module in ["skills", "mcp"]), "first rows")
    results["both_first_rows_ms_including_ipc"] = round((time.monotonic() - start) * 1000, 1)
    wait_for(lambda state: uses(state, "skills") is not None and uses(state, "mcp") is not None, "first counts")
    wait_for(lambda state: all(state[module].get("activity") for module in ["skills", "mcp"]), "first activity")
    before = inventory()
    results["first_rows_ms"] = {module: before[module]["firstRowsAt"] - before[module]["openedAt"] for module in ["skills", "mcp"]}
    record = {"type": "assistant", "timestamp": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()), "message": {"content": [
        {"type": "tool_use", "id": "inventory-perf-skill-" + str(time.time_ns()), "name": "Skill", "input": {"skill": "performance-alpha"}},
        {"type": "tool_use", "id": "inventory-perf-mcp-" + str(time.time_ns()), "name": "mcp__performance-alpha__search", "input": {}}
    ]}}
    start = time.monotonic()
    written_at = write("/home/omarchy/.claude/projects/fileblade-inventory-perf/session.jsonl", json.dumps(record) + "\n", append=True)
    wait_for(lambda state: all(uses(state, module) == uses(before, module) + 1 for module in ["skills", "mcp"]), "live counts")
    live = inventory()
    results["live_counts_ms"] = {module: live[module]["countsAt"] - written_at for module in ["skills", "mcp"]}
    results["both_live_counts_ms_including_ipc"] = round((time.monotonic() - start) * 1000, 1)
    for item in record["message"]["content"]:
        item["id"] += "-open"
    script = "import time; f=open('/home/omarchy/.claude/projects/fileblade-inventory-perf/session.jsonl','a'); f.write(" + repr(json.dumps(record) + "\n") + "); f.flush(); print(time.time_ns() // 1000000,flush=True); time.sleep(3); f.close(); print(time.time_ns() // 1000000,flush=True)"
    with subprocess.Popen([ovm, "ssh", shlex.join(["python3", "-c", script])], stdout=subprocess.PIPE, text=True) as writer:
        written_at = int(writer.stdout.readline())
        live = wait_for(lambda state: all(uses(state, module) == uses(before, module) + 2 for module in ["skills", "mcp"]), "counts while writer remains open")
        closed_at = int(writer.communicate(timeout=10)[0])
        assert writer.returncode == 0
        assert all(written_at <= live[module]["countsAt"] < closed_at for module in ["skills", "mcp"]), live
        results["open_writer_counts_ms"] = {module: live[module]["countsAt"] - written_at for module in ["skills", "mcp"]}
    wait_for(lambda state: all(activity_total(state, module) == activity_total(before, module) + 2 for module in ["skills", "mcp"]), "updated activity")
    start = time.monotonic()
    written_at = write(fixture + "/.claude/skills/performance-beta/SKILL.md", "---\nname: performance-beta\ndescription: Newly created skill\n---\n")
    wait_for(lambda state: any(row["name"] == "performance-beta" for row in rows(state, "skills")), "new skill")
    results["new_skill_ms"] = inventory()["skills"]["itemsAt"] - written_at
    results["new_skill_ms_including_ipc"] = round((time.monotonic() - start) * 1000, 1)
    start = time.monotonic()
    written_at = write(fixture + "/.mcp.json", json.dumps({"mcpServers": {"performance-alpha": {"command": "not-executed"}, "performance-beta": {"command": "not-executed"}}}))
    wait_for(lambda state: any(row["name"] == "performance-beta" for row in rows(state, "mcp")), "new MCP server")
    results["new_mcp_ms"] = inventory()["mcp"]["itemsAt"] - written_at
    results["new_mcp_ms_including_ipc"] = round((time.monotonic() - start) * 1000, 1)
    run("shot", "inventory-live-counts")
    control("closeBlade", "left")
    control("closeBlade", "right")
    start = time.monotonic()
    control("openBlade", "left")
    control("openBlade", "right")
    wait_for(lambda state: all(uses(state, module) == uses(before, module) + 2 for module in ["skills", "mcp"]), "cached reopen")
    results["both_reopen_ms_including_ipc"] = round((time.monotonic() - start) * 1000, 1)
    reopened = inventory()
    results["reopen_ms"] = {module: reopened[module]["firstRowsAt"] - reopened[module]["openedAt"] for module in ["skills", "mcp"]}
    run("shot", "inventory-reopened")
    print(json.dumps(results, indent=2))
finally:
    for edge in ["left", "right"]:
        control("closeBlade", edge)
        document = base64.b64encode(json.dumps(original[edge]["slots"]).encode()).decode()
        control("setBladeSlots", edge, "base64:" + document)
        if original[edge]["open"]:
            control("openBlade", edge)
    control("setRoot", original_root)
    guest(shlex.join(["rm", "-rf", "--", fixture, "/home/omarchy/.claude/projects/fileblade-inventory-perf"]))
PY
