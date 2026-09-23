This file was written by an agent.

# Skills: how uses are counted

The Skills blade's Uses columns, its heatmap and its per-day filter all read
the private usage store described in [agent usage history](../agent-usage.md).
This page is the skills-specific contract and the audit of it.

## The numbers

```yaml
uses:          usesAgent + usesUser
usesAgent:     kind skill events: the agent loaded the skill on its own
usesUser:      kind command events with origin user whose name is a skill name: you typed /name
usesScheduled: kind command with origin scheduled (Claude Code scheduled tasks); not in uses
failed:        skill events whose tool result was an error
name match:    an item counts events named exactly as the skill, plus <namespace>:<skill> for namespaced skills
```

Per agent, a skill event is: Claude Code's `Skill` tool call; a Codex turn
that reads a `SKILL.md` (one per turn and skill); a Copilot `skill.invoked`
that is not user-invoked; an Antigravity or Pi read of a `SKILL.md`; an
OpenCode `skill` tool call. A user event is a typed `/name` (Claude Code,
Antigravity), a `$skill` mention (Codex) or a user-invoked Copilot skill.
Every typed command is stored; only names that match a discovered skill are
counted, so `/model` or `/goal` never appear.

## Audit, 2026-09-17, on the live store of this machine

- 231 typed commands and 111 Claude skill calls: zero cases of one typed
  command followed within two minutes by a Skill call of the same name, so a
  typed skill is not counted twice
- one day's 71 uses were 7 Claude calls, all from sub-agents (`isSidechain`),
  and 64 Codex reads of `SKILL.md`, 34 of them the same skill in one session.
  Codex counts one event per turn and skill, so a long session that re-reads a
  skill inflates `usesAgent`. That is the documented rule, not a bug; a
  per-session dedupe is an open product decision
- sub-agent uses carry `subagent = 1` in the store but are not shown apart
- a transcript is read once past its stored offset. A file whose device,
  inode, size and mtime match its row and whose offset equals its size is only
  `stat`ed, never opened; `tests/core_modules_usage_golden.rs` proves it with a
  file made unreadable after its first read

## Refresh policy

```yaml
transcript write (inotify, 2.5 s debounce):  usage-counts for the listed skills and usage for the heatmap;
                                              no skill directory scan
60 s poll while transcript dirs are watched:  the same two calls
skill directory change, Shift+R, a mutation:  full discovery (the Scanning status), then counts
usage-counts:                                 takes the item stubs (id, name, source) as --items, answers counts per id
```

## Heatmap tooltip and day filter

The tooltip shows the date, the day's total in the theme blue, the unit in
dim text, a stacked bar of agent, you and scheduled shares, and the legend.
A left click or Enter on a day asks `usage-day --day YYYY-MM-DD` and shows
only the skills used that day; the header status says how many; Esc, or a
second click on the same day, clears it. Days are local dates.
