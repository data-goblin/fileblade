This file was written by an agent.

# Agent-written documentation

Technical documentation maintained by coding agents. The project `README.md`
is maintained separately.

## Documents

- [Plugin-system design](design.md): why FileBlade extensions work the way
  they do, what Omarchy owns, what FileBlade owns, and what is still planned
- [Architecture](../../ARCHITECTURE.md): how the whole application is split up
  and how data moves through it
- [Keybindings](keybindings.md): user configuration for pane navigation,
  folding, search and help
- [Dragging files](dragging-files.md): which mouse gesture keeps a drag inside
  FileBlade, which one hands the files to another application, and the setting
  that chooses between them
- [Git status](git-status.md): status markers and repository summary preferences
- [Extensions](../../EXTENSIONS.md): the public contract for blade modules and
  other FileBlade extension points
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
- [Agent guidelines](../agent-guidelines.md): rules coding agents must follow in
  this repository

## Filename convention

Lowercase `design.md` means **technical product design**: behavior, ownership,
boundaries, decisions, and tradeoffs.

Uppercase `DESIGN.md` is reserved for a **visual design system** if FileBlade
ever needs one: colors, typography, spacing, components, and writing style.
FileBlade does not currently have that separate document.
