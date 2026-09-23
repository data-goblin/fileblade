This file was written by an agent.

# v0.2.0 release readiness

**The release candidate passes the local engineering gates.** Publication still
requires the owner's exact-commit marketplace verification and release publication.
There are **113 feature guides with 113 reviewed screenshot/MP4 pairs**.

## Confirmed checks

| Check | Result |
| --- | --- |
| Complete local `tests/run` | Passed: 761 Rust checks and 832 QML checks |
| Generated extensions and native theme reload | Passed |
| Bundled backend | Source, checksum, static linkage and byte-identical rebuild passed |
| Native install and removal | Verified payload installed; owned roles restored; runtime removed |
| Real Omarchy plugin lifecycle | Add, activate, disable, update, remove and fresh shell restart passed |
| Bar popout | Registered extension loaded through FileBlade's own production bar widget; clicks, Escape, repeat, refusal and teardown passed |
| Shipped Hyprland bindings | Actual Super+B opened and closed blades in both native and plugin mode |
| Native autostart | Installed desktop entry cold-launched through GIO and restored the real tree |
| Folder and Reveal | GIO folder handoff and automatically activated FileManager1 selected the requested file |
| File chooser | Real portal frontend/backend activation; completed response 0 with the selected file URI |
| Desktop-role reversal | All five roles restored their owned entries without leftovers |
| Real Omen SFTP | Connect, refresh, list 230 entries and disconnect passed; separate live regression passed |
| Feature artifacts | 113/113 pass; 2560×1440, 4–8 seconds, original speed |
| Branding and framing | Wallpaper, original full SVG artwork, equal visible 32px padding; each blade below one-third |
| Isolation | Separate local Hyprland desktop on workspace 9 with private HOME, XDG paths and D-Bus |
| VM qualification | Skipped at the owner's explicit instruction |

The complete code-gate log is retained locally at
`target/release-review/media-sort-full-gate.log`. Production integration receipts and
new recordings are in `target/feature-evidence/release/`; media ordering proof is
in `target/feature-evidence/media-sort/`. The final release
artifact receipt, `target/release-review/final-artifacts.json`, binds the clean
main commit to the staged payload, archive and package. Older recording source
identities remain in the [coverage manifest](../catalog.json); they are not
silently relabelled as captures of the final commit. Raw takes and prior exports
are retained locally.

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
| Extension popouts relied on an unavailable foreign service lookup | FileBlade owns its bar entry and uses Omarchy's self-scoped service API; real registered extension passed |
| A loaded popout did not receive keyboard focus | Shared focus transfer and Escape dismissal; installed-session regression passed |
| Shipped bindings always targeted the native shell | Shared public CLI routing; actual plugin and native shortcuts passed |
| Valid long state paths blocked native startup and removal | Descriptor-relative socket addressing preserves the owned directory; real long-path CLI regression passed |
| Media inherited tree order and displayed irrelevant metadata controls | Saved newest/oldest arrow replaces Git and column controls; real mouse, Space and restart proof passed |

The tmux regression covers separate and joined `-L` arguments, explicit and relative `-S` paths, and a custom `TMUX_TMPDIR`. The source-contract assertion was updated to recognize the corrected picker expression; the final complete gate passed afterward.

## Evidence boundaries and publication

- The bar popout is a plugin-mode integration. Native extensions open in blades.
- Autostart proves the installed desktop entry starts the real runtime. It does
  not claim a complete logout/login transition.
- Recovery and migration use disclosed fixtures. Short clips prove their named
  workflows, not every possible input or failure condition.
- Omen evidence is read-only and specific to the authorized peer and directory.
- The working desktop was not restarted or reinstalled during background
  qualification. The isolated desktop used real installed components.
- The root installation guide is updated under the owner's instruction to
  complete these release tasks.
- Publishing, tagging and marketplace verification remain separate from local
  readiness. Release from `main`, name its full SHA in the Plugin verification
  issue, and keep that commit fixed throughout review. No marketplace post was
  made by the agent; repository instructions reserve that step for the owner.

The marketplace's [submission requirements](https://github.com/omacom/omarchy-plugin-marketplace/blob/main/SUBMISSION.md)
require commit-specific verification. The earlier
[verification issue](https://github.com/omacom/omarchy-plugin-marketplace/issues/7339)
does not verify this candidate.

Jev classified the new runtime checks as `production_paths_exercised` (1.00),
autostart as `desktop_entry_launch` (0.98), and the popout as a
`plugin_integration` (1.00). These classify the supplied facts; they are not
measured probabilities of release success. The [decision record](decisions.md)
retains all distributions and the earlier, superseded limitations.
