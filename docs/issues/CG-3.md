# CG-3: Directed construction bypasses the deployed-generation mutation fence

- Status: Resolved
- Priority: P1
- Area: Construction / M26 authority
- Found: 2026-09-08
- Reviewed revision: `bc4bff1` (workspace 2.6.0)
- Verification: Source-confirmed defect; signed lifecycle and release HTTP remediation regression

## Problem

The directed route passes the trusted managed backend directly to `directed_ingest`. Ordinary and governed ingest instead acquire the promotion transition lock and reject spaces with an M26 deployment. Directed reconciliation can replace chunks, mentions, and facts for a deployed space without a signed deployment, leaving the live projection inconsistent with its immutable generation and deployment receipt.

## Evidence

- `crates/cognigraph-server/src/routes/construct.rs:97-160` — direct trusted-backend writes.
- `crates/cognigraph-server/src/routes/construct.rs:193-210,260-284` — fenced ordinary/governed paths.
- `crates/cognigraph-server/src/materialized_repairs.rs:2575-2620` — tenant health, deployed-space check, and transition lock.
- `docs/decisions/decision_m26_verified_semantic_repair_materialization.md:315-377` — signed atomic deployment and legacy-ingest exclusion.

## Reproduction / failure sequence

Failure sequence: deploy an M26 generation, submit `/api/construct/directed` for the same space and a retained chunk ID, and let its completion return a different accepted fact set. The directed route has no call to the deployed-space guard before the shared occurrence reconciler writes. The full signed deployment lifecycle was not rerun for this specific finding; evidence is the contrasted call paths.

## Acceptance criteria

- [x] Route every occurrence writer, including directed ingestion, through one tenant/incarnation-aware transition boundary.
- [x] Reject directed writes to deployed spaces before creating or changing any state.
- [x] Add an M26 lifecycle test proving directed ingestion cannot alter the deployed projection, including a concurrent deployment attempt.

## Resolution — 2026-09-08

Directed ingestion acquires the promotion transition lock before incarnation
resolution, space creation, or provider execution. Ordinary, governed, and
directed occurrence writers share the tenant-health and deployed-space
admission check. Directed ingestion holds the lock through reconciliation so
a concurrent signed deployment waits for its completion.

The signed M26 lifecycle now verifies that ordering and rejects a subsequent
directed request with no provider calls or snapshot changes. Release-server
HTTP regressions in resident and paged Native modes, including restarts,
returned 409 for a retained deployed chunk, preserved the complete snapshot,
and grounded one fact for an undeployed space. Formatting, Clippy, and all 848
tests passed. See [verification and scope](directed-fence-2026-09-08.md).

Run the repository Rust gates and a focused runtime regression before closing this issue. See [review evidence](review-2026-09-08.md).
