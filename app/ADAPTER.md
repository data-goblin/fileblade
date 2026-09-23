This file was written by an agent.

# Native application runtime

The production payload places this directory beside `bin/fileblade`.
The M1 visual gate passes under the coordinator's declared pixel tolerance.
The shutdown failure reproduces in an empty Quickshell with `-d`; foreground
launching exits cleanly with FileBlade's original backend cleanup. A process supervisor must launch Quickshell in the foreground.

`shell.qml` loads the existing `Service.qml` through an absolute file URL.
Existing QML imports and browser sources are unchanged. The shell facade reads
the real shell configuration for bar placement; the private kit reads the
current Omarchy theme. `BarVisibility.qml` subscribes through the resident
backend to the real Omarchy toggle directory and reads its `bar-off` flag.
The facade follows explicit hide and restore changes without idle polling;
timed hover-autohide remains unqualified.
Bar visibility, compositor rounding/gaps and font matching are queried through
the existing resident backend. BackendClient is the only QML process owner;
the kit starts no shell, hyprctl or fc-match processes itself.

`launch` resolves its physical payload and runs `bin/fileblade`, inheriting
normal XDG directories. With no arguments it starts the authority and native
view, or activates the existing view. It does not change desktop roles.
`launch native ipc -- TARGET METHOD [ARG ...]` preserves the shared QML IPC
argv. `launch _backend COMMAND [ARG ...]` sends the existing backend argv to
the resident authority and returns its JSON payloads, fetching completed
mutation results after delivery. It never falls back to an ownerless writer.

The authority pins the selected state, config and recovery roots. Normal
startup prepares the current plugin namespace in place with core's importer
before persistence or recovery. Ready enables writes; active or unknown
legacy writers and refused migration keep the app read-only with a reason
and receipt path. No pre-import rename selects the old `omarchy/filetree`
namespace. Durable records open relative to pinned directory descriptors;
identity loss invalidates the authority and stops accepted work at its next
barrier with an explicit result.

`qualification/launch` retains the isolated M1 fixture entry point. It requires
`FILEBLADE_SPIKE_HOME` and a local release build, isolates config/state while
inheriting desktop data, and enables writes only after verifying those fixture
roots. Its environment also reaches child applications. It is separate from
the production payload launcher.

Accepted mutations survive view shutdown and retain results in the authority
until fetched or 24 hours. Process-death recovery still uses durable journals;
the in-memory result cache does not survive authority process death.

`launch native drain --timeout-ms 30000 --json` flushes ordinary Notes and
state/layout through their existing writers, waits for active operations,
pauses new admission, and requires a final QML acknowledgment before exit.
It waits for the authority lease and matching portal processes to release.
Its schema-1 JSON reports `drained` or `already_stopped` with exit 0, `busy`
with exit 3, or `error` with exit 1; invalid arguments return 2. The result
also carries `action`, `operation_ids`, `dirty_note_ids` and `error`.
The deadline accepts 1–300000 ms. Busy/error forbids an installer transition;
there is no forced view or authority kill. Temporary Notes popouts are
refused because their text is not durable. UI discovery is bounded at 20000
objects and fails closed when exhausted. On timeout, the authority resumes
on caller EOF and the QML drain token expires; no cleanup extends the CLI
deadline. Restart means successful drain followed by no-argument launch.

## Source accounting

The kit derives from Omarchy v4.0.2, commit
`346e69e1cec6c4e8924531874af6ba010a1bc99e`.
`upstream.json` records the original source paths and SHA-256 values of all
15 copied files. Each matched the installed Omarchy 4.0.2-1 package before
adaptation. `licenses/omarchy-MIT.txt` retains the upstream copyright and
permission notice. No upstream UI path is symlinked into this directory.

The inventory found five direct FileBlade UI dependencies: BorderOverlay,
BorderSurface, Button, PanelToolTip and TextField. Goblins additionally imports
BarWidget, BarIconButton and KeyboardPanel; the latter components bring
WidgetButton and OpticalGlyph into the closure. Commons supplies Color, Style,
Util, Border and BorderGeometry. Supported companion QML also consumes this
theme surface. The live importer gate loads all eight built-in modules and the unchanged
Goblins bar widget and KeyboardPanel popout. Its opt-in fixture supplies the
bar host and provider descriptor; production extension discovery and helper
activation remain separate qualification.

Changes from upstream:

- Explanatory code comments are removed; the license is retained separately.
- Util retains only clamp, clampAlpha, alpha and fileUrl.
- Color omits notification, polkit, lock and image-picker surface objects.
- Theme colors and shell values use file-change reloads, because the native
  instance does not receive Omarchy shell theme IPC.
  The stable `current/theme.name` completion marker reloads both theme files
  after Omarchy replaces their containing directory, including switching back
  to a previously used theme. The folder palette follows the same marker.
- BarWidget and KeyboardPanel accept a dynamic bar facade, matching their
  existing accesses to host-provided members.
- KeyboardPanel.close() catches a throwing owner and hides the panel itself.
  The panel takes input over the whole screen, so an owner whose close path
  throws would otherwise leave every click on the desktop going to it.
- Module registration files expose only this dependency closure.

## Evidence limits

Harness A, SSH 2422: Qt base 6.11.2-2, Qt declarative 6.11.2-1,
Quickshell 0.3.1-1, Omarchy 4.0.2-1, Hyprland 0.56.2-1, software rendering.
Cold native loading, fixture listing/selection and live Tokyo Night to
Catppuccin repaint were observed with the FileBlade plugin unavailable.
Docking, conversion to an ordinary window, and docking again work through
the native authority. The Never preference was saved through the QML client
and verified on disk.

Matched Catppuccin plugin/native blade captures differed at three of 400,520
pixels, each by one RGB channel level. R12 declares a pass at no more than one
level per channel and fewer than 0.1 percent differing pixels. This passes.

The final bar property typing changes in shell.qml, BarWidget and
KeyboardPanel were rerun in the VM. Guest qmllint exited zero with three
KeyboardPanel warnings inherited from its upstream PanelWindow/contentItem
declarations and one BackendClient QProcess::ExitStatus metadata warning.

Detailed source identities, screenshots, commands, shutdown diagnostics and
contract requests are in Sootscale's ignored aim note and evidence directory.
Sixteen native authority tests pass in harness A, including the 256 MiB copy
after EOF, a multi-file move with a lost progress subscriber, explicit cancel,
result fetching, lock identity, and the Qt pipe-availability regression.
The retained server and persistence suites pass another seventeen tests.
The live native authority expectation also passes: a two-file move continues
after the QML view quits, preserves both mappings and the same authority,
and remains queryable after view relaunch until explicitly fetched.
A real QML copy interrupted by root replacement leaves the replacement
sentinel unchanged and retains an explicit authority-lost result after view
exit.
Monitor and hotplug qualification passes in harness A. The matched
single-output plugin/native focus and exclusive-zone checks pass. Lock/unlock interaction, remaining retained expectations and chooser
remain separate qualification work.

`ovm-spike` adapts the retained expectation scripts to harness A and the native
IPC target without changing their source. Other shell targets still address
the real Omarchy bar. Guest commands use the native XDG roots and binary;
Trash fixtures use the inherited desktop data profile. The retained suite's
fixed direct `trash-list` query is routed through the existing authority by
`qualification/trash-list`, preserving native ownership checks. Restart
requests restart the foreground native view and the real shell while keeping
the authority alive. Push, install and reset require explicit staging outside
this adapter.

R19 is integrated. Retained E-14-11 passes the original-path metadata and
GIO checks with the desktop data profile inherited. The final retained E14
run passes all 23 checks, including trash keyboard selection. Collapse the
unrelated Properties preview for a stable OCR fixture; earlier OCR failures
are retained in the evidence. Native and legacy server integration tests
also verify their distinct artifact roots and
clipboard ownership across view EOF and authority shutdown using a mock
foreground clipboard process. Real desktop clipboard interoperability is a
separate operations gate.

## Reproducing the opt-in R15 checks

Launch the existing native fixture with `FILEBLADE_QUALIFICATION=1` and
`FILEBLADE_GOBLINS_FIXTURE` pointing to the unchanged Goblins source fixture.
The probe is absent from a normal launch. After restarting, verify that the
live module catalog contains all eight built-ins and capture an open blade
before running a suite. These scripts target harness A only.

- `tests/vm/expectations/41-native-importers.sh`: E41-01 loads each built-in
  importer; E41-02 opens the real Goblins bar popout and closes it with Escape.
  Guest evidence is under `/tmp/fileblade-r15-importers`.
- `tests/vm/expectations/42-native-layout.sh`: exercises retained E20-01
  through E20-12 with live input, rendered geometry and screenshots. The
  merged drag-card fix and visible-card probe pass all 18 assertions with
  the bar visible and hidden, including unobscured titles during tab insertion.
- `tests/vm/expectations/44-native-bar-state.sh`: compares internal blade
  coordinates with actual compositor geometry for visible, hidden and
  restored bars at all four edges, then twenty rapid hide/show cycles.
  Its fourteen captured states pass; guest config and visibility are restored.
- `tests/vm/expectations/43-native-parity.sh native`: E43-01 tests keyboard
  capture and release to a terminal; E43-02 measures docked exclusive zones
  and their release; E43-03 records real bar hide/restore geometry. The
  `plugin` argument runs the same procedure against an already prepared
  plugin baseline. Guest-only staging and activation are authorized fixture
  operations under R43; host and package installation remain outside this procedure.

E20 needs `/tmp/fileblade-qualification-pointer` in the guest. Build it using
`app/qualification/build-pointer EXISTING_PROTOCOL_XML /tmp/fileblade-qualification-pointer`
and the existing C compiler, wayland-scanner and wayland-client development
files. The XML is `wlr-virtual-pointer-unstable-v1.xml` from the existing local
protocol sources. No dependency installation is part of this procedure.
The helper runs only on hostname `omarchy-test`, requires one output and an
owned FIFO, and supplies one persistent pointer for a complete drag gesture.
The scripts use a single 1920 by 1080 Virtual-1 output with a top 26-pixel bar.
Host evidence for E20 and E43 is under `.claude/evidence/sootscale/r15/`.

On the measured tuple, hiding the real bar changes its top reservation from
26 to zero. Hyprland expands native blade surfaces from y=26, height=1054 to
y=0, height=1080. The facade now reports barHidden=true and surfaceOriginY=0;
restoring the bar restores the 26-pixel internal origin. Bottom, left and
right reservations also match internal coordinates after each transition.
The visibility flag is named bar-off, so `omarchy-toggle-bar on` hides it and
`off` shows it. The earlier matched plugin/native run has identical compositor
geometry and reservations at all eight captured phases. These are explicit
visibility-toggle measurements; timed hover-autohide and lock/unlock
interaction remain unqualified.


## Chooser session consumer

The R50 optional chooserSession seam defaults to null. A chooser uses the
same Service, TreePane, PickerController and PickerBar, with unique in-memory
state/layout identities, no ordinary IPC or blade surfaces, and an adapter
over the existing BackendClient. The adapter forwards browsing reads, matches
caller filters in Rust, and refuses unrelated writes and visit side effects.
No chooser component owns a Process. Scope supplies the session object because
Item already owns the focus property required by the callback interface.

ChooserManager loads app/chooser/ by file URL, as shell.qml loads Service.
Static imports into shared directories hit Quickshell's qs-blackhole scanner
boundary on the qualified tuple. ChooserBrowser only overrides confirmation
so the Rust request boundary owns Save validation and overwrite decisions.

The transport consumer is always on; FILEBLADE_CHOOSER=0 is a development
override that disables it, and the "File chooser" desktop role owns portal
activation. The qualification-only ChooserProbe supplies isolated UI fixtures;
E45 explicitly does not claim that those fixtures complete a portal request. Parented foreign
windows and actual browser upload remain required integration checks.

Opening a chooser calls the existing blade focus handoff before showing its
window. E45 starts with a focused ordinary blade, selects distinct files with
the real pointer in two chooser windows, and cancels one with Escape. The
ordinary root and persisted state/layout remain unchanged, and the two views
share one resident backend process. The UI fixture uses the existing browsing
backend; broker completion, live filters and Save validation are exercised separately
by E46 against the registered native authority. The caller's custom accept label is not yet
wired into the shared footer.
