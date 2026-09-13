# Read replicas and manual standby assigned to Community

- Date: 2026-09-10
- Status: Unreleased
- Kind: Licensing / Product decision

## Decision

Record the owner's decision that read replicas for read scaling and manual
standby with operator-controlled promotion belong in Community. Automatic
failover, sharding, consensus clustering and multiple writers remain Enterprise.
Other Enterprise capabilities retain their classification when replicated.

Align the [licensing decision](../decisions/decision_licensing.md),
[licensing map](../../LICENSING.md), draft Enterprise license definition,
[replica design](../architecture/design-notes/read-replicas.md), implementation
plan and active product summaries. Earlier changelog records retain the pending
decision as historical context.

Replicas remain proposed and unimplemented. This decision changes the tier
boundary without adding runtime behavior or changing the single-writer Helm
deployment.

## Validation

Documentation, decision-index and issue-registry checks cover this amendment;
cross-repository link validation includes the sibling product docs. No Rust,
UI or deployment configuration changed in this amendment.
