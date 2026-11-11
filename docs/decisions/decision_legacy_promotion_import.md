# Decision: snapshot import does not introduce unsigned promotion authority

Status: accepted and implemented 2026-11-11 for [CG-113](../issues/CG-113.md).
Amends D11 of the
[M19 decision](decision_m19_signed_governance_separation_of_duties.md).

## Context

M19 made promotion authority signed: a policy author, an approver and a
promoter each sign, and Admin authentication alone cannot create evidence or
a promote or reject decision. D11 kept unsigned M18 records readable and
recoverable as history.

Snapshot import treated that history as acceptable input from anyone who may
import. Every check on a v1 record is structural or an unkeyed digest, so a
tenant Admin could write four succeeded evaluation jobs, one v1 evidence
bundle and one v1 `promote` decision into a snapshot and import it. Preflight
passed, reconcile wrote a head, and the channel was then bound to the
unsigned generation, so signed evidence was refused there. A head looked the
same whether a signed or an unsigned decision had selected it.

## Decision

1. **Import refuses unsigned records it would introduce.** During preflight,
   v1 evidence or a v1 decision that the target store does not already hold,
   byte for byte, is refused with 403 before anything is written. A store
   can still take its own snapshot back.
2. **Legacy restore is an operator switch.** With
   `COGNIGRAPH_ALLOW_LEGACY_PROMOTION_IMPORT=true` the server accepts them.
   The switch is process configuration, set by whoever runs the server, not
   something a tenant Admin can set through the API. It is off by default and
   logs a warning at start when on.
3. **Heads say when nothing signed selected them.** A head whose applied
   decision is unsigned carries `unsigned_legacy: true`. Signed heads are
   stored and digested exactly as before. Reconcile rewrites a legacy head
   stored before this change.
4. **Rejected: no switch at all.** D11 promises that M18 history stays
   recoverable; a disaster restore into a fresh instance needs a way in.
5. **Rejected: a recorded cutover time** after which unsigned records are
   invalid. Timestamps in unsigned records are not evidence of when they
   were written.
6. **Rejected: refusing unsigned records already in the store.** They are
   existing history and D11 keeps them.

## Consequences

- With the switch on, the old exposure exists for as long as it is on: an
  Admin can import an unsigned chain. Use it for the restore and remove it.
- A signed rollback that names prior v1 evidence produces a signed decision,
  so its head is not marked, although the evidence it selects is unsigned.
- Unsigned evaluation jobs in a snapshot are still imported; without evidence
  and a decision they select nothing.
- Legacy heads in an existing store gain the marker, and so a new projection
  digest, the first time they are reconciled.
- This was a default chosen while fixing the ticket. If no deployment will
  ever need to restore M18 history, the switch can be removed.
