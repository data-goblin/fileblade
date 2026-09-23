This file was written by an agent.

# Core migration validation

Native migration and persistence are integrated in the current authority.
The milestone and guest instructions below record historical migration
qualification, including its original dependency boundaries; they do not
describe unfinished release work. Current startup ownership is described in
[the lease README](../src/lease/README.md), and installed runtime validation in
[the native installation guide](../docs/agent-written/native-install.md).

`migration::prepare(&legacy, &native, &authority)` consumes runtime's root-bound
`lease::Authority`. Its implementation requires the authority API published at
`lane/runtime` commit da0b1a2; core does not select the backend write mode.
Runtime must call it after `acquire_bound`, before recovery or document hydration,
and map Ready to Full and both other outcomes to ReadOnly.

The importer selects state.json, module state, settings.json, keybindings.json,
blades.json, colors.json, hook labels, module configuration, Hooks/MCP recovery
records and the legacy artifact bin. JSON bytes and unknown fields survive;
module storage directory aliases resolve to core names. Conflicting aliases or
native destinations refuse import. Layout IDs remain readable through the core
registry's existing aliases and singleton recovery policy.

The private `migration-020/receipt.json` under the native state root is a version 2
snapshot of original bytes, directory entries, stored link targets, supported user
extended attributes and root identities. Older receipts are refused without reinterpretation. Publication never overwrites a different destination. `copied.json`
certifies that destination publication finished before legacy artifact retirement;
`complete.json` certifies the completed move. These checkpoint documents match
the receipt exactly. Retry verifies unfinished sources and completed steps.
A completed import does not overwrite subsequent native edits. Legacy state and
configuration remain available; retired artifact-bin objects remain under the original bin root in
`.migration-020-retired/<receipt entry index>`, as well as in the receipt. There is no automatic rollback CLI in this slice.

The importer does not alter shell activation, desktop bindings, companion
checkouts, generated desktop snippets, journal replay or unrelated shell components.
Active and unknown legacy-writer evidence refuse writable ownership.

Run the product tests only in harness C. Compile the test executables with
`cargo test --release --locked --no-run --test migration_prepare --test migration_writer --test migration_documents --test migration_preservation`.
Select the fresh runtime using FILEBLADE_BINARY wherever a suite launches it.
Copy the migration_prepare executable to target/release/rivet-migration-prepare
before the harness push, then restart and verify the Service module catalog and
an open-blade screenshot. In C run that executable and
`python3 -B tests/vm/fixtures/run-migration.py`. The latter generates real
legacy-route Hooks/MCP records using the selected backend; shell/IPC detection
is isolated with deterministic executable fixtures, never host activation.

`tst_migration_state.qml` checks unknown state fields across repeated writes;
`migration_preservation` checks keybindings metadata updates preserve unknown
fields and refuse newer/malformed originals. The importer suite checks first
and repeated launch, resumable publication, conflicting edits, malformed/newer
schema, missing/mismatched helper evidence, module alias conflicts, stored
symbolic links and authority-root replacement. Keep the combined source revision
and any core overlay in the lane milestone; it is not a standalone core gate
until the runtime dependency has been integrated.

The 5.5 consumer acquires both the legacy journal's shared OFD/flock locks and
the artifact bin's exclusive mutation lock and both helper recovery-store locks after the initial stopped/absent
proof. It repeats the proof while holding them, retains them through publication
and cleanup, and checks root/lock identity before every mutation boundary.
Contention and changed activation evidence yield read-only startup. The probe
uses direct no-follow legacy reads even when native persistence is not registered.
Native documents are also preflighted on repeated startup, so a completed receipt
does not authorize downgrading newer state. The authority stays read-only until
runtime explicitly accepts the preparation outcome. Tests include held locks,
lock replacement, changed evidence, native startup environment and shared roots.

S11 validation adds 24 first/completed native-document refusal cases, late new or
changed source objects, helper lock contention, and a killed process during real
artifact retirement followed by retry. Retirement uses verified source-root
descriptors and non-overwriting rename into retained storage; an unexpected object
is kept and restored to its old name when possible. Inventory is checked before
and after retirement. The importer never unlinks retired source objects.

Supported user xattrs are captured, compared on retry/conflict, installed before
publication and synced. Unsupported attribute namespaces are refused so private
storage modes are not weakened. Artifact schema validation mirrors the bounded
production Manifest/StoredItem rules and checks exact stored kind, size and link
bytes. Core helper pairs are checked against their record schema; historical
payload-based lookup and unrelated extension routes stay supported unchanged.

The real fixture runs artifact put, migration, native bin reads for all four
modules, and native file/directory restore with xattr equality. Hooks/MCP payload
restore is checked with the production consumer after relocating the imported
objects to its legacy data-root paths; that data-format check is explicitly not
native helper continuation qualification. S2-C3 remains the native helper route
dependency. The combined source used for this check is published runtime da0b1a2
plus the recorded core overlays; the exact version-branch merge gate follows
runtime integration.

Known Hooks and MCP recovery payloads also pass their production restore
validators before Ready, including when revisiting a completed receipt.
`src/migration/artifacts.rs` validates the inventoried payloads in process with
the same restore parsers the modules use; it never opens a live source or
recovery store. Hooks restoration shares `validate_record`; JSON/TOML MCP
restoration shares `validate_json_record`/`validate_toml_record`, including
typed fingerprints and the isolated TOML table. Unknown extension payloads
remain opaque. A parser refusal preserves the originals and refuses writable
migration. No additional runtime dependency is introduced.
