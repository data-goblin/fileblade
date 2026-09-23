This file was written by an agent.

# Changelog

## 0.2.0 (unreleased)

This file was written by an agent.

- Native installation now supports verified runtime payloads, explicit desktop
  roles, migration receipts, safe shutdown, updates and rollback. Release archives
  refuse mixed versions; the bootstrap verifies the advertised version and target
  before running the payload installer. Desktop integration handles launcher paths
  containing spaces and reserved characters.
- Native CLI backend helpers reuse the running authority for reads, persistence
  and mutation admission. `fileblade doctor` reports blocked startup recovery as
  unhealthy and explains the failure even when the backend and view still answer.
- Skills, Memory, Hooks and MCP are built into core and run through the Rust
  backend without Python runtime helpers.
- Theme changes reload native colors in both directions. Holding Space before a
  drag leaves the blade still opens the drop wheel; Herdr targets its focused pane.
  Hunk is disabled when the selected path has no Git changes, and opening a file in
  a new terminal starts Neovim. Hourly media timelines retain each day with 24 bars.

- The Branches module opens as a tab beside Properties. Branches come first and a linked worktree nests under the branch it has checked out, with its path as summary; Enter on it opens that folder. The main checkout is no longer listed as a worktree. The Status column uses the same `↑3 ↓1 M4 A1 ?2` layout and colours as the repository summary on the tree's root row.
- The Skills heatmap tooltip shows the date, the day's total in blue, the unit, and a stacked bar of agent, you and scheduled shares. Left click or Enter on a day shows only the skills used that day; Esc clears. The header sits under the heatmap. A transcript write refreshes the counts and the heatmap without rescanning skill directories, so "Scanning" only shows for real rescans. `docs/agent-written/skills/usage-counting.md` records the counting audit.
- The native app's Settings gain a Desktop integration section with five independent switches: open folders with FileBlade, reveal in FileBlade, file chooser, Hyprland bindings and start at login. All are off after install, update and first launch; turning one off restores the previous handler when it has not changed since and keeps a newer choice otherwise, and the row says which. `fileblade native roles status|enable|disable` drives the same switches, and removal reverses every owned entry.
- The Delete dialog on a Skills, Memory, Hooks or MCP row now reads Cancel, Delete <thing>, Deactivate, and adds Delete symlink when the row is a link, so a linked skill can lose its link without losing the files it points at. A deactivated symlink says Delete symlink forever.
- Every blade tab puts its search field first and the tab header under it: Files, Skills, Memory, Hooks, MCP and Branches share the order. The header's Search button is gone; `/` reveals the field as before.
- Dragging a blade edge no longer throws the pointer to the middle of the screen or leaves the blade at its maximum width. While a resize drag is in progress the pointer watch that hands focus back to the workspace stays quiet, so no Hyprland focus dispatch and no cursor warp happens mid-drag.
- A thin blue bar under the file toolbar shows how full the drive holding the open folder is, as `df` reports it. Hover it for the used, total and free space and the percentage. `fileblade space [PATH]` prints the same numbers, and "Drive usage under the toolbar" in Files settings hides the bar per tab.
- Skill Uses count every agent FileBlade manages, not only Claude Code: Codex `$skill` mentions and SKILL.md reads, OpenCode `skill` tool calls from its SQLite store, Copilot CLI `skill.invoked` events, Antigravity slash commands and SKILL.md reads, and Pi SKILL.md reads. MCP Uses add OpenCode and Copilot CLI calls.
- An open Skills or MCP tab watches the agents' transcript directories and refreshes its counts and heatmap a few seconds after an agent writes, with a 60-second safety refresh, so Uses climb while agents run.
- Disabled skills stay listed. The bin listing keeps its last rows and retries when a read fails, and the Skills tab re-reads the bin after every rescan.
- Right-clicking a disabled skill opens the same menu as any other skill, acting on the bin's copy; Open and Reveal on a disabled skill go to that copy too.
- Keybindings from a newer or older FileBlade no longer cancel each other: unknown actions and unparseable bindings are dropped with a notice instead of rejecting the whole file, and neither `keybindings.json` nor `settings.json` has its `filebladeVersion` moved backward by an older FileBlade. Both reads report who wrote the file.
- A Branches module lists every branch and worktree of the current repository with kind, status, last update, author and subject, and switches branches from the list, including remote-only ones. Open it from `Expand into Branches` in the Switch branch popup or with `fileblade branches`; `fileblade branches close` removes it and `fileblade branches list` prints it.
- The tree header no longer lists volumes, which the Drives view already lists. Turn "Volumes in the tree" back on in Files settings to restore them, which also restores the one-click route to a connected tailnet peer that lived in that panel.
- Choose which file toolbar buttons appear, in a Toolbar group in Files settings. Every hidden button except Drives keeps a keyboard route.
- Row density returns to five stops named XS to XL. A saved percentage that is not a stop resolves to the nearest one.
- Delete acts on Skills, Memory, Hooks and MCP rows, or says why it cannot. It previously did nothing at all: the consent refusal never reached a module, and Trash was offered for definitions that have no file of their own.
- A hook modification is refused when the resulting configuration file would not parse, or would lose a top-level key it did not mean to remove. The original is left byte-identical.
- The Notes footer shows when the open note was last edited and its word and character counts, in place of the save state and byte total. Save failures and conflicts still show in the notice above the editor.
- Skills and MCP tabs show a daily activity heatmap under the search field. Hover a day, or move to it with the arrow keys, for its uses; the Activity button in the tab header hides it.
- Skill and MCP use is kept as a private history in `~/.local/state/omarchy/fileblade/agent-usage.sqlite3`, so Uses no longer shrinks when an agent deletes old transcripts. `fileblade usage skills` and `fileblade usage mcp` print the daily history, and `fileblade usage forget [--before YYYY-MM-DD]` deletes it. The old `~/.cache/omarchy/fileblade/agent-usage.json` cache is removed.
- MCP server rows expand to the tools, resources, resource lists and prompts agents used through them, with use and failure counts. MCP Uses now counts Codex calls, resource and prompt use, and servers whose names contain characters such as `.`, which previously always showed 0.
- Skill Uses counts plugin skills called as `<plugin>:<skill>`. A typed skill command is no longer lost when another tab reads the transcript first.

- Image gallery primitives for modules: `ImageGrid` with month sections and a cursor, `ThumbnailCache` over the backend thumbnail request, a right-hand `TimelineScrubber` with years, month dots and a scrub pill, and a five-step `ImageSizeControl`.
- Module definitions accept an `icon` image; the picker, the settings sheet and `PaneHeader` draw it tinted through `ModuleIcon`.
- `BladePopout` hosts any module under a bar icon through a `BladeContext` popout seam.
- The plugin catalog treats a companion that also declares `bar-widget` as enabled when `shell.json` lists it, not only when a bar entry names it.

## 0.1.3

- UI changes
  - Hide internal caches and reset Quick Nav selection for new queries.
  - Font size: scale the text in every blade from the General settings.
- Update notification
  - Name the available release version instead of counting commits.
  - List companion updates separately, one bullet per extension.

## 0.1.2

- Add a pinned workflow with signed backend build provenance.

## 0.1.1

- Bound plugin operations and require explicit consent before cleanup.
- Ask once whether FileBlade may empty the trash automatically.

## 0.1.0

- Install the Welcome extensions at reviewed commits and refuse installation if a pin is unavailable.
- Keep recent trash-test fixtures relative to the test run so retention checks do not fail as dates pass.
- Synchronize trash confirmations across monitors and bind answers to their requests.
- Super+B and Super+Shift+B close open blades with one press.
- Choose Active, mirrored All, or a named monitor lock in Settings.
- Clamp blade widths to each monitor without changing saved preferences.
- Cancel close-tab confirmations when their slot or tabs change.
- Keep delayed navigation focus on its original monitor.
- Cancel pending trash requests when their confirming pane hides or unloads.
- Cancel menus, wheels, drags, and focus when their monitor disconnects.
- Resolve hover, drops, and explicit window focus across visible monitors.
- Keep directional focus routing on each blade's assigned monitor.
- Open undocked blades on their monitor, preserving ordinary window movement.
- Recognize Foot server windows as terminal targets.
- Prevent terminal actions from reaching another shared window.
