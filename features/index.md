This file was written by an agent.

# FileBlade features

Focused guides to each observable workflow, grouped by workspace, pane, file operation, integration and delivery. Each completed page includes a native-resolution screenshot and a short original-speed MP4. Coverage: all 111 workflows have reviewed screenshot/MP4 pairs, including real read-only SFTP browsing on Omen.

[Evidence standard](evidence.md) · [Coverage](catalog.json) · [v0.2.0 release notes](release/release-notes.md) · [v0.2.0 readiness](release/readiness.md) · [Jev decisions](release/decisions.md)

## Workspace

- [Open and close sidebars](workspace/blades.md)
- [Move focus between blades and applications](workspace/focus.md)
- [Dock and float a blade](workspace/docking.md)
- [Resize a blade](workspace/resizing.md)
- [Arrange sections](workspace/sections.md)
- [Switch pane tabs](workspace/tabs.md)
- [Move tabs between sections](workspace/move-tabs.md)
- [Choose where blades appear](workspace/monitors.md)
- [Restore saved layouts](workspace/persistence.md)
- [Follow Omarchy themes](workspace/theme.md)
- [Follow desktop bar placement](workspace/bar.md)
- [Configure blade behavior](workspace/settings.md)
- [Customize tree keybindings](workspace/custom-keys.md)

## Panes / Files

- [Navigate the file tree](panes/files/navigation.md)
- [Expand and collapse folders](panes/files/folding.md)
- [Select multiple files](panes/files/selection.md)
- [Follow filesystem changes](panes/files/watching.md)
- [Enter and revisit paths](panes/files/location.md)
- [Show hidden entries](panes/files/hidden-files.md)
- [Sort file rows](panes/files/sorting.md)
- [Choose file columns](panes/files/columns.md)
- [Adjust row density](panes/files/density.md)
- [Customize the tree toolbar](panes/files/toolbar.md)
- [Search the current folder](panes/files/search.md)
- [Combine search filters](panes/files/search-syntax.md)
- [Search nested folders](panes/files/deep-search.md)
- [Jump to recent folders](panes/files/quick-navigation.md)
- [Pin favorite locations](panes/files/favorites.md)
- [Color folders](panes/files/folder-colors.md)
- [See Git changes](panes/files/git-status.md)
- [Choose repository summary fields](panes/files/git-summary.md)
- [See drive capacity](panes/files/drive-space.md)
- [Browse mounted drives](panes/files/drives.md)
- [Browse images and video posters](panes/files/media-grid.md)
- [Sort media newest or oldest first](panes/files/media-sort.md)
- [Resize media thumbnails](panes/files/media-size.md)
- [Navigate the media calendar](panes/files/media-calendar.md)
- [Inspect hourly media activity](panes/files/media-hours.md)
- [Mount, unmount and eject drives](panes/files/device-actions.md)

## Panes / Properties

- [Inspect file properties](panes/properties/metadata.md)
- [Preview text](panes/properties/text-preview.md)
- [Preview images and posters](panes/properties/media-preview.md)

## Panes / Notes

- [Write persistent notes](panes/notes/editing.md)
- [Manage note tabs](panes/notes/notebooks.md)
- [Read note counts](panes/notes/counts.md)

## Panes / Skills

- [Browse agent skills](panes/skills/inventory.md)
- [Inspect skill activity](panes/skills/activity.md)
- [Filter skills by day](panes/skills/day-filter.md)
- [Share and unshare skills](panes/skills/management.md)

## Panes / Memory

- [Browse agent instructions](panes/memory/inventory.md)
- [Share instruction files](panes/memory/management.md)

## Panes / Hooks

- [Inspect agent hooks](panes/hooks/inventory.md)
- [Manage shared hooks](panes/hooks/management.md)

## Panes / Mcp

- [Inspect MCP servers](panes/mcp/inventory.md)
- [Inspect MCP calls](panes/mcp/activity.md)
- [Manage shared MCP definitions](panes/mcp/management.md)

## Panes / Branches

- [Inspect Git branches](panes/branches/branches.md)
- [Discover linked worktrees](panes/branches/worktrees.md)
- [Inspect worktree status](panes/branches/status.md)

## Panes / Welcome

- [Start with the Welcome pane](panes/welcome/onboarding.md)
- [Choose built-in panes](panes/welcome/catalogue.md)
- [Read offline help](panes/welcome/help.md)

## Operations

- [Create a file](operations/new-file.md)
- [Create a folder](operations/new-folder.md)
- [Rename a file](operations/rename.md)
- [Copy and paste files](operations/copy-paste.md)
- [Move files](operations/move.md)
- [Drag files between locations](operations/drag-drop.md)
- [Resolve destination conflicts](operations/conflicts.md)
- [Trash files](operations/trash.md)
- [Restore from Trash](operations/restore.md)
- [Permanently delete Trash items](operations/permanent-delete.md)
- [Undo and redo file operations](operations/undo-redo.md)
- [Inspect archive contents](operations/archive-list.md)
- [Extract an archive](operations/archive-extract.md)
- [Create an archive](operations/archive-create.md)
- [Change file permissions](operations/permissions.md)
- [Track and cancel work](operations/progress-cancel.md)
- [Inspect operation history](operations/audit.md)

## Integrations

- [Open files in applications](integrations/open-files.md)
- [Edit in the configured editor](integrations/editor.md)
- [Open a file in a new terminal](integrations/terminal.md)
- [Copy and paste paths](integrations/copy-path.md)
- [Choose a drag action](integrations/drop-wheel.md)
- [Customize the drop wheel](integrations/wheel-configuration.md)
- [Target terminal panes](integrations/terminal-panes.md)
- [Review changed files with Hunk](integrations/git-review.md)
- [Pick files from the CLI](integrations/picker.md)
- [Use shell selection widgets](integrations/shell-widget.md)
- [Expose selection to coding agents](integrations/agent-context.md)
- [Register native autostart](integrations/autostart.md)
- [Install blade keybindings](integrations/bindings.md)
- [Open folders with FileBlade](integrations/folder-default.md)
- [Reveal a file through the desktop](integrations/reveal.md)
- [Choose files for applications](integrations/portal.md)
- [Browse an SFTP location](integrations/sftp.md)
- [Discover tailnet peers](integrations/tailnet.md)
- [Add an extension pane](integrations/extensions.md)
- [Run an extension action](integrations/custom-actions.md)
- [Browse an extension image gallery](integrations/gallery.md)
- [Scaffold an extension](integrations/scaffolding.md)

## Delivery

- [Install the native app](delivery/native-install.md)
- [Build the Arch package](delivery/package.md)
- [Migrate legacy state](delivery/migration.md)
- [Update and roll back](delivery/update-rollback.md)
- [Remove a native installation](delivery/remove.md)
- [Drain before maintenance](delivery/safe-shutdown.md)
- [Check runtime health](delivery/diagnostics.md)
- [Recover interrupted operations](delivery/recovery.md)
- [Verify release payloads](delivery/release-verification.md)

## Panes / Activity

- [Forget stored agent usage](panes/activity/retention.md)
