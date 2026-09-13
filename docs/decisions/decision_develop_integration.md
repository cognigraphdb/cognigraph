# Develop integration and Renovate dependency updates

- Date: 2026-09-13
- Status: Active
- Issue: [CG-73](../issues/CG-73.md)

## Decision

`develop` is the default integration branch. Feature and maintenance PRs target
`develop`; `main` remains the release branch consumed by the production database.
Promotion from develop to main requires a separately authorized, verified PR.
The permanent branch set is main and develop; topic branches are temporary and
are removed after their work is included or explicitly superseded. A dependency
bot necessarily creates temporary branches while its PRs are open.

Replace Dependabot version and automatic security-update PRs with the hosted
Renovate GitHub App, scoped to `cognigraphdb/cognigraph`. Keep GitHub vulnerability
alerts and secret protections enabled. Renovate reads the configuration on the
default branch and has exactly one base branch: develop. Weekly updates cover
Cargo, UI Bun packages and pinned GitHub Actions. React and CodeMirror families
are grouped to reduce incompatible partial updates. Vendored source is excluded.
There is no dependency automerge. Security updates retain Renovate's advisory
handling; they also target develop and require review and CI.

Both permanent branches require PRs, an up-to-date passing `CI required` check,
resolved review conversations, and protection against force-push and deletion.
CI runs on PRs and pushes for both branches. A workflow regression rejects
Renovate or Dependabot PRs aimed at main. Docker publication remains an explicit
main-only dispatch; a develop push does not authorize deployment.

## Transition and preservation

The initial develop candidate includes all six incoming Dependabot heads plus
compatibility repairs, the pending phone-login fix and deployment evidence.
Qualify it before the initial branch push; activate develop protections after
creation. Change the default branch only after the new branch exists. Retarget
and close included dependency PRs, then delete reviewed topic refs. Preserve
historical local-only work in a private recovery bundle outside this public
repository before deleting its obsolete branch pointers. Never merge historical
private ancestry into the fresh repository or publish with `--all` or `--mirror`.

The [CI guide](../operations/ci.md) owns configuration and verification. The
[push guide](../operations/push.md) owns incoming review, per-candidate versioning,
local checks and safe cleanup. [CG-73](../issues/CG-73.md) records executed results;
this decision does not itself assert that remote setup has completed.

## Sources

Renovate documents [installation and default-branch configuration](https://docs.renovatebot.com/getting-started/installing-onboarding/),
[baseBranchPatterns](https://docs.renovatebot.com/configuration-options/#basebranchpatterns)
and [Bun lockfile support through the npm manager](https://docs.renovatebot.com/modules/manager/npm/).
