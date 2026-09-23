This file was written by an agent.

# MCP: how calls are counted

The MCP blade's Uses columns and heatmap read the same store as Skills; the
rules per agent are in [agent usage history](../agent-usage.md), section
"What is recorded". In short: a call is a Claude Code `mcp__<server>__<tool>`
tool use, a Codex `McpToolCall` item, a Copilot `tool.execution_start` with an
MCP server name, or an OpenCode tool whose name is not built in. Resource
lists and reads count under their server. A failed result marks the call
failed. Codex `function_call` items with a namespace are not MCP calls.

Server matching follows the name the agent recorded, sanitized the way each
agent does it (`my.server` becomes `my_server` for Claude Code). Namespaced
definitions also retain the owning package identity when matching calls.

The heatmap tooltip is the shared `UsageHeatmap` component, so it shows the
same date, total, unit and stacked bar as the Skills blade. There is no
per-day filter for MCP yet. Transcript writes still refresh the MCP lanes in
full; the counts-only refresh exists for Skills only.
