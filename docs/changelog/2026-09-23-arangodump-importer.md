# v2.7.31 — Offline ArangoDB dump importer

- Date: 2026-09-23
- Status: v2.7.31
- Kind: CLI, Native storage and migration

## Changes

`cognigraph import --from-arangodump DIR --output STORE [--dry-run] [--report FILE]`
turns an ArangoDB 3.11 or 3.12 `arangodump` directory into a new Native store,
offline and in both editions ([CG-66](../issues/CG-66.md),
[contract](../reference/arangodump-import.md),
[decision record](../decisions/decision_arangodump_importer.md)).

- Streams plain, gzip and split data files within the record, expansion and
  file-count limits; refuses VPack, encrypted and multi-database dumps,
  incompatible or reserved collection names, duplicate keys, identity
  mismatches, malformed edges and dangling edges with contract error codes.
- Carries unique persistent/hash/skiplist indexes of document collections as
  enforced constraints under their ArangoDB names and reports everything else.
- Stages in a paged store beside the destination (the dry run in the system
  temporary directory), re-verifies the source, and publishes by hard link to
  a path that must not exist. Exit codes 0 accepted/published, 2 usage,
  3 rejected, 4 failed or source changed, 5 published but report not written.
  The report (`cognigraph-arangodump-report-v1`) is printed and optionally
  written; it never carries document bodies.
- Native gains `NativeBackend::bulk_insert`, a verbatim, all-or-nothing
  multi-document insert for offline tools; the ordinary write path's
  timestamps and edge defaults are not applied to imported documents.
- The reference reader checks unique constraints inline, so within a
  collection the first problem in file order is reported by both
  implementations.
- The CLI guide, README, AQL migration guide and contract describe the
  command. The CLI crate adds `cognigraph-core`, `cognigraph-native`, `tokio`
  and `flate2`.

The workspace version moves to 2.7.31.

## Validation

All 25 contract fixtures reach their recorded outcome, and every accepted
fixture publishes exactly the expected documents and constraints. Tests cover
existing and unsafe destinations, the dry run, injected failures before
publication, a changed source, a failed report after publication and the
refused retry, a 5,000-document split gzip dump with multi-page edge
resolution, file-order precedence across chunks, the limits, and a SIGKILL
during a 400,000-document import. Three deliberate regressions were each
caught by the intended test. Native bulk insert is tested in all four storage
modes. A live harness reads imported stores through a real server in both
editions: 744 checks each over two source versions, three persistent storage
modes and a restart. A private acceptance on a real ArangoDB backup (29
collections, 2,773 documents) matched the reference reader and read back
every document exactly; its record is kept in the private evidence
repository.
