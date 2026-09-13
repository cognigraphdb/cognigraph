# CG-31 hybrid cache identity — 2026-09-08

CG-31 is resolved in the local worktree. Changes remain uncommitted and
unpublished; the React UI was outside this batch.

## Change

Hybrid search builds its result-cache key with a versioned JSON representation
of the deserialized request parameters. Arrays retain their ordering and
element boundaries; JSON escapes names containing commas, semicolons, quotes,
and backslashes. All result-shaping request fields participate automatically.
Query text remains a separate normalized key component, preserving the
existing contract: exact parameter identity, similarity matching on query
embeddings only.

Existing assisted-hit fixtures now call the production `cache_key()` builder
instead of copying its encoding. Three new tests cover all 12 result-shaping
fields, normalized/default equivalence and field ordering, and handler-level
collisions over real Native BM25. Collision cases cover field arrays, escaped
field names, collection/view boundaries, and view/field boundaries, through
exact hits, strong similarity (cosine 1.0), and weak similarity (cosine 0.8).
Same-parameter assisted hits and exact repeats remain functional.

## Validation

All required gates passed in order:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
```

The full suite passed **872 tests**, zero failed or ignored, across 66 result
summaries; the server passed 263 tests. The release server build and
`git diff --check` also passed.

The first focused regression run failed because its fixture omitted the empty
embeddings collection required by the vector leg. Adding that collection
corrected the fixture; the production search behavior was not changed to
silence the failure. All final gates and runtime probes passed.

## Release HTTP evidence

A saved pre-fix release reproduced all four collision families with exact,
strong-similarity, and assisted cache lookup. The fixed release passed the
same requests. Tests used disposable authenticated databases and a synthetic
loopback embedding HTTP service; no external provider or production data was
used. No ArangoDB instance was exercised: Native HTTP tests include the view
parameter in identity checks, while the shared key builder is backend-neutral.

| Configuration | Pre-fix collision reproduced | Fixed isolation | Restart |
|---|---|---|---|
| Resident / memory cache | Yes | Pass | Pass |
| Resident / persistent cache | Yes | Pass | Pass |
| Paged / memory cache | Yes | Pass | Pass |
| Paged / persistent cache | Yes | Pass | Pass |

There were 52 collision probes per executable: four parameter pairs × three
lookup paths × four configurations, plus one post-restart probe per
configuration. Before the fix, exact/strong hits returned the wrong document;
weak hits added the previous request's document to fresh results. After the
fix, changed parameters returned only the intended fresh document, repeated
requests hit the correct entry, and same-parameter weak matches still assisted.

Restarts left result caches empty. The persistent cache reused the previously
warmed embedding with zero provider calls; the memory cache made one call.
The fingerprint change requires no persistent-cache migration because result
entries are memory-only. Embedding cache keys and storage are unchanged.

- [Pre-fix observations and executable hash](../evidence/engineering-historical-checks.md#artifact-8ed5eee7d7eb56dca74f)
- [Fixed observations and executable hash](../evidence/engineering-historical-checks.md#artifact-e0e12819ab9233215ffb)
- [Runnable regression](../evidence/engineering-historical-checks.md#artifact-670fb75e536415f259a8)

```bash
cargo build --release -p cognigraph-server
python3 docs/issues/evidence/hybrid-cache-http.py
```

Use `--binary PATH --expect-vulnerable` to check a saved pre-fix executable.
The script prints its temporary evidence path and stops all its test servers.

## Follow-up

Cross-checking the other search routes found two additional lossy encodings,
recorded as [CG-32](CG-32.md): semantic search conflates omitted and empty
model filters, and graph-augmented search has an ambiguous collection-name
encoding. The semantic case was additionally reproduced over resident/paged
release HTTP; the graph key equality is source-confirmed, with its live
traversal regression still pending. Neither route was changed in this batch.

The registry now contains **13 Resolved and 19 Open issues** (18 P2 and one
P3; no open P1). CG-32 is the next bounded cache-correctness batch.
