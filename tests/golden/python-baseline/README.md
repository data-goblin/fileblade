# Python baseline fixtures

These files are the behaviour of the Python helpers in `python/`, frozen on
2026-09-17, before the port to Rust. They exist so the Rust replacements can be
compared against what the Python version actually produced, not against a
reading of the code.

The generator and the `python/` tree it ran are gone; these fixtures are frozen
and cannot be regenerated. They remain the reference the Rust modules are
compared against. When it still existed, the generator fixed fixture
timestamps, fixed the removal transaction id, and replaced every value that
still varied with a placeholder (`{ROOT}` for the sandbox root, `{TODAY}` for
the local date a query ran on, `{CREATED_AT}` for the recovery record's
creation time), so the JSON fixtures were reproducible at the same root.
A row's `created` is the file's real birth time and was not normalized.

`usage/agent-usage.sqlite3` is not reproducible that way: its `source` table
keeps the real device and inode numbers of the machine it was built on. It is a
frozen artifact, kept as it was written.

Row ids hash real paths, so the sandbox root is part of the contract: the
fixtures were built under `/tmp/fileblade-python-baseline`, the generator's
default, and the Rust suites reproduce that root when they compare.

```yaml
manifest.json:            the root, the frozen date and the row counts per module
skills/list.json:         agent-skillsctl list on the tests/core_modules/skills fixtures
memory/list.json:         agent-memoryctl list on the tests/core_modules/memory fixtures
hooks/list.json:          agent-hooksctl list on the tests/core_modules/hooks fixtures
                          (the Rust hooks module is compared against it row for row)
hooks/prepare-remove.json:   the prepared removal payload for the first removable row
hooks/remove-prepared.json:  the result of performing that prepared removal
hooks/recovery-record.json:  the record fileblade_recovery.RecoveryStore wrote for it
mcp/list.json:            agent-mcpctl list on the tests/core_modules/mcp fixtures
                          (the Rust mcp module is compared against it row for row)
mcp/prepare-remove.json:     the prepared removal payload for the first removable row
mcp/remove-prepared.json:    the result of performing that prepared removal
mcp/recovery-record.json:    the record fileblade_recovery.RecoveryStore wrote for it
mcp/toml-recovery-record.json: a codex config.toml, the text agent_mcp.records.detach_toml
                             left behind, and the TOML recovery record it wrote, so a Rust
                             restore can be checked at the recorded character offset
usage/agent-usage.sqlite3:   the store python/agent_usage wrote from a synthetic
                             claude transcript of three days of skill, command and
                             MCP calls. A Rust store must answer this file identically
usage/store-dump.json:       the schema version, the tables and every event row of
                             that store, for comparison without opening SQLite
usage/queries.json:          skill_counts, skill_usage and mcp_usage over that store
```

The fixture inputs contain deliberate canary strings (`CANARY-…`,
`SECRET_SENTINEL_DO_NOT_EMIT`). They are invented values, not credentials. A
canary appearing in a listing payload is a redaction failure; a canary
appearing in a removal payload is correct, because that payload is the
preimage the restore writes back.
