---
name: rust-release-workflow
description: Prepare and verify a CogniGraph Rust release, including incoming PR review, scoped dependency updates, version and changelog changes, and authorized publication. Use for explicit release intent, not routine checks or dependency review alone.
---

# Rust Release Workflow

Follow [root instructions](../../../AGENTS.md). The gates below preserve decisions
about release scope, versions and publication. Apply authorization already given
in the conversation; ask only for a missing decision, with the concrete diff or
action ready for review. This skill is not blanket permission to merge, push or publish.

The root [push workflow](../../../AGENTS.md#push-workflow) applies before every
push, including one that does not create a tag or publish a package. It requires
incoming-PR processing, a product version bump and all applicable CI checks locally.

## 0. Establish release state

- Inspect the worktree, branch, remotes, tags, workspace/member manifests and
  release configuration. Fetch the selected remote for current state. Preserve
  unrelated work; prepare a clean isolated checkout if needed.
- Discover the actual target branch and protection. Integration uses `develop`;
  `main` is the release branch. Do not infer protection or
  approval rules from a local branch name.
- Inspect automation before promising it. At this revision,
  [ci.yml](../../../.github/workflows/ci.yml) verifies PRs and pushes to main/develop, also
  supports manual dispatch, and there is
  no `release.yml` artifact workflow. A tag push alone does not run release CI
  or produce binaries. Re-check this at release time.
- Identify the previous release tag relevant to the target and compare its
  manifest with current versions. The workspace inherits
  `[workspace.package].version`; verify member inheritance and internal
  dependency requirements rather than editing every `version` string.

**Gate — version state:** Compare the candidate with remote product history as
well as tags. An already-pushed version may legitimately have no release tag:
every push bumps the version, while tagging is a separate action. Use an existing
unpublished bump if it belongs to this same candidate; do not reuse the version
for a later outgoing change set. Resolve conflicting tags,
unexpected ancestry or a manifest behind the intended release tag before tagging.
Handle a first release explicitly when no prior tag exists.

## 1. Incoming PRs

Read current open PRs, target branches, review state, mergeability and checks
using available GitHub tooling. Present which changes affect the release.

**Gate — inclusion:** Resolve which PRs, if any, belong in the release before
merging. If none are ready, confirm continuation unless already directed.
Review included changes; review is not merge authorization. Re-check incoming
PRs immediately before an authorized push or develop merge and resolve new
inclusion decisions. Use the shared [develop gate](../../../docs/operations/develop-gate.md)
for exact-head accounting and fresh review/check snapshots; source CI repeats
the incoming check, and the Docker job checks again after image qualification.

## 2. Dependency scope

Before every develop integration or release, run
`python3 scripts/verify.py --suite dependencies` and the advisory suite.
The [develop gate](../../../docs/operations/develop-gate.md) owns freshness scope
and exceptions. The checker resolves updates in a disposable copy, including
local Cargo patches; it never changes the candidate. Apply authorized updates
deliberately, inspect manifests/lockfiles and qualify the combined result.

**Gate — major updates/advisories:** Present major-version drift or audit findings
and resolve whether to fix or defer. Already-authorized fixes may proceed.
Record accepted deferrals with exact current/latest versions, rationale,
decision reference and expiry. They cannot waive a fixable vulnerability or
compatible resolver drift. A failed or unavailable audit is not a passing result.

## 3. Verify the candidate

Run the repository Rust gates and build release artifacts:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo build --release
python3 scripts/check-docs.py
python3 scripts/check-decision-index.py
```

Before pushing, run all additional checks required by the current CI workflows
and root push procedure, including the server modularity guard and the Docker
build and Helm backup checks for every outgoing branch. Run applicable UI checks from repository instructions.
Exercise changed features through the real binary with disposable data. Record
credential-gated skips separately. Resolve failures before proceeding; when
fixes are already authorized, make them and rerun the affected gates.

## 4. Version and change records

Review commits since the previous release alongside actual API, storage and query
compatibility. Commit prefixes are hints, not the sole basis for versioning.
Documentation-only work may not need a tagged release, but it still requires a
product version bump before pushing under the standing push policy.

**Gate — version:** Follow the standing push policy for routine version increments
and honor a version already chosen by the user. Present the chosen version and
reasons. Ask only when a material release/version decision remains unresolved;
do not request approval again for the required routine patch bump.

Update the workspace version and relevant internal dependency requirements,
then refresh Cargo.lock with a build. Assign selected
[change records](../../../docs/changelog/README.md) to `vX.Y.Z` and add a dated
release summary linking them. Preserve frozen measurement conclusions and keep
the root CHANGELOG as a navigation pointer. Regenerate the index with
`python3 scripts/check-docs.py --write-changelog-index`.

Read back resolved versions and inspect the exact manifest, lockfile and changelog
diff. Validate the final candidate after version edits using Stage 3 and relevant
behavior checks. Stage explicit paths; `git commit -am` misses new records and can
include unrelated work.

## 5. Commit, integrate and tag

Complete authorized local preparation first. When committing is authorized, use
a release commit naming the version. Show the candidate, included records, test
evidence and intended refs before the publication decision.

**Gate — remote writes:** Obtain missing authorization before the first branch
push, PR merge or tag push. Follow actual PR/protection requirements; use the documented
develop-to-main promotion flow only for an authorized release. Prepare multiline PR text in a file for
`gh ... --body-file`, or use a structured tool argument.

Once integrated, verify the final commit belongs to the intended release branch
and its manifests match the approved version. Revalidate if integration changed
the candidate. Check tag absence, then create an annotated tag at that exact
commit when tagging is authorized. Verify its resolved commit, branch ancestry
and the tagged manifest. Push only the specific authorized refs.

Record the automatic PR/branch workflow run and commit tested. Use manual
dispatch when verification is needed on a branch without a PR. A local build or push
does not establish green remote CI. Report which binaries, image, GitHub release
or registry packages were actually produced.

**Gate — crates.io:** Only for requested package publication, inspect publishable
members and dependency order, run `cargo publish --dry-run -p <crate>` for each
applicable package, then obtain missing publication approval. Publish from the
verified tagged commit and confirm registry results. A tag is not package publication.

## Recovery

Inspect partial state first. Use a corrective commit or remove only a local tag
created by this attempt when appropriate; do not hard-reset the shared worktree.
If a remote write is uncertain, inspect remote state before retrying. For an
incorrect published tag or package, stop and propose a concrete correction.
Never rewrite remote release history or force-push as automatic recovery.
