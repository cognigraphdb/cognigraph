# Continuous verification before publication

Date: 2026-09-12

## Context

Native is now the only runtime backend. Manual CI and locally optional runtime
checks leave a gap between a passing source suite and a usable packaged product.
The website is a separate repository and previously had no CI workflow. The owner
has paused deployment while authorizing CI branches and pull requests.

## Decision

The owner authorizes shared local and GitHub verification for both repositories.
PRs targeting main and main pushes run automatically; manual dispatch remains
available. The code gate includes locked Rust validation in both editions, UI
checks and browser regression, Native release acceptance, Helm rendering and
packaged backup/restore, workflow lint and dependency advisory scans. The website
gate includes frozen Bun checks and a hardened packaged-server smoke test.

The public code repository requires an up-to-date successful aggregate check
and a PR before main changes. Every prerequisite must succeed; skipped or
cancelled jobs cannot qualify a candidate. Full-SHA Action references, read-only
workflow credentials and secret push protection limit repository exposure.
Dependency updates arrive as reviewable PRs. Informational advisories remain
visible and have explicit issue follow-up rather than silent suppression.

Verification and publication retain separate authority. Docker credentials and
image publication remain restricted to an explicit manual run on official main.
CI never deploys either application. The website's private-repository plan does
not currently permit branch rulesets; changing visibility or billing is outside
this authorization. Its check results therefore require owner enforcement.

## Outcome

[CG-69](../issues/CG-69.md) records local and GitHub qualification. The
[CI guide](../operations/ci.md) owns commands, tool prerequisites, current settings
and publication boundaries. Repository settings take effect immediately; workflow
and Dependabot changes on the default branch require a separate authorized merge.
The current work publishes candidate PRs only. [CG-70](../issues/CG-70.md) tracks
the outstanding transitive dependency warnings.

## Amendment — develop integration, 2026-09-13

The [develop/Renovate decision](decision_develop_integration.md) extends automatic
verification and branch protection to develop, makes it the default integration
branch, and replaces Dependabot update PRs with Renovate. Main remains the
production release branch. The original CG-69 authorization and settings above
are a historical checkpoint; current setup is recorded in CG-73 and the CI guide.
