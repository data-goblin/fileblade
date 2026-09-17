This file was written by an agent.

# Changelog

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
