# CG-34 Native sidecar model filtering — 2026-09-09

CG-19 and CG-20 were committed locally as `fb49c78` before this work, excluding
unrelated drafts. CG-34 corrects the Native sidecar model-selection discrepancy
found during CG-20's Arango verification.

## Implementation and compatibility

The mmap base and incremental delta both filter the requested model before
the top-candidate cutoff. Base model names are interned in memory with a
four-byte model ID per vector slot, reconstructed from the same redb revision
and slot order as the sidecar's key list. The delta carries the current optional
model name with each vector. Existing write paths publish them together, so a
model-only update moves candidates between model populations without changing
the vector or rebuilding the file.

A delta always shadows its base slot, including moves to another model,
deleted rows, and removed embeddings. New models first appearing in the delta
are searchable. Missing/null/nonstring model names are distinct from every
string filter, including the empty string. Case-sensitive matching, exact
re-ranking, thresholds, returned row identities, and the existing candidate
oversampling factor remain unchanged.

The `CGVEC2` file format is unchanged; no data migration is needed. The existing
base-file scan remains parallel, and filtered requests do not load every full
vector for exact scoring. See the
[sidecar decision](../decisions/decision_vector_sidecar.md#model-selection-cg-34-2026-09-09).

## Regression coverage

The [shared fixture](../../crates/cognigraph-native/tests/fixtures/sidecar-model-filter.json)
starts with 88 synthetic vectors, including 80 higher-scoring model-A rows ahead
of model B. Nine phases specify 25 queries with explicit expected keys/scores:
model selection, limits, thresholds, absent/empty/case-sensitive names,
model-only moves, delta inserts, model removal, deletions, embedding removal,
batch changes, delete/recreate, new model names, and a 70-row rebuild trigger.

Three Rust lifecycle tests execute the fixture in resident/embedded,
resident/sidecar, and paged/sidecar modes. They assert exact scores, model-only
updates preserving full-precision vectors, no rebuild during small deltas,
the rebuild threshold, and true unchanged-revision file reuse after reopening
both the original and rebuilt stores. The pre-fix implementation passed the
embedded control and failed both sidecar tests on the first model-filter case.

The [release harness](../evidence/engineering-historical-checks.md#artifact-1f814aafa876b8b70981) seeds authenticated
servers with the saved pre-fix release, then opens those stores with the
corrected release. It runs 24 numeric-threshold cases through HTTP and Lua,
then repeats the final three cases after another server restart: 54 checks per
mode, 162 checks total, and six restarts. The direct Rust API additionally tests
`threshold: None`, which the HTTP/Lua interfaces do not expose.

The corrected release passes all 162 checks. The pre-fix release produced
38 mismatches in each sidecar configuration, 76 total, while the embedded
control passed. The corrected stores preserve the quantized vector payloads
written by the previous binary, absorb small mutations without rewriting the
base file, rebuild when the delta threshold is crossed, and preserve the
rebuilt vector payload across the final restart.

## Startup reuse finding and corrected attempt

The first baseline HTTP attempt incorrectly expected the entire vector file
to remain unchanged across authenticated server startup. It failed because
Native `ensure_collection` commits even for an existing collection, and auth
startup ensures three collections. This advances the global revision and
refreshes the vector file's header when the next search rebuilds it. The
vector payload remains identical. This separate performance issue is [CG-35](CG-35.md),
subsequently resolved with [collection-ensure verification](collection-ensure-2026-09-09.md).
The dedicated CG-35 harness requires exact file reuse through unchanged
authenticated restarts; this report retains the original CG-34 observations.

The harness was corrected to retain strict whole-file equality during delta
writes, require a rebuild after the threshold, and compare vector payloads
across server restarts. Direct NativeBackend tests still require zero rebuilds
and exact file reuse when the revision is unchanged. The complete baseline and
corrected release runs passed these checks after the harness correction.
No final Rust gate or corrected-release behavior check failed.

## Validation and reproduction

- Focused Native suite: PASS, 55 tests, including the three new lifecycle tests.
- `cargo fmt --all -- --check`: PASS.
- `cargo clippy --all-targets -- -D warnings`: PASS.
- `cargo test --all`: PASS, 928 reported tests, zero failures or ignored tests.
  This Native-only run did not configure Arango: its eight integration entries
  returned without live database calls. The unchanged Arango implementation
  was verified live for the preceding `fb49c78` batch. The existing OpenAI and
  Gemini embedding smoke tests executed with local configuration and passed.
- `cargo build --release -p cognigraph-server`: PASS.
- Release HTTP/Lua lifecycle: PASS, 162 checks across three configurations.

```bash
cargo test -p cognigraph-native
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo build --release -p cognigraph-server
python3 docs/issues/evidence/sidecar-model-filter-http.py \
  --seed-binary /path/to/saved-pre-cg34-server \
  --output /tmp/cg34-http.json
```

The harness also runs without `--seed-binary`, using the corrected binary for
seeding. Reproduce the old failure with `--binary PATH --expect-vulnerable`.
All stores, credentials, and vectors are synthetic. The harness makes no model
calls, edits no environment file, and terminates its own processes. Model
benchmarking remains deferred and Luna remains the economical baseline.

- [Pre-fix Rust failures](../evidence/engineering-historical-checks.md#artifact-50823d655873df3d7044)
- [Pre-fix release observations](../evidence/engineering-historical-checks.md#artifact-6f8353f4af73c38ebcbd)
- [Corrected release, old-store continuity, file hashes, and restart evidence](../evidence/engineering-historical-checks.md#artifact-abb2ebcfecb602580661)
- [Validation totals, source and binary hashes, and corrected attempt](../evidence/engineering-historical-checks.md#artifact-761b8668177e1d455dcc)

## Completion

CG-34 was committed locally as `8164f4e`. The preceding
CG-19/CG-20 work was committed as `fb49c78`. Nothing was pushed, and unrelated
research/draft changes were preserved. The registry now contains 24 resolved
and 11 open issues, including the separate startup performance finding CG-35.
Next is concurrent neuron acceptance, CG-21.
