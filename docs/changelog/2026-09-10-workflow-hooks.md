# Shared CI runner and checked pre-push workflow

- Date: 2026-09-10
- Status: Unreleased
- Kind: Workflow

## Change

Add a [shared verification runner and tracked Git hook](../operations/push.md).
The gate checks the clean outgoing commit, version/lockfile/change-record agreement,
fresh incoming PR dispositions, branch ancestry and annotated version tags. It
runs CI checks, the main-branch Docker build and affected UI checks, then rejects
changes to the local candidate, remote refs or incoming PRs during verification.
CI now calls the same runner; no duplicate set of shell checks is maintained.

Harden Claude file protection: test lexical and symlink-resolved paths, reject
malformed edit payloads and prevent the `.env.example` exception from bypassing
protected parent directories. Extend the compaction reminder with the push policy.

## Validation

Twelve regression tests passed using disposable Git repositories and synthetic
edit payloads. They cover failed-command short-circuiting, protected paths and
symlinks, dirty trees, missing version bumps, stale lockfiles, missing change
records, unreviewed/changed PR heads, GitHub failure, wrong candidates, deletions,
tag/version mismatch and remote changes during verification.

Installed with `python3 scripts/install-hooks.py`; `core.hooksPath` resolves to
`.githooks`. A real `git push --dry-run origin main` rejected the dirty candidate
with the expected clean-tree error before starting validation or publishing refs.
The clean-candidate rehearsal runs the full shared checks and is reported with
the outgoing checkpoint. The hook does not publish on its own.
