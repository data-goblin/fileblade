pub const COPILOT_PASCAL_ALIASES: [(&str, &str); 12] = [
    ("SessionStart", "sessionStart"),
    ("SessionEnd", "sessionEnd"),
    ("UserPromptSubmit", "userPromptSubmitted"),
    ("PreToolUse", "preToolUse"),
    ("PostToolUse", "postToolUse"),
    ("PostToolUseFailure", "postToolUseFailure"),
    ("Stop", "agentStop"),
    ("SubagentStop", "subagentStop"),
    ("ErrorOccurred", "errorOccurred"),
    ("PreCompact", "preCompact"),
    ("PermissionRequest", "permissionRequest"),
    ("Notification", "notification"),
];

pub const CANONICAL_EVENTS: [(&str, &[(&str, &str)]); 13] = [
    (
        "SessionStart",
        &[
            ("claude-code", "SessionStart"),
            ("codex", "SessionStart"),
            ("copilot-cli", "sessionStart"),
        ],
    ),
    (
        "SessionEnd",
        &[
            ("claude-code", "SessionEnd"),
            ("codex", "SessionEnd"),
            ("copilot-cli", "sessionEnd"),
        ],
    ),
    (
        "UserPromptSubmit",
        &[
            ("claude-code", "UserPromptSubmit"),
            ("codex", "UserPromptSubmit"),
            ("copilot-cli", "userPromptSubmitted"),
        ],
    ),
    (
        "PreToolUse",
        &[
            ("claude-code", "PreToolUse"),
            ("codex", "PreToolUse"),
            ("copilot-cli", "preToolUse"),
            ("antigravity", "PreToolUse"),
        ],
    ),
    (
        "PostToolUse",
        &[
            ("claude-code", "PostToolUse"),
            ("codex", "PostToolUse"),
            ("copilot-cli", "postToolUse"),
            ("antigravity", "PostToolUse"),
        ],
    ),
    (
        "PostToolUseFailure",
        &[
            ("claude-code", "PostToolUseFailure"),
            ("copilot-cli", "postToolUseFailure"),
        ],
    ),
    (
        "PermissionRequest",
        &[
            ("claude-code", "PermissionRequest"),
            ("codex", "PermissionRequest"),
            ("copilot-cli", "permissionRequest"),
        ],
    ),
    (
        "Notification",
        &[
            ("claude-code", "Notification"),
            ("copilot-cli", "notification"),
        ],
    ),
    (
        "Stop",
        &[
            ("claude-code", "Stop"),
            ("codex", "Stop"),
            ("copilot-cli", "agentStop"),
            ("antigravity", "Stop"),
        ],
    ),
    (
        "SubagentStart",
        &[
            ("claude-code", "SubagentStart"),
            ("codex", "SubagentStart"),
            ("copilot-cli", "subagentStart"),
        ],
    ),
    (
        "SubagentStop",
        &[
            ("claude-code", "SubagentStop"),
            ("codex", "SubagentStop"),
            ("copilot-cli", "subagentStop"),
        ],
    ),
    (
        "PreCompact",
        &[
            ("claude-code", "PreCompact"),
            ("codex", "PreCompact"),
            ("copilot-cli", "preCompact"),
        ],
    ),
    (
        "PostCompact",
        &[("claude-code", "PostCompact"), ("codex", "PostCompact")],
    ),
];

pub fn native_event(agent: &str, event: &str) -> String {
    if agent == "copilot-cli"
        && let Some((_, native)) = COPILOT_PASCAL_ALIASES
            .iter()
            .find(|(alias, _)| *alias == event)
    {
        return (*native).to_string();
    }
    event.to_string()
}

pub fn canonical(agent: &str, event: &str) -> String {
    let name = native_event(agent, event);
    for (canonical_name, table) in CANONICAL_EVENTS {
        for (candidate, native) in table {
            if *candidate == agent && *native == name {
                return canonical_name.to_string();
            }
        }
    }
    format!("{agent}:{name}")
}

pub fn to_agent(canonical_name: &str, target: &str) -> Option<String> {
    for (name, table) in CANONICAL_EVENTS {
        if name != canonical_name {
            continue;
        }
        return table
            .iter()
            .find(|(agent, _)| *agent == target)
            .map(|(_, event)| (*event).to_string());
    }
    None
}

pub fn is_canonical(name: &str) -> bool {
    CANONICAL_EVENTS
        .iter()
        .any(|(candidate, _)| *candidate == name)
}

pub fn mapped_event(source_agent: &str, event: &str, target_agent: &str) -> Option<String> {
    let key = canonical(source_agent, event);
    if is_canonical(&key) {
        return to_agent(&key, target_agent);
    }
    if target_agent == source_agent {
        Some(native_event(source_agent, event))
    } else {
        None
    }
}
