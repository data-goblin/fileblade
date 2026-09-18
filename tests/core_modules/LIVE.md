# Core module integration

The Skills, Memory, Hooks and MCP modules are implemented in the backend under
`src/core_modules/` and answer `helper-read` / `helper-write` in process. Their
behaviour is covered locally by the `tests/core_modules_*.rs` suites, the frozen
payloads under `tests/golden/python-baseline/`, and `tests/qml/tst_core_lifecycle.qml`,
which drives all four providers through the shared `ArtifactInventory`: two-view
sharing, final detach, late callbacks and accepted mutation arguments after a
detach or project change.

The guest runners that drove the live, lifecycle and settings scenarios were
retired together with the Python tree. What remains here are the probes and the
slot check, which run against a real shell.

## Declared slots

`run-slots` writes a temporary root probe that loads `CoreSlotsProbe.qml` under
`qs`, prints the resolved core module slots and fails on `CORE_SLOTS_FAIL`. It
needs a running compositor and no guest preparation:

```sh
tests/core_modules/run-slots
```

## Guest instrumentation

Run only in the assigned Omarchy test guest. Build the runtime before staging,
push the checkout with the harness, and restart the shell. Confirm
`omarchy-shell data-goblin.fileblade bladeModules` lists all four built-ins.

`tests/vm/fixtures/core-live.py prepare` adds a test-only Loader to each guest
core module after backing up its source. `LiveProbe.qml` receives the real
module object and uses its existing Service, provider, mutation and artifact-bin
handlers. It does not instantiate a substitute Service or inventory. No test
hook is present in the production module files.

Select the fixture project with
`omarchy-shell data-goblin.fileblade.control setRoot /tmp/rivet-core-live/project`,
then inspect inventory, apply/unapply, removal/restore and close/reopen for each
module by hand. Skills and Memory must refuse without agent-management consent.
Hooks and MCP recovery records must match the artifact manifest and disappear
after a successful restore. Their Codex apply targets are guest user
configuration files: save their bytes before the run and restore them after.

Save the guest's original layout, root and preferences first. The scenarios
change the right slot and agent-management consent. Restore those values after
collecting evidence. Snapshots and fixture sources remain under
`/tmp/rivet-core-live`; a second prepare refuses to overwrite that directory's
existing project or instrumentation backups.

Always remove instrumentation, including after a failed scenario:

```sh
python3 -B tests/vm/fixtures/core-live.py restore
```

Restart the shell once more, verify all four definitions still resolve, and
capture an open core blade without the probe. The screenshots prove rendering;
IPC and filesystem assertions prove the tested state transitions. These
plugin-shape checks do not qualify the native authority continuation or the
complete worker lifetime contract.

`LifecycleProbe.qml` is the persistent probe used for the same kind of manual
lifecycle inspection: after opening and closing each core view twice, the
backend must return to its closed-view baseline with no observers, scans,
watches or pending subscription callbacks left, and no child process. Hook and
MCP declarations carry an execution sentinel; viewing them must leave it absent,
and MCP credentials must remain redacted.
