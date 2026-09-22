# Develop requires reviewed incoming work and current dependencies

- Date: 2026-09-14
- Status: Active
- Owner decision: Require incoming PR processing, current dependencies and every available CVE fix before develop integration.

## Decision

Treat dependency freshness as a required integration check, alongside advisory
scans and functional acceptance. The [develop gate guide](../operations/develop-gate.md)
owns the sequence and the executable checks. Direct application dependencies
must use their current release channel or an explicitly approved, exact-version,
dated exception. Transitive dependencies must match a fresh compatible
resolution. Exceptions cannot waive fixable vulnerabilities or resolver drift.

Resolve updates in a disposable copy. A gate must never silently change the
candidate it validates. Re-run functional and runtime acceptance after updates;
the existence of a newer version alone does not prove its compatibility.
Unfixed vulnerabilities remain visible and reviewed under the
[container policy](decision_container_vulnerability_gate.md).

The local push hook and GitHub workflow both inspect current develop and all
incoming PR heads, targets, reviews and checks. Account for every head; a changed
head requires review again, and an included PR with requested changes outstanding
cannot qualify. Recheck after verification and immediately before merge.
The manifest-bound initial-publication/history-removal procedures retain their
own remote/PR checks because their approved ancestry intentionally differs;
they still run dependency, advisory and acceptance checks.

These checks do not automatically merge, publish or authorize deferrals. Main
promotion and hosted QA remain separate decisions. CI is a qualification-time
observation; an external release or PR arriving afterward requires a new check.

## Consequences

The first live check correctly blocks the current candidate on five direct
Cargo updates, six direct UI updates and transitive resolution drift.
[CG-82](../issues/CG-82.md) records gate implementation;
[CG-83](../issues/CG-83.md) owns the dependency refresh required before publication.
The earlier CG-80 acceptance remains a dated result, not evidence that this
stricter gate has passed. No freshness exceptions have been granted.

This extends the [integration branch decision](decision_develop_integration.md)
and [continuous verification decision](decision_ci_verification.md). Existing
bounded patch decisions, including [CG-70](../issues/CG-70.md), must be reviewed
when a new upstream release becomes available; they do not silently exempt a
dependency from the freshness check.
