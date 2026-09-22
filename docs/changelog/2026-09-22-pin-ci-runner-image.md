# v2.7.20 — Pin CI jobs to the ubuntu-24.04 runner image

- Date: 2026-09-22
- Status: v2.7.20
- Kind: CI and workflow policy

## Changes

GitHub announced that the `ubuntu-latest` label migrates to Ubuntu 26.04
between 2026-10-19 and 2026-11-19 (actions/runner-images issue 14748), and
every run on the alias now carries a migration warning. The three jobs in
`.github/workflows/ci.yml` move from `ubuntu-latest` to the explicit
`ubuntu-24.04` label, which is the image the alias resolves to today and the
image every qualified run has used.

Pinning removes the warning and keeps the runner reproducible across the
migration window. It also means the move to 26.04 becomes a reviewed change
instead of alias drift; [CG-93](../issues/CG-93.md) tracks that migration
with the same gates. Renovate's `github-actions` manager updates pinned
action SHAs but does not change runner labels, so the ticket is the tracking
mechanism.

`docs/operations/ui-testing.md` names the pinned image. No job steps, tool
versions or gates change. The workspace version moves to 2.7.20 because
every push carries a version and a change record.

## Validation

`actionlint`, the script regressions covering workflow policy, the docs and
issue-registry checks pass locally; the pre-push hook and the pull-request CI
on the pinned image confirm the workflow runs unchanged. Not a release.
