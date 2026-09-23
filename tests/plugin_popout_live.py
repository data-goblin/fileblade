#!/usr/bin/env python3
import argparse
import json
import subprocess
import time

parser = argparse.ArgumentParser(description="Exercise an installed plugin popout in an isolated Wayland session")
parser.add_argument("module")
parser.add_argument("--confirm-isolated-session", action="store_true", required=True)
args = parser.parse_args()


def run(*command):
    return subprocess.check_output(command, text=True, timeout=10).strip()


def state():
    return json.loads(run("qs", "ipc", "-n", "-p", "/usr/share/omarchy/shell", "call", "--",
                          "data-goblin.fileblade.popout", "status"))


for _ in range(2):
    assert run("fileblade", "popout", args.module) == "opened"
    time.sleep(0.5)
    opened = state()
    assert opened["opened"] and opened["loaded"] and not opened["notice"], opened
    refused = subprocess.run(["fileblade", "popout", "missing-release-check-module"],
                             capture_output=True, timeout=10)
    assert refused.returncode != 0
    assert state() == opened
    run("wtype", "-k", "Escape")
    time.sleep(0.2)
    closed = state()
    assert not closed["opened"] and not closed["loaded"], closed
assert run("fileblade", "popout", args.module) == "opened"
assert run("fileblade", "popout") == "closed"
assert not state()["loaded"]
print("Installed popout: open, refusal, keyboard focus, Escape, repeat and unload passed")
