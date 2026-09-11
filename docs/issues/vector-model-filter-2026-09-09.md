# CG-20 Arango vector candidate filtering — 2026-09-09

This work follows the completed CG-19 change on top of `36a4a19`. Both batches
were subsequently committed locally as `fb49c78`.
It fixes model filtering and distinct-parent result underfill in Arango's
indexed and fallback vector retrieval. See the
[contract and compatibility decision](../decisions/decision_arango_vector_candidates.md).

## Changes

The indexed AQL now filters `embed.model_name` inside the index lookup, before
candidate truncation. Fallback already filtered before ranking. Both modes
replace the fixed `3 × limit` window with increasing candidate windows, retaining
the highest-scoring chunk for each distinct parent until enough hits are found
or eligible candidates are exhausted. Nonstring/missing parent IDs continue to
use the embedding row's own identity. Zero limit returns immediately and window
growth saturates safely.

Filtered indexed search requires ArangoDB ≥3.12.6; use fallback on older servers.
The tested instance is 3.12.11. No existing data or index needs migration.
Approximate recall and search effort remain index configuration concerns;
duplicate-heavy searches can perform additional queries, and fallback repeats
its scan when expanding the window.

## Fixture and baseline

The [12-case fixture](../../crates/cognigraph-arango/tests/fixtures/vector-model-filter.json)
contains 126 two-dimensional vectors. Eighty higher-scoring model-A rows crowd
the global prefix, while model B has 40 chunks for one parent and four other
eligible identities. Additional rows have missing and empty model names.
Expected row keys and scores are explicit; no model API creates these vectors.

The new Rust tests ran against the pre-CG-20 implementation and failed in both
modes. Indexed search returned no requested-model hit; fallback returned only
one parent when four were expected. The
[saved release observations](evidence/vector-model-filter-baseline-http-2026-09-09.json)
reproduced 24 indexed and 16 fallback HTTP/Lua mismatches across two server
lifetimes. The [live Rust failures](evidence/vector-model-filter-baseline-contract-2026-09-09.txt)
are retained as regression evidence.

The [release harness](evidence/vector-model-filter-http.py) seeds synthetic
embeddings through authenticated document routes, creates a trained cosine
index, and executes `/api/search/vector` and Lua `graph.similarity`. Eleven
cases have a numeric threshold and run through both surfaces; the twelfth,
`threshold: None`, is covered directly by Rust because HTTP/Lua expose numeric
thresholds. Both Arango modes run before and after a CogniGraph server restart:
11 × 2 surfaces × 2 modes × 2 lifetimes = 88 checks.

A loopback proxy captures the actual vector queries from the release binary,
including candidate-window sizes. The captured indexed query is passed to
Arango's `EXPLAIN` endpoint to verify an `EnumerateNearVectorNode`. One vector
list makes all fixture vectors searchable without varying inter-cell recall.

The harness also probes one model-selection case on each Native configuration,
through HTTP/Lua before and after restart: 12 observations. Resident/embedded
passes all four. Resident/sidecar and paged/sidecar each miss the requested model
in all four observations. This separate defect remains open as [CG-34](CG-34.md);
those eight Native mismatches are expected reproductions, not resolved by CG-20.

## Runtime environment and reproduction

A disposable `arangodb:latest` container runs on loopback with synthetic
credentials, `--vector-index true`, and separate contract/baseline/corrected
databases. The resolved version is 3.12.11 on Linux ARM64, repository digest
`sha256:39bbca489179ea03f2b24b7ea4e4c4cb5258f6474f8c1c4d9bd65f7cd6d211a5`.
The HTTP harness refuses a database that already contains user collections.
No local environment file is edited. The CG-20 harness makes no model calls.
The full Rust suite includes the existing credential-gated OpenAI/Gemini
embedding smoke tests; model benchmarking remains deferred.

```bash
# Set ARANGO_URL/ARANGO_USER/ARANGO_PASSWORD for a disposable vector-enabled
# server and create separate empty cg20_contract and cg20_http databases.
export ARANGO_DB=cg20_contract
export COGNIGRAPH_VECTOR_SEARCH_MODE=fallback
cargo test -p cognigraph-arango -- --nocapture
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo build --release -p cognigraph-server
python3 docs/issues/evidence/vector-model-filter-http.py \
  --arango-url "$ARANGO_URL" --arango-db cg20_http \
  --output /tmp/cg20-http.json
```

The focused mixed-model tests explicitly select indexed and fallback modes;
the fallback environment setting lets the older, generic conformance fixture
run without creating a vector index. For baseline reproduction, pass a saved
pre-CG-20 release using `--binary PATH --expect-vulnerable` and another empty
database. The caller removes the disposable Arango container and its volumes
afterward; the harness stops its own servers and proxy.

## Final verification and corrected attempts

- Focused Arango suite: PASS, 50 unit tests and eight live integration tests.
- Focused Core/Native/Server suites: PASS, 376 tests.
- `cargo fmt --all -- --check`: PASS after formatting two line wraps.
- `cargo clippy --all-targets -- -D warnings`: PASS.
- `cargo test --all`: PASS, 925 tests, zero failed/ignored across 69 summaries.
  Live Arango credentials were set and all eight integration tests executed.
  The existing OpenAI and Gemini embedding smoke tests also passed.
- `cargo build --release -p cognigraph-server`: PASS.
- Arango release HTTP/Lua: PASS, all 88 checks and two server restarts.
  Actual AQL uses an `EnumerateNearVectorNode` with a model filter. In both
  modes, the four-parent case expands windows through `[4, 8, 16, 32, 64]`.
- Native controls: four resident/embedded checks pass; eight sidecar checks
  reproduce open CG-34 across resident/paged storage and their restarts.
- The disposable Arango container and its anonymous volumes were removed.

The new production module is 127 lines and its integration helper is 61 lines;
vector retrieval is extracted from the existing graph-backend module. The initial
formatting gate failed only on line wrapping in the new source/test modules.
After `cargo fmt --all`, the full formatting/Clippy/test sequence passed.

The first baseline HTTP attempt completed its behavior checks, then failed a
harness assertion that used the plural name `EnumerateNearVectorsNode`. Arango
reports `EnumerateNearVectorNode`. The assertion was corrected and the entire
baseline reran successfully in a fresh database. The corrected release needed
no additional source or harness repair. The two pre-fix Rust contract failures
and the 40 pre-fix Arango HTTP/Lua mismatches were intentional reproductions.

The corrected release SHA-256 is
`29424f808e3d0f530afc305d72d8364719d353ab9f21528a62ac62860f83112f`.
CG-20 is resolved. CG-19 and CG-20 were committed locally as `fb49c78`; nothing
was pushed and unrelated drafts remain untouched. The registry has 23 resolved
and 11 open issues. Next is CG-34, then the remaining registry beginning with
CG-21. The deferred model benchmark and Luna baseline remain unchanged.

- [Corrected release results, candidate windows, and actual indexed query plan](evidence/vector-model-filter-http-2026-09-09.json)
- [Validation totals, source hashes, and corrected attempts](evidence/vector-model-filter-validation-2026-09-09.json)

Follow-up: the separately reproduced Native model-filtering gap was resolved
by [CG-34](CG-34.md). Its original failing observations above remain baseline
evidence; the harness now expects the corrected Native controls to pass.
