# Decision: No optimization without a before/after row; rejections recorded

Date: 2026-07-02 (Phase 10) · Status: ACCEPTED (standing rule)

## Context
The Coilara-inspired perf backlog (quantization, mmap, rayon) risked
checkbox-driven engineering.

## Decision
A dependency-free bench harness (`cargo bench -p cognigraph-native`,
deterministic dataset) gates all performance work: every optimization lands
with a measured row in docs/benchmarks.md; measured regressions are
rejected and the rejection recorded; deferrals carry their architectural
reason, not a TODO.

## Outcome
Delivered 47× vector search, 159× BM25, 2.7–3.1× CGQL scans, 7.3× vector
RAM, 100% recall@10 and 10/10 hybrid agreement — and the record also shows
one rejected approach (per-doc HashMap BM25, measurably slower) and one
initially-refused checkbox (mmap, until the sidecar made it real).
