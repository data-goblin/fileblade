This file was written by an agent.

# Native release classification decisions

Jev classified the supplied facts using the keyring credential with the owner's consent. These are classifier distributions, not measured probabilities of
release success or permission to publish.

The [machine-readable record](classifications.json) preserves each retained
record's context, full probability distribution and confidence. Earlier
distribution-specific records remain at the Git commit recorded there.
Historical classifications describe their consultation time;
[release readiness](readiness.md) records the current state.

The documentation cleanup is `native_docs_with_legacy_drift` (1.00). The
existing Built for Omarchy badge category is `app` (1.00). The catalogue now
contains 111 native workflows; the two shell-hosted guides and their recordings
remain in Git history.

| Consultation | Question | Chosen class | Probability | Confidence |
| --- | --- | --- | ---: | ---: |
| collision | defect | `blocking_missing_ui_wiring` | 1.00 | 1.00 |
| collision | repair | `reuse_existing_dialog` | 1.00 | 1.00 |
| sftp-proof | sftp_prerequisite | `missing_user_target` | 1.00 | 1.00 |
| capture-correction | evidence_status | `recapture_required` | 0.57 | 0.42 |
| capture-correction | new_geometry | `meets_requested_geometry` | 0.96 | 0.95 |
| capture-standard | old_evidence | `fails_new_explicit_requirements` | 1.00 | 1.00 |
| capture-standard | new_sample | `compliant_except_final_padding_recheck` | 0.98 | 0.97 |
| capture-standard | remaining_delivery | `incomplete` | 0.94 | 0.91 |
| reinstall-timing | timing_constraint | `timing_conflict` | 1.00 | 1.00 |
| release-readiness | release_state | `locally_validated_but_release_blocked` | 1.00 | 1.00 |
| release-readiness | docs_segmentation | `coherent_user_workflow_structure` | 0.91 | 0.87 |
| release-readiness | remaining_proof | `bounded_partial_evidence` | 1.00 | 1.00 |
| picker-media | behavior | `interaction_defect` | 1.00 | 1.00 |
| picker-media | correction | `reuse_existing_state_boundary` | 1.00 | 1.00 |
| reveal-selection | reveal | `selection_focus_race` | 1.00 | 1.00 |
| reveal-selection | repair_scope | `shared_state_guard` | 1.00 | 1.00 |
| tmux-socket | defect | `named_socket_discovery` | 1.00 | 0.99 |
| tmux-socket | repair | `reuse_native_metadata` | 1.00 | 1.00 |
| omen-alias-retention | defect | `alias_retention_bug` | 1.00 | 1.00 |
| omen-alias-retention | repair | `consistent_validated_host_matching` | 0.99 | 0.99 |
| omen-complete-evidence | evidence | `complete_bounded_catalog` | 1.00 | 1.00 |
| omen-complete-evidence | release | `locally_validated_release_blocked` | 1.00 | 1.00 |
| omen-complete-evidence | grouping | `integrations_with_tailnet` | 1.00 | 1.00 |
| socket | defect | `unix_address_length_defect` | 1.00 | 1.00 |
| socket | repair | `directory_descriptor_path` | 1.00 | 1.00 |
| media-sort | state_scope | `media_pane_preference` | 1.00 | 1.00 |
| media-sort | documentation | `files_media_sort` | 1.00 | 0.99 |
| native-documentation-and-badge | documentation_scope | `native_docs_with_legacy_drift` | 1.00 | 1.00 |
| native-documentation-and-badge | badge | `app` | 1.00 | 1.00 |
| native-test-documentation | remaining_docs | `in_scope_documentation_drift` | 1.00 | 1.00 |
| native-documentation-complete | documentation | `complete` | 0.92 | 0.89 |
| native-documentation-complete | release | `validated_working_tree_pending_release_artifacts` | 1.00 | 1.00 |
| release-notes-crops-and-branches | screenshots | `complete_focused_crops` | 0.99 | 0.98 |
| release-notes-crops-and-branches | branches | `fully_merged_inactive_refs` | 1.00 | 1.00 |
| consolidated-release-history | structure | `single_unversioned_release_history` | 1.00 | 1.00 |
| release-history-ancestry | history | `superseded_content_unmerged_history` | 1.00 | 1.00 |

The earlier branch classification used an incorrect ancestry assertion. Direct
Git checks found seven original review commits outside main ancestry, with their
implementation already incorporated and superseded. The corrected classification
records the need to join that history while retaining the validated current tree.
