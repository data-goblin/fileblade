This file was written by an agent.

# v0.2.0 release readiness

**Not ready to publish.** Local code, the x86_64 package and native archive pass their checks. There are **112 feature guides with 112 reviewed screenshot/MP4 pairs**. The final release identity and qualification gates remain open.

The latest complete gate, staged payload and packages identify review snapshot `298244cd58b89977411a35822bbf333c44c610de`. This candidate includes the SSH-alias retention fix found during real Omen testing. Its application code and backend match the working checkout. The earlier recordings retain their original source identities in the coverage manifest; Omen’s new capture identifies this corrected candidate. This is not a released main commit. Concurrent inventory changes were included and preserved.

## Confirmed checks

| Check | Result |
| --- | --- |
| Complete local `tests/run` | Passed: 761 Rust checks and 832 QML checks |
| Generated extension scaffolds | Passed |
| Native theme replacement and return | Passed |
| Bundled backend | Source, checksum, static linkage and byte-identical rebuild passed |
| Corrected native payload | Staged, verified and installed in the disposable capture profile |
| Arch package | Built; extracted payload identity verified |
| Native archive and `latest.json` | Built; checksum, version and extracted payload verified |
| Feature artifact gate | 112/112 pass |
| Real Omen SFTP | Connect, refresh, list 230 entries and disconnect passed; separate live regression passed |
| Visual evidence | 2560×1440, 5.47–7.93 seconds, wallpaper visible, full SVG artwork, equal 32px padding, each blade below one-third |
| Desktop isolation | Separate local Hyprland compositor on workspace 9; owned processes stopped and fixture removed |
| VM qualification | Not run, at the owner's explicit instruction |

The complete gate log is retained locally at `target/release-review/omen/full-gate.log`. The candidate is `target/release-review/omen-payload`; package and release-archive outputs are in `target/release-review/omen-package` and `target/release-review/omen-release-assets`. `target/release-review/final-artifacts.json` records their identities. These are local artifacts, not published release assets. Raw recordings, prior exports and capture provenance are retained in `target/feature-evidence/`.

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

The tmux regression covers separate and joined `-L` arguments, explicit and relative `-S` paths, and a custom `TMUX_TMPDIR`. The source-contract assertion was updated to recognize the corrected picker expression; the final complete gate passed afterward.

## Remaining release gates

- Qualify production integration where evidence is intentionally narrower: some desktop-role clips show registration; the popout uses its qualification host; the legacy plugin clip shows installation without activation. The portal and Reveal clips do include real completed handoffs. Recovery and migration use disclosed fixtures.
- Review the prepared root README patch. The repository explicitly reserves that file for the owner; it remains unapplied pending authorization.
- Schedule replacement of the working installation and shell restart. Installation drains the active runtime, so these remain pending under the later instruction not to interrupt the session. The corrected candidate was installed only in the disposable profile.
- Select the final main commit and bind release verification to that exact SHA. Recheck changes made after this snapshot. No release, tag, push or marketplace post was made during this review.
- Qualify and publish the intended distribution assets. The x86_64 candidates are local and must identify the final release commit.

The [coverage manifest](../catalog.json) is authoritative about each recording's source, hashes, review and limits. `python3 tests/feature_evidence.py` passes for all 112 workflows. [Omen’s SFTP proof](../integrations/sftp.md) is read-only; connection, refresh and disconnect were checked separately from the 5.97-second browsing clip.

The marketplace requires commit-specific verification; the existing [verification issue](https://github.com/omacom/omarchy-plugin-marketplace/issues/7339) concerns the older release. See the [submission requirements](https://github.com/omacom/omarchy-plugin-marketplace/blob/main/SUBMISSION.md), [v0.1.3 assets](https://github.com/data-goblin/fileblade/releases/tag/v0.1.3).

Jev classified the updated facts as `locally_validated_release_blocked` (1.00), the bounded catalogue evidence as complete (1.00), and SFTP’s placement beside tailnet discovery under integrations as appropriate (1.00). These classify supplied evidence; they are not measured probabilities of release success. The [decision record](decisions.md) preserves the distributions and limitations.
