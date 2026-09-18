# Core module integration

The Skills, Memory, Hooks and MCP modules are implemented in the backend under
`src/core_modules/` and answer `helper-read` / `helper-write` in process. Their
behaviour is covered locally by the `tests/core_modules_*.rs` suites, the frozen
payloads under `tests/golden/python-baseline/`, and `tests/qml/tst_core_lifecycle.qml`,
which drives all four providers through the shared `ArtifactInventory`: two-view
sharing, final detach, late callbacks and accepted mutation arguments after a
detach or project change.

The guest runners `run-live.py`, `run-lifecycle.py` and `settings.py` drive a
real shell over `omarchy-shell` IPC; they never called the retired Python
helpers and still apply unchanged.

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

Restart the guest shell again, then select the fixture project and run the
scenarios:

```sh
omarchy-shell data-goblin.fileblade.control setRoot /tmp/rivet-core-live/project
python3 -B tests/core_modules/run-live.py skills
python3 -B tests/core_modules/run-live.py memory
python3 -B tests/core_modules/run-live.py hooks
python3 -B tests/core_modules/run-live.py mcp
```

Each runner requires an empty artifact bin for its module and emits
`CORE_LIVE_PASS <module>` only after inventory, apply/unapply, removal/restore
and close/reopen pass. Skills and Memory additionally verify consent refusal.
Hooks and MCP verify the recovery record matches the artifact manifest and
disappears after a successful restore. Their Codex apply targets are guest user
configuration files; the runner saves their initial bytes under the fixture
directory and restores them after unapply. If a run fails before that point,
inspect the retained backup and result before restoring the target.

## Lazy lifecycle

`run-lifecycle.py prepare` creates an isolated declaration fixture and backs up
the guest Service before adding a persistent test probe. It explicitly selects
the freshly built `target/release/fileblade` for the Service backend. Restart C,
verify the populated catalog and inspect an open blade before running
`python3 -B tests/core_modules/run-lifecycle.py check` in the staged checkout.
Do not change staged files during the check: the plugin watcher reloads Service.

The check opens and closes each real core view twice, waits for both inventory
watches, then requires zero observers, scans, watches and pending subscription
callbacks. Backend thread count must return to the closed-view baseline, with
no child process left. Hook/MCP declarations contain an execution sentinel;
viewing them must leave it absent, and MCP credentials must remain redacted.
The check restores its original root and layout in `finally`. Results remain
in `/tmp/rivet-core-lifecycle/results.json` inside C. Run
`python3 -B tests/core_modules/run-lifecycle.py restore`, then restart and inspect
the plain Service. Only the temporary Service source is instrumented.

## Settings version changes

`settings.py` runs only from the plugin staged in the allocated guest. Build
and push the current binary first. Its `prepare` phase backs up Service,
StateController, state and layout under `/tmp/rivet-settings-version`, selects
the fresh binary, and attaches a temporary IPC probe to the real Service.
Restart the shell and check modules plus an open-blade screenshot before each
following qualification phase:

```sh
python3 -B tests/core_modules/settings.py prepare
```

After restarting, run `settings.py seed`. It saves a choice equal to the Git
default while leaving property icons untouched, through the real state writer.
Run `settings.py revise`, restart and inspect again, then `settings.py verify`.
The guest-only revised defaults must change the untouched icons preference
while retaining the explicit Git choice and unknown fields. The same check
exercises linked-column, search and sort markers and reset-to-defaults.

Stop the guest shell before `settings.py restore` so no queued write can
replace the restored documents. Restart once more, inspect the original
layout, and retain the logs and screenshots. The backup directory remains
available for recovery; archive or remove it before a new run. This qualifies
state/settings evolution in the plugin Service; desktop binding-role receipts
are a separate runtime integration contract.

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
