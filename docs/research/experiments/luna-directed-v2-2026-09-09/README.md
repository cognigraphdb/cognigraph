# Directed policy v2 development candidate

> Public reading copy; original research captures and scripts are private.
> Original path: `fixtures/semantic-neurons/luna-directed-v2-2026-09-09/README.md`.
> Original SHA-256: `ef82dd964654b9b8c1b9ff7b5819861566c36f789ea5e881b867c59f33f27934` (3166 bytes).
> Navigation changed; dated results and limitations retain their original scope.


This package measures CG-39 and CG-40 against the frozen
[Luna document baseline](../luna-documents-2026-09-09/README.md), checkpointed as
`a0e4b24`. Historical source data, inputs, gold, captures and the V2 scoring
amendment remain in that package without edits. Its complete manifest is pinned.

The candidate adds Unicode word boundaries around both endpoint mentions and
exact request-derived enums for chunk IDs and relation names. It stamps
`directed-policy-v2`. Quote lookup, the prompt, taxonomy, Luna-low configuration,
document cohort and alias scoring policy remain unchanged. The Unicode policy
uses UTS #18 word characters; apostrophes and hyphens separate tokens. It is not
full name resolution or semantic validation. The construction writer's existing
ASCII-derived entity-key restriction still excludes names with no ASCII letters
or digits; the new boundary matcher itself supports Unicode scripts.

The candidate is frozen before execution by `freeze.py`, including a production
diff, the new Rust tests, all relevant source hashes, binary hash, scripts and the
baseline manifest. The runner accepts only the 40 development documents. First,
it replays the original provider proposals through the release server. This
isolates gate effects without new model output. After replay verification, one
fresh Luna-low pass measures the request schema with a $1 conservative reservation
ceiling and no retries. Only `max_completion_tokens=8192` is added by the recorder.
All databases are disposable Native stores; embeddings are synthetic loopback
vectors. The 80-document holdout remains unrun.

The scorer retains the baseline V2 exact annotated-alias matching, invalid-ID
penalty, symmetric spouse handling, deduplication, byte-span verification and
cache-aware token accounting. No heuristic ID repair, fuzzy name repair or
model-based judging is used. Published references are incomplete: unmatched
proposals are not independently proven false, and a single pass cannot separate
schema effects from generation variability. Independently reviewed full-document
labels and verified negatives remain a separate qualification task.

```bash
export PYTHONDONTWRITEBYTECODE=1
python3 fixtures/semantic-neurons/luna-directed-v2-2026-09-09/freeze.py
python3 fixtures/semantic-neurons/luna-directed-v2-2026-09-09/run.py --mode replay --output fixtures/semantic-neurons/luna-directed-v2-2026-09-09/replay
python3 fixtures/semantic-neurons/luna-directed-v2-2026-09-09/verify.py --replay-only
python3 fixtures/semantic-neurons/luna-directed-v2-2026-09-09/run.py --mode live --output fixtures/semantic-neurons/luna-directed-v2-2026-09-09/live
python3 fixtures/semantic-neurons/luna-directed-v2-2026-09-09/verify.py
```

These commands create captures only in absent output directories. Once sealed,
verification can be repeated, but do not rerun generation over existing evidence
or edit this package to accommodate later production changes. Costs use the
[baseline's frozen official tariff](https://developers.openai.com/api/docs/pricing),
not an invoice. This candidate does not revisit the user's model selection.
