This file was written by an agent.

# Agent-written documentation

Technical documentation maintained by coding agents. The project `README.md`
is maintained separately.

## Documents

- [Feature documentation](../../features/index.md): observable workflows,
  logically grouped by workspace, pane, operation, integration and delivery,
  with screenshot and short video coverage tracked for each feature
- [v0.2.0 readiness](../../features/release/readiness.md): reviewed fixes, local validation and remaining release gates.
- [Release classification decisions](../../features/release/decisions.md): Jev context and probability distributions.
- [Feature evidence standard](../../features/evidence.md): capture geometry, SVG branding, visual review, and provenance
- [Plugin-system design](design.md): why FileBlade extensions work the way
  they do, what Omarchy owns, what FileBlade owns, and what is still planned
- [Architecture](../../ARCHITECTURE.md): how the whole application is split up
  and how data moves through it
- [Keybindings](keybindings.md): user configuration for pane navigation,
  folding, search and help
- [Git status](git-status.md): status markers and repository summary preferences
- [Agent usage history](agent-usage.md): how the Skills and MCP blades record
  skill and MCP use, the private store, helper methods, the `fileblade usage`
  CLI and known limits
- [Skills usage counting](skills/usage-counting.md): what counts as a skill use,
  the 2026-09-17 audit, the refresh policy, the heatmap tooltip and day filter
- [MCP usage counting](mcp/usage-counting.md): what counts as an MCP call and how
  servers are matched
- [Hooks module](hooks/README.md): what the Hooks blade reads and changes; nothing is counted
- [Memory module](memory/README.md): what the Memory blade reads; nothing is counted
- [Extensions](../../EXTENSIONS.md): the public contract for blade modules and
  other FileBlade extension points, including image galleries and bar popouts
- [Security](../../SECURITY.md): trust boundaries, filesystem protections,
  command execution, previews, IPC, and known limits
- [Build provenance](build-provenance.md): manual GitHub delivery builds,
  exact-commit artifact attestations and independent verification
- [Contributing](../../CONTRIBUTING.md): repository layout and development
  workflow
- [Third-party notices](../../THIRD_PARTY_NOTICES.html): generated dependency
  licenses and source links accompanying the bundled backend
- [UI expectations](../../tests/EXPECTATIONS.md): behavior checked in the
  headless desktop test
- [Agent guidelines](../AGENTS.md): rules coding agents must follow in
  this repository

## Filename convention

Lowercase `design.md` means **technical product design**: behavior, ownership,
boundaries, decisions, and tradeoffs.

Uppercase `DESIGN.md` is reserved for a **visual design system** if FileBlade
ever needs one: colors, typography, spacing, components, and writing style.
FileBlade does not currently have that separate document.
