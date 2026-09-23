This file was written by an agent.

# v0.2.0 release readiness

**The native engineering review passed; the release remains unpublished.**
The updated documentation and rebuilt backend need a fresh release snapshot and
artifact manifests before publication. Checks using the published downloads are
pending publication.
There are **111 feature guides with 111 reviewed screenshot/MP4 pairs**.

## Confirmed checks

| Check | Result |
| --- | --- |
| Complete local `tests/run` | Passed: 761 Rust checks and 832 QML checks |
| Generated extensions and native theme reload | Passed |
| Bundled backend | Source, checksum, static linkage and byte-identical rebuild passed |
| Native install and removal | Verified payload installed; owned roles restored; runtime removed |
| Shipped Hyprland bindings | Actual Super+B opened and closed blades in the installed native app |
| Native autostart | Installed desktop entry cold-launched through GIO and restored the real tree |
| Folder and Reveal | GIO folder handoff and automatically activated FileManager1 selected the requested file |
| File chooser | Real portal frontend/backend activation; completed response 0 with the selected file URI |
| Desktop-role reversal | All five roles restored their owned entries without leftovers |
| Real Omen SFTP | Connect, refresh, list 230 entries and disconnect passed; separate live regression passed |
| Feature artifacts | 111/111 pass; 2560×1440, 4–8 seconds, original speed |
| Branding and framing | Wallpaper, original full SVG artwork, equal visible 32px padding; each blade below one-third |
| Isolation | Separate local Hyprland desktop on workspace 9 with private HOME, XDG paths and D-Bus |
| Post-publication VM qualification | Requested; pending publication |

The complete code-gate log is retained locally at
`target/release-notes-capture/final-gate.log`; it includes the final embedded
documentation and rebuilt backend. The documentation sweep checked 148 Markdown
files and retained diagrams, and the bundled CLI generated four native extension
guides with a valid manifest and SVG banner. Production integration receipts and
new recordings are in `target/feature-evidence/release/`; media ordering proof is
in `target/feature-evidence/media-sort/`. The final release
artifact receipt, `target/release-review/final-artifacts.json`, binds the recorded
runtime source commit to the staged payload, archive and package. Later readiness
documentation and embedded extension-guide changes require fresh release artifacts;
the previous receipt remains evidence for its recorded commit. Older recording source
identities remain in the [coverage manifest](../catalog.json); they are not
silently relabelled as captures of the final commit. Raw takes and prior exports
are retained locally.

The consolidated [release notes](release-notes.md) include four additional
feature crops captured on isolated workspace 9. Their local capture and cleanup
receipts are in `target/release-notes-capture/`; the active workspace and focused
window were unchanged, and all owned capture processes have stopped.

## Defects fixed during review

| Problem | Correction and evidence |
| --- | --- |
| Release archives could combine different application versions | Mixed versions are rejected; regression passed |
| Bootstrap metadata could disagree with the selected artifact | Schema, version, target and hash agreement are checked before installer execution |
| Native desktop roles mishandled reserved path characters | Shared command quoting preserves reserved characters; real GIO/D-Bus regressions passed |
| Doctor could report healthy while recovery blocked writes | Blocked recovery makes the diagnostic unhealthy |
| CLI operations could bypass the active native backend | Public operations use shared authority routing with bounded timeouts |
| Refused UI controls could exit successfully | Invalid control targets and unavailable screens return failure |
| MP4 posters failed with trailing metadata | Bounded, sealed, seekable input feeds FFmpeg; real MP4 regression passed |
| Copy collisions had no visible decision dialog | Files binds the existing dialog; real Keep both interaction preserves both files |
| A picker inherited the media grid and hid text files | Picker mode uses the ordinary tree without clearing the saved media preference; real selection passed |
| Reveal could select the parent instead of the requested file | Initial focus preserves explicit selection while the model loads; real FileManager1 handoff passed |
| tmux pane actions lost named socket identity | Discovery retains client socket selection and native socket metadata; real PTY and compositor regressions passed |
| Refresh discarded connections made through a validated tailnet SSH alias | Canonical hosts and currently validated aliases share retention rules; real Omen regression and native pane passed |
| Deep snapshot roots confused an inventory regression | The regression supplies its explicit application root |
| Shipped bindings always targeted the native shell | Shared public CLI routing; actual native shortcuts passed |
| Valid long state paths blocked native startup and removal | Descriptor-relative socket addressing preserves the owned directory; real long-path CLI regression passed |
| Media inherited tree order and displayed irrelevant metadata controls | Saved newest/oldest arrow replaces Git and column controls; real mouse, Space and restart proof passed |

The tmux regression covers separate and joined `-L` arguments, explicit and relative `-S` paths, and a custom `TMUX_TMPDIR`. The source-contract assertion was updated to recognize the corrected picker expression; the final complete gate passed afterward.

## Evidence boundaries and publication

- Native extensions open in blades.
- Autostart proves the installed desktop entry starts the real runtime. It does
  not claim a complete logout/login transition.
- Recovery and migration use disclosed fixtures. Short clips prove their named
  workflows, not every possible input or failure condition.
- Omen evidence is read-only and specific to the authorized peer and directory.
- The working desktop was not restarted or reinstalled during background
  qualification. The isolated desktop used real installed components.
- The root installation guide is updated under the owner's instruction to
  complete these release tasks.
- This release distributes the native application. Publishing the native package,
  archive and version tag remains pending, followed by the requested installation
  and core-feature checks using the published downloads in an isolated VM.
- Documentation edits since the prepared artifact commit must be included in a
  fresh release snapshot and its source/hash manifests before publication.

There are no known unresolved product blockers from the engineering review.
Publication and post-publication checks have not been completed. See the
[decision record](decisions.md) for the native release classifications.
