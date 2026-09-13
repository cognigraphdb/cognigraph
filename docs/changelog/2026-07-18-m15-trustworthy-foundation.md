# M15 trustworthy foundation

- Date: 2026-07-18
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:1214-1234` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **M15 trustworthy foundation.** Closed the authorization boundary around
  opaque backend-native queries (Admin-only), gated every typed Lua mutation,
  and hardened system-collection checks across direct/batch edge payloads,
  returned results, and runtime CGQL traversal. Construction now stores
  independent evidence occurrences and atomically replaces supplied native
  chunks while rejecting sanitized-key collisions, legacy ambiguous chunks,
  and non-atomic backends. Backend parity now covers unbounded scans and
  confidence-weighted traversal; weak cache matches rerun fresh retrieval,
  strong results preserve response shape, deleted cache ids disappear, managed
  cross-collection dependencies invalidate safely, and the cache index is
  deadlock/expiry-safe. Database readiness now returns 503 on probe failure and
  actively probes persistent native storage; cache statistics resolve the
  active tenant. Live Arango verification found and fixed reserved auth-control
  collection creation/drop (`isSystem`) and passed the shared contract plus an
  auth-enabled HTTP matrix. It also restored first-write collection
  materialization for Arango document creates, edge creates, and relationship
  upserts, now enforced by the shared contract. Verification included
  1,205-row cursor pagination and fail-before-write batch/construction
  capability checks. Migration constraints and verification evidence are
  recorded in `docs/decisions/decision_m15_foundation.md`.
