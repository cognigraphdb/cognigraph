# Decision: all signed authority of a tenant shares one Idempotency-Key namespace

Status: accepted and implemented 2026-11-11 for [CG-112](../issues/CG-112.md).

## Context

Seven kinds of signed record make up a tenant's authority: governance keys,
key revocations, policy revisions, policy approvals, artifact attestations,
and Semantic Repair revisions and reviews. Each stores the SHA-256 hash of
the `Idempotency-Key` it was submitted with. Stored-authority validation,
which runs on every reconcile, on recovery and on snapshot preflight,
requires those hashes to be unique across all seven kinds.

Admission did not check the same set. The five governance kinds looked only
at each other, so a key already spent on a Semantic Repair revision or
review was accepted for any of them. The stored record then failed
validation for good: the records are append-only, there is no delete route,
and promote, rollback, governed ingest, repair build and deploy stayed
blocked for the tenant.

## Decision

1. **One namespace, defined once.** `AuthorityRecordKind` in
   `governance/idempotency_namespace.rs` lists the seven kinds with their
   collections. Admission and stored-authority validation both iterate it.
2. **Admission refuses what validation would refuse.** Every one of the
   seven admission paths calls one check,
   `ensure_authority_idempotency_available_locked`, under the transition
   lock and after the exact-replay lookup. A spent key returns 409 and stores
   nothing.
3. **Replays are unchanged.** Resending the identical request with its own
   key still returns the original record.
4. **Rejected: relaxing validation to per-kind uniqueness.** It would repair
   the poisoned state, but the key hash is part of what a signed promotion
   statement binds, and one key naming two different signed records makes
   audit trails ambiguous.
5. **Not in scope:** promotion evidence and decisions, repair generations
   and deployment decisions keep their own key rules.

## Consequences

- A client that reused one key across record kinds now gets 409 on the
  second request instead of a tenant that cannot promote.
- A new signed record kind must be added to `AuthorityRecordKind`; the
  compiler then requires its collection and its hash source.
- A store that already holds a cross-kind reuse is not repaired by this
  change. Validation still refuses it, and clearing it needs an offline
  repair. No such store is known; the product has no deployed tenants with
  Semantic Repair authority.
