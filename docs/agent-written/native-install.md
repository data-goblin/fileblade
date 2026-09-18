This file was written by an agent.

# Local native payloads

`tools/native` builds an inventoried runtime directory from a committed
source tree, a matching prebuilt backend and that target's notices. It does
not acquire software, build dependencies or enable desktop roles.

```
tools/native stage SOURCE BACKEND TARGET NOTICES OUTPUT
tools/native verify OUTPUT
tools/native check OUTPUT
```

Supported target identifiers are `x86_64-unknown-linux-gnu`,
`x86_64-unknown-linux-musl`, `aarch64-unknown-linux-gnu` and
`aarch64-unknown-linux-musl`. GNU inputs must name the corresponding glibc
interpreter; musl inputs must be static. `verify` checks the whole file set,
SHA-256, modes, required files and ELF architecture/ABI without executing the
backend. `check` additionally requires the matching machine, runtime packages
and a runnable backend with the declared version. Runtime package checks
currently qualify Arch/Omarchy only. Installed packages do not prove a
working portal session or graphical runtime.

`packaging/runtime.json` declares source roots, required files, executable
dependencies and the tested package floor. `payload.json` records the source
commit and target plus every shipped file's relative path, digest and mode.
The manifest itself is outside its own digest set. Inventories reject links,
special files and unsupported names/modes. Runtime directories use mode 755;
files use 644 or 755. Missing files and extra files both fail validation.
The source tree must have no tracked changes during staging. Untracked files
are not runtime inputs. Output publication uses a temporary sibling
directory and refuses to replace an existing destination.

The producer is responsible for building the supplied backend and notices
from the named source/target. A digest detects corruption; it is not a signed
build attestation. These local payloads carry no remote provenance claim.
Cross-target verification is not an ARM runtime or physical-hardware pass.

The production runtime launcher `app/launch` resolves its backend at
`bin/fileblade`; the payload retains that path without requiring
Cargo or a source checkout at launch. All QML relative imports are preserved and the
core modules are answered by that backend, with no interpreter in the payload. The app-private adapter and its upstream license are
included. Developer-only app/ovm-spike and app/qualification helpers are
excluded. Runtime-owned service/portal metadata, when committed beneath app/,
is included; packaging does not invent or enable a chooser implementation.

Run `tests/vm/expectations/90-delivery-payload.sh SOURCE BACKEND TARGET NOTICES`
inside the assigned Omarchy guest for integrity, malformed-inventory,
wrong-architecture and missing-dependency checks. The fresh staged app launch,
module catalog and screenshot are separate live evidence; this script does
not claim them.

## User-local installation

```
PAYLOAD/tools/native install PAYLOAD
PAYLOAD/tools/native status
PAYLOAD/tools/native rollback
PAYLOAD/tools/native remove
```

The installer is self-contained in the payload. It stores runtime versions
under `$XDG_DATA_HOME/fileblade/installation/versions`, falling back to
`~/.local/share`, and creates `~/.local/bin/fileblade`. The receipt is
`installation/active/receipt.json`. A payload manifest's SHA-256 names its
version directory, so successive development builds with the same version
number remain distinct and recoverable. No setting or Note is stored there.

Installation validates the input, copies it to a temporary sibling, validates
the copy and publishes it. A generation pairs the receipt with a symlink to
its runtime. One atomic pointer switch activates that pair. The receipt
records the previous payload, and rollback performs the same activation in
reverse. Existing runtime generations and interrupted staging directories
are retained; no automatic pruning is implemented. Settings and desktop
defaults are untouched. Removal preserves generation receipts and user data.
Role reversal uses runtime's published maintenance interface.

The stable launcher resolves its physical runtime once under a shared
installation lock. Maintenance verifies the active payload under a shared
lock, calls its lifecycle CLI, then takes the exclusive lock and rechecks
the activation pointer. A changed pointer requires retry. Update and rollback
call `app/launch native drain --timeout-ms 30000 --json`; removal first calls
`app/launch native roles disable --all --json`. When the active runtime does not
recognize the `native roles` entry point, removal reports that no desktop
role was ever enabled and skips that optional step. A present reversal must
succeed; nonzero exits or unknown, malformed or incomplete success results
preserve the runtime. Drain is required for removal, update and rollback; a
failed drain after reversal can leave roles disabled, which the diagnostic
reports. Update and rollback currently have no optional lifecycle steps.
Closing the view alone leaves the authority holding the lock.
A runtime launched directly outside the stable launcher
does not participate in this delivery lock and must not be updated this way.

Package-owned conventional executable paths and unrelated launchers are
refused. Modified owned launchers, invalid receipts and receipt/pointer
disagreement are also refused. Arch package mapping is described below;
external package removal recovery remains task 9.4 work. Current payload and installer
dependency contracts must match unless the new payload lists the digest of
the current contract under `upgrades` in `packaging/runtime.json`. The digest
is `sha256sum` of `jq -cS '{schema, backend, commands, packages}'` over that
file. The Python-era contract is listed so the interpreter-free runtime
installs over it, and rollback to that version stays possible.

`tests/vm/expectations/91-delivery-install.sh PAYLOAD` checks collision
preservation, repeated install, distinct activation/rollback, busy refusal,
shared-lock status, settings/Notes preservation, and real process-group kills
during copy and immediately before/after the activation rename. Its alternate
transaction payload manifests differ only in legal JSON whitespace. A separate
case changes a compatible dependency floor and verifies that the prior
contract prevents activation while preserving rollback. Test-only
command wrappers inject interruptions; production code has no fault hooks.
These are process-interruption checks, not physical power-loss tests.

For actual payload lifecycle qualification in the assigned guest, run
`tests/vm/expectations/95-delivery-installed.sh PHASE PAYLOAD` with phases
`install`, `check`, `update`, `rollback`, and `remove`. Use the expected active
payload for each phase, including the genuine previous payload for rollback.
Run the scoped native UI expectations between install and update. These
checks require production runtime lifecycle support; they do not simulate it.
`FILEBLADE_NATIVE_TOOL` can select a candidate installer outside the payload
when checking maintenance of an older, unchanged installed runtime.

## Migration receipt identity

The version-2 migration receipt lives at
`$XDG_STATE_HOME/omarchy/fileblade/migration-020/receipt.json`, with the usual
`~/.local/state` fallback. Its `Binding` records retain `path`, `device` and
`inode`. `Binding` equality deliberately compares only **path and inode**;
`binding == other` ignores `device`, because device numbers can change
across boots. A different path or inode still refuses migration.

When the same roots have new device numbers, migration re-records the current
devices and reports the refresh. The receipt, copied checkpoint and completion
checkpoint keep schema 2 and converge to matching bytes. An interrupted
refresh may differ only in binding devices; other receipt changes still
refuse. Completed migration does not replay saved source data over current
Notes or settings. Live authority checks on held descriptors remain strict.

## Arch package from the same payload

```
packaging/build PAYLOAD OUTPUT_DIRECTORY
```

Run this maintainer helper on the payload's architecture with the existing
Arch `makepkg`, `fakeroot` and `bsdtar` tools. It accepts stable versions,
requires an absent output directory, and installs no build dependencies.
The generated PKGBUILD takes its version, architecture and package
dependencies from the verified payload. Its three local sources are hashed;
there is no download step. The output contains the package archive,
PKGBUILD and its local inputs. Build helpers are not added to the runtime.

The `fileblade-native` package owns the unchanged payload beneath
`/usr/lib/fileblade`, a thin `/usr/bin/fileblade` launcher, a
`/usr/bin/fileblade-bin` backend link and a standard license link. Stripping
and debug splitting are disabled. The builder extracts the resulting package
and verifies its inner payload again before publishing the output.
The payload root is normalized to mode 755, as in direct installation, so
a private input directory does not become a root-only installed runtime.

Installing a package selects no desktop
roles, defaults, bindings or autostart. User-level installations remain
separate; the direct installer detects conventional pacman-owned paths and
refuses updates when they coexist. Package files must be updated or removed
through pacman. A direct launcher may still shadow the package after an
external pacman install; the installer diagnoses that collision and preserves
both trees.

Package mapping does not qualify that startup, provide a chooser, or register
runtime-owned desktop/service metadata that has not yet been implemented.
Mapping those descriptors and checking real companion-mode coexistence remain
part of native integration and task 9.4. An ARM package requires an actual
ARM payload and matching Arch build environment; no ARM execution follows
from the inventory format accepting an ARM target.

Inside the assigned guest,
`tests/vm/expectations/92-delivery-package.sh SOURCE PAYLOAD` builds and
extracts the package, compares its payload/dependencies, installs it through
pacman, verifies ownership, checks direct-update refusal, removes the package
and checks preservation of the direct receipt and personal-default fixtures.
It refuses to replace a preexisting fileblade-native package and cleans up
its own package fixture on failure. Structural fixtures are not a native app
launch or a real chooser/reveal fallback pass.

## Removal and stale activation

`remove` uses the same owner and exclusive-lock checks as installation. It
refuses package ownership, changed launchers and changed inventoried payloads.
After validation it withdraws activation, unlinks its launchers and moves
owned versions into a private discard directory before deleting them.
`installation/removing` retains the receipt during deletion; retry `remove`
if interrupted. Installation refuses while this marker exists. Successful
removal retains `installation/removed/receipt.json` and generation history.
Unknown entries and incomplete staging directories are preserved. No setting,
Note, recovery journal or desktop default is deleted. Repeated removal is
safe, and a later explicit local installation can reuse the installation root.

Rollback validates its recorded previous payload before requesting drain.
Missing/damaged current payloads or missing activation with receipt history
require restoring the verified current runtime/owned pointer first: delivery
cannot prove a surviving authority stopped by calling a different candidate.
Invalid receipts are refused. Recovery never guesses a generation from
timestamps or directory order. Runtime recovery for an unavailable active
maintenance entry point remains unqualified.

The package launcher takes a shared lock on `/usr/share/fileblade-native/lock`
before checking pacman's configured database lock and the held lock inode. Existing launches block
the ALPM idle preflight; new launches refuse throughout a package transaction,
including after that preflight. Unrelated transactions using the same
database also prevent launch until their lock is released. A stale pacman lock requires pacman's normal
recovery; FileBlade does not delete it. Qualification covers the configured
system database, not ad-hoc `--dbpath` or `--config` overrides. The hook edits
no user defaults. Removal runs `native roles disable --all --json` before
`drain` and requires the document described under Desktop integration.
External stale role recovery uses that command, never the receipt directly.

`tests/vm/expectations/93-delivery-remove.sh PAYLOAD` checks stale activation,
ownership preservation, busy refusal and an actual process-group kill during
runtime deletion followed by retry. E92 additionally holds the package lock
and checks that real pacman removal aborts while preserving installed files.

`tests/vm/expectations/94-delivery-lifecycle.sh SOURCE PAYLOAD` creates an
explicit maintenance fixture, checks failure/result handling, the active
launcher path, shared-lock ordering and activation identity changes, then runs
E91/E93 with it. These are caller/transaction checks; they do not implement or
qualify runtime draining or desktop-role reversal.

## Desktop integration

Installing, updating, packaging and first launch change nothing on the
desktop. The Settings sheet of the native app has a "Desktop integration"
section with five independent switches, all off until the person turns one
on; the plugin has no such section. The switches and the files each one
owns:

```yaml
Open folders with FileBlade:  $XDG_DATA_HOME/applications/fileblade.desktop and the
                              inode/directory key of $XDG_CONFIG_HOME/mimeapps.list
Reveal in FileBlade:          $XDG_DATA_HOME/dbus-1/services/org.freedesktop.FileManager1.service
File chooser:                 $XDG_DATA_HOME/xdg-desktop-portal/portals/fileblade.portal,
                              $XDG_DATA_HOME/dbus-1/services/org.freedesktop.impl.portal.desktop.fileblade.service
                              and the FileChooser key of $XDG_CONFIG_HOME/xdg-desktop-portal/portals.conf
Hyprland bindings:            $XDG_CONFIG_HOME/hypr/fileblade-bindings.lua and one marked dofile
                              line in $XDG_CONFIG_HOME/hypr/bindings.lua
Start at login:               $XDG_CONFIG_HOME/autostart/fileblade.desktop
```

Every `Exec` points at the stable launcher, `~/.local/bin/fileblade` for a
user-local install or `/usr/bin/fileblade` for the package, never at a
versioned payload. Enabling refuses when no stable launcher exists.

The receipt `$XDG_CONFIG_HOME/omarchy/fileblade/desktop-roles.json` (mode
0600, schema 1) records, per role, each written path, the key or marker it
owns, the exact prior bytes or their absence, and the bytes written. A corrupt
or newer receipt refuses every role command without being rewritten. No
receipt means no owned entries; nothing is guessed.

Turning a role off compares each owned entry with its current content. An
unchanged entry is restored to its recorded prior state, deleting files that
did not exist before; an entry the person changed since is left alone and
ownership released. The row reports "restored the previous handler" or
"kept your newer choice". Turning on "Reveal in FileBlade" while another
application owns `org.freedesktop.FileManager1` still enables the role and
reports who owns the name; log out and in for FileBlade to take over, nothing
is killed. Enabling or disabling bindings runs `hyprctl reload`; chooser and
reveal changes do not restart `xdg-desktop-portal` or the session bus.

The same switches are on the command line:

```
fileblade native roles status [--json]
fileblade native roles enable --role ROLE [--json]
fileblade native roles disable (--role ROLE | --all) [--json]
```

Output is always the JSON document; exit 0 for complete or already-in-state,
1 for partial or refused, 2 for usage. Removal runs `disable --all` before
`drain` and requires `status: complete` with every role in `restored`,
`preserved_newer` or `already_off`. `tests/vm/expectations/48-native-roles.sh`
checks coexistence with the prior handler, folder and chooser enabling,
byte-exact restoration, a preserved newer handler, the reveal conflict text
and the removal document, all against temporary XDG roots.

## Installed expectation adapter

Select the adapter and the native shape for installed expectations:

```bash
export OVM="$PWD/tests/vm/native-ovm"
export OVM_REAL="$HOME/.claude/skills/test-omarchy-plugin/scripts/ovm"
export SKIP_PUSH=1 FILEBLADE_SHAPE=native
"$OVM" ipc data-goblin.fileblade status
```

Keep the assigned `OVM_HOME` and `OVM_SSH_PORT`. The adapter resolves the
installed stable user launcher, or the packaged launcher when no user launcher
exists. `FILEBLADE_NATIVE_LAUNCHER` can name an explicit guest stable launcher.
It checks the direct activation receipt and manifest identity; installation
and qualification own the full payload inventory check. It exports the
published native payload/backend/state environment for native commands.

`ipc` routes FileBlade targets through `native ipc --` on the stable launcher.
Other targets, `ssh` command strings and other ovm verbs forward unchanged.
`restart` and `restart-shell` drain first, restart through the stable launcher
and wait for the installed view and eight built-ins. A refused or incomplete
drain leaves activation intact. Launch output is retained in
`$FILEBLADE_NATIVE_STATE_ROOT/native-ovm.log` in the guest.

Push is refused by default. To explicitly install a previously built payload
whose manifest source matches the given tree's HEAD:

```bash
FILEBLADE_NATIVE_PAYLOAD=/absolute/path/to/payload SKIP_PUSH=0 "$OVM" push "$PWD"
```

Push transfers that payload into a temporary guest directory, invokes its
`tools/native install` and removes the temporary copy. Build the payload using
the staging command above; the adapter does not choose a backend or notices.
The shared runner also accepts a relative OVM path. Runtime owns native
control and stop/restart selection in the shared helpers; wheel owns its
remaining plugin-specific callers. Production launcher/drain qualification
and the four backend IPC commands remain pending runtime. SSH strings
forward unchanged; shape selection belongs in their callers.

## Native integration audit

This file was written by an agent.

The September 17, 2026 audit found that desktop bindings still addressed the
removed Omarchy host plugin. Native bindings now use the stable FileBlade
launcher and `native ipc`, retaining the 400 ms timeout and Hyprland fallback.
The installed binding template unbinds Super+B before registering it, including
when Omarchy has already assigned that key. Existing personal bindings are
user-owned; updating the application alone does not rewrite them.

Generated extensions check the native launcher and view before consulting the
legacy host plugin state. A stopped native view reports starting, not missing,
and the guard asks the user to start FileBlade. Legacy plugin detection and its
explicit Enable action remain available for actual legacy installations.

Demo reset commands use public CLI verbs, which select the proper transport.
The keymap reread instructions use native IPC. Doctor reports `rootPath` from
the live status and gives native startup advice when the native view is down.
Legacy VM fixtures and plugin-only enable paths intentionally retain their
Omarchy transport. `app/ovm-spike` is an older qualification adapter excluded
from native payloads.

The host audit covered personal Hyprland bindings, shell startup hooks, local
launchers, systemd user units, desktop/autostart entries, portal/MIME routing,
installed extension helpers, and the corresponding source templates. The
installed launcher, authority and view passed health checks; Goblin Images'
existing native-host fix reported ready. Its unrelated in-progress edits were
preserved. Managed autostart, bindings, chooser, folder and reveal roles were
off, with no conflicts; personal shortcuts remain independently configured.
These role choices were preserved. Historical backups and retired companion
repositories were not rewritten.

Reproduce the generated-binding and extension checks in an isolated installed
native VM with `FILEBLADE_SHAPE=native`, the native OVM adapter and
`tests/vm/expectations/49-native-bindings.sh`. The scenario installs the bindings
twice, sends Super+B, Super+Shift+B and Super+Z through virtual keyboard input,
checks that Super+Z undoes a file creation and does not open quick navigation,
and runs a newly generated extension's host check.

VM qualification exposed another native migration gap: the public preferences
command tried to access protected state directly instead of using the resident
authority. Both reads and writes now reuse the native backend transport. The
regression starts a real authority process, saves and reads preferences, then
proves that a write with the authority stopped fails without changing the file.
The raw mutating backend entry point remains refused; this change routes the
public command through admission rather than granting it independent ownership.

Personal fallback commands were also migrated from old Hyprland dispatcher
strings to the current Lua expressions, keeping their original actions. The
close-window focus predicate now treats missing/null focus as unfocused and uses
Omarchy's existing shell-quoting helper. These were user-config repairs, separate
from the source candidate.

Validation of code commit `9e1899a` used native payload
`d73b542134c3f0de0e5dc19c5a60345e345303a984776bbf6864b00d91be67c2`
in an isolated VM. All 12 native-binding scenario checks passed. With FileBlade
drained, virtual keyboard input also moved focus between two owned windows,
swapped a window, toggled floating, increased its width from 1920 to 2020 pixels,
and closed the selected probe. Compositor state confirmed each action. Keep the
guest unlocked and disable idle for this scenario; a locked display intercepts
the shortcuts before they reach FileBlade.

The final local gate passed formatting, Clippy, QML lint and 488 Rust checks
across 100 suite results, with seven existing ignored checks. It stopped at five
Branches QML failures. Running the remaining suites separately gave 809 passing
QML checks across 85 suites, with those five failures and one heatmap tooltip
failure. The affected source and test files are unchanged from baseline
`568d46f`; these are outstanding baseline failures, not a green full gate.
All Python and companion checks passed. The final static bundle reproduced
byte for byte. The authority regression also confirmed preferences writes fail
without changing disk state when the resident process is stopped.

The source candidate stays on `fix/native-integration-audit`; it was not
installed over the working desktop's separate UI changes. Personal shortcut
repairs are live, with all 22 fallback expressions parsed and Hyprland reporting
no configuration errors after reload. The human-maintained root README still
contains legacy plugin installation/removal instructions and was left unchanged
under its explicit editing restriction. The owned VM and temporary build data
were removed after qualification. Complexity review: Lean already.

### Integrated desktop installation

This file was written by an agent.

The integrated snapshot `d1d4e17` preserves the working UI changes captured on
September 17 and includes the native integration repairs. Its full local gate
passed: 529 Rust checks, 818 QML checks, Python and companion checks, and a
byte-for-byte bundle rebuild. All 12 native VM checks passed. Payload
`d83c736a64944a0d83fee3ed37a282d70b35f48070d39580caa1c48f74ac95fd`
was installed on the desktop through the native installer after a successful
drain. The running app reports that payload; doctor and CLI preferences pass.
Subsequent unrelated source edits remain in the working checkout.

Goblin Images was also still registered as both an Omarchy service and hidden
bar widget, alongside its independent native FileBlade extension. Its stale
Omarchy host guard caused the repeated warning. Both Omarchy config entries
and only the Omarchy registration symlink were removed. The source checkout,
native extension symlink, enabled receipt and library remain intact. Native
status still lists `kurt.goblin-images/goblin-images`; Omarchy no longer lists
the plugin and the compositor reports no host-guard layer. Native installations
should register this extension only under `~/.config/fileblade/extensions`.
