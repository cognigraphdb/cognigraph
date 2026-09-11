# CG-19 traversal confidence parity — 2026-09-09

CG-5 and CG-13 were committed locally as `36a4a19` before this work. This batch
fixes the typed traversal confidence contract on Arango. Native's existing
resident and paged semantics serve as the compatibility baseline.

## Behavior

Every edge must meet `min_confidence`, including edges before `min_depth`.
Missing, null, and nonnumeric confidence count as 1.0. Numeric zero and negative
values retain their values. Depth zero has no edge to reject and keeps score
1.0. The threshold is inclusive and applies to each edge, independently of the
path's cumulative confidence and decay. See the
[shared contract decision](../decisions/decision_traversal_confidence.md).

Arango now prunes a path as soon as an edge fails, filters that failing endpoint,
and guards the null edge at depth zero. Native and both backends' scoring code
already implement the chosen rules. Existing data needs no migration. This can
remove Arango results that crossed a failing intermediate edge and restore
default-confidence/depth-zero results that the old AQL excluded.

## Regression design

The [shared fixture](../../crates/cognigraph-core/src/contract/traversal-confidence.json)
specifies 15 cases with expected complete vertex-key paths and numeric scores.
The Rust helper checks path sets, duplicate paths, depths, edge counts, and
scores against those explicit expectations. Existing Native resident and paged
conformance tests and the Arango suite execute the helper.

Cases include low intermediate/final edges, a failing hop below the minimum
depth, absent/null/string/boolean/array/object confidence, inclusive thresholds,
numeric zero and negative values, no threshold, thresholds above the default,
depth zero alone and within a larger depth range, outbound/inbound/any traversal,
and valid paths whose product or decayed score is below the edge threshold.

The [release harness](evidence/traversal-confidence-http.py) seeds only synthetic
records through authenticated public HTTP routes, then executes the same
fixture through `/api/graph/traverse` and Lua `graph.traverse`. It repeats every
case after restarting the CogniGraph server over the same store. Each run covers
15 cases × 2 surfaces × 2 server lifetimes × 4 backend configurations = 240
checks. Configurations are Native resident/embedded, resident/sidecar,
paged/sidecar, and Arango. The Arango database process itself is not restarted.

## Live baseline and environment

The saved release binary from `36a4a19` reproduced 24 Arango HTTP/Lua mismatches
before restart and the same 24 afterward. The three Native configurations
passed all 180 checks. The old Arango implementation also failed the new Rust
contract on its first case, admitting `a/low/low_end` and losing depth zero and
several default-confidence paths. These are intentional pre-fix failures:

- [Pre-fix live contract failure](evidence/traversal-confidence-baseline-contract-2026-09-09.txt)
- [Pre-fix release observations and binary hash](evidence/traversal-confidence-baseline-http-2026-09-09.json)

Live Arango used a disposable, loopback-only Docker container with synthetic
credentials and separate contract, baseline HTTP, and corrected HTTP databases.
The unpinned `arangodb:latest` image resolved to ArangoDB 3.12.11 on Linux ARM64,
with repository digest
`sha256:39bbca489179ea03f2b24b7ea4e4c4cb5258f6474f8c1c4d9bd65f7cd6d211a5`.
Conformance uses vector fallback mode; indexed vector filtering remains CG-20.
The CG-19 harness calls no model APIs and edits no environment file. The full
workspace test command also runs the existing OpenAI and Gemini live embedding
smoke tests, which load configured credentials from the local `.env`. Both
passed over the existing State of the Union fixture. This is existing provider
validation; the broader model comparison remains deferred.

## Reproduction

Run a disposable Arango instance, create separate empty databases for the
contract and HTTP harness, and set `ARANGO_URL`, `ARANGO_USER`, and
`ARANGO_PASSWORD` to that instance. The harness refuses a database that already
has user collections. Use the same live instance for the full Rust suite:

```bash
export ARANGO_DB=cg19_contract
export COGNIGRAPH_VECTOR_SEARCH_MODE=fallback
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo build --release -p cognigraph-server
python3 docs/issues/evidence/traversal-confidence-http.py \
  --arango-url "$ARANGO_URL" --arango-db cg19_http \
  --output /tmp/cg19-http.json
```

For pre-fix reproduction, use a separate empty database and pass
`--binary /path/to/saved-pre-cg19-server --expect-vulnerable`. The harness
terminates its own CogniGraph processes; the caller removes the disposable
Arango container and its volumes after the tests. No user data is required.

## Final verification

- Focused live Arango integration suite: PASS, six tests executed against the
  disposable database, including the complete shared backend contract.
- Focused Core/Native/Server suites: PASS, 376 tests.
- `cargo fmt --all -- --check`: PASS.
- `cargo clippy --all-targets -- -D warnings`: PASS.
- `cargo test --all`: PASS, 923 passed, zero failed or ignored across 69
  summaries. Live Arango credentials were configured for this run, so the
  six integration tests executed against Arango rather than returning early.
  The two existing live embedding smoke tests also passed as described above.
- `cargo build --release -p cognigraph-server`: PASS.
- Release HTTP/Lua regression: PASS, all 240 checks and four server restarts.
  All path sets, depths, edge counts, and scores match the explicit fixture.
- The disposable Arango container and its anonymous volumes were removed.

No corrected Rust gate or runtime check failed. The recorded baseline failures
were deliberate reproductions before the production fix. Initial discovery
commands included stale file paths/globs; those were corrected by file listing
and did not affect validation. No test harness repair was needed.

The corrected release SHA-256 is
`b8c44a660874a718608e6ee11ef0fcad646a0562517eb4167fa5ca910f3eb964`.
CG-19 was committed locally with CG-20 as `fb49c78`; nothing was
pushed. The registry has 22 resolved and 11 open issues. Next is CG-20,
indexed vector search model filtering before candidate truncation.

- [Corrected release paths, scores, and restart evidence](evidence/traversal-confidence-http-2026-09-09.json)
- [Validation totals, source hashes, image identity, and runtime summary](evidence/traversal-confidence-validation-2026-09-09.json)
