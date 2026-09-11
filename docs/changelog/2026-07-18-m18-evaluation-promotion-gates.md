# M18 evaluation promotion gates

- Date: 2026-07-18
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:1124-1161` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **M18 evaluation promotion gates.** Added versioned `PromotionContext`
  snapshots to durable deterministic construction evaluations, strict
  promotion-only EvalSpec validation, and immutable four-job evidence bundles
  linking candidate and baseline original/replay runs. The policy engine uses
  integer comparisons and keeps denominator, exact-replay, comparability,
  recall, restraint, regression, and oracle-separation gates independent; a
  weighted headline cannot hide a failed gate. Tenant Admins can record
  idempotent promote, reject, blocked, and explicit rollback decisions with
  reasons and ABA-safe expected-decision compare-and-set. Immutable evidence
  and decisions are authority; the per-target head is a decision-first,
  restart-reconciled projection. Protected collections, cursor API/CLI,
  auth-disabled fail-closed mutations, generic health, fixed-cardinality
  metrics, tenant-incarnation isolation, and snapshot conflict preflight keep
  the control plane governed. M18 selects a control-plane candidate only: it
  does not deploy, claim HA, or pretend a live backend supplies a portable
  immutable revision. The detailed contract and verification record live in
  `docs/decisions/decision_m18_evaluation_promotion_gates.md`.
  V1 is deliberately closed: exclusions and allowed exclusion reasons are
  empty, `allowed_candidate_differences` is a sorted four-value enum allowlist,
  and oracle separation requires the exact build/tuning/construction/evaluation
  stage sequence with reads forbidden in the first three stages and zero
  overlap counters. Pinned scorer/verifier hashes are documented as
  operator-trusted semantics identities rather than executable verification.
  Added full tenant recovery (`POST /api/admin/promotions/recover`,
  `cognigraph promotion recover`) and a degraded-authority mutation fence;
  authenticated-Admin snapshot preflight cross-checks evidence against
  hot/archive jobs while explicitly remaining an unsigned trusted-root restore
  boundary. Target chains are capped at 10,000
  actionable decisions and full recovery still materializes tenant-wide
  authority. The checked authoring example is
  `fixtures/m18/promotion-evaluation.json`.
  Verification passed the exact workspace Rust gates and freshly rebuilt
  v2.5.0 release-binary lifecycle probes on persistent Native storage and live
  ArangoDB 3.12.9-1. The probes covered immutable replay/conflict behavior,
  attributed promote/reject/blocked/rollback decisions, stale CAS, restart
  persistence, forged or missing derived-head recovery, mutation fencing,
  protected-query denial, and exact cleanup. Observed evidence is recorded in
  the M18 decision.
