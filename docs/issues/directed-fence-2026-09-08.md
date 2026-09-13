# Directed deployment fence remediation — 2026-09-08

This batch addresses [CG-3](CG-3.md). Changes remain local and uncommitted;
unrelated shared work was preserved.

## Behavior

Directed construction now acquires the promotion transition lock before
resolving the tenant incarnation or creating a space. The shared admission
check rejects paused tenants, degraded promotion authority, and spaces owned
by an active M26 deployment. Ordinary ingestion, durable ingest jobs, and
governed ingestion use that same admission check.

The directed route holds the lock through completion-provider execution and
occurrence reconciliation. Deployment cannot overtake an admitted ingestion;
after deployment, a new directed request fails with HTTP 409 before reaching
the provider or changing stored data. The lock is released before search-cache
invalidation and by normal future cleanup on error or cancellation.

## Verification

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo test --all`: passed, 848 tests reported passed, 0 failed, 0 ignored
  across 65 result summaries. Environment-gated external integration tests may
  return early; these totals do not establish live external-service coverage.
- `cargo build --release -p cognigraph-server`: passed.
- Three new route tests passed: undeployed ingestion succeeds and releases the
  lock; paused and degraded tenants reject before space creation or completion.
- The extended signed M26 lifecycle passed in 96.87 seconds. A held directed
  completion retains the transition lock and blocks an actual signed
  deployment. Once released, ingestion finishes and deployment installs its
  exact projection. A subsequent directed request makes zero provider calls
  and preserves the entire snapshot.

Live verification used the real release server, synthetic authentication,
disposable Native stores, and a counted completion stub over loopback HTTP.
The fixture came from the full signed lifecycle (A/B activation, control-plane
rollback, and deployment rollback to A), exported using
`COGNIGRAPH_M26_LIVE_FIXTURE_DIR` with the targeted lifecycle test. No external
provider or production data was used.

| Check | Resident Native | Paged Native |
|---|---|---|
| Directed ingestion for a retained deployed chunk | HTTP 409 | HTTP 409 |
| Completion calls on that rejected request | 0 | 0 |
| Full exported snapshot after rejection | Unchanged | Unchanged |
| Same rejection after graceful restart | Passed | Passed |
| Directed ingestion into an undeployed space | HTTP 200, one fact | HTTP 200, one fact |

The [sanitized HTTP observations](../evidence/engineering-historical-checks.md#artifact-82a5f0c427abd427f836)
record the responses and invariants. An initial live pass used a new chunk ID;
the final pass strengthened the probe to reuse the exact retained deployed
chunk ID. Both passed. An unused import in the new test module was removed
before the final Clippy gate.

## Scope and tradeoff

This retains the existing singleton transition lock. A slow directed
completion therefore delays other governance transitions until that bounded
request finishes or is cancelled. Concurrent deployment is tested through the
real signed Rust lifecycle; live HTTP checks cover rejection, preservation,
positive ingestion, and persistence, without claiming a concurrent HTTP race.

General request-to-tenant-incarnation pinning remains [CG-12](CG-12.md).
Directed ingestion now drains under the promotion fence before tenant
retirement, but this does not bind every HTTP request to the tenant incarnation
seen by authentication. No live ArangoDB or external model coverage is claimed.

The next bounded batch is paged-cache validity, [CG-10](CG-10.md) and
[CG-14](CG-14.md).
