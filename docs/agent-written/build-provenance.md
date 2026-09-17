This file was written by an agent.

# Backend build provenance

[Build backend provenance](../../.github/workflows/bundle-provenance.yml) is a
manually dispatched delivery build. It runs the existing `tools/bundle build`
recipe on GitHub, requires its output to equal the committed bundle, and signs
the backend's digest with GitHub Actions build provenance. The test suite still
runs locally; this workflow does not run `tests/run`, create commits, publish a
release, change tags, or update marketplace threads.

This workflow is an authorized exception to the project's former prohibition
on hosted automation, specifically for independently verifiable build provenance.
Adding it does not establish provenance for an earlier commit. A successful
hosted run must exist for the exact source commit a reviewer is asked to accept.

## Release sequence

The local version hook requires Python 3.11 or newer. Enable it with
`git config core.hooksPath tools/hooks`. It compares the effective Git index's
manifest, package, lockfile, newest versioned changelog heading and bundled
backend against one another and the release branch name. Partial commits use
Git's temporary index; unstaged work is preserved. Missing, unresolved or
symlinked sources are refused. Cargo must declare the FileBlade package version
directly, and the lockfile must identify exactly one local FileBlade package.

The hook runs the staged backend in a private temporary directory inside the
Git directory, with a five-second deadline and a 4 KiB combined output limit.
It requires executable mode and a successful `fileblade <version>` response;
temporary files and any remaining process group are removed afterwards. This
checks version alignment and does not replace the full local gate or bundle
verification. The hook's staged-commit regression tests run in `tests/run`.

`core.hooksPath` is shared by this repository's worktrees, but its relative path
resolves inside each worktree. A branch without `tools/hooks/pre-commit` runs
no version hook. Other hooks must also live in `tools/hooks` while it is enabled.

1. Finish the candidate, including release versions, companion pins and the
   locally rebuilt bundle. Run the local gate before publishing that candidate.
2. After the owner authorizes landing it, record the complete core commit SHA.
   The workflow must first exist on the default branch for GitHub to accept a
   manual dispatch. Selecting another branch is possible afterwards, but the
   workflow and checked-out source must come from the same selected commit.
3. Dispatch the workflow with that SHA as `expected_commit` and the corresponding
   branch or tag as `--ref`. If the selected ref has moved, the run fails instead
   of attesting a different candidate. It also requires `github.workflow_sha`
   to equal the selected source SHA before the build starts.

```bash
candidate=$(git rev-parse HEAD)
gh workflow run bundle-provenance.yml --repo data-goblin/fileblade \
  --ref main --field expected_commit="$candidate"
```

4. Wait for both jobs to succeed. Download the final
   `fileblade-backend-<SHA>-<run-attempt>` artifact, not the intermediate
   `fileblade-build-...` artifact. The final artifact contains `fileblade-bin`,
   its checksum and source fingerprint, third-party notices, `source-commit.txt`,
   `fileblade-bin.sigstore.json` and the verification result.
5. Verify the binary using the command below, then attach the attestation bundle
   alongside those same binary bytes to the owner-managed release. Workflow
   artifacts expire after 90 days; retain the portable attestation with the
   release. The intermediate transfer expires after one day.
6. Refresh validation and the core marketplace target to that exact commit,
   include the successful run and verification command, and keep core HEAD still
   through review. Unchanged companion commits retain their own review identities;
   a core-only workflow change does not itself create new companion SHAs.

There is no push, pull-request, schedule or release trigger. One workflow run
executes at a time, with a 30-minute build deadline and 10-minute attestation
deadline. A new run does not cancel a running one; GitHub may replace an older
pending run with a newer dispatch. Reruns use a distinct artifact name and do
not overwrite an earlier attempt's evidence.

## Independent verification

Obtain `candidate` from the reviewed commit, not from an unverified downloaded
text file. Use a GitHub CLI version supporting the source and signer digest
checks shown here:

```bash
candidate=FULL_REVIEWED_40_CHARACTER_SHA
gh attestation verify ./fileblade-bin \
  --repo data-goblin/fileblade \
  --signer-workflow data-goblin/fileblade/.github/workflows/bundle-provenance.yml \
  --source-digest "$candidate" \
  --signer-digest "$candidate" \
  --deny-self-hosted-runners
```

To supply the downloaded attestation instead of retrieving it from the repository,
add `--bundle ./fileblade-bin.sigstore.json` to the same command. This is portable
attestation evidence; fully offline verification also needs a trusted Sigstore
root, as described in the [GitHub CLI documentation](https://cli.github.com/manual/gh_attestation_verify).
The JSON verification result supplied with the download is convenient evidence,
not a substitute for verifying the signed bundle yourself.

The repository, signer workflow and both commit digests matter: checking only
the artifact checksum or repository owner could accept the wrong build or an
older attestation for the same bytes. Verification uses SLSA provenance v1 by
default. A failed signature or identity check must not be treated as success.

`tools/bundle verify` remains the independent local source-to-binary check at
that same commit. ZIP downloads from Actions do not preserve executable modes;
use `chmod +x fileblade-bin` before executing a downloaded backend. Changing that
mode does not change its digest.

## Build and permission boundaries

The build job has only `contents: read`. It checks out `github.sha` with checkout
credentials not persisted, installs the exact version in `rust-toolchain.toml`,
and uses a fresh Cargo home with the locked dependencies. It invokes the existing
bundle script: musl target, pinned Rust linker, static linkage, path remapping,
version checks and generated notices. It byte-compares the binary, checksum,
source fingerprint and notices to the selected commit, and rejects tracked source
changes. There is no cross-run dependency or build cache.

The second job downloads only the immutable artifact ID returned by this run's
build job and refuses an archive digest mismatch. It compares all four output
files against a separate checkout of the exact commit before signing. It does
not execute the binary, Cargo, the repository's build scripts or tests. Only this
job receives `id-token: write` and `attestations: write`; it has no repository
content, release, package or tag write permission. Artifact storage records are
disabled, so `artifact-metadata: write` is unnecessary.

The pinned `actions/attest` action generates the standard build-provenance
predicate and signs it through GitHub's OIDC identity and Sigstore. The workflow
verifies the resulting certificate and artifact against its own source and
workflow commit before uploading the final delivery artifact. A mismatch in the
rebuild or downloaded bytes prevents the attestation step from running.

Both jobs use GitHub-hosted `ubuntu-24.04`. The hosted image is maintained by
GitHub and is not an immutable OS image; the build logs record its image version
and `rustc -vV`. The Rust release and Cargo checksums are pinned, and byte equality
is mandatory across environments. This is build provenance, not a hermetic build,
SLSA Build Level 3 certification or a security audit of the source. Rust build
dependencies execute only in the job without attestation permissions.

## Pinned actions

Resolved from the official action repositories on 2026-09-09. Workflow code uses
the full commit, never the mutable release tag. Update pins as a reviewed source
change and produce new exact-commit evidence afterwards.

| Action | Release | Pinned commit |
| --- | --- | --- |
| [actions/checkout](https://github.com/actions/checkout/releases/tag/v7.0.1) | v7.0.1 | `3d3c42e5aac5ba805825da76410c181273ba90b1` |
| [actions/upload-artifact](https://github.com/actions/upload-artifact/releases/tag/v7.0.1) | v7.0.1 | `043fb46d1a93c77aae656e7c1c64a875d1fc6a0a` |
| [actions/download-artifact](https://github.com/actions/download-artifact/releases/tag/v8.0.1) | v8.0.1 | `3e5f45b2cfb9172054b4087a40e8e0b5a5461e7c` |
| [actions/attest](https://github.com/actions/attest/releases/tag/v4.2.2) | v4.2.2 | `1e69f48acb82d1966a394da916b4c1698aa569d6` |

GitHub recommends [actions/attest](https://github.com/actions/attest) for new
implementations; `attest-build-provenance` is now its wrapper. See GitHub's
[attestation guide](https://docs.github.com/en/actions/how-tos/secure-your-work/use-artifact-attestations/use-artifact-attestations)
and [manual dispatch requirements](https://docs.github.com/en/actions/how-tos/manage-workflow-runs/manually-run-a-workflow).
