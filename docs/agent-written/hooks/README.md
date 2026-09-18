This file was written by an agent.

# Hooks module

The Hooks blade lists the hook definitions each agent's settings declare and
lets you add, label, remove and restore them through the in-process hooks core module
(`list`, `recovery-list`, `apply`, `label`, `prepare-remove`,
`remove-prepared`, `restore`, `discard`). A modification that would leave the
settings file unparsable, or drop a top-level key it did not mean to remove,
is refused and the original stays byte-identical.

Nothing is counted for hooks. The usage store (`agent-usage.sqlite3`) holds
skill, command and MCP events only; hook executions never reach it, so the
Hooks blade has no Uses column and no heatmap.
