# CG-23 roadmap and workload status verification — 2026-09-09

The roadmap and workload decision now identify D1–D12 as delivered, separate
the initial July failure analysis from subsequent results, and point to the
CG registry for current defects. The original timing tables and intermediate
semantic differences are preserved. This is a documentation correction; no
Rust implementation, query grammar, model selection, or benchmark result changed.

## Source comparison

Checked against local revision `9fb4933`, which committed the completed CG-33
and CG-22 work. D1–D12 were originally delivered July 21–22. D1 has two parts;
D12 concerns directed construction rather than the eight CRM queries.

| Delivery | Current implementation | Existing regression evidence |
|---|---|---|
| D1a: endpoint-equality candidates | [Native document reads](../../crates/cognigraph-native/src/memory/documents.rs), `adjacency_candidates` and `list_documents_filtered_impl` | [Native backend tests](../../crates/cognigraph-native/tests/native_backend.rs) exercise endpoint candidates and remaining predicates |
| D1b: correlated equality candidates | [Query runner](../../crates/cognigraph-query/src/executor/run.rs), `SiteRows::IndexedDocs`; string-key candidates retain the full filter | [Executor tests](../../crates/cognigraph-query/tests/executor.rs), `correlated_subquery_equality_matches_the_unindexed_expansion` |
| D2: correlated traversal and one/two/three bindings | [Grammar](../../crates/cognigraph-query/src/grammar.pest), [materialization](../../crates/cognigraph-query/src/executor/materialize.rs), and [shared resolver](../../crates/cognigraph-query/src/executor/resolve.rs) | Executor traversal tests cover enclosing rows, deeper paths, subqueries, and later `FOR`; [backend test](../../crates/cognigraph-query/tests/backend_executor.rs) counts distinct starts |
| D3: conditional/null expressions | Grammar and [function registry](../../crates/cognigraph-query/src/functions/mod.rs), `COALESCE` / `NOT_NULL` | Executor conditional tests |
| D4: identifier helpers and `DOCUMENT()` | Function registry and shared resolver | Executor identifier tests; backend lookup, nesting, and filter/collect tests |
| D5: shorthand, bare count, expression subquery | Grammar and [parser desugaring](../../crates/cognigraph-query/src/parser/mod.rs) | Executor shorthand/count, expression-subquery, and post-collect rejection tests |
| D6: array helpers | [Value functions](../../crates/cognigraph-query/src/functions/value.rs), `SLICE`, `FLATTEN`, `INTERSECTION`, `MINUS` | Executor array-helper tests |
| D7: fixed-duration `DATE_ADD` | [Date functions](../../crates/cognigraph-query/src/functions/date.rs); `DATE_DIFF` already existed | Executor date-add/round-trip tests |
| D8: casts and regex | Value functions and [regex functions](../../crates/cognigraph-query/src/functions/regex.rs) | Executor cast/regex tests |
| D9: sorted arrays | Value functions, `SORTED` / `SORTED_UNIQUE` | Executor sort-clause agreement, mixed types, and deterministic slicing tests |
| D10: defer projection-only calculations | [Planner](../../crates/cognigraph-query/src/planner.rs), `defer_return_only_lets`; disabled for collect/mutations | [Planner tests](../../crates/cognigraph-query/tests/planner.rs); backend test verifies only surviving targets are fetched |
| D11: retry only the phase that missed | Shared resolver's head/tail loops | Backend tests assert one head scan for deferred/projection-only/nested document reads |
| D12: directed construction | [Construction implementation](../../crates/cognigraph-construct/src/directed.rs) and [HTTP routes](../../crates/cognigraph-server/src/routes/construct/mod.rs) (then `routes/construct.rs`, before CG-26) | [Route tests](../../crates/cognigraph-server/src/routes/construct_directed_tests.rs) and the same revision's [CG-22 release verification](api-contract-2026-09-09.md) |

The source comparison supports delivery, not unrestricted applicability.
Correlated candidate indexing is bounded to supported equality shapes; D10 does
not move a result limit ahead of `COLLECT`; query authors still select join order.
The approved [v2 amendment](../decisions/decision_cgql_v2.md) governs expression
subqueries. The spec's contradictory exclusion remains the separate
[CG-28](CG-28.md) ticket. Later query correctness changes retain their own
[CG-7](CG-7.md), [CG-8](CG-8.md), [CG-15](CG-15.md), and [CG-16](CG-16.md) evidence.

## Verification

`cargo test -p cognigraph-query` passed **144 tests**, zero failed or ignored,
including the file-driven corpus. The source snapshot is unchanged from CG-22's
successful formatting, strict Clippy, and full workspace run (**950 reported
passes**, eight unconfigured Arango entries early-returned). Those full gates
were not repeated for this documentation-only update.

The [runnable capability examples](evidence/roadmap-parity-http.py) passed
**58 HTTP/Lua checks** through the existing release binary: fourteen query
examples and matching analyzed result counts, plus a Lua `graph.query()` call,
in each of resident/sidecar and paged/sidecar Native storage. D1–D11 examples
cover endpoint and correlated filters, all three traversal binding forms,
conditional/null handling, identifier helpers, nested document lookups,
shorthand/count/subqueries, arrays, dates, casts, regex, and deferred projection.
Internal optimization effects retain the existing instrumentation tests above;
these small examples do not measure speedups.

The seed comes from the checked-in [query corpus](../../crates/cognigraph-query/tests/corpus/dataset.json).
Edges use the public relationship route, so their generated keys differ from
the in-memory corpus; comparisons use endpoints and stable sorting. The server
uses authentication, explicit configuration with `/api/query` enabled, disposable
stores, and no embedding or completion provider. It starts outside the checkout
to avoid loading local environment files, and both processes stop afterward.

[HTTP evidence](evidence/roadmap-parity-http-2026-09-09.json) records the queries,
expected and observed values, modes, and binary/dataset/harness hashes.
[Validation evidence](evidence/roadmap-parity-validation-2026-09-09.json) records
test counts, documentation checks, and preservation of unrelated files.
D12's live grounding/replacement/error checks were already run with this exact
binary for CG-22; no new provider request was needed here.

Two initial harness setup failures were corrected before the successful run:
HTTP 400 when seeding an Edge collection through the Document route, then HTTP
404 because `/api/query` was not enabled. The corrected harness uses the
relationship route and explicit `COGNIGRAPH_CGQL_MUTATIONS_ENABLED=true`; its
query examples are read-only. No product code change was required.

Re-run with:

```bash
cargo test -p cognigraph-query
cargo build --release -p cognigraph-server
python3 docs/issues/evidence/roadmap-parity-http.py --output /tmp/cg23-http.json
```

## Measurement and scope limits

The original July CRM tables remain historical. Their eight-query completion
and row counts are not a new September measurement or a full equivalence proof.
The later [D10/D11 benchmark record](../research/benchmarks/native-backend.md#d10-move-calculations-down-2026-07-22)
separately reports full-answer equality for its selected formulations and the
780 ms / 927 ms board total after D11. Build profiles and formulation-specific
tradeoffs remain attached to those records. The CRM dataset was not re-run;
there is no new AQL comparison or general latency guarantee.

The CUAD quality claims also remain historical; [CG-25](CG-25.md) still requires
a repository reproduction package. No research drafts, semantic-neuron papers,
or `ui/` files were edited. The July CI billing observation is now historical,
and the manual workflow is checked against the local file; remote CI status
was not queried. Model benchmarking remains deferred with Luna as the economical
baseline. CG-23 was committed locally with CG-28 as `b9084e6`; nothing was pushed.
