# Issue allocation guard, CI check, and cognigraphdb organization URLs

- Date: 2026-09-10
- Status: Unreleased
- Kind: Workflow

## Change

Add `scripts/issue.py`. `new "Title" --priority --area` allocates the next
`CG-N` from the working tree, index, HEAD and the registry counter, creates
the file with `O_EXCL` so an existing record is never overwritten, appends
the registry row and bumps `Next available identifier` in one step. `check`
fails when a committed issue's title changed (an identity was reused), when a
file and its registry row disagree on status or title, when a row or file is
missing, when identifiers are not contiguous, or when the counter is stale.
`check` joins the CI suite in `scripts/verify.py` between the decision-index
check and Clippy; `scripts/tests/test_issue_registry.py` covers the guard in
disposable repositories. [AGENTS.md](../../AGENTS.md#documentation-and-evidence)
now names the script as the only way to create an issue.

Motivation: a hand-written `CG-42.md` overwrote a record committed minutes
earlier by another session. The identity was restored from HEAD and the new
record became CG-45.

Record the product organization: `repository`/`homepage` in the workspace
manifest (inherited by every crate), the Helm chart's `home`/`sources`, the
README, `LICENSING.md`, and both documentation indexes now point at
`https://github.com/cognigraphdb/cognigraph` and `https://cognigraphdb.com`.
The user confirmed `cognigraphdb/cognigraph` as the code repository.

## Validation

The final guard reports 47 issues, 47 rows and no errors. Eleven issue-registry
regressions pass, including committed reuse/deletion and concurrent allocation;
the complete Python workflow suite passes 26 tests on Python 3.14. The earlier
Python 3.10 import failure does not recur with the supported local runtime.
[CG-47](../issues/CG-47.md) records the pre-publication corrections: compare
original committed titles, require full history, validate links/priorities and
serialize allocation. CI now uses a full-history checkout. Documentation and
manifest checks pass.
