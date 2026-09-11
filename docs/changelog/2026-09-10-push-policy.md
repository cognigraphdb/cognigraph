# Incoming PR, local CI and version gates before every push

- Date: 2026-09-10
- Status: Unreleased
- Kind: Workflow

## Change

Record the user's standing [push policy](../../AGENTS.md#push-workflow): inspect
and process incoming PRs, verify integrated behavior, and pass all applicable CI
checks locally before pushing. Re-check incoming work immediately before the
remote write. Missing required evidence blocks push readiness.

Bump the product workspace version for every outgoing change set, including
documentation changes. Keep Cargo.lock and change records consistent; routine
compatible fixes/docs use a patch increment. A candidate's branch and tag refs
share one version, and an unchanged retry retains it. A later outgoing change set
requires a new version even when the previous push did not create a release tag.

Align check, incoming-review and release skills with the shared policy. Current
CI includes formatting, the server modularity guard, both documentation checks,
strict Clippy, workspace tests and a Docker image build for `main`. Future pushes
must re-read CI so this gate follows changes to the actual workflow.

## Validation

Documentation/changelog links, the decision index, skill/frontmatter validation
and whitespace checks passed. All seven commands currently executed by CI were
checked against the documented local push gate, including the conditional Docker
build. This verifies policy coverage, not execution of those CI commands.

This records policy only: no PRs have been processed, no version has been bumped
and no commit or push was performed by this instruction change.
