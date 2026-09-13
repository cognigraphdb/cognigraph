# GitHub branch and Renovate activation — v2.7.9

- Date: 2026-09-13
- Status: v2.7.9
- Kind: Maintenance evidence

## Changes

[CG-73](../issues/CG-73.md) records the verified GitHub transition: develop is the
default branch, both permanent branches require PRs and strict CI, and Renovate
is installed only for the code repository in Interactive mode. Dependabot update
automation is retired while vulnerability alerts remain enabled.

All six incoming dependency heads are included in develop and their PRs closed.
The generic Renovate onboarding PR is superseded. Nine obsolete remote refs and
four obsolete local branches are removed, with private historical work preserved
outside the public repository. Only main/develop and the primary worktree remain
at the activation checkpoint. The temporary evidence PR branch is removed after
merge. Main and the production v2.7.7 database remain unchanged.

This follow-up contains documentation and the required per-push version bump;
it does not change runtime source or dependencies from v2.7.8. The issue record
links the verified v2.7.8 candidate and its scoped local/browser evidence. Normal
local CI/Docker and protected-PR checks also apply to this outgoing candidate.

## Verification

The versioned evidence candidate passes the full shared local CI and Docker
suites, with both Rust editions, Native acceptance, 15 browser regressions and
both packaged-image/Helm backup checks. Protected develop integration requires
the PR's remote aggregate check to pass as well.
