# Contributing

This file was written by an agent.

Hello! Any contributions are welcome. If you contribute, I'd ask that you use
the [PR template](.github/PULL_REQUEST_TEMPLATE.md), ensure that
[UI expectations](tests/EXPECTATIONS.md) are updated, and honor the vision
and design aesthetic of the original software. It's important to note that FileBlade
does not strive to be a file explorer or traditional file manager. Rather, it envisions
an IDE-like experience built into the OS, and attempts to cater specifically to users
who are using coding agent CLIs in terminals as their primary interface for the computer.
Therefore, FileBlade strives to provide quick, streamlined ways to view or find information,
which is typically files in a project, but could be other data. Hopefully, in the future,
FileBlade can be a place like the NavBar where people can put whatever information they want,
including extensions. FileBlade also intentionally neglects some traditional IDE features
like a space to write commit messages for Git, for instance, under the assumption that most
people have agents doing this for them, anyways. I expect (and hope) that FileBlade will
continue to evolve and optimize in the direction of the most optimal experience for agent-
driven computer work.

The rest of this document is agent-written, containing some specific instructions
for your agent about how to contribute to the project.

If you develop with coding agents, point them at
[AGENTS.md](AGENTS.md) as well. It holds the project's rules for agent work:
design intent, versioning, testing, authorship, and cleanup. Coding agents
read it at the repository root on their own.

---

Bug reports should include reproduction steps, expected behavior, and the
Omarchy version. Include screenshots for visual or focus issues.

## Running from a checkout

```bash
git clone https://github.com/data-goblin/fileblade
cd fileblade
cargo build --release --locked
FILEBLADE_BINARY="$PWD/target/release/fileblade" ./fileblade --version
```

Normal installation uses the bundled static backend and requires no Cargo build.
For development, set `FILEBLADE_BINARY`
to test a local build, or run `tools/bundle build` to refresh the bundle before
staging a native payload for a test VM. Use the installer to drain and replace
the runtime, then start a fresh view; do not rely on hot reload for QML changes.

```yaml
requires:
  rust:      rustup with the pinned rust-toolchain.toml toolchain, edition 2024
  omarchy:   up-to-date Omarchy with Quickshell and the packages in packaging/runtime.json
  hyprland:  for docking, the focus grab, and the blade-aware keybinds
optional:
  plocate:   `scope:everywhere` search
  git:       status in the tree
```

## Layout

```yaml
Service.qml, blades/, modules/, controllers/, panes/, ui/, lib/:  QML and JS
src/, crates/:                                                     Rust
tests/*.rs:      cargo integration and end-to-end tests, plus the source contract tests
tests/qml/:      QML and JavaScript regression tests
tests/vm/:       scripts for a headless Omarchy VM
examples/:       a minimal extension, also the reference for EXTENSIONS.md
demos/:          scripted tours used for screenshots and recordings
assets/:         logos, icons, and other visual assets
```

[ARCHITECTURE.md](ARCHITECTURE.md) describes the implementation,
[EXTENSIONS.md](EXTENSIONS.md) the module contract, and
[SECURITY.md](SECURITY.md) the trust boundaries.

## Tests

```bash
mkdir -p target/bundle-staging
export FILEBLADE_BUNDLE_TMPDIR="$PWD/target/bundle-staging"
tools/bundle build # When native source or build inputs change.
tests/run
```

This file was written by an agent.

Corners stay square. `tools/design-check` fails on any non-zero corner radius in
QML that is not recorded in `tools/design-exceptions.json` with the reason it is
round by design, and `tests/design_rules.rs` runs the same check inside
`tests/run`. Remove the binding rather than widening the registry unless the
surface really is a dot, knob or handle.

`tests/run` is the whole gate and runs locally only. The manual
[backend delivery build](docs/agent-written/build-provenance.md) runs on GitHub
solely to reproduce and attest the bundled backend; it does not run tests there.
It isolates state in a temporary directory and requires the test dependencies,
including Qt 6 declarative tools, Quickshell for native theme reloads, Python 3 for the developer hooks, `nvim`,
`git`, `bsdtar`, `jq`, `readelf`, and `dbus-run-session`. Desktop-role regressions
launch generated entries through GIO and a private D-Bus session. Set
`FILEBLADE_OFFLINE=1` to use cached Cargo dependencies.

Keep `target/` and bundle staging on a disk-backed filesystem. Bundle builds
otherwise use the temporary directory, which may be memory-backed and fill up.
Remove the staging directory when finished. A native change must include the
updated `fileblade-bin`, `fileblade-bin.sha256`, and `fileblade-bin.source`.

The gate runs Rust formatting, checks, Clippy and tests, Qt 6 lint and regression
tests, the scaffold checks, and a byte-for-byte rebuild of the bundled backend.
The pinned Rust toolchain is required even when only verifying the bundle.

`tests/version_contract.rs` and the pre-commit hook in `tools/hooks/pre-commit`
both refuse a version that is not strictly above every released version,
compared as SemVer with prerelease precedence and build metadata ignored. The
released set is the union of the `v<version>` git tags and the `## <version>`
headings of `features/release/release-notes.md` below the newest one, without `(unreleased)`.

Source contracts check process ownership, bounded models, command boundaries,
and UI conventions. Explain deliberate contract changes with the implementation.

Focus, keybinds, docking, hover, and drag changes also need a live VM check.
Update [UI expectations](tests/EXPECTATIONS.md) when behavior changes.

### Testing the GUI in a VM

Use an isolated QEMU/KVM guest installed from the [official Omarchy ISO](https://iso.omarchy.org).
Keep its HOME, XDG roots, compositor and input separate from the working desktop.
Set `OVM_REAL` to the external `ovm` harness executable, and use a dedicated
`OVM_HOME` and `OVM_SSH_PORT` for your guest.

Stage a native payload from a clean committed tree after the code gate passes:

```bash
tools/native stage "$PWD" "$PWD/fileblade-bin" x86_64-unknown-linux-musl "$PWD/THIRD_PARTY_NOTICES.html" "$PWD/target/native-payload"
export OVM="$PWD/tests/vm/native-ovm"
FILEBLADE_NATIVE_PAYLOAD="$PWD/target/native-payload" SKIP_PUSH=0 "$OVM" push "$PWD"
export SKIP_PUSH=1 FILEBLADE_SHAPE=native
"$OVM" restart
tests/vm/expectations/09-search.sh
```

The native adapter verifies the installed payload identity, routes CLI calls
through its stable launcher, and drains before restarting. `SKIP_PUSH=1`
preserves the installed bytes during testing. For release verification, install
the published download in the guest and run against that payload; do not push
local source over it.

Drive keys through QMP and clicks through virtual-pointer input. Compare captures
with compositor state and `fileblade status`. Run only the affected scenarios
unless a broader release gate is required. Report expected and observed behavior,
and identify pending checks. Stop the owned guest when finished and remove only
your temporary captures and fixture state.

## Conventions

- Keep core filesystem and command logic in Rust; QML renders and routes actions.
  An extension helper may be any executable that speaks JSON over stdin and stdout.
- document implementation details in the technical docs; do not add code comments
- modules over regions: split a large file along a seam instead of stacking sections in one
- external commands take argument vectors, never a shell string
- mutations report completed work and journal failures separately; see SECURITY.md for recovery limits
- dynamic text from the filesystem is `Text.PlainText` and length-bounded before it enters a model
- Built-in and user modules use `blade.json`; extension modules register through
  their manifest. Share scanners, watchers, and processes through the host
  contract rather than duplicating them in each view; see [EXTENSIONS.md](EXTENSIONS.md).
- commit messages start with a prefix: `Fix:`, `Feat:`, `Clean:`, `Docs:`

## Pull requests

- one change per PR; keep refactors separate from fixes
- say what you tested live and on which Omarchy version
- if you changed a contract test, explain why in the description
- screenshots or a short recording help a lot for anything visual

## Security

Read [SECURITY.md](SECURITY.md) before changing `src/filesystem/`, `src/operations.rs`,
`src/trash/`, `src/secure/`, or the IPC handlers in
`controllers/FileTreeIpc.qml`. Report vulnerabilities privately as described
there, not in a public issue.
