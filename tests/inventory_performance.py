import argparse
import json
import os
from pathlib import Path
import statistics
import selectors
import subprocess
import tempfile
import time

parser = argparse.ArgumentParser()
parser.add_argument("binary", type=Path)
parser.add_argument("--transcripts", type=int, default=1000)
parser.add_argument("--legacy", action="store_true")
parser.add_argument("--resources-only", action="store_true")
args = parser.parse_args()
binary = args.binary.resolve()
with tempfile.TemporaryDirectory(prefix="fileblade-inventory-perf-") as temporary:
    home = Path(temporary)
    skills = home / ".claude/skills"
    sessions = home / ".claude/projects/test"
    sessions.mkdir(parents=True)
    for index in range(100):
        skill = skills / f"skill-{index}"
        skill.mkdir(parents=True)
        (skill / "SKILL.md").write_text(f"---\nname: skill-{index}\ndescription: Performance fixture\n---\n")
    (home / ".claude.json").write_text(json.dumps({"mcpServers": {f"server-{index}": {"command": "not-executed"} for index in range(40)}}))
    for index in range(args.transcripts):
        record = {"type": "assistant", "timestamp": "2026-09-23T10:00:00Z", "message": {"content": [
            {"type": "tool_use", "id": f"skill-{index}", "name": "Skill", "input": {"skill": "skill-0"}},
            {"type": "tool_use", "id": f"mcp-{index}", "name": "mcp__server-0__search", "input": {}}
        ]}}
        (sessions / f"{index}.jsonl").write_text(json.dumps(record) + "\n")
    env = os.environ | {key: str(home / directory) for key, directory in {
        "HOME": ".", "XDG_STATE_HOME": "state", "XDG_CACHE_HOME": "cache", "XDG_CONFIG_HOME": "config",
        "XDG_DATA_HOME": "data", "CLAUDE_CONFIG_DIR": ".claude", "CODEX_HOME": ".codex",
        "COPILOT_HOME": ".copilot", "GEMINI_HOME": ".gemini", "PI_HOME": ".pi"
    }.items()}
    samples = {}
    def request(module, method, arguments):
        command = [str(binary), "_backend", "helper-read", "--provider", f"fileblade.core.{module}", "--plugin-dir", "", "--helper", "inventory", "--method", method, "--arguments", json.dumps(arguments)]
        started = time.perf_counter()
        result = subprocess.run(command, env=env, capture_output=True, check=True)
        elapsed = (time.perf_counter() - started) * 1000
        document = json.loads(result.stdout)
        assert document["ok"], document
        return document, elapsed
    for module in ([] if args.resources_only else ["skills", "mcp"]):
        listing_args = ["--json", "--project", str(home)] + ([] if args.legacy else ["--no-usage"])
        listing, elapsed = request(module, "list", listing_args)
        samples[f"{module}_first_rows_ms"] = round(elapsed, 1)
        times = [request(module, "list", listing_args)[1] for _ in range(5)]
        samples[f"{module}_warm_rows_ms"] = round(statistics.median(times), 1)
        if module == "skills":
            stubs = [{key: row.get(key, "") for key in ["id", "name", "source"]} for row in listing["items"]]
            counts_args = ["--json", "--items", json.dumps(stubs)]
            count_started = time.perf_counter()
            calls = 0
            while True:
                counts, _ = request(module, "usage-counts", counts_args)
                calls += 1
                if not counts["usageIngestPending"]:
                    break
                assert time.perf_counter() - count_started < 120
            samples["remaining_ingest_ms"] = round((time.perf_counter() - count_started) * 1000, 1)
            samples["ingest_requests"] = calls
            row = next(row for row in stubs if row["name"] == "skill-0")
            assert counts["counts"][row["id"]]["uses"] == args.transcripts
            appended = json.loads((sessions / "0.jsonl").read_text())
            appended["message"]["content"][0]["id"] = "new-skill"
            appended["message"]["content"][1]["id"] = "new-mcp"
            with (sessions / "0.jsonl").open("a") as transcript:
                transcript.write(json.dumps(appended) + "\n")
            counts, elapsed = request(module, "usage-counts", counts_args)
            assert counts["counts"][row["id"]]["uses"] == args.transcripts + 1
            samples["append_to_count_ms"] = round(elapsed, 1)
    process = subprocess.Popen([str(binary), "serve", "--no-recover"], env=env, stdin=subprocess.PIPE, stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    selector = selectors.DefaultSelector()
    selector.register(process.stdout, selectors.EVENT_READ)
    buffered = bytearray()
    def send(document):
        process.stdin.write(json.dumps(document).encode() + b"\n")
        process.stdin.flush()
    def receive():
        deadline = time.monotonic() + 10
        while b"\n" not in buffered:
            assert process.poll() is None, "server exited"
            assert selector.select(max(0, deadline - time.monotonic())), "server response timed out"
            buffered.extend(os.read(process.stdout.fileno(), 65536))
        line, _, rest = buffered.partition(b"\n")
        buffered[:] = rest
        return json.loads(line)
    def switches():
        total = 0
        for task in Path(f"/proc/{process.pid}/task").iterdir():
            for line in (task / "status").read_text().splitlines():
                if line.startswith("voluntary_ctxt_switches:"):
                    total += int(line.split()[1])
        return total
    try:
        send({"v": 1, "type": "hello"})
        assert receive()["ok"]
        for index in range(3):
            send({"v": 1, "type": "subscribe", "id": f"watch-{index}", "generation": 1, "topic": "filesystem", "paths": [str(skills)]})
            assert receive()["type"] == "subscribed"
        first = switches()
        time.sleep(2)
        samples["idle_context_switches_per_second_3_watches"] = round((switches() - first) / 2, 1)
        status = Path(f"/proc/{process.pid}/status").read_text()
        samples["idle_backend_rss_kib"] = int(next(line for line in status.splitlines() if line.startswith("VmRSS:")).split()[1])
        started = time.perf_counter()
        (skills / "watch-event").write_text("event")
        while receive()["type"] != "event":
            pass
        samples["filesystem_event_ms"] = round((time.perf_counter() - started) * 1000, 2)
        started = time.perf_counter()
        send({"v": 1, "type": "cancel", "id": "watch-0", "generation": 1})
        while True:
            frame = receive()
            if frame["type"] == "response" and frame["id"] == "watch-0":
                assert frame["cancelled"]
                break
        samples["watch_cancel_ms"] = round((time.perf_counter() - started) * 1000, 1)
    finally:
        process.stdin.close()
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            process.kill()
            process.wait()
        selector.close()
    print(json.dumps({"transcripts": args.transcripts, "legacy": args.legacy, **samples}, indent=2))
