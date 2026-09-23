# v2.7.39 — Embedding recall re-benchmark, five models and five languages

- Date: 2026-09-23
- Status: v2.7.39
- Kind: Research and verification (no product behavior change)

## Changes

- **Re-benchmark.** It covers embedding models for side-view retrieval
  ([CG-98](../issues/CG-98.md),
  [research note](../research/benchmarks/embedding-recall-2026-09-23.md)).
  - The run used 200 DailyMed labels, 2,000 frozen side views, and 100 lay
    queries in English, Portuguese, Spanish, German and French.
  - It went through the product's own hybrid search, with six
    configurations per embedder.
  - MRR@10 with side views: `gemini-embedding-2` 0.846 to 0.881,
    `text-embedding-3-large` 0.752 to 0.793, Qwen3-Embedding 8B 0.691 to
    0.733, `text-embedding-3-small` 0.566 to 0.668, BGE-M3 0.416 to 0.543.
- **Decision.** The
  [side-view decision](../decisions/decision_sideviews_provider_and_benchmark.md#recall-re-benchmark-2026-09-23)
  keeps `gemini-embedding-2` as the default and recommends
  `text-embedding-3-large` for OpenAI-only deployments. It supersedes the
  July side-view lift, now measured at +0.01 to +0.05 for the strong models,
  and drops the 0.35 weight suggestion. The configuration guide now points
  to the benchmark.
- **Harness.** A new live harness,
  `docs/verification/harnesses/embedding-recall-benchmark.py`, has a
  provider-free `preflight` mode. Its pure logic is covered by
  `scripts/tests/test_embedding_recall_benchmark.py`. Its frozen inputs are
  private evaluation data and live in the evidence repository.
- **New tickets:**
  - [CG-99](../issues/CG-99.md) (P1): concurrent text-index rebuild race
    after writes;
  - [CG-100](../issues/CG-100.md) (P3): fixed 120 s Ollama timeout;
  - [CG-101](../issues/CG-101.md) (P3): hybrid fusion sums chunk ranks;
  - [CG-102](../issues/CG-102.md) (P3): no query/document embedding hint.

The workspace version moves to 2.7.39.

## Validation

- **Harness logic:** 11 unit tests cover selection, chunking, leak checks,
  ranks, metrics, bootstrap and paired intervals, and retry
  classification.
- **Preflight:** it passed against a local fake embedder.
- **Live run:** each run checked the server-reported model against the
  pinned one and failed on any search that lost a leg.
- **Gate:** the full local gate passes.
