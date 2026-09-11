# CG-16 document lookup errors — 2026-09-08

`DOCUMENT()` now propagates backend failures through the shared normal,
precompiled-plan, and EXPLAIN ANALYZE resolver. Only `Ok(None)` becomes an
absent-document null after a backend lookup. Non-string inputs and strings
without a collection/key separator retain their existing null behavior without
calling the backend. The first failed lookup aborts the batch and the query;
neither partial rows nor a successful analysis report escapes.

Forbidden errors keep their typed `ExecutionError::Forbidden` mapping. A new
`ExecutionError::Connection` preserves backend connection failures, including
other fetch/mutation paths using the shared error converter. Both HTTP query
routes translate these to 403 and 503 respectively; other backend execution
failures remain 500. Lua preserves the connection error's type through callback
wrapping, for both `graph.query()` and direct graph bindings. An uncaught error
therefore has the same HTTP status; a script can still handle it with `pcall`.
User-authored error text cannot impersonate a forbidden or connection failure.

The resolver retains its existing batched retry semantics and source-unit
accounting. This changes the treatment of failed fetch attempts, not the query
grammar or backend lookup contract. CG-7's mutation restriction remains in
force, and plain EXPLAIN does not execute lookups.

## Regressions

Three query tests exercise five injected backend error categories across
seven query shapes, normal and analyzed forms, read-only and read-write entry
points, and precompiled plans: **210 expected query failures**. Shapes include
projection, arrays, filters, deferred LETs, dependent document lookups, nested
read subqueries, and COLLECT INTO projections. They also verify genuine absence,
non-identifiers, lookup counts, stopping after the first failed fetch, and inert
plain EXPLAIN. The initial red run reproduced successful null results and a
batch that incorrectly continued after a failed lookup.

Three server tests run requests through the production routers with an
injected backend. They cover **96 error/status cases**, present/missing/malformed
controls, plain EXPLAIN, Lua catching, direct Lua lookup connection failures,
and ordinary Lua error strings. Connection and storage failures are injected
deterministically in these tests; no physical disk or network outage is induced.

## Release regression

The saved pre-fix optimized binary reproduced **72 successful responses** for
forbidden document lookups across `/api/query`, `/api/search/query`, and Lua
`graph.query()`, including analyzed queries, in resident and paged Native
storage. Literal, bound, array, filter, dependent, and deferred forms all
silently suppressed the denial. Seventy-two controls checked present, missing,
missing-collection, malformed, non-string, and array-position behavior. Plain
EXPLAIN remained inert in 36 cases, 12 Lua pcall cases falsely reported success,
and both stores retained their exact documents across restart.

The harness runs the real server with authentication and only synthetic data
in disposable loopback stores, with embeddings disabled. It does not edit local
environment files or call external providers. Its initial array-position control
used unsupported indexing syntax and returned HTTP 400; the control now uses
the supported array FOR source. An initial route-test wording assertion was
also corrected to the existing `system-reserved` message. Neither correction
required a product syntax or permission change.

Final formatting, Clippy with warnings denied, and **all 907 tests** passed,
with zero failures or ignored tests across 68 result summaries. The focused
query crate passed 144 tests. The optimized server built successfully. The
updated response reference passed all 10 OpenAPI drift checks. The
corrected release returned **403 for all 72 forbidden queries**, passed all
72 controls and 36 plain EXPLAIN checks, let all 12 Lua pcall cases catch the
denial, and preserved exact documents across both storage-mode restarts.
No final validation or live check failed. HTTP 503/500 fault mapping is covered
by the injected route tests, while the release run uses real Native missing
documents and protected-collection denials.

CG-16 is resolved locally. These changes and the preceding CG-8/CG-15 batch
were committed as `41490db` on 2026-09-09; unrelated research/draft changes
are preserved and nothing was pushed. At this checkpoint the registry had
19 resolved and 13 open issues. The next bounded fix was CG-5, construction
evidence normalization and byte-span integrity.

```bash
cargo test -p cognigraph-query
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo build --release -p cognigraph-server --bin cognigraph-server
python3 docs/issues/evidence/document-errors-http.py
```

- [Reproducible provider-free release HTTP regression](evidence/document-errors-http.py)
- [Pre-fix responses and binary hash](evidence/document-errors-baseline-http-2026-09-08.json)
- [Corrected release responses and binary hash](evidence/document-errors-http-2026-09-08.json)
