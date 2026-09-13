# Decision: Search-only embeddings in an mmap int8 sidecar

Date: 2026-07-02 (M10, design session held first) · Status: ACCEPTED, delivered

## Context
Quantization alone bought speed but not memory (JSON kept f64 arrays); mmap
against RAM-resident vectors would have been theater — both said so in
benchmarks.md before this design existed.

## Decision (owner: skitsanos, five explicit calls)
Search-only embeddings behind COGNIGRAPH_VECTOR_MODE=sidecar (breaking:
`RETURN d` omits embedding; opt-in, default unchanged); persistent backends
only; memmap2; exact re-rank via redb point-reads; one file per collection.
v2 addendum: incremental writes via an in-memory delta shadowing the
immutable base (replaces in-file tombstones), rebuild past 10% delta.

## Outcome
7.3× vector RAM reduction (10.2 MB → 1.4 MB paged), 0.85–1.08 ms search,
recall unchanged (exact two-stage). The lifecycle test caught the flagged
hazard exactly: the first implementation's partial update merged against the
stripped doc and would have destroyed vectors — fixed to merge against redb
truth before it ever shipped.

## Model selection (CG-34, 2026-09-09)

Base-file and incremental-delta candidates must match the requested model
before the stage-one top-candidate cutoff. The existing fourfold oversampling
with a minimum of 32 candidates is for quantization recall within that model;
it cannot serve as the correctness mechanism for a model filter.

The base sidecar now has an in-memory model ID per vector slot, with model
names interned in a dictionary. This adds four bytes per base vector plus the
unique-name dictionary. Model metadata is reconstructed during the existing
redb scan used to open or rebuild the sidecar, under the same publication lock
and revision check as its key list. It is not added to the `CGVEC2` file.

Delta entries carry the current optional model name alongside their quantized
vector. All existing write paths publish both together, including model-only
updates. A delta entry shadows its base slot even when it moves to another
model; deletions and removed embeddings keep their tombstone behavior.
Missing, null, and nonstring model names never match a string filter. An empty
string matches only an explicitly empty stored model name, and case remains
significant. Exact re-ranking, thresholds, and returned row identities stay
unchanged.

No sidecar format migration is required. NativeBackend warm-open tests verify
file reuse without a revision change. Full authenticated server restarts
currently issue no-op collection writes and refresh the revision, causing a
rebuild with unchanged vector payload; that separate startup problem is
[CG-35](../issues/CG-35.md). See the
[CG-34 verification report](../issues/sidecar-model-filter-2026-09-09.md).

Validation passed: 55 Native tests, formatting, strict Clippy, all 928 reported
Rust tests, and 162 release HTTP/Lua lifecycle checks across the three Native
configurations. The saved release reproduced 76 HTTP/Lua mismatches. An initial
runtime assertion expecting whole-file reuse after authenticated startup failed;
the corrected harness verifies unchanged vector payloads across those restarts,
while Rust tests enforce exact reuse when the revision is unchanged. The report
records that correction and the unconfigured Arango integration-test scope.
