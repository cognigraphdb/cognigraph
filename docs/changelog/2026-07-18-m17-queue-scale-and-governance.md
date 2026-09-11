# M17 queue scale and governance

- Date: 2026-07-18
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:1172-1194` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **M17 queue scale and governance.** Replaced the default full-history job
  list with tenant/filter-bound cursor pagination over a repairable projected
  catalog, while retaining capped hot-only offset pagination as an explicit
  compatibility path. Added global and per-tenant active-job limits, tenant
  `max_active_jobs` quotas, HTTP 429 backpressure with `Retry-After`, and a
  central tenant-round-robin dispatcher that yields long ingests at durable
  checkpoints without reordering one tenant's queue. Quota updates and final
  admission now share one barrier; suspended jobs remain globally counted; and
  worker panic cleanup cannot strand dispatcher occupancy. Terminal retention now
  moves records copy-first into a protected immutable archive; there is no
  purge operation. Bounded dry-run/apply archival and three-phase catalog
  reconciliation, operator status, health degradation, fixed-cardinality
  metrics, API/CLI controls, tenant-retirement cleanup, canonical catalog-key
  validation, v2 resumable cursors, and fail-closed archive-lineage checks cover
  recovery and partial-failure states. The singleton-writer/no-HA boundary
  remains. Verification passed the full Rust gates and release-binary native
  and ArangoDB lifecycle probes: bounded v2 listing, backpressure/replay,
  archive/reconciliation, restart durability, CLI/health visibility, and
  Arango's fail-closed atomic-ingest boundary. Exact observations and cleanup
  are recorded in `docs/decisions/decision_m17_queue_scale_governance.md`.
  Evaluation Promotion Gates were delivered separately in M18; M17 itself does
  not promote quality candidates merely because an evaluation job succeeded.
