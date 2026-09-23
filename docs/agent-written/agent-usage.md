This file was written by an agent.

# Agent usage history

How the Skills and MCP blades count skill and MCP use, where that history is
kept, and how the activity heatmap and `fileblade usage` read it.

Coding agents already write a transcript of every session. FileBlade reads
those transcripts, keeps one row per skill call, typed command and MCP call in
a private SQLite store, and answers the Uses columns, the observed MCP rows and
the daily heatmap from that store. Events outlive the transcripts they came
from, so the store holds history that cannot be rebuilt once an agent deletes
old transcripts.

Six agents feed the store. Their store names are shorter than the labels the
Skills blade shows:

```yaml
claude:       Claude Code
codex:        Codex CLI
opencode:     OpenCode
copilot:      GitHub Copilot CLI
antigravity:  Google Antigravity CLI (agy)
pi:           Pi
```

The code is the Rust module `src/core_modules/usage/`:

```yaml
records.rs:  turns one transcript record of any agent into events
store.rs:    store path, schema, lock, the per-agent sources, the budgeted ingest and the
             directories a blade watches for new transcript bytes
query.rs:    name matching, the list totals, the daily history and forget
mod.rs:      the environment the store reads, the watch-path list and the MCP core routes
```

The Skills routes (`list`, `usage`, `usage-counts`, `usage-day`) and the MCP
`usage`, `list`, `usage-counts` and `usage-forget` routes are answered in this process:
`CoreRoute` dispatch in `src/module_helpers.rs` routes per method into
`src/core_modules/`. No route spawns a helper program.

## The store

```yaml
path:         $XDG_STATE_HOME/omarchy/fileblade/agent-usage.sqlite3
              (XDG_STATE_HOME falls back to ~/.local/state; a relative value is ignored)
lock:         agent-usage.sqlite3.lock beside it
permissions:  directory created 0700, database and lock created 0600; SQLite creates the
              -wal and -shm files with the database's permissions
engine:       the sqlite3 command-line tool over the same file, run as a supervised subprocess
              with -batch -bail -json and the statements on stdin (-readonly for the foreign
              opencode database). The shipped binary is a static musl build linked with rust-lld
              and no C toolchain, so no SQLite library can be linked into it; the file format is
              unchanged. Packaging requires the sqlite3 command and the sqlite package.
              WAL journal, busy_timeout 5000 through .timeout on every invocation
version:      PRAGMA user_version = 3. Version 1 gains retention, pending-failure and forgotten-identity
              tables, version 2 gains the agent column on failure, both without losing history.
              Unknown versions are refused; existing tables are never dropped
old cache:    $XDG_CACHE_HOME/omarchy/fileblade/agent-usage.json and agent-usage.tmp are
              deleted whenever the store opens. Nothing is migrated from them
```

It is state, not cache: a cache cleaner must not erase history. Uninstalling
the plugin keeps it with the rest of `~/.local/state/omarchy/fileblade/`.

The `-json` output of the sqlite3 tool renders a TEXT column as its characters
and a BLOB column as one character per byte. Reading a column byte for byte is
therefore only correct when the statement casts it: `SELECT CAST(<column> AS
BLOB)` paired with `Row::blob` or `Row::text_bytes`, which reverse that
per-byte rendering. Reading an uncast TEXT column through those accessors
truncates every code point to its low byte, so `雪` arrives as the single
byte `0xea`. Columns read without a cast use `Row::text`, and a column whose
bytes matter (NUL-carrying identities, foreign OpenCode payloads that may be
stored as either TEXT or BLOB) is cast in the statement and decoded once,
before anything parses it.

```sql
CREATE TABLE source (
  id INTEGER PRIMARY KEY,
  agent TEXT NOT NULL,
  path TEXT NOT NULL UNIQUE,
  device INTEGER NOT NULL,
  inode INTEGER NOT NULL,
  size INTEGER NOT NULL,
  mtime INTEGER NOT NULL,
  offset INTEGER NOT NULL,
  project INTEGER REFERENCES project(id)
);
CREATE TABLE project (id INTEGER PRIMARY KEY, path TEXT NOT NULL UNIQUE);
CREATE TABLE coverage (agent TEXT PRIMARY KEY, first_at INTEGER NOT NULL);
CREATE TABLE event (
  agent TEXT NOT NULL,
  call TEXT NOT NULL,
  at INTEGER NOT NULL,
  kind TEXT NOT NULL,
  origin TEXT NOT NULL,
  server TEXT NOT NULL DEFAULT '',
  name TEXT NOT NULL,
  subagent INTEGER NOT NULL DEFAULT 0,
  project INTEGER REFERENCES project(id),
  failed INTEGER NOT NULL DEFAULT 0,
  PRIMARY KEY (agent, call)
) WITHOUT ROWID;
CREATE INDEX event_time ON event (kind, at);
CREATE INDEX event_name ON event (kind, server, name, at);
CREATE TABLE retention (id INTEGER PRIMARY KEY CHECK (id = 1), before INTEGER NOT NULL);
CREATE TABLE failure (call TEXT PRIMARY KEY, at INTEGER NOT NULL, agent TEXT NOT NULL DEFAULT 'claude') WITHOUT ROWID;
CREATE TABLE forgotten (identity BLOB PRIMARY KEY) WITHOUT ROWID;
```

```yaml
source:    one row per transcript file: identity, size, mtime in nanoseconds, and the byte
           offset read so far. Codex, Copilot, Antigravity and Pi sources also carry their
           session's project. For an OpenCode database the offset is the newest time_updated
           read, size and mtime fold in the -wal and -shm files, and a read cut short by the
           budget stores size -1 so the next call resumes it
project:   working directories seen in transcripts
coverage:  per agent, the earliest record timestamp ever read. Kept as the minimum across
           runs, so deleting old transcripts never moves it later
retention: one monotonic UTC millisecond cutoff below which ingest cannot insert events
forgotten: SHA-256 digests of forgotten call identities with future timestamps, so a bad clock
           cannot either replay those calls or move the retention cutoff into the future
failure:   failure call IDs, timestamps and agent awaiting their call, allowing newest-first
           ingestion to read a result in a resumed transcript before its original call
event:     one row per use. at is UTC epoch milliseconds
kind:      skill | command | tool | resource-list | resource
origin:    agent | user | scheduled
call:      the agent's own call, item, part or event id where it has one, or the record uuid
           for a typed command (suffixed :1, :2 and so on when one record holds several
           commands). Codex implicit reads use <turn id>:<skill>, Antigravity steps use
           <transcript path>:<step>:<call index>, Antigravity slashes use <conversation or
           workspace>:<timestamp>
```

## What is recorded

Claude Code transcripts:

```yaml
skill:          assistant tool_use named Skill: kind skill, origin agent, name = input.skill
                exactly as written, which may be <plugin>:<skill>
tool:           tool_use named mcp__<server>__<tool>: kind tool, origin agent, server = the
                second segment, name = the rest joined by "__"
resource-list:  tool_use ListMcpResourcesTool: server = input.server or "", name ""
resource:       tool_use ReadMcpResourceTool or ReadMcpResourceDirTool: server =
                input.server as written, name = input.uri without its query or fragment,
                cut to 512 UTF-8 bytes
command:        user record text containing <command-name>/NAME</command-name>, NAME made of
                letters, digits, ":", "_" and "-": kind command, name without the slash,
                origin scheduled when the record has scheduledTaskId, else user. Every
                command is stored, skill or not; classification happens at query time
failure:        dated user tool_result with is_error true marks the matching claude event failed;
                unmatched failures wait for their call, then the pending row is removed
subagent:       1 when isSidechain is true
project:        the record's cwd
```

Codex rollouts (`~/.codex/sessions/YYYY/MM/DD/rollout-*.jsonl` and
`archived_sessions`, checked against codex-cli 0.154.0 source):

```yaml
command:  a response_item message with role user whose passthrough content_item_kinds names
          skills.selected_skill_instructions, or whose text starts with <skill>: the user typed
          $skill. kind command, origin user, name from the <name> tag
skill:    event_msg item_completed with a CommandExecution item whose parsed_cmd has a read of a
          path ending in /SKILL.md (or such a path in the command when parsed_cmd is missing):
          kind skill, origin agent, name = the SKILL.md directory, one event per turn and skill
          because Codex dedupes the same way. In legacy history_mode files the same rule reads
          the custom_tool_call exec input string
tool:     item_completed with an McpToolCall item: server = item.server (the configured name),
          name = item.tool, failed when item.status is not "completed" or item.result.isError is
          true. A response_item function_call with a namespace is not an MCP call: that key
          also marks Codex's built-in collaboration and clock tools, so it is ignored
project:  session_meta payload.cwd
```

OpenCode databases (`$XDG_DATA_HOME/opencode/opencode*.db`, one per release
channel, read with a query-only connection; the old JSON `storage/` tree is not
read):

```yaml
stable:   part rows whose data.type is tool with state.status completed or error. tool "skill"
          is kind skill named state.input.name; any other tool name containing "_" and not a
          built-in is kind tool with an empty server and the full name, resolved to a server at
          query time. call = data.callID, at = state.time.end, start or time_created, project =
          the session's directory, failed when the status is error
beta:     session_message rows of type assistant: every content item of type tool is read the
          same way, with call = item.id (or <message id>:<index>) and at = time.completed or
          created
watermark: rows are read where time_updated is above the stored offset, so a running call is
          counted once it completes
```

Copilot CLI sessions (`$COPILOT_HOME/session-state/*/events.jsonl`, default
`~/.copilot`, shapes from the Copilot SDK event schema):

```yaml
skill:    skill.invoked: trigger user-invoked is kind command, origin user; any other trigger
          except context-load is kind skill, origin agent; context-load is a preload and is
          skipped. name = data.name, call = the event id
tool:     tool.execution_start with mcpConfigServerName or mcpServerName: server = that name,
          name = mcpToolName (else toolName), call = toolCallId. Built-in tools have no server
          and are not recorded
failure:  tool.execution_complete with success false marks the call failed
project:  session.start or session.context_changed data.context.cwd
subagent: 1 when the event carries agentId
```

Antigravity CLI (`~/.gemini/antigravity-cli`):

```yaml
command:  history.jsonl records with type slash_command: kind command, origin user, name = the
          word after "/". Every slash is stored; skills are classified at query time
skill:    brain/<conversation>/.system_generated/logs/transcript_full.jsonl steps with a
          tool_calls entry named view_file whose args.AbsolutePath ends in /SKILL.md: kind skill,
          origin agent, name = the SKILL.md directory, failed when the step status is ERROR
project:  history.jsonl workspace; transcript steps carry no directory
mcp:      not recorded; the transcript's MCP tool-name form is undocumented
```

Pi (`~/.pi/agent/sessions/**/*.jsonl`, session format 3):

```yaml
skill:    assistant message content of type toolCall whose arguments.path (or file_path) ends in
          /SKILL.md, or a bash toolCall whose command names such a path: kind skill, origin
          agent, name = the SKILL.md directory, call = the toolCall id
failure:  a toolResult message with isError true marks its toolCallId failed
project:  the session header cwd
mcp:      not recorded; Pi reaches MCP only through extensions with their own tool-name schemes
```

A record without a usable timestamp or identity is skipped.

Never recorded: tool arguments, skill arguments, command arguments, tool
results, message text, thinking, token counts, resource query strings and
fragments, and the `attributionSkill`, `attributionMcpServer` and
`attributionMcpTool` fields. Those attribution fields describe context rather
than the call, lag behind the real call, and are left alone; a prompt's
follow-up records carrying `attributionSkill: mcp__<server>__<prompt>` are not
a skill use.

## Reading transcripts

This file was written by an agent.

The blades request `list --no-usage` for their project and user lanes. Discovery
returns rows without opening the usage database or reading transcripts. The
shared provider keeps those rows when its last view closes, so reopening paints
the previous inventory while a fresh scan checks for changes. The first scan
starts on the next event-loop turn; subsequent filesystem bursts coalesce for
50 ms without indefinitely postponing a scan.

Both blades use one `usage-counts` request at a time. Skills passes its current
item identities; MCP resolves all declarations together to preserve ambiguous
server attribution. Counts merge into existing rows, survive unrelated rescans,
and continue ingesting when the activity grid is hidden. Daily history uses
`usage --no-ingest` after counts refresh, avoiding another transcript ingest.
Closing the last view cancels reads and watches; an accepted mutation retains
its existing completion lifecycle.

Each listing and counts response supplies `usageWatchPaths`: agent roots, the
nearest existing ancestor of a missing root, 24 recent Claude project folders,
today's and yesterday's Codex day folders and their year/month ancestors,
16 recent Copilot sessions, 8 recent Antigravity log folders and 8 recent Pi
session folders, capped at 96 directories. A transcript event schedules counts
after 100 ms. Only the usage subscription opts into write notifications, so
records become visible even while the agent keeps its transcript open. Events
arriving while a request runs queue one follow-up. Continuous
writes cannot keep restarting the debounce and postpone refresh indefinitely.
The 60-second fallback covers directories outside this bounded watch set.
New watch installation reconciles once to cover the subscription setup gap.

Filesystem subscriptions block on inotify. Their cancellation check runs every
500 ms when idle, reducing idle timeout wakeups from 20 to 2 per second per
watch. File events wake the poll immediately; cancellation can take up to 500 ms.

```yaml
roots:        claude: $CLAUDE_CONFIG_DIR/projects, else ~/.claude/projects, every *.jsonl
              below, subagent transcripts included
              codex: $CODEX_HOME/sessions and $CODEX_HOME/archived_sessions, else ~/.codex/...
              opencode: $XDG_DATA_HOME/opencode/opencode*.db
              copilot: $COPILOT_HOME/session-state/*/events.jsonl, else ~/.copilot/...
              antigravity: $GEMINI_HOME/antigravity-cli/history.jsonl and
              brain/*/.system_generated/logs/transcript_full.jsonl, else ~/.gemini/...
              pi: $PI_HOME/agent/sessions, else ~/.pi/agent/sessions, every *.jsonl below
walk cap:     8192 regular files per agent, taken in directory walk order
order:        newest mtime first
lock:         non-blocking flock on the lock file, retried every 50 ms for up to 4 s. A helper
              that never gets it skips ingest and reports pending
budget:       3 s from the start of ingest, lock wait included. Checked between records,
              including pieces of oversized records; unfinished work reports pending
unchanged:    a fully read file whose device, inode, size and mtime match its source row is skipped
offsets:      reading resumes at the stored offset. A changed device or inode, or a size below
              the offset, restarts the file at byte 0
partial line: reading stops at the first line without a trailing newline, so a half-written
              record waits for the next call
record cap:   lines over 4 MiB are skipped using bounded reads; offsets inside these discarded
              lines resume skipping until their newline. Malformed or excessively nested JSON is skipped
prefilter:    each agent's lines are parsed only when they contain a marker of a record that can
              produce an event (Claude: "Skill", mcp__, McpResource, command-name or "is_error";
              Codex: McpToolCall, SKILL.md, selected_skill_instructions, <skill> or session_meta;
              Copilot: skill.invoked, tool.execution_, session.start or
              session.context_changed; Antigravity: SKILL.md or slash_command; Pi: SKILL.md,
              toolResult or "session"). On a read from byte 0, lines carrying "timestamp" or
              "created_at" are also parsed until the first record with a real timestamp, so
              coverage starts where the file does
transaction:  transcript chunks append to a SQL batch; batches flush at 1 MiB of SQL or at
              the end of the ingest budget. Events, failures, coverage and source offsets
              commit together. Retention is read once per ingest, under the same file lock
              now held by forget, so erased events cannot be replayed by a concurrent ingest.
              This removes two retention-query processes and a write process per transcript.
duplicates:   the (agent, call) key makes re-reading idempotent and drops a command copied into
              a resumed session
unreadable:   files that cannot be stat'ed or opened are counted and reported, never fatal
vanished:     after a scan that finished within budget, source rows whose file no longer exists
              are deleted. Events are never deleted by ingest
```

Measured during the review with 1,008 transcripts (8.28 GiB): the revised
store completed in calls of 3.02 s, 3.01 s, 3.01 s and 1.41 s during concurrent
validation; a warm call took 0.02 s. An earlier run of the previous reader
took 3.04 s and 2.02 s under different load, so these are not a controlled
speed comparison. Both produced identical event totals (233 commands, 104
skill calls and 788 tool calls). Chunk commits bound recovery work if a
helper is killed during a large file.

## Inventory performance checks

This file was written by an agent.

On 2026-09-23, the isolated fixture in `tests/inventory_performance.py`
contained 100 skills, 40 MCP declarations and 1,000 short transcripts. The
baseline was the bundled backend from `f22c85c`; both ran on the same host.

| Measurement | Before | After |
| --- | ---: | ---: |
| First Skills listing | 3,063 ms | 25 ms |
| First MCP listing, history already ingested | 53 ms | 10 ms |
| Idle voluntary context switches per second, three watches | 80 | 8 |

The new backend ingested the fixture in one 165 ms request and reflected an
appended record in a 52 ms count request. These CLI measurements exclude UI
scheduling. On the real local inventory, discovery returned 41 skills in
35 ms and six MCP definitions in 22 ms. No history was read for either scan.

The isolated Omarchy 4.0.2 VM used four CPUs, 4 GiB RAM and software rendering.
A fresh runtime exposed Skills rows in 252 ms and MCP rows in 149 ms. Across
two runs, live counts updated in 234–503 ms, counts from an open writer in
94–349 ms, and a new skill or MCP declaration in 57–286 ms. Reopening exposed
cached rows in the same event-loop turn. These are inventory model timestamps,
not measured frame presentation times; screenshots confirmed rendered rows
and counts. The VM scenario also checks that daily activity gains both uses.
Large first-time histories still fill progressively under the ingest budget.

`tests/run` passed on an isolated copy of the current tracked source and these
new regression files: 757 Rust tests passed, seven existing tests were ignored,
and 832 QML checks passed. The bundled backend rebuilt byte for byte. Unrelated
untracked feature experiments in the shared checkout were excluded. The affected
VM scenario passed separately; the complete GUI scenario suite was not run.

Run the self-contained backend benchmark with:

```bash
python3 tests/inventory_performance.py ./fileblade-bin
python3 tests/inventory_performance.py /path/to/previous/fileblade-bin --legacy
```

`--resources-only` limits it to idle watches, event delivery and cancellation.
The VM regression is `tests/vm/expectations/48-inventory-performance.sh`.
It requires `OVM` pointing at a dedicated running guest and a source runtime
at `/home/omarchy/fileblade-inventory-perf` (override with
`FILEBLADE_VM_SOURCE`), launched with `FILEBLADE_QUALIFICATION=1` and the current
backend in `bin/fileblade`. It restores blade configuration and removes its
fixture. Stop the runtime before copying source updates into the guest.

## Name matching

Matching lives in `query.rs` and is shared by `list` and `usage`.

```yaml
sanitize(name):   every character outside [a-zA-Z0-9_-] becomes "_"; for a name starting with
                  "claude.ai " also collapse runs of "_" and strip them from both ends.
                  This is the rule Claude Code 2.1.258 uses to build mcp__ tool names
skill row:        events of kind skill or command named exactly the row name, or
                  <plugin>:<row name> when the row source is plugin:<plugin>@<marketplace>
mcp, claude:      the row's event server is sanitize(name). A plugin-scope row uses
                  plugin_<sanitize(plugin)>_<sanitize(name)>, using source.plugin from the
                  manifest name or installed registry identity, independent of cache layout
                  Stored Claude servers are sanitized before comparison, so a resource call
                  recorded with input.server "my.server" or "plugin:toolkit:docs" meets the
                  same row as mcp__my_server__ or mcp__plugin_toolkit_docs__ tool calls
mcp, codex:       the event server equals the configured name
mcp, copilot:     the event server equals the configured name (mcpConfigServerName, else the
                  display name the CLI recorded)
mcp, opencode:    OpenCode names a tool sanitize(server) + "_" + tool with the same character
                  rule as Claude. Events are stored with the full name and an empty server;
                  at query time the longest sanitized name of the OpenCode definitions in the
                  scan that prefixes the tool name owns the event, and the remainder is the
                  tool name shown under observed
mcp, other:       definitions of any other agent never match and report 0
ambiguity:        two or more definitions of one agent resolving to the same event server all
                  report 0, carry usageAmbiguous true and list no observed entries. MCP scans
                  all scopes before filtering the requested lane, so splitting the UI into
                  project and user lanes cannot hide a collision
mcp prompt:       a claude command named mcp__<server>__<prompt> counts as kind prompt for that
                  server, origin user or scheduled
```

Totals, for a skill row and for an MCP definition:

```yaml
usesAgent:      skill: skill events. mcp: tool, resource and resource-list events
usesUser:       skill: command events with origin user. mcp: prompt events with origin user
usesScheduled:  command or prompt events with origin scheduled
uses:           usesAgent + usesUser; scheduled runs are left out
failed:         skill: failed skill events. mcp: failed events of any kind
```

## Helper methods

Both helpers are core-module inventory helpers, reached through the resident
backend's `helper-read` and `helper-write` requests. `CoreRoute::permits` in
`src/module_helpers.rs` admits `usage` as a read for Skills and MCP and
`usage-forget` as a write for MCP only; both modules' `source.json` declare the
same methods. A store failure (a SQLite or filesystem error) never fails a
whole inventory.

### `list` (both)

Unchanged payload shape. Each row carries `uses`, `usesAgent`, `usesUser`,
`usesScheduled` and `failed` at top level and inside `metrics`. The document
adds:

```yaml
usageTranscripts:    source rows for the agents that feed the helper (skills: all six;
                     mcp: claude, codex, opencode and copilot)
usageWatchPaths:     the transcript directories a blade should watch, see Reading transcripts
usageUnreadable:     files this call could not read
usageIngestPending:  true when the budget or the lock left transcripts unread
usageAmbiguous:      mcp only, the number of ambiguous definitions, 0 when none
usageError:          "usage store unavailable" in place of the fields above when the store
                     failed; rows then have no counts
```

When the store answers, every MCP definition also carries `observed`, `[]`
when empty: up to 64 entries sorted by uses descending, then name, then kind.

```json
{"kind": "tool", "name": "execute_csharp_script", "uses": 133, "failed": 1, "lastUsed": "2026-09-14"}
```

`kind` is `tool`, `resource`, `resource-list` or `prompt`; `name` is `""` for a
resource list; `uses` leaves scheduled prompt runs out, so a prompt only a
timer ran is listed with `uses` 0; `lastUsed` is the local date of the newest
event.

### `usage` (both, read)

```text
skills usage --json --project PATH [--exact] [--items JSON]
mcp    usage --json
```

```json
{"ok": true, "schemaVersion": 1, "kind": "skill", "coverageStart": "2026-05-06",
 "until": "2026-09-16", "ingestPending": false,
 "days": [["2026-09-14", 6, 4, 2, 1, 0]]}
```

```yaml
kind:           "skill" or "mcp"
days[]:         [local date, uses, agent, user, scheduled, failed], only days with at least
                one event, ascending, limited to the 160 weeks (1120 days) ending today
local date:     date(at / 1000, 'unixepoch', 'localtime'), so TZ in the backend's environment
                decides the day
coverageStart:  local date of the earliest coverage.first_at of the contributing agents
                (skill: all six; mcp: claude, codex, opencode and copilot), null when nothing
                was ever read
until:          today's local date
skill days:     every skill event of any name, plus command events whose name matches a skill
                discovered for --project (scope all, same matching as list)
--items:        skills only. A JSON array of row stubs ({id, name, source}) replaces discovery
                and scopes the answer to those rows: only skill and command events named by
                them are counted, and no ingest runs. It is how a blade asks about rows it
                already holds without walking the filesystem again
mcp days:       every tool, resource and resource-list event of every agent, plus claude
                commands named mcp__<server>__<prompt>
failure:        {"ok": false, "schemaVersion": 1, "kind": ..., "error": "usage store
                unavailable"}, and the CLI exits 1
```

The helper starts in the app root, not the caller's directory, so the skills
project always arrives through `--project`.

### `usage-counts` and `usage-day` (Skills helper, read)

```text
skills usage-counts --json --items JSON
skills usage-day --json --day YYYY-MM-DD [--items JSON]
```

```yaml
usage-counts:  answers totals for row stubs without discovery: {"ok": true, "schemaVersion": 1,
               "counts": {"<row id>": {uses, usesAgent, usesUser, usesScheduled, failed}},
               usageTranscripts, usageUnreadable, usageIngestPending}. A stub with no id is
               skipped; an unknown name answers zeroes. At most 1024 stubs are read
usage-day:     the rows used on one local day: {"ok": true, "schemaVersion": 1, "day": ...,
               "items": [{id, name, uses, usesAgent, usesUser, usesScheduled, failed}]},
               rows with no uses and no scheduled runs left out, sorted by uses descending then
               name. Without --items the rows come from discovery for --project
day window:    at >= strftime('%s', day, 'utc') * 1000 and below the same for day + 1 day, so the
               window is the real local day. Using the local midnights rather than comparing
               formatted dates keeps a daylight-saving day 23 or 25 hours long
ingest:        neither method ingests; both answer from what is already committed
refusals:      a --day that is not YYYY-MM-DD answers {"ok": false, "schemaVersion": 1,
               "error": "day must be YYYY-MM-DD"}
```

### `usage-forget` (MCP helper, write)

```text
mcp usage-forget [--before YYYY-MM-DD] --json
```

```yaml
with --before:  deletes events before local midnight at the start of that day, raises coverage
                to that moment and records a retention cutoff
without:        deletes every event, coverage row and project row (source.project is cleared).
                Pending failure IDs are removed too. The cutoff advances through now; digests
                of any already-recorded future call identities prevent their replay
privacy:        PRAGMA secure_delete = ON overwrites deleted SQLite cells. No post-commit VACUUM
                can turn completed deletion into an error; allocated space is reused
kept:           source rows and offsets, plus the monotonic retention cutoff. Replaced,
                truncated, copied and previously unread transcripts cannot restore older events
result:         {"ok": true, "schemaVersion": 1, "removed": N}; on a store failure ok false,
                removed 0, error "usage store unavailable", and the CLI exits 1
audit:          the backend's helper-write audit line records provider, helper and method only
```

Forget does not ingest first. The persisted cutoff also excludes unread history
and is checked atomically by concurrent writers. `removed` counts stored rows
deleted, not unread transcript records. Supplying an earlier cutoff later never
restores history. Deleting the database itself resets this retention policy.
Secure deletion does not erase a reader's existing WAL snapshot, filesystem
snapshots, storage blocks or backups.

## In the blades

`ui/ArtifactInventory.qml` has an opt-in history request. The Skills and MCP
providers set `activityMethod: "usage"`; Skills passes `["--json", "--project",
anchorPath]` plus its project arguments, MCP passes `["--json"]`. A request is
queued on `refresh()` (which a finished write also triggers) and on an anchor
change only while at least one heatmap is loaded. It starts after 180 ms and
runs one at a time. A pending answer schedules another request after 500 ms;
completion refreshes the inventory counts. Closing, hiding or shortening all
activity views stops the requests; ordinary inventory counts still refresh. A write or an
anchor change cancels the running request, and a stale generation's answer is
dropped. An anchor change clears the previous activity. A failed answer keeps
the last good payload in `activity`, sets `activityError` and shows the error
in the module header. Pending history displays “Reading activity…”.

`ui/UsageHeatmap.qml` takes that payload, a `calendarRule` for the locale's
first day of the week, and a `unitLabel` ("skill uses" or "MCP calls"). Cells
are `Style.space(8)` with a `Style.space(2)` gap; weeks are
`min(160, floor((width + gap) / pitch))`. Colour levels split at the 25th, 50th
and 75th percentile of the non-zero `uses` across the whole payload, so
widening a blade never recolours a cell. Days before `coverageStart`, or every
day when it is null, have no fill. The modules load it under the tab header, which itself sits under the search field,
only while the tab is open, the tab's `activity` view state is on (the header's
Activity button, default on) and the module is at least `Style.space(300)`
tall. Tab from search explicitly focuses the grid; Tab or Escape from the
grid focuses the tree, and Shift-Tab reveals and focuses search. Hiding a
focused grid returns focus to the tree.

The MCP module makes a definition with a non-empty `observed` list expandable.
`ArtifactTree.expansionKey` keys that expansion by the definition `id`, never
by the configuration path, because several definitions share one file. Each
observed entry becomes a leaf child with a kind glyph, its `uses` and `failed`
as metrics, and no actions. Right or `l` expands a definition, then moves to
its first child; physical folder navigation keeps its existing behavior.

Skills scopes its heatmap to the explicitly selected skill, including selected
files below that skill. A group restores aggregate history. Each tab owns a
debounced, cancellable `usage --items` request; responses from an earlier
selection cannot replace the current view. These scoped reads reuse loaded
skill identities and the existing usage store without discovery or ingestion.
The shared inventory continues refreshing aggregate history and counts.

Day selection sends the current skill identities to `usage-day --items`,
avoiding a filesystem discovery pass. Local-day boundaries become timestamp
ranges so SQLite can use the existing event-time index, including daylight-saving
changes. Changing the selected day retains the previous rows until the new
answer arrives, and generation checks reject stale answers. The tree builds a
set for the returned IDs instead of searching the ID list for every item.
A selected square uses an inset theme foreground border and the other cells
render at 25% opacity.

User-visible behaviour is listed in section 48 of the
[UI expectations](../../tests/EXPECTATIONS.md).

## The CLI

```text
fileblade usage skills                           helper-read fileblade.core.skills usage
                                                 --project <current directory> --json
fileblade usage mcp                              helper-read fileblade.core.mcp usage --json
fileblade usage forget [--before YYYY-MM-DD]     helper-write fileblade.core.mcp usage-forget
```

```yaml
text:       one "YYYY-MM-DD<TAB>uses" line per day, nothing when there are no days;
            forget prints "removed N"
json:       the global -o json / --output json, before or after the subcommand, prints the
            helper document unchanged. There is no subcommand --json
--before:   must be a real zero-padded calendar date; 2026-9-01, 2026-02-30, 20260901 and
            +2026-09-01 exit 2 before the helper runs
errors:     a helper answer with ok false exits 1 with the helper's error
shell:      not needed. The CLI dispatches the backend request in its own process, and the
            usage module runs in that process with the caller's environment
```

## Known limits

```yaml
codex skills:        a $skill mention is recorded by Codex as an injected user fragment, which
                     counts as a typed use. An implicit use is a shell read of SKILL.md, the
                     same signal Codex itself uses for its telemetry; a script run from a
                     skill's scripts/ directory is not counted
codex legacy mcp:    a legacy history_mode rollout that never persisted an McpToolCall item
                     leaves its MCP calls uncounted; function_call namespaces cannot tell an
                     MCP server from a built-in tool group
opencode mcp:        a tool name whose server cannot be resolved from the scanned OpenCode
                     definitions counts for no definition
copilot:             record shapes come from the Copilot SDK event schema; no local session with
                     a skill or MCP event was available when this was written
antigravity:         no MCP events, no project for transcript steps, and a skill slash typed by
                     the user counts through history.jsonl only; whether a skill slash expands
                     into the transcript with a marker is unknown
pi:                  a /skill:name slash leaves no marker in the session file and is not counted;
                     only reads of SKILL.md are
mcp prompts:         only the spelling <command-name>/mcp__<server>__<prompt></command-name> is
                     counted, verified on Claude Code 2.1.258 by typing /probe:greet (MCP). If a
                     later release records prompts differently they stop counting, although
                     the raw command events are still stored for a corrected rule to classify
transcript format:   Claude Code and Codex transcripts are undocumented internal formats and
                     can change in any release. Record types, field names and the command-name
                     tag are assumptions checked against 2.1.258 and local rollouts
unobserved records:  Claude resource tool records come from the 2.1.258 binary's schemas and one
                     probe session, not from everyday history. Codex failure detection has only
                     seen completed calls
plugin mcp servers:  redacted server names or plugin identities report 0 rather than guessing
first read:          history fills progressively, newest transcripts first. The CLI reports
                     ingestPending; visible heatmaps continue until ingestion finishes
walk cap:            beyond 8192 transcripts per agent, which ones are read follows directory
                     walk order, not age
native install:      with FILEBLADE_NATIVE_STATE_ROOT set, fileblade usage forget is refused with
                     "native owner-unavailable", like other CLI mutations; skills and mcp work
erasure:             secure_delete overwrites SQLite cells, not filesystem blocks or backups.
                     Existing readers can retain deleted pages in their WAL snapshot
```
