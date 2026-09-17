from __future__ import annotations

import json
from pathlib import Path

def write(path: Path, text: str = "# heading\n\nbody\n") -> Path:
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(text, encoding="utf-8")
    return path

AUDIT_CANARY = "CANARY-CONFIG-CONTENT"

def build_audit_fixtures(home: Path, project: Path) -> None:
    write(home / ".codex" / "config.toml", "\n".join([
        'model = "gpt-5"',
        'project_doc_max_bytes = 65536',
        'project_doc_fallback_filenames = ["CONTRIBUTING.md", "../escape.md", "ok.md", ".hidden"]',
        f'instructions = "{AUDIT_CANARY}"',
        "",
    ]))
    write(project / "CONTRIBUTING.md", "# contributing fallback\n")
    write(project / "ok.md", "# second fallback\n")

    write(home / ".claude" / "settings.json", json.dumps({
        "claudeMdExcludes": [str(project / "CLAUDE.md"), str(project / ".claude" / "rules" / "api.md")],
        "apiKeyHelper": AUDIT_CANARY,
    }))
    write(project / "CLAUDE.md", "@AGENTS.md\n\n# claude project\n")

    write(home / ".config" / "opencode" / "opencode.json", json.dumps({
        "instructions": ["local-guide.md", "docs/*.md", "https://example.invalid/remote.md",
                         "/etc/passwd", "../outside.md"],
        "apiKey": AUDIT_CANARY,
    }))
    write(home / ".config" / "opencode" / "local-guide.md", "# opencode local guide\n")
    write(home / ".config" / "opencode" / "docs" / "one.md", "# globbed one\n")
    write(home / ".config" / "opencode" / "docs" / "two.md", "# globbed two\n")
    write(home / "outside.md", "# must not be reached\n")

    write(home / "copilot-extra" / "AGENTS.md", "# copilot extra dir\n")
    write(home / "copilot-extra" / ".github" / "instructions" / "nested" / "deep.instructions.md", "# copilot nested\n")

def build_home(home: Path) -> None:
    write(home / ".claude" / "CLAUDE.md", "# user claude\n")
    write(home / ".claude" / "rules" / "style.md", "---\npaths:\n  - \"src/**\"\n---\n# style\n")
    write(home / ".claude" / "projects" / "repo" / "memory" / "MEMORY.md", "# index\n")
    write(home / ".claude" / "projects" / "repo" / "memory" / "feedback_tests.md",
          "---\ntype: feedback\nmodified: 2026-08-31T00:00:00Z\n---\nprefers pytest\n")
    write(home / ".claude" / "projects" / "repo" / "session.jsonl", "{}\n")
    write(home / ".codex" / "AGENTS.md", "# codex user\n")
    write(home / ".config" / "opencode" / "AGENTS.md", "# opencode global\n")
    write(home / ".pi" / "agent" / "SYSTEM.md", "# pi system\n")
    write(home / ".copilot" / "copilot-instructions.md", "# copilot user\n")
    write(home / ".copilot" / "instructions" / "team.instructions.md", "# copilot modular\n")
    write(home / ".gemini" / "GEMINI.md", "# antigravity global\n")
    write(home / ".gemini" / "antigravity-cli" / "plugins" / "demo" / "rules" / "one.md", "# plugin rule\n")
    (home / ".codex" / "memories_1.sqlite").write_bytes(b"SQLite format 3\x00")

def build_project(root: Path) -> None:
    (root / ".git").mkdir(parents=True, exist_ok=True)
    write(root / "AGENTS.md", "# shared agents\n")
    write(root / "CLAUDE.md", "@AGENTS.md\n\n# claude project\n")
    write(root / "CLAUDE.local.md", "# local only\n")
    write(root / ".claude" / "rules" / "api.md", "# api rules\n")
    write(root / ".agents" / "rules" / "workspace.md", "# antigravity workspace\n")
    write(root / ".github" / "copilot-instructions.md", "# repo copilot\n")
    write(root / "GEMINI.md", "# gemini project\n")
