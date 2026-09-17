# Security

This document is agent-written.

---

Omarchy FileBlade is a schema-v1 Omarchy Quattro plugin. Its QML and Rust
binary run unsandboxed with the authority of the desktop user. FileBlade is a
file manager, so that authority intentionally includes reading user-selected
filesystem metadata and content and mutating paths after explicit actions.

This document describes implemented boundaries; it is not a claim that the
project or every dependency has received an independent security audit.

## Trust model

Treat these inputs as untrusted data:

- filenames, symlink targets, file metadata, Git output, desktop files, and
  mounted filesystems;
- clipboard data, picker options, public CLI arguments, and IPC strings;
- Freedesktop Trash metadata and directory-size indexes;
- user blade definitions and installed plugin manifests.

User or satellite QML modules are executable code, not data. Once loaded, they
share the same Quickshell process and user authority as FileBlade. Another
enabled schema-v1 plugin can interfere with FileBlade or inspect its in-process
state. Extension review remains the user's responsibility.

This file was written by an agent.

Ordinary browsing and file operations initiate no network request. The optional
update check reads remote ref IDs with `git ls-remote`; it downloads no Git objects
and changes no refs or checkout files. Each remote check has a 20-second deadline,
64 KiB stdout and 16 KiB stderr caps, and at most 512 returned refs. The same
request includes `v*` release tags; there is no additional endpoint or content
request. Version strings are valid SemVer, capped at 64 bytes. Existing local
manifest objects take precedence; otherwise only the highest release tag resolving
(lightweight or annotated) to the selected remote commit supplies a version.
Tags rely on the publisher's convention that their version matches the manifest;
without a local object, FileBlade cannot independently verify that convention.
Missing or invalid versions stay unknown; same-version and older-version changes
are distinguished. Local history comparisons use existing objects only, with
promisor lazy fetching disabled. Automatic checks run at most once per six hours;
the attempt is saved before requesting the network.
Set `"checkUpdates": false` in plugin settings to disable automatic checks.

Installation clones the source and bundled static backend from GitHub. There
is no install hook, runtime build, or first-run executable download. Maintainer
builds use the pinned Rust toolchain and locked dependencies; `tests/run`
checks the bundled checksum, source fingerprint, and byte-identical rebuild.
These checks establish correspondence, not trust in the source or publisher.

Anyone can establish that correspondence themselves, without trusting the
publisher's checksum, because the bundle is a reproducible build:

```bash
git clone https://github.com/data-goblin/fileblade
cd fileblade && git checkout <commit>
tools/bundle verify
```

That rebuilds the backend with the toolchain pinned in `rust-toolchain.toml`
against `x86_64-unknown-linux-musl`, with locked dependencies, static linking,
no stripping, and the checkout, cargo home and target directories remapped out
of the binary, then compares the result byte for byte with the committed
`fileblade-bin` and fails if they differ. The build does not depend on where
the repository sits or which machine runs it, so a matching rebuild shows the
shipped bytes are that commit's source. What a user installs is the binary
inside the cloned commit; the release asset is a copy of the same bytes.

The manual [backend provenance workflow](docs/agent-written/build-provenance.md)
rebuilds that same recipe on GitHub and refuses attestation unless its output is
byte-identical to the selected commit's bundle. A separate job signs the backend
digest using GitHub's workflow identity, then checks both source and signer commit
digests. Every action is pinned to a full commit. Only the signing job has OIDC
and attestation write permissions; neither job can write repository contents or
releases. A successful run and verified attestation must exist for the exact
reviewed commit before claiming hosted provenance. The workflow's presence alone
is not that evidence, and attestations are not a security audit of the code.

The update checker reads branch/tag IDs and local repository state; it never merges, resets,
validates, builds, or rescans plugins, and it never changes checked-out source.
Updates happen outside FileBlade with the shell stopped before replacing
watched plugin files, followed by a fresh shell start. Disabling only a pane
does not stop Omarchy's plugin watcher.

The Welcome tab's explicit Install action acquires only the four full commit IDs
compiled into `src/plugin_install.rs`. Shallow, no-checkout clones disable templates,
submodules, automatic maintenance and checkout hooks. Acquisition has a 180-second
command deadline, 16 MiB per-file and 512 MiB address-space limits. A staging budget
of 128 MiB and 16,384 entries is checked every 50 ms and at completion; this is a
monitored aggregate limit, not an OS disk quota, so transient overshoot is possible.
Before checkout, each pinned tree must contain at most 2,048 regular files, eight
MiB total, and paths at most 16 segments/1,024 bytes. Symlinks and submodules are
refused. The final HEAD is verified and Omarchy validates the checkout before
publication or enablement. Detached pinned checkouts support Omarchy's
`fetch origin HEAD` / `merge --ff-only FETCH_HEAD` update path.

Unique, exclusive private staging has a durable device/inode identity; cleanup
refuses a replacement directory. Publication uses no-replace rename. A private
lock excludes overlapping installs, and progress survives plugin reloads. Existing
checkouts are preserved. Welcome enables them only when the origin matches, HEAD
is exactly the reviewed pin, the tree has no tracked, untracked or ignored changes,
and validation succeeds. Otherwise it asks for an explicit update or enable action.
No installation happens on startup. Companion host-enable buttons use an overall
20-second timeout with a one-second termination grace and never acquire code.

FileBlade sends no telemetry, uses no privilege elevation, and does not install
system packages or modify Hyprland, systemd, sudoers, or udev configuration.

## Dependencies and previews

FileBlade does not install packages or use elevated privileges. Omarchy Quattro 4.0.2 or newer
supplies its normal desktop stack: Bash, Quickshell, Hyprland/`hyprctl`, `gio`,
`gtk-launch`, `xdg-mime`, `xdg-terminal-exec`, `omarchy-launch-editor`,
Nautilus, and the Omarchy plugin commands. The x86-64 backend is bundled, so
installation and updates need no Rust toolchain. Source builds use the pinned
maintainer toolchain; the in-app update check does not build or apply updates.

Optional integrations are detected at runtime and fail closed when absent.
`udisksctl` mounts, unmounts and ejects volumes; it is tried first with
`--no-user-interaction` and retried without it only when udisks2 answers
`NotAuthorizedCanObtain`, so the desktop's polkit agent is what prompts and
FileBlade never handles the password. Volume enumeration reads
`/proc/self/mountinfo`, `/sys/class/block` and the udev database under
`/run/udev/data`, all read-only and all unprivileged; a volume marked
`UDISKS_IGNORE` is never listed. An action only accepts a device path that
enumeration already returned.
Directories passed to the shared default-open primitive stay inside FileBlade;
Nautilus is reserved for an explicit reveal request. `uwsm-app` is used when
available to launch that reveal through the desktop's application-session
manager. When `zoxide` is present, FileBlade records opened directories with
`zoxide add`, updating zoxide's normal per-user database.
Eligible image previews load automatically on selection and accept only regular,
non-symlink JPEG, PNG, or WebP files up to 16 MiB. The selected file is never
decoded by the shell: a short-lived `fileblade` child process renders a bounded
PNG thumbnail into `~/.cache/fileblade/thumbnails/`, and the shell displays
that. Other images can still be opened in their normal external app.

Removal deletes the plugin checkout but deliberately preserves your layout,
settings, history, audit log, and disabled-module bins under
`~/.config/omarchy/fileblade/`, `~/.local/state/omarchy/fileblade/`, and
`~/.local/share/fileblade/`. Delete those directories manually only if you also
want to erase that data. Companions also retain private recovery under
`$XDG_STATE_HOME/fileblade/mcp-recovery` and `hooks-recovery` (normally beneath
`~/.local/state/`). Uninstall preserves these copies too. Files in the normal Freedesktop Trash are not
owned by the plugin and are never removed during uninstall.

## Resident backend boundary

The QML service creates one child process, `fileblade serve`, with anonymous
stdin and stdout pipes. The version-1 protocol is newline-delimited JSON. It
does not bind a Unix/TCP socket, create a FIFO, write a protocol log, or publish
a shared endpoint. Payloads sent by QML, including artifact documents, travel
through the pipe rather than the child argv or environment.

The server requires a hello handshake and keys work by a bounded `(id,
generation)` pair. It bounds protocol lines, response bytes, argument count,
identifiers, remembered request keys, concurrency, and subscription paths.
Requests have bounded deadlines and cooperative cancellation; filesystem
subscriptions are bounded and close on cancellation or EOF. Server shutdown
cancels active work, joins workers, and is also tied to parent death on Linux.
On plugin teardown, one detached cleanup command waits at most three seconds
for the old backend to exit and restores its recorded window-border changes.
It does not restore borders claimed by a replacement backend or modified by
another application. It exits after cleanup; no background daemon remains.

Anonymous pipes prevent an unrelated process from discovering and connecting
to an ambient FileBlade backend service. They do not provide encryption or
protection from a process already able to debug/read the same-user shell or
child, a compromised plugin, or a compromised desktop session. The public
`fileblade` CLI also necessarily exposes its own command-line arguments through
ordinary process metadata.

Live public commands use bounded Quickshell IPC. Local backend-backed public
commands call the Rust dispatcher in-process, with deadlines and independent
output budgets; they do not place a second backend payload in a spawned argv.
The hidden `_backend` compatibility surface is not a security boundary or a
stable public API.

Quickshell IPC is split between the read-only `data-goblin.fileblade` target and the
mutating `data-goblin.fileblade.control` target. Neither status surface serializes
complete blade module state. The control target accepts file operations directly;
permanent Trash deletion still requires an explicit `--yes` argument through the
public CLI.

These are privileged desktop-integration APIs, not a file-selection portal.
Read responses can disclose private paths, selection, search, and history, and
control calls alter live FileBlade state. Do not expose or proxy them to
untrusted applications or plugins. Use a real desktop portal for sandboxed
file selection.

## Filesystem mutations

Copy, move, rename, create, and FileBlade's own removal helpers use
descriptor-relative Linux filesystem operations:

- absolute paths are normalized and their parent directories are opened from
  the filesystem root with `openat2` containment/anti-magic-link flags;
- security-sensitive walks and the safe fallback use component-wise
  `openat(..., O_NOFOLLOW)`;
- leaf files are opened/stat'ed without following symlinks when identity or
  type matters;
- publication uses `renameat2(RENAME_NOREPLACE)` so an existing target is not
  silently overwritten;
- creates use exclusive no-follow opens or `mkdirat`;
- recursive removal is descriptor-bound and rechecks entry identity.

Unix filename bytes are preserved across the JSON/QML boundary using local
file URIs when a name is not UTF-8. Escaped display labels are separate from
actionable paths; a raw byte and a Unicode replacement character cannot select
the same file through lossy decoding. See the
[path contract](ARCHITECTURE.md#filesystem-path-identity).

Copy operations build an item inside a private `0700` staging directory in the
destination parent, preserve regular-file/directory metadata, xattrs, and
symlinks, verify the source did not change, then publish it atomically without
replacement. Cancellation removes the private stage. Same-filesystem moves use
no-replace rename. Cross-filesystem moves first quarantine the source under a
random sibling name, copy and publish safely, then remove the quarantine; on a
copy failure FileBlade attempts to restore the original source name.

These controls prevent common symlink-swap, partial-publication, and accidental
overwrite failures. They cannot make a multi-item operation globally atomic:
already completed items remain reported if a later item fails or the operation
is cancelled.

Archive extraction uses `bsdtar`. A private uncompressed PAX snapshot is bounded
by a 1 GiB file limit before extraction. Its headers are checked before destination
writes: at most 50,000 headers, 1 GiB logical expanded bytes including sparse sizes,
and 64 KiB per metadata header. The snapshot and extracted tree can together use
up to approximately 2 GiB, plus bounded filesystem metadata; special files are
refused. Decoder address space is limited to 1 GiB and commands to 300 seconds. New or empty destinations are populated in a
private stage and published only after successful extraction. Failed or
cancelled extraction leaves the destination unchanged. An explicit merge into
a populated directory writes directly to that pinned directory, can replace
members, and is not undoable; a failure reports the destination in `paths` and
sets `partial`. Missing destination parents are created inside the stage and
published together, so failed extraction does not leave empty parent folders.

## Private state and undo

The private JSON state, config, journal, and artifact helpers create or tighten
their directories to `0700` and files to `0600`. Their reads open once with
`O_NOFOLLOW | O_NONBLOCK`, verify same-user regular files, and bound content before
materialization. Atomic writes use exclusive same-directory temporary files,
flush and `fsync`, atomic rename, and directory `fsync`. Advisory journal
locking uses a separately verified private file.

Audit append, rotation, and reads share a cross-process private lock. Existing
files must be same-user regular files with private permissions. Complete JSON
lines are appended and synced; reads report malformed records from older or
damaged logs. Logging errors do not roll back the file operation, so the audit
log is not a transactional or exhaustive history.

Regular-file read boundaries use nonblocking opens before verifying type, so
FIFOs are rejected without waiting for a writer. Network or unhealthy
filesystems can still delay kernel I/O despite cooperative cancellation.

The undo/redo journal retains at most 100 entries. Transfers checkpoint after
32 completed items, when a completion finds that 250 ms have elapsed, and at
the end of the operation. An abrupt exit can leave completed items outside the
last saved checkpoint.

A completed create or rename returns its path or mapping even if journaling
fails, with a separate `journal_warning`. Multi-item failures likewise report
completed work. A failure response is not a general promise that nothing changed.

Recovery sweeps private operation intents at backend startup. Each operation
holds a kernel lock on its published intent; recovery skips locked intents
and holds the same lock while recovering abandoned work. Quarantine refuses
to move a source if it cannot publish its intent. `fileblade doctor` uses a
`serve --no-recover` handshake and does not recover or audit operations.

Undo of a copy or create checks the destination's device and inode, file type,
size, modification time, and change time. Directory fingerprints incorporate
each descendant's relative name, identity, permissions, size, modification
time, and change time across at most 50,000 entries. These are metadata
fingerprints, not content hashes. Incomplete scans and older aggregate-only
fingerprints require explicit force, as do detected edits. Force retains the
identity, collision, containment, and
no-follow checks. Corrupt journals are not executed and may be quarantined for
diagnosis.

State safety does not replace backups. A crash, hardware failure, filesystem
bug, or user-authorized forced/destructive action can still lose data.

## FileBlade Trash

The first-class Trash view combines the Freedesktop Trash layout with
FileBlade-managed satellite snapshots. It discovers the home Trash, applicable
mount-local stores, and private satellite stores with bounds on mounts, stores,
modules, candidates, metadata, response size, and errors. It parses `.trashinfo`
and directory-size data through bounded no-follow regular-file reads.

Trashing first secures the selected entry against pathname replacement. After
the desktop trash operation, FileBlade publishes a fresh stored name with the
correct original-path metadata already written, so desktop clients cannot keep
using cached staging-path metadata. Publication uses no-replace moves and keeps
the current stored identity available for rollback. This follows the
[Freedesktop metadata-before-payload ordering](https://specifications.freedesktop.org/trash/latest/)
and accounts for [GVfs caching entries by stored name](https://github.com/GNOME/gvfs/blob/master/daemon/trashlib/trashdir.c).

Catalog entry IDs bind store/name and observed file identities. Restore,
permanent delete, and empty re-resolve and revalidate selected entries before
mutation. Restore uses no-replace relocation, refuses unsafe/colliding
destinations, and can recreate a missing recorded parent only when the caller
explicitly requests it. Public permanent delete and empty commands require
`--yes`; the QML view requires confirmation.

Trash metadata and payloads are desktop/user data rather than secrets owned by
FileBlade. Other desktop applications using the same Freedesktop stores can
change them concurrently; FileBlade reports stale/refused entries rather than
assuming a prior listing is still authoritative.

Retention cleanup is disabled when configured as Never and otherwise removes
only entries whose parsed deletion time is at or before the requested cutoff.
Missing, malformed, ambiguous, or changed timestamps are retained. Cleanup
uses the same identity checks and bounded mutation paths as explicit permanent
deletion.

## Module artifact bins

Shared artifact-tree rows marked `kind: "bin"` expose the bin's restore/purge
flow, not ordinary file actions against their historical pathname. A new file
created at that pathname is not the disabled item.

Artifact bins back the satellite entries shown in FileBlade Trash. Module IDs,
entry names, manifests, item counts, nesting, per-file bytes, total bytes,
listing work, and responses are bounded. Stored directories are private `0700`
and regular files `0600`; symlink targets and supported metadata are preserved
without following the symlink as content.

Restore constructs each root privately, publishes without replacement, and
checkpoints completed roots so an interrupted exact restore can resume. An
occupied non-identical destination is refused. Manifests are private,
bounded, and written atomically. Logical removal saves a visible core record and
transaction ID before the companion prepares private recovery. Preparation is a
write operation. The complete payload and helper-input limits are checked and
saved before source removal. Interrupted preparation can restore by its stored ID;
it cannot disappear into an invisible helper quota. A cross-process lease excludes
purge and retention during active mutations. Confirmed restore checkpoints completion
before cleanup, so retry does not repeat a successful write.

Purge, retention and completed restore call the companion's declared `discard`
method before removing the visible bin entry. Each transaction has its own record;
legacy payload matching preserves records referenced by another bin entry. A
failed cleanup keeps the visible entry. Each helper store scans at most 512 names
before sorting, reads through held no-follow directories and nonblocking private
regular-file descriptors, and caps records at about 1 MiB, aggregate bytes at
16 MiB and pending removals at 64. Pending undo does not expire independently of
the bin; direct helper restores retain idempotent completion records for one week.
Pre-fix payloads without a stored recovery record are not promoted into trusted
undo. They remain listed and can be purged, but cannot be replayed safely.

The first start without a recorded retention answer opens a modal on the left
FileBlade blade, opening it if needed and using its own window when undocked:
“Should FileBlade automatically empty the trash?” Never is selected initially.
Never, 1 day, 7 days, 30 days and 90 days require explicit Confirm. Existing implicit
seven-day defaults do not count as consent. Until a choice is durably saved,
automatic cleanup is off. The choice applies to shared desktop Trash and artifact
bins, without per-item ownership markers. It can be changed in settings.
`settings.json` and `keybindings.json` carry schema `version` and the backend's
`filebladeVersion`; custom keybindings survive metadata migration.

Skills and Memory browsing is available without write consent. Their management
operations through FileBlade require the saved “Manage agent files” opt-in; enabling
it explains that links and instruction/skill files influence coding agents.

Hooks/MCP configuration writes and Memory/Skills link changes share the native
filesystem boundary. Configuration replacement compares the opened identity and
byte-exact preimage, quarantines the old entry with a durable recovery intent,
and publishes without overwriting an intervening entry. New files are `0600`;
existing permission bits are retained. Exclusive link creation and
identity-checked unlink refuse replacement entries. These protections do not
isolate enabled code from other same-user processes, including a process that
already holds a writable file descriptor.

## External commands

External tools are started with explicit argument vectors rather than shell
interpolation. Untrusted positional values use option terminators where the
tool supports them. Captured stdout/stderr is drained with producer-side
retention limits. Blocking probes have deadlines. Captured native commands have
a Linux supervisor that cleans up the owned process group after leader exit,
cancellation, timeout or backend death, including SIGKILL. The leader remains
unreaped until the final group signal, preventing process-group ID reuse during
cleanup. Nonblocking input/output and a bounded final drain prevent inherited
pipes from holding a completed request open. Cleanup reports failure if the
kernel cannot complete it within its bound.

Git's Python helper uses a separate supervisor to stop its owned command group
on timeout or helper death. TERM waits for cleanup before the helper exits.
Deliberately detached groups, such as credential agents, remain independent;
they cannot hold the output reader indefinitely. Commit messages use bounded
stdin instead of argv. Neither this supervisor nor FileBlade sandboxes Git
hooks or configured credential helpers.

Manifest-declared inventory helpers use the same native supervisor. Every dispatch,
including reads, restore and purge, requires a complete current catalog, a matching
installed directory and authoritative enabled state. Unknown or disabled companions
are refused; the user must explicitly re-enable before their helpers run. Their
relative executable must be a regular executable inside the provider checkout;
read and write methods cannot overlap. Timeouts are limited to 30 seconds,
stdout to 2 MiB, stderr to 4 KiB, and private stdin to 64 KiB. Nonzero exits
cannot be reported as success. Helper-write audit records retain only declared
provider/helper/method identifiers and success, never private input, helper
arguments, output or detailed errors. Enabled plugins remain trusted session
code; declaring a helper does not sandbox it.

Intentional desktop application launches are detached and may outlive the
request. FileBlade resolves optional programs through `PATH`, so the desktop
session's `PATH` and installed executables are part of the trusted computing
base. Do not run FileBlade with an untrusted `PATH`.

Script actions contributed by plugins run as argv vectors read from the
manifest on disk, with `argv[0]` confined to the plugin directory, the
selection in the environment or a private file, bounded output, a deadline, a
concurrency cap, and an audit line. They are unsandboxed, like the plugin's
QML, but they run outside the shell process. The backend re-reads the manifest
at run time, so neither the UI nor an IPC caller can supply a command vector.
Actions that declare `confirm` need an explicit approval, in the menu or with
`--yes`. Your own actions under `~/.config/omarchy/fileblade/actions/` may name
a program on `PATH`; a plugin's may not.

## QML and extensions

Dynamic filesystem, command, manifest, and error values are treated as plain
display text and bounded before entering long-lived models. Blade definitions
must be bounded regular JSON files with safe relative entry paths. Manifest
contributions are namespaced to their provider and loaded only while that
provider is enabled.

Selecting a regular, non-symlink JPEG, PNG, or WebP file no larger than 16 MiB
automatically renders a size-constrained inline preview out of process. Other
images open through an external application. The resident backend spawns a
one-shot `fileblade _backend thumbnail-render` child for each new file; the
child caps its own address space at 512 MiB, reads the file without following
links, refuses anything whose bytes are not PNG, JPEG, or WebP, rejects sources
wider or taller than 16384 pixels or above 64 megapixels, decodes under the
image crate's allocation limits, and writes a PNG of at most 1024 pixels per
edge to `~/.cache/fileblade/thumbnails/` keyed by path, stat fingerprint, and
size. The shell only ever hands Qt that PNG. A crash, timeout, or decoder error
in the child ends that one render and shows "Preview unavailable"; the shell
and the resident backend are not affected. Before the child is even started,
the type, link, byte, and target-size gates limit exposure.
Application icons read from desktop files are limited to bounded theme icon
names. Path and URL icon values are ignored and use the fallback glyph instead,
so opening an application menu does not decode a desktop-file-selected image.

Thumbnail directories are `0700` and files are atomically published as `0600`.
Cache reads reject symlinks, non-regular files, unsafe permissions, files over
8 MiB, and dimensions outside the requested bounds. Legacy permissive files
are regenerated inside the private cache directory.

Those checks validate discovery data; they do not sandbox the QML referenced by
an accepted definition. Review user modules and satellite plugins for plain
text rendering, bounded models, process/URL sinks, teardown, and persistence
before enabling them. FileBlade does not execute a discovered module's
unrelated hooks, MCP commands, or agent configuration merely to display it.

## Reporting

Report vulnerabilities privately to the repository owner. Include the exact
commit, reproduction steps, affected paths, and whether the issue requires the
plugin to be enabled. Do not include credentials, private file contents, Trash
payloads, or other personal data in the report.
