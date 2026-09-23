This file was written by an agent.

# Instructions for agents

Communicate concisely in plain language. Preserve the project's design intent.

## About this project

- This is a native FileBlade extension that contributes the
  `{{MODULE_ID}}` blade module to FileBlade (`data-goblin.fileblade`). Read
  FileBlade's EXTENSIONS.md before changing anything the host contract covers.
- `Provider.qml` owns shared work; `blades/Module.qml` stays visual. Do not add
  a `Process`, a second cache or a watcher to the module.
- Reuse the host's services and conventions: `context.service("files")` for
  opening and revealing, `context.ui.url(...)` for shared widgets,
  `context.settings` for declared settings, `context.metrics` for columns.

## Versions

- Stay at `0.1.0` until the first release. `hostContract` is a compatibility
  number, not a release version.

## Testing

- `tests/run` is the local gate: the manifest contract, the banner generator,
  QML tests and qmllint.
- Test live changes in an isolated VM, not the working desktop. After any QML
  change, inspect native FileBlade's logs for import and runtime errors.
- Report skipped or pending checks honestly.

## Authorship

- Begin agent-written Markdown with `This file was written by an agent.`
- `README.md` is human-maintained. Edit it only when the owner names the file.

## Cleaning up

- Remove temporary files, screenshots and staging directories when finished.
  Leave only deliverables untracked.
