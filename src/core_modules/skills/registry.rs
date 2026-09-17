pub struct Root {
    pub agent: &'static str,
    pub kind: &'static str,
    pub anchor: &'static str,
    pub path: &'static str,
    pub precedence: Option<i64>,
}

pub const AGENT_LABELS: [(&str, &str); 6] = [
    ("claude-code", "Claude Code"),
    ("codex", "Codex"),
    ("opencode", "OpenCode"),
    ("pi", "Pi"),
    ("copilot-cli", "GitHub Copilot CLI"),
    ("antigravity", "Google Antigravity"),
];

pub const MANAGED_CLAUDE: [(&str, &str); 3] = [
    ("linux", "/etc/claude-code/.claude/skills"),
    (
        "darwin",
        "/Library/Application Support/ClaudeCode/.claude/skills",
    ),
    ("win32", "C:\\Program Files\\ClaudeCode\\.claude\\skills"),
];

pub static ROOTS: [Root; 23] = [
    Root {
        agent: "claude-code",
        kind: "managed",
        anchor: "managed-claude",
        path: "",
        precedence: Some(0),
    },
    Root {
        agent: "claude-code",
        kind: "user",
        anchor: "home",
        path: ".claude/skills",
        precedence: Some(1),
    },
    Root {
        agent: "claude-code",
        kind: "project",
        anchor: "walk",
        path: ".claude/skills",
        precedence: Some(2),
    },
    Root {
        agent: "codex",
        kind: "project",
        anchor: "walk",
        path: ".agents/skills",
        precedence: Some(0),
    },
    Root {
        agent: "codex",
        kind: "user",
        anchor: "home",
        path: ".agents/skills",
        precedence: Some(3),
    },
    Root {
        agent: "codex",
        kind: "system",
        anchor: "absolute",
        path: "/etc/codex/skills",
        precedence: Some(4),
    },
    Root {
        agent: "opencode",
        kind: "project",
        anchor: "walk",
        path: ".opencode/skills",
        precedence: None,
    },
    Root {
        agent: "opencode",
        kind: "project",
        anchor: "walk",
        path: ".claude/skills",
        precedence: None,
    },
    Root {
        agent: "opencode",
        kind: "project",
        anchor: "walk",
        path: ".agents/skills",
        precedence: None,
    },
    Root {
        agent: "opencode",
        kind: "user",
        anchor: "home",
        path: ".config/opencode/skills",
        precedence: None,
    },
    Root {
        agent: "opencode",
        kind: "user",
        anchor: "home",
        path: ".claude/skills",
        precedence: None,
    },
    Root {
        agent: "opencode",
        kind: "user",
        anchor: "home",
        path: ".agents/skills",
        precedence: None,
    },
    Root {
        agent: "pi",
        kind: "user",
        anchor: "home",
        path: ".pi/agent/skills",
        precedence: Some(0),
    },
    Root {
        agent: "pi",
        kind: "user",
        anchor: "home",
        path: ".agents/skills",
        precedence: Some(1),
    },
    Root {
        agent: "pi",
        kind: "project",
        anchor: "walk",
        path: ".pi/skills",
        precedence: Some(2),
    },
    Root {
        agent: "pi",
        kind: "project",
        anchor: "walk",
        path: ".agents/skills",
        precedence: Some(3),
    },
    Root {
        agent: "copilot-cli",
        kind: "user",
        anchor: "home",
        path: ".copilot/skills",
        precedence: None,
    },
    Root {
        agent: "copilot-cli",
        kind: "user",
        anchor: "home",
        path: ".agents/skills",
        precedence: None,
    },
    Root {
        agent: "copilot-cli",
        kind: "project",
        anchor: "walk",
        path: ".github/skills",
        precedence: None,
    },
    Root {
        agent: "copilot-cli",
        kind: "project",
        anchor: "walk",
        path: ".claude/skills",
        precedence: None,
    },
    Root {
        agent: "copilot-cli",
        kind: "project",
        anchor: "walk",
        path: ".agents/skills",
        precedence: None,
    },
    Root {
        agent: "antigravity",
        kind: "user",
        anchor: "home",
        path: ".gemini/antigravity-cli/skills",
        precedence: None,
    },
    Root {
        agent: "antigravity",
        kind: "project",
        anchor: "walk",
        path: ".agents/skills",
        precedence: None,
    },
];

pub const RESERVED_CLAUDE_SUBDIR: &str = "synced";
pub const PROJECT_MARKERS: [&str; 1] = [".git"];

pub fn agents() -> Vec<&'static str> {
    AGENT_LABELS.iter().map(|(id, _)| *id).collect()
}

pub fn label(agent: &str) -> Option<&'static str> {
    AGENT_LABELS
        .iter()
        .find(|(id, _)| *id == agent)
        .map(|(_, label)| *label)
}

pub fn managed_claude(platform: &str) -> Option<&'static str> {
    MANAGED_CLAUDE
        .iter()
        .find(|(key, _)| *key == platform)
        .map(|(_, path)| *path)
}
