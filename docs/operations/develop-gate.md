# Qualification before develop integration

The owner requires incoming work, current dependencies and all available
vulnerability fixes to be handled before a candidate enters develop.
This extends the [push workflow](push.md), the [branch decision](../decisions/decision_develop_integration.md)
and the [container vulnerability gate](../decisions/decision_container_vulnerability_gate.md).

## Prepare and review

1. Fetch develop and inspect **all** open PRs: exact head, target, draft/review
   state and checks. Integrate reviewed work or record an owner-authorized
   `superseded`, `deferred` or `needs-changes` disposition with the exact head,
   reason and decision in [pr-dispositions.json](pr-dispositions.json).
   An included PR with requested changes still outstanding blocks qualification.
   Account for dependency bots before doing a manual refresh, avoiding duplicate
   or conflicting updates. Bots target develop; neither bots nor these gates merge.
2. Run the dependency and advisory checks, then apply authorized updates in the
   candidate. Review changed APIs, authentication compatibility, storage formats
   and optional features as appropriate. Run additional live verification when
   those contracts change. Keep required version bumps and change records under
   the normal push policy.
3. Run full shared CI and both-edition Docker qualification against the final
   combined candidate. Every fixable image vulnerability blocks at every severity;
   Cargo and Bun advisory errors also block. Preserve visible unfixed findings
   with their review disposition; do not label a passing gate zero-CVE.
4. Immediately before the authorized push/merge, refresh incoming heads and
   dependency/advisory observations. Changed code, dependencies, PR heads or base
   requires review and affected qualification again. Merge through the protected
   PR flow. Verify the develop post-merge CI and exact remote commit; main promotion,
   package publication and hosted QA remain separately authorized operations.

```sh
python3 scripts/check-incoming.py
python3 scripts/verify.py --suite dependencies
python3 scripts/verify.py --suite advisories
python3 scripts/verify.py --suite ci
python3 scripts/verify.py --suite docker
```

## Meaning of current dependencies

For all workspace members, the freshness checker inspects resolved direct Cargo
dependencies with all features, including optional/dev dependencies and the
vendored Tantivy patch. It compares against crates.io's current stable release.
Libraries with no stable release, such as the already-adopted optional `ort`,
use their latest published release channel; this does not authorize adopting a
prerelease in a previously stable dependency. UI production, development and
optional dependencies are compared against npm's `latest` release tag.

Transitive versions must match a fresh Cargo/Bun resolution **within the current
manifest constraints**. An old transitive major required by an upstream library
cannot simply be replaced by the newest major without changing that library.
The direct-release check exposes old parent versions; advisory checks still
block vulnerable transitive packages. Freshness is not a substitute for reviewing
upstream constraints or intentionally pinned requirements.

Resolution runs in a disposable copy with `cargo update` and
`bun update --lockfile-only --ignore-scripts --no-cache`. Source manifests and
lockfiles stay unchanged, dependency install scripts are not run, and the gate
checks candidate input hashes. Registry, resolver or metadata failures fail the
gate. Cargo's dry-run exit code alone is insufficient: an observed
`cargo update --dry-run --locked` exited zero despite offering 105 changes.
`cargo-outdated` also failed to copy our path patch, so it is not the gate.

Pinned CI tool versions and Action SHAs stay under the CI/tooling review and
Renovate workflow. This application-library checker does not assert that every
build tool on a developer's machine is the latest available release.

## Explicit pin decisions

[dependency-exceptions.json](dependency-exceptions.json) is initially empty.
Only an actual owner-approved compatibility/security decision may add a record:

```json
[{"ecosystem":"cargo","package":"example","current":"1.0.0","latest":"2.0.0",
  "reason":"A reviewed migration is pending","decision":"docs/issues/CG-N.md",
  "expires":"2026-10-01"}]
```

Use a real existing engineering record, not the illustrative path above. Records
match exact versions and expire by UTC date. New releases, expired decisions,
duplicates, missing rationale and unused exceptions require review. Exceptions
cannot suppress advisory findings or waive a fresh compatible resolution. Do
not add a record merely because CI is failing.

## Enforcement and limits

The local pre-push hook and CI share exact-head PR disposition logic. Source CI
checks incoming work before verification and compares its candidate/develop/PR
snapshot afterward. The Docker job checks incoming work again after both images
qualify. It needs full Git history and read-only GitHub PR/check/status access.
Review and check metadata is captured; ancestor inclusion alone is not proof
that a person reviewed every line. Current-candidate CI may supersede failures
on an earlier included PR; its acceptance still needs review.

Reports live under `target/ci/incoming.json` and `target/ci/dependencies/run-*/`;
CI retains them with acceptance diagnostics. The protected `CI required` check
depends on both jobs, so these checks apply to GitHub merges as well as local
pushes once the workflow is published. A local hook alone cannot enforce a UI merge.

These are observations at qualification time. A new PR or registry release can
arrive after CI finishes; GitHub does not rerun a green check merely because an
external package was published. The final pre-merge recheck is mandatory. These
scripts neither supply merge authorization nor form a security boundary against
someone intentionally changing repository rules or verification code.

[CG-82](../issues/CG-82.md) records implementation and activation status.
