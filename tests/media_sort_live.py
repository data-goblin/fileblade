#!/usr/bin/env python3
import argparse
import json
import subprocess
import time
from pathlib import Path

parser = argparse.ArgumentParser(description="Check media date ordering in a real isolated Wayland session")
parser.add_argument("newest", type=Path)
parser.add_argument("oldest", type=Path)
parser.add_argument("--sort", nargs=2, type=int, required=True, metavar=("X", "Y"))
parser.add_argument("--first-tile", nargs=2, type=int, required=True, metavar=("X", "Y"))
parser.add_argument("--output", required=True)
parser.add_argument("--confirm-isolated-session", action="store_true", required=True)
args = parser.parse_args()


def run(*command):
    return subprocess.check_output(command, text=True, timeout=10).strip()


def click(point):
    run("democtl", "click", *map(str, point), "--output", args.output)
    time.sleep(0.3)


def first_is(path):
    click(args.first_tile)
    actual = json.loads(run("fileblade", "status"))["selectedPath"]
    assert actual == str(path.resolve()), (actual, path)


first_is(args.newest)
click(args.sort)
first_is(args.oldest)
click(args.sort)
run("wtype", "-k", "space")
time.sleep(0.3)
first_is(args.oldest)
click(args.sort)
first_is(args.newest)
print("Media order: newest default, mouse reversal, keyboard reversal and newest restoration passed")
