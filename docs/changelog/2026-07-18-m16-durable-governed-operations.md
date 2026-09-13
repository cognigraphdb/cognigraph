# M16 durable governed operations

- Date: 2026-07-18
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:1195-1213` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **M16 durable governed operations.** Added persistent tenant-scoped jobs for
  `construct.ingest` and `construct.evaluate`: required idempotency keys,
  immutable submitted and frozen execution input, atomic embedded state/audit
  transitions in protected `_cognigraph_jobs`, one in-process worker,
  at-least-once restart recovery from construction batch checkpoints,
  cooperative cancel and resume/restart retry, attributed audit history,
  tenant-scoped API/CLI controls, `/health/jobs`, and fixed-label aggregate
  metrics. Tenant incarnations, suspension/deletion drain, snapshot-import
  fencing, and explicit background tenant scopes prevent stale or cross-tenant
  execution. List responses use bounded projected summaries. A release-native
  5,000-chunk job recovered after `SIGKILL` at checkpoint 184 and produced
  exactly 5,000 unique restart-probe occurrences; `SIGTERM` checkpointed and
  requeued active work. Live Arango testing exposed its built-in `_jobs`
  collection, which is why CogniGraph uses the qualified name; projected list
  reads and durable evaluation survived reconnect, while ingest recorded an
  atomic-capability failure before graph writes. M16 keeps the singleton-writer
  boundary and does not claim distributed scheduling or HA. Full evidence is in
  `docs/decisions/decision_m16_durable_governed_operations.md`.
