# v2.7.54 — Snapshot import refuses unsigned promotion records

- Date: 2026-11-11
- Status: v2.7.54
- Kind: Security fix (Enterprise governance)

## Changes

[CG-113](../issues/CG-113.md) is resolved under the
[legacy promotion import decision](../decisions/decision_legacy_promotion_import.md).

Since M19, promotion authority is signed by three separate principals.
Snapshot import still accepted unsigned M18 evidence and decisions from any
tenant Admin: a hand-written snapshot with four evaluation jobs, one evidence
bundle and one `promote` decision passed every check and produced a head.
The channel was then bound to the unsigned generation.

- `POST /api/admin/import` refuses unsigned promotion evidence and decisions
  that the store does not already hold, with 403, before writing anything.
- `COGNIGRAPH_ALLOW_LEGACY_PROMOTION_IMPORT=true` allows it for a disaster
  restore of M18 history. It is off by default and warns at start.
- A head selected by an unsigned decision carries `unsigned_legacy: true`.
  Signed heads are unchanged.

**Behaviour change:** restoring a snapshot that contains M18 promotion
history into a fresh instance now needs the switch.

## Validation

`cargo fmt --all -- --check`, both Clippy gates with `-D warnings`,
`cargo test --locked --all` (868 passed, 2 ignored) and
`cargo test --locked --all --features enterprise` (1,098 passed, 2 ignored)
pass. These runs also cover the final state of v2.7.53. The new regression
test imports an unsigned chain into a fresh store and checks that nothing
was written. The Enterprise release binary was started with the switch off
and on; it logs the warning only when on. The registry, docs, decision-index
and server-modularity checks pass.

Not run: a snapshot import over HTTP (an unsigned chain cannot be produced
through the API), and the UI, Native, Helm, Docker, advisory and dependency
suites, which run on the finished P1 batch. Nothing was pushed, released or
deployed.
