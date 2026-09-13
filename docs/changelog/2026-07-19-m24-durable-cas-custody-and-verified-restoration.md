# M24 durable CAS custody and verified restoration

- Date: 2026-07-19
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:936-951` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **M24 durable CAS custody and verified restoration.** Added a deterministic,
  tenant-incarnation-bound recovery plan for immutable M21-M23 evidence. The
  live Admin endpoint validates the candidate and baseline M20 artifact sets,
  deduplicates their exact digest/length inventory under closed count and byte
  limits, and never dereferences signed locations. Shared offline tooling
  creates and rereads a closed content-addressed bundle, verifies an
  independently pinned plan digest and scope, and restores only a completely
  absent CAS scope through commit-last no-replace publication followed by a
  full reread and external restore receipt. The online server CAS remains
  read-only; every M18-M23 promotion wire remains unchanged. Receipts are
  explicitly deterministic unkeyed operation observations, not proof of backup
  freshness, continuing custody, availability, replication, encryption, or
  HA. The exact Rust gates and authenticated release-binary persistent-Native
  and live-ArangoDB restore lifecycles passed; timings, digests, restart proof,
  and exact cleanup evidence are recorded in the M24 decision.
