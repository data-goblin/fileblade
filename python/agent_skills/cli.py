from __future__ import annotations

import argparse
import json
import os
import sys
from typing import Any

from fileblade_paths import NativePath, parse_path
import agent_usage
from fileblade_inventory import MAX_OUTPUT_BYTES, SCOPES, WatchPlan, encoded

from . import apply as applying, discovery, registry

def emit(payload: dict[str, Any]) -> None:
    sys.stdout.buffer.write(encoded(payload))
    sys.stdout.buffer.flush()

def environment(args: argparse.Namespace) -> discovery.Environment:
    home = parse_path(args.home) if args.home else os.path.expanduser("~")
    return discovery.Environment(
        home=os.path.abspath(home),
        anchor=parse_path(args.project) if args.project else "",
        exact=bool(getattr(args, "exact", False)),
        platform=args.platform or sys.platform,
        prefix=os.path.abspath(parse_path(args.prefix)) if args.prefix else "",
        environ=dict(os.environ),
        enforce_secure_system=not args.prefix,
        scope=getattr(args, "scope", "all"),
    )

def listing(args: argparse.Namespace) -> dict[str, Any]:
    with WatchPlan() as plan:
        payload = plan.finish(discovery.collect(environment(args)))
    payload.update(agent_usage.attach_skills(payload["items"]))
    payload["usageWatchPaths"] = [NativePath(path) for path in agent_usage.watch_paths()]
    return payload

def usage(args: argparse.Namespace) -> dict[str, Any]:
    if args.items is not None:
        return agent_usage.skill_usage(item_stubs(args.items), scoped=True)
    return agent_usage.skill_usage(discovery.collect(environment(args))["items"])

def item_stubs(raw: str) -> list[dict[str, Any]]:
    try:
        decoded = json.loads(raw or "[]")
    except ValueError:
        return []
    return [entry for entry in decoded if isinstance(entry, dict)][:1024] if isinstance(decoded, list) else []

def usage_counts(args: argparse.Namespace) -> dict[str, Any]:
    return agent_usage.skill_counts(item_stubs(args.items))

def usage_day(args: argparse.Namespace) -> dict[str, Any]:
    items = item_stubs(args.items) if args.items is not None else discovery.collect(environment(args))["items"]
    return agent_usage.skill_day(items, args.day)

def roots(args: argparse.Namespace) -> dict[str, Any]:
    env = environment(args)
    rows = [
        {
            "agent": entry.agent,
            "kind": entry.kind,
            "path": NativePath(path),
            "precedence": entry.precedence,
            "exists": os.path.isdir(path),
            "doc": entry.doc,
        }
        for entry, path in discovery.resolve_roots(env)
    ]
    return {"ok": True, "schemaVersion": 1, "count": len(rows), "roots": rows}

def apply(args: argparse.Namespace) -> dict[str, Any]:
    return applying.apply(environment(args), args.id, list(args.agent), args.state)

def agents(_: argparse.Namespace) -> dict[str, Any]:
    return {
        "ok": True,
        "schemaVersion": 1,
        "agents": [
            {"id": key, "name": label, "doc": registry.DOCS.get(key, "")}
            for key, label in registry.AGENT_LABELS.items()
        ],
    }

def build_parser() -> argparse.ArgumentParser:
    parser = argparse.ArgumentParser(prog="agent-skillsctl")
    commands = parser.add_subparsers(dest="command", required=True)
    for name, handler, helptext in (
        ("list", listing, "List discovered agent skills"),
        ("usage", usage, "Daily skill use history for a project"),
        ("usage-counts", usage_counts, "Use counts for the given skill stubs, without discovery"),
        ("usage-day", usage_day, "Skills of a project used on one local day"),
        ("roots", roots, "List every documented root and whether it exists"),
        ("agents", agents, "List supported agents and their documentation"),
        ("apply", apply, "Link or unlink one skill for one or more agents"),
    ):
        command = commands.add_parser(name, help=helptext)
        command.add_argument("--project", default="")
        command.add_argument("--exact", action="store_true")
        command.add_argument("--home", default="")
        command.add_argument("--prefix", default="")
        command.add_argument("--platform", default="")
        command.add_argument("--json", action="store_true")
        command.set_defaults(handler=handler)
        if name == "list":
            command.add_argument("--scope", choices=SCOPES, default="all")
        if name == "usage-counts":
            command.add_argument("--items", default="[]")
        if name in ("usage", "usage-day"):
            command.add_argument("--items")
        if name == "usage-day":
            command.add_argument("--day", required=True)
        if name == "apply":
            command.add_argument("--id", required=True)
            command.add_argument("--agent", action="append", required=True)
            command.add_argument("--state", choices=("on", "off"), required=True)
    return parser

def main(argv: list[str] | None = None) -> int:
    args = build_parser().parse_args(argv)
    payload = args.handler(args)
    emit(payload)
    return 0 if payload.get("ok", True) else 1
