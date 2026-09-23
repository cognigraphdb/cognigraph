# Embedding recall with side views, five models, five languages

**Date:** 2026-09-23
**Issue:** [CG-98](../../issues/CG-98.md). Amends the July recall evaluation in the
[side-view decision](../../decisions/decision_sideviews_provider_and_benchmark.md#recall-re-benchmark-2026-09-23).
**Verdict up front:** Gemini `gemini-embedding-2` retrieved best in every
language and every configuration. OpenAI `text-embedding-3-large` was second
and `text-embedding-3-small` a distant fourth. Gemini lost little when queries
were not in English. Side views helped much less than the July evaluation
reported.

## Setup

- **Corpus:** 200 current DailyMed prescription labels from the local pilot
  corpus (catalog snapshot of July 10, 2026). They were chosen by SHA-256
  rank of a fixed seed and Set ID, one label per distinct first title word,
  each with an indications section of at least 200 characters. Labels were
  split by section into 4,084 chunks of at most 2,000 characters.
- **Queries:** 100 target labels, one lay query each, without drug, brand,
  ingredient or labeler names. A leak check rejects any word that starts with
  a name token, ignoring case and accents. The queries were written by Claude
  from each label's indications excerpt and title. Each query was then
  translated into European Portuguese, Spanish, German and French, giving
  500 queries in all. The labels stay in English, so non-English queries
  measure cross-lingual retrieval.
- **Side views:** generated once with `gpt-5.6-luna`, 10 per label, 2,000 in
  total, and frozen. Every embedder embedded the same side-view questions.
- **Path:** the product's own `POST /api/search/hybrid` on a fresh 2.7.38
  Enterprise release server per embedder. Reciprocal rank fusion (RRF)
  combines BM25 over the labels, chunk vectors (collapsed to their label),
  and side-view vectors (collapsed to their label). The similarity threshold
  was disabled, the limit was 10 and the query cache was off.
- **Configurations:** BM25 only; vector only; vector plus side views; hybrid
  (BM25 plus vector); hybrid plus side views at weight 0.5 (the default) and
  at 0.35.
- **Embedders:** OpenAI `text-embedding-3-small` and `text-embedding-3-large`,
  Gemini `gemini-embedding-2`, and through Ollama `qwen3-embedding:8b`
  (Q4_K_M) and `bge-m3`. All used provider defaults, with no dimension
  reduction and no task type or query instruction. CogniGraph cannot send
  those yet ([CG-102](../../issues/CG-102.md)).
- **Metrics:** MRR@10, recall@1, recall@5 and recall@10 against the single
  target label. Intervals are 95% bootstrap intervals over queries. Model
  differences are paired, on the same queries.
- **Harness:** [`embedding-recall-benchmark.py`](../../verification/harnesses/embedding-recall-benchmark.py),
  with a provider-free `preflight` mode.

## Results

MRR@10 with side views, at the default hybrid weights:

| Embedder | EN | PT | ES | DE | FR |
|---|---:|---:|---:|---:|---:|
| Gemini `gemini-embedding-2` | 0.881 | 0.846 | 0.854 | 0.853 | 0.873 |
| OpenAI `text-embedding-3-large` | 0.793 | 0.752 | 0.752 | 0.783 | 0.777 |
| Qwen3-Embedding 8B (Ollama, Q4_K_M) | 0.730 | 0.691 | 0.710 | 0.733 | 0.716 |
| OpenAI `text-embedding-3-small` | 0.668 | 0.601 | 0.659 | 0.566 | 0.611 |
| BGE-M3 (Ollama) | 0.543 | 0.524 | 0.480 | 0.416 | 0.471 |

Recall@1 in the same configuration:

| Embedder | EN | PT | ES | DE | FR |
|---|---:|---:|---:|---:|---:|
| Gemini `gemini-embedding-2` | 0.80 | 0.75 | 0.75 | 0.76 | 0.79 |
| OpenAI `text-embedding-3-large` | 0.66 | 0.59 | 0.59 | 0.65 | 0.63 |
| Qwen3-Embedding 8B | 0.59 | 0.53 | 0.55 | 0.60 | 0.58 |
| OpenAI `text-embedding-3-small` | 0.48 | 0.43 | 0.48 | 0.36 | 0.42 |
| BGE-M3 | 0.37 | 0.37 | 0.33 | 0.27 | 0.29 |

Gemini put the target in the top 5 for 98 to 99 of 100 queries in every
language. OpenAI large did so for 95 to 98, Qwen3 for 88 to 96, OpenAI small
for 85 to 94 and BGE-M3 for 62 to 75.

English MRR@10 by configuration:

| Embedder | BM25 | Vector | Vector + side views | Hybrid | Hybrid + side views | Hybrid + side views at 0.35 |
|---|---:|---:|---:|---:|---:|---:|
| Gemini | 0.667 | 0.848 | 0.880 | 0.848 | 0.881 | 0.872 |
| OpenAI large | 0.667 | 0.772 | 0.793 | 0.765 | 0.793 | 0.789 |
| Qwen3 8B | 0.667 | 0.526 | 0.725 | 0.614 | 0.730 | 0.724 |
| OpenAI small | 0.667 | 0.654 | 0.659 | 0.670 | 0.668 | 0.668 |
| BGE-M3 | 0.667 | 0.535 | 0.516 | 0.553 | 0.543 | 0.551 |

## Findings

1. **Gemini is best, and the margin is real.** Against Gemini on the same
   queries (hybrid with side views), every other model is behind in every
   language, and every paired 95% interval excludes zero:

   | Model | MRR@10 behind Gemini |
   |---|---|
   | OpenAI large | 0.07 to 0.10 |
   | Qwen3 8B | 0.12 to 0.16 |
   | OpenAI small | 0.20 to 0.29 |
   | BGE-M3 | 0.32 to 0.44 |
2. **Cross-lingual loss is small for the strong models.** From English to
   the lowest other language, Gemini loses 0.035 MRR and OpenAI large 0.041.
   OpenAI small loses 0.10 (German). BGE-M3, a multilingual model, is weakest
   in every language here.
3. **Side views help modestly.** The paired lift from adding side views to
   hybrid search was +0.01 to +0.05 MRR for Gemini and OpenAI large. Many
   intervals include zero; the positive ones are Gemini in English, and
   OpenAI large in German, French and Portuguese. For OpenAI small and BGE-M3
   the lift was indistinguishable from zero.
   - **Qwen3 is the exception.** Side views lifted it by +0.12 in English
     (interval +0.05 to +0.18) and +0.10 in French.
   - **Why Qwen3 benefits.** Its query-to-chunk retrieval is weak (0.526 in
     English), but query-to-question matching works. This is consistent with
     a model that expects a query instruction CogniGraph does not send. The
     model card puts that effect at only 1 to 5%, so quantization or the
     Ollama runtime may also contribute ([CG-102](../../issues/CG-102.md)).
4. **Hybrid fusion does not add BM25's strength.** BM25 alone reaches 0.667
   in English. Hybrid search equals vector-only for Gemini and falls below
   BM25 alone for BGE-M3 (0.553). This is consistent with the vector leg
   adding one RRF contribution per matching chunk, so it outvotes BM25's
   single contribution per label ([CG-101](../../issues/CG-101.md)).
5. **Weight 0.35 makes no difference.** The July suggestion does not hold
   here: across all 25 model and language pairs, 0.35 differed from 0.5 by
   at most 0.018 MRR, higher in 11 pairs and lower in 14.

## Comparison with July 2026

The July evaluation (130 labels, 1,300 side views, 30 queries) reported
Gemini vector-only MRR rising from 0.538 to 0.703 with side views, and
recall@5 from 63% to 93%. Its script, queries and side views are not
preserved, so it cannot be re-run, and the numbers are not comparable. Here
Gemini's vector-only MRR is already 0.848, leaving less headroom, and side
views add 0.032. In July, OpenAI small scored higher than Gemini without side
views (0.625 against 0.538) and lower with them (0.656 against 0.703). Here
Gemini leads OpenAI small in every configuration. July's local model was a
different one (`nomic-embed-text`).

## Consequences

- **Default embedder:** keep `gemini-embedding-2`, including for multilingual
  deployments.
- **OpenAI-only deployments:** use `text-embedding-3-large`, not `small`.
  Small costs recall in every language, most in German.
- **Self-hosted:** of the two local models, only Qwen3-Embedding 8B is usable
  here, and mainly with side views. BGE-M3 is not recommended for this
  workload. Qwen3 needed about an hour to embed this corpus on a 24 GiB Apple
  Silicon machine, against minutes for the API models.
- **Side views:** they remain a small, mostly positive addition. The
  evidence no longer supports calling the lift large. They matter most when
  the query embedding is weak, as with Qwen3.

## Spend and runtime

- **Side-view generation (`gpt-5.6-luna`):** 200 calls in the recorded run,
  plus 76 in a first attempt that failed at Gemini's rate limit after 75
  labels.
- **Embeddings, per embedder:** 4,084 chunks and 2,000 side-view questions,
  plus one query embedding for each of the 3,000 searches.
- **Runtime:** under 4 minutes for each OpenAI model, and just over 4 for
  BGE-M3. Gemini took 22 minutes with one worker, because of its rate
  limits. Qwen3 took 67 minutes.

The server does not meter provider cost.

## Limitations

- **One query author and one corpus.** Queries come from indications text,
  which favors lexical overlap (hence BM25's 0.667). Every query has a single
  relevant label.
- **An imperfect selection.** Two targets are atorvastatin labels (a brand
  and a generic) that the first-word de-duplication did not catch. A few
  targets share an indication with other labels. This limits recall@1 for
  every model equally.
- **Translations.** They were written by the same author. Cross-lingual
  retrieval against an English corpus is not the same as retrieval over a
  multilingual corpus.
- **Defaults only.** The models ran without task types, instructions or
  dimension settings, and the local models used Ollama's default
  quantization. Microsoft's Harrier-OSS and other leading open multilingual
  models were not tested; they are not packaged for Ollama.
- **One side-view generation.** Side-view text varies between generations,
  and so would the lift.
- **Private inputs.** The frozen inputs and raw results are sealed in the
  private evidence repository, so this run cannot be replayed from the
  public checkout alone.
