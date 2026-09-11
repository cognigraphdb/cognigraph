# Luna directed-construction baseline — 2026-09-09

**Completed:** sixteen real provider calls, with captured outputs and exact
offline score replay. See the [results and recommendation](results.md).

This replaces CUAD as the active evaluation direction by user decision. The
[historical recovery](../cuad-2026-07-22/README.md) stays intact; this experiment
does not reconstruct its missing model settings or execution trace.

## Frozen scope

`corpus.json` contains 48 original fictional excerpts in four relation
categories: supply, ownership, software dependencies, and headquarters.
There are 32 gold triples and 20 negative excerpts. All text and gold labels
were authored together by Codex before provider execution, without a second
annotator. This is a synthetic regression baseline, not a held-out customer
quality estimate or independent benchmark. Templates are shared across
categories; repeated runs do not create additional independent examples.

The twelve scenarios per category cover active and passive wording,
paraphrases, negation, speculation, future plans, denied quotations, relation
direction, distractors, multiple facts, Unicode, and embedded instructions.
No CUAD text, customer material, external dataset, or external data license is
required. The candidate models receive excerpts and taxonomy descriptions,
but not scenario labels or gold answers.

`protocol.json` freezes the corpus, all Rust source/manifests, harness/scorer,
release binary, model IDs, generation settings, batching, repetition count,
matching policy, and current standard token rates. There are four batches of
twelve excerpts and two repetitions per model: sixteen provider calls total.
Luna runs first, then Astra; provider load and cache order may affect latency.

- `gpt-5.6-luna`, reasoning `none`: current economical configuration.
- `gpt-6-astra`, reasoning `medium`: stronger reference configuration.

Both use the same real `POST /api/construct/directed` route, prompt, schema,
taxonomy, excerpts, and Native writer. A loopback recorder forwards requests
to OpenAI, adding explicit effort and a 4,096 completion-token limit. It retains
both incoming and outgoing request bodies and full response bodies, usage,
request IDs, timing, and status. Authentication headers/credentials are excluded.
The actual key is reused from the existing authorized project configuration
and is held only by the recorder. The disposable server has a dummy key;
no environment file is changed. Embeddings use a synthetic local provider
and are outside the measured scope.

The local request budget is $5, conservatively reserving UTF-8 request bytes
plus 4,096 input tokens of margin and the full completion limit at uncached
standard rates before every call. This is a request admission estimate, not
a billing cap imposed by OpenAI. Actual token cost is estimated from returned
usage, including reasoning tokens and reported cached inputs. No automatic
retries, prompt changes, or output-based selection are allowed. Every failed
attempt remains recorded; a failure stops execution.

## Reproduce

Python 3.11+ standard library only. The release binary must match the frozen
hash. The recorded worktree includes CG-26's uncommitted mechanical refactor
on base commit `3fa7db7`; source and binary hashes identify the exact evaluated
state more precisely than that parent commit alone.

```sh
python3 -m unittest discover -s fixtures/semantic-neurons/luna-baseline-2026-09-09 -p 'test_*.py'
python3 fixtures/semantic-neurons/luna-baseline-2026-09-09/run.py --mode mock --output /tmp/luna-control-new
python3 fixtures/semantic-neurons/luna-baseline-2026-09-09/score.py /tmp/luna-control-new --output /tmp/luna-control-scores.json
# Live command spends the frozen budget; always use a fresh output directory.
python3 fixtures/semantic-neurons/luna-baseline-2026-09-09/run.py --mode live --output /tmp/luna-live-new
python3 fixtures/semantic-neurons/luna-baseline-2026-09-09/score.py /tmp/luna-live-new --output /tmp/luna-live-scores.json
```

`prepare.py` deterministically recreates the corpus. Stored output scoring is
deterministic; fresh model responses may vary. Provider aliases and returned
model IDs are recorded, but the provider does not expose immutable weights.
The full prompt/schema are in every provider request capture.

`verify.py --output /tmp/luna-validation.json` additionally validates captured
request parity, source/binary hashes, negative integrity probes, links and
registry state. It uses the existing project key only for a local credential
leak scan and sends no requests. Optional `--preservation-snapshot PATH` checks
an earlier map of user-owned file hashes. This post-capture audit script is
outside the pre-execution harness hash set and has its own recorded hash.

## Counting and interpretation

Score distinct `(chunk ID, source, relation, target)` triples. Normalize names
with NFC, whitespace collapse and case folding; relations and chunk IDs remain
exact. No fuzzy substring, evidence-overlap, or LLM-judge credit is granted.
Repeated occurrences of the same triple in one chunk count once; identical
triples in different chunks remain distinct. Count every unmatched gold triple
as one false negative. Empty-set precision/recall/F1 are null when undefined.

Report nominations before gates and accepted facts after gates separately,
with per-category and scenario counts, false positives, misses, negative
excerpts with facts, rejection text, duplicate occurrence counts, schema and
HTTP failures, latency, and token cost. Each accepted occurrence is checked
against canonical UTF-8 byte offsets, stored entities, and model attribution.
These provenance checks do not establish semantic truth: the frozen gold
labels supply that limited reference.

This test does not qualify a judge, validate Semantic Repair promotion,
measure retrieval/answer utility, replace the existing side-view experiment,
or justify general precision claims. Wider DeepSeek/GLM/other-model work and
independently reviewed representative documents remain separate follow-ups.

Official model and pricing documentation checked on 2026-09-09:
[Luna](https://developers.openai.com/api/docs/models/gpt-5.6-luna),
[Astra](https://developers.openai.com/api/docs/models/gpt-6-astra),
[standard pricing](https://developers.openai.com/api/docs/pricing).
