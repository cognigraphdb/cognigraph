# v2.7.52 — One Idempotency-Key namespace for signed authority

- Date: 2026-11-11
- Status: v2.7.52
- Kind: Data-integrity fix (Enterprise governance)

## Changes

[CG-112](../issues/CG-112.md) is resolved under the
[authority idempotency namespace decision](../decisions/decision_authority_idempotency_namespace.md).

A tenant's signed records must each have their own `Idempotency-Key`.
Stored-authority validation enforced that across seven record kinds, but
admission of governance keys, key revocations, policy revisions, policy
approvals and artifact attestations did not look at Semantic Repair
revisions and reviews. A key reused across those kinds was accepted, and
the stored record then failed every reconcile and recovery. Promotion,
rollback, governed ingest and repair deployment stayed blocked for the
tenant, with no route to remove the record.

- All seven admission paths now check the same namespace that validation
  checks. A spent key returns 409 and stores nothing.
- The namespace is defined once and used by both sides.
- Exact replays are unchanged.

**Behaviour change:** a client that reused one key for a Semantic Repair
record and a governance record now gets 409 on the second request.

## Validation

`cargo fmt --all -- --check`, both Clippy gates with `-D warnings`,
`cargo test --locked --all` (848 passed, 2 ignored) and
`cargo test --locked --all --features enterprise` (1,077 passed, 2 ignored)
pass. The new regression test covers all five governance record kinds
against both Semantic Repair kinds, reconcile, recovery, snapshot import
and replay, and fails without the fix. The registry, docs and
decision-index checks pass.

Not run: the HTTP routes against a release binary (requests need externally
signed statements), and the UI, Native, Helm, Docker, advisory and
dependency suites, which run on the finished P1 batch. Nothing was pushed,
released or deployed.
