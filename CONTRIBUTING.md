# Contributing

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
including other plugins. FileBlade also intentionally neglects some traditional IDE features
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
installing the checkout in a test VM. Stop the VM shell before replacing watched
plugin files, then start a fresh shell; do not rely on hot reload to validate
QML changes.

```yaml
requires:
  rust:      rustup with the pinned rust-toolchain.toml toolchain, edition 2024
  omarchy:   Quattro (v4) with omarchy-shell; schema v1 plugins
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
examples/:       a minimal dependent plugin, also the reference for EXTENSIONS.md
scripts/:        developer helpers the CLI embeds, such as the extension banner generator
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
including Qt 6 declarative tools, Python 3 for the developer hooks, `nvim`,
`git`, and `bsdtar`. Set
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
headings of `CHANGELOG.md` below the newest one, without `(unreleased)`.

Source contracts check process ownership, bounded models, command boundaries,
and UI conventions. Explain deliberate contract changes with the implementation.

Focus, keybinds, docking, hover, and drag changes also need a live VM check.
Update [UI expectations](tests/EXPECTATIONS.md) when behavior changes.

### Testing the GUI in a VM

Use an isolated QEMU/KVM guest installed from the [official Omarchy ISO](https://iso.omarchy.org).
Do not test focus grabs on the working desktop. The external `ovm` harness is
provided by the `test-omarchy-plugin` skill, not bundled in this repository.
Set `OVM` to its executable and provision its base image before running scenarios.

```bash
tests/vm/expectations/run '07-hidden-and-git.sh'
```

The [runner](tests/vm/expectations/run) builds the bundle, pushes the checkout,
restarts the guest shell, and runs matching scripts. Omit the pattern for the
full suite; normally run only scenarios affected by your change. Use
`SKIP_PUSH=1` only when the guest already has the exact build being tested.
The runner can reset an unbootable guest overlay, so use a disposable test VM.

This file was written by an agent.

For manual pushes, stop the guest shell before replacing watched plugin files,
then restart it:

```bash
tests/vm/stop-shell
"$OVM" push "$PWD"
"$OVM" restart-shell
```

The stop helper terminates the guest's Omarchy shell supervisor and waits for
Quickshell to stay stopped. Killing Quickshell alone lets the supervisor relaunch
it while files are being replaced. Restarting that replacement can trigger
[Quickshell's shutdown IPC crash](https://github.com/quickshell-mirror/quickshell/issues/956).
The helper avoids that overlap; it does not patch the upstream shutdown bug.

Load the [binding example](examples/fileblade-bindings.lua)
in the guest. Drive keys through QMP and clicks through virtual-pointer input;
compare screenshots with compositor and plugin status. If a restart still
shows old QML, clear only the guest's QML cache with its shell stopped.

Report what you exercised, the expected and observed behavior, and any pending
or untested scenarios. Remove your temporary captures after verification.

## Conventions

- Keep core filesystem and command logic in Rust; QML renders and routes actions.
  An extension helper may be any executable that speaks JSON over stdin and stdout.
- document implementation details in the technical docs; do not add code comments
- modules over regions: split a large file along a seam instead of stacking sections in one
- external commands take argument vectors, never a shell string
- mutations report completed work and journal failures separately; see SECURITY.md for recovery limits
- dynamic text from the filesystem is `Text.PlainText` and length-bounded before it enters a model
- Built-in and user modules use `blade.json`; plugin modules register through
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
