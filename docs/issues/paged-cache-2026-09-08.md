# Paged-cache remediation — 2026-09-08

This batch addresses [CG-10](CG-10.md) and [CG-14](CG-14.md). Changes remain
local and uncommitted; unrelated shared work was preserved.

## Behavior

- Paged update, replacement, and deletion keep the state write lock through
  durable commit, document-cache changes, vector-delta publication, and triple
  index invalidation. Earlier operations cannot publish after later writes.
- Paged point reads hold the state read lock through catalog/key validation,
  redb fetch, and cache insertion. A write or drop waits for an older fill,
  then updates or purges it before completing.
- Collection drop removes matching LRU entries, recency records, and byte
  accounting, plus the loaded vector sidecar, under the same write lock.
  Other collections retain their data and cache entries.
- Sidecar snapshot/build/publication holds a state read lock. Cold builds
  serialize and recheck the loaded sidecar after waiting. This also closes a
  related race found during remediation: an old build could miss a write,
  replace the current sidecar, and later acquire a current generation stamp
  from an unrelated key's delta while still missing the earlier update.

No public API, storage schema, cache-size setting, or dependency changed.

## Deterministic regression evidence

Test-only channels pause an operation at the actual publication boundary.
On the unfixed code, the competing writer completes before the paused
operation resumes; with the fix, it waits for the state lock. Assertions
compare cached documents with committed redb records, search results with
current embeddings, and exports before/after reopening the database.

Six regressions failed before the fix: update/update returned version 1 after
version 2 committed; replace/delete and delayed fills resurrected deleted
rows; delete/create hid the new vector; warm collection drop exposed the old
document; and a pending fill repopulated a dropped/recreated collection. A
separate sidecar-build probe with 40 distractor vectors confirmed that a lost
update could exclude the correct document from the candidate set.

Nine new tests now pass, covering those schedules, read/update and read/delete,
concurrent cold searches sharing one build, reopen/export consistency, and
LRU byte/recency accounting after collection purge. The pause hooks are absent
from production builds.

## Validation

- `cargo test -p cognigraph-native`: passed, 46 tests across 10 summaries.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed after moving a new test
  module below the implementation, as required by Clippy.
- `cargo test --all`: passed, 857 tests reported passed, 0 failed, 0 ignored
  across 65 result summaries.
- `cargo build --release -p cognigraph-server`: passed.

Live checks used the release server with synthetic authentication, disposable
databases, and loopback HTTP. No external provider or production data was used.
The [sanitized observations](evidence/paged-cache-http-2026-09-08.json) cover:

| Check, per profile | Paged, 1 MiB cache | Paged, zero cache | Resident sidecar |
|---|---|---|---|
| Concurrent updates/replacements | 120 successful | 120 successful | 120 successful |
| Concurrent point reads | 200 successful | 200 successful | 200 successful |
| Concurrent cold vector searches | 8 successful | 8 successful | 8 successful |
| Point read after writers finish | Equals durable export | Equals durable export | Equals durable export |
| Vector after writers finish | Matches committed embedding | Matches committed embedding | Matches committed embedding |
| GET after drop and empty recreation | 404 both times | 404 both times | 404 both times |
| New row with the same key | Visible and searchable | Visible and searchable | Visible and searchable |
| Other collection and graceful restart | Preserved | Preserved | Preserved |

Live document deletion also removed the old vector; recreation and restart
preserved the replacement row and its embedding. All HTTP checks passed on
the first run. Exact race schedules are proven by Rust tests; HTTP concurrency
is a workload regression, not proof that every scheduler interleaving occurred.

## Scope and tradeoff

The lock boundary is local to one backend instance. Tenant/database
incarnation binding remains [CG-11](CG-11.md) and [CG-12](CG-12.md). This does
not add multi-operation snapshot isolation to search or traversal.

Cold point reads and sidecar builds can hold up writers, and cold sidecar
builders serialize. Concurrency throughput was not benchmarked. The existing
cache-size and sidecar delta/rebuild policies remain unchanged. External
ArangoDB integration was not exercised live; environment-gated test counts do
not establish external-service coverage.

The next priorities are tenant derivative isolation (CG-11), followed by
request-to-tenant-incarnation pinning (CG-12).
