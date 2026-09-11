# Terra low and Luna low: SemEval comparison extension

This separate package adds `gpt-5.6-terra` and `gpt-5.6-luna`, both at
`reasoning_effort: low`, to the completed [GLM Flash versus Luna
trial](../glm-luna-semeval-2026-09-09/README.md). That package remains intact.
DeepSeek V4 Flash remains retired. This experiment changes no product default.

**Completed:** Terra low scored 72.42% and Luna low 70.08%. Terra's 2.33-point
gain costs about 10.4 times as much in this run. Luna low and GLM Flash remain
the useful pair for representative document qualification; both still have
substantial restraint errors. Read the [results and recommendation](results.md),
[comparison](comparison.json), [accounting](accounting.json), and
[verification](validation.json). [Package hashes](package-manifest.json) bind
the retained artifacts; [workspace checks](workspace-validation.json) record
preservation and credential scanning.

## Frozen experiment

The [protocol](protocol.json) was frozen before the new provider calls.
The [corpus](corpus.json) is byte-identical to the earlier trial: 600 distinct
SemEval-2010 Task 8 test sentences, 490 positive pairs and 110 Other pairs.
Published human labels, nine directed relations, candidate nominal pairs,
selection, prompts, taxonomy gates, JSON mode, scoring and the real release
binary are unchanged. Both repetitions use the same fifty 12-case batches;
model order alternates and reverses in repetition two. Each call uses a fresh
authenticated CogniGraph server and isolated Native database. Embeddings are
fixed loopback vectors; retrieval quality is not measured.

Two training-only preflights precede 200 measured calls. Each request has an
8,192-token output allowance and a 110-second provider timeout. No retries,
output repair, prompt tuning or replacement cases are allowed. A failed
preflight or three consecutive failures halts an arm, which cannot be ranked
as a complete trial. Failed batches score FAILED for all their cases; they
never count as successful abstention. Provider sampling and cache defaults
are retained. API keys are loaded in memory from the existing environment;
only public corpus input fields are forwarded, with no gold labels or comments.

The primary contrast is Terra low minus Luna low, using concurrent interleaved
arms. Secondary contrasts reuse the earlier Luna none and GLM Flash low
captures. Those historical comparisons have identical task inputs but were
recorded earlier: serving, timing and cache conditions are not randomized
across the four configurations. Identical effort names across providers do
not establish equal compute. Confidence intervals use 10,000 paired batch
bootstrap samples carrying both repetitions together; they do not treat
1,200 observations as 1,200 independent sentences. Multiple secondary
intervals are exploratory, without familywise correction.

Primary quality is exact raw supplied-pair accuracy. The secondary metrics
include directed nine-relation macro F1, false assertions on Other, direction
errors, repeat disagreement, operational failures, post-gate quality, latency
and token costs. A prespecified sensitivity check retains only sentences with
successful observations for all four configurations in both repetitions;
the full-denominator scores remain primary. No gold labels are altered and
no LLM judge is used.

## Source and interpretation

The unchanged [source and attribution](../glm-luna-semeval-2026-09-09/README.md#source-and-limits),
[original release notice](../glm-luna-semeval-2026-09-09/source/README.txt),
[provenance](../glm-luna-semeval-2026-09-09/source/provenance.json),
[source text](../glm-luna-semeval-2026-09-09/source/test.txt), and
[data-quality checks](../glm-luna-semeval-2026-09-09/data-quality.json)
remain in the earlier package, whose manifest hash is pinned here.
The source is CC BY 3.0, by Iris Hendrickx, Su Nam Kim, Zornitsa Kozareva,
Preslav Nakov, Diarmuid Ó Séaghdha, Sebastian Padó, Marco Pennacchiotti,
Lorenza Romano and Stan Szpakowicz. See the [dataset paper](https://aclanthology.org/S10-1006/).
The corpus adaptation removes entity markup and supplies the nominal strings.

This public 2010 dataset may have appeared in model training. Its independently
authored human labels are not newly commissioned annotation or proof against
contamination. Supplying nominal pairs and the benchmark-only pair instruction
does not measure entity discovery, exhaustive extraction, customer-document
accuracy or judge qualification. Vocabulary gates validate evidence and a
finite policy; they do not establish semantic truth. Representative documents
with independently checked exhaustive labels remain the next qualification step.

## Cost accounting

[Official OpenAI pricing](https://developers.openai.com/api/docs/pricing)
checked on September 9, 2026 lists the following standard short-context rates
in USD per million tokens:

| Model | Ordinary input | Cached input | Cache writes | Output |
|---|---:|---:|---:|---:|
| Luna | 0.20 | 0.02 | 0.25 | 1.20 |
| Terra | 2.00 | 0.20 | 2.50 | 12.00 |

Following [OpenAI's cache accounting](https://developers.openai.com/api/docs/guides/prompt-caching),
ordinary input equals total prompt tokens minus cached tokens minus cache-write
tokens. Reasoning tokens are already included in completion usage. Missing
cache counters remain unknown and yield bounded cost estimates; absent request
usage stays unknown. The historical OpenAI helper omitted the cache-write
premium, so this package supplies additive corrected estimates for comparison
without changing old captures or reports. Estimates use published tariffs and
reported counters, not billing invoices; no cache-control optimization is added.

The conservative local reservation cap is $20 for this new schedule. Every
request reserves its byte-count input bound plus 4,096 tokens at the highest
input/cache-write rate and its entire output allowance. No key, invoice, tax
or private account information belongs in the artifacts.

## Reproduction

From the repository root, use a new output directory:

```bash
python3 -B -m unittest discover -s fixtures/semantic-neurons/terra-luna-low-semeval-2026-09-09 -p test_score.py
python3 -B fixtures/semantic-neurons/terra-luna-low-semeval-2026-09-09/run.py --mode mock --output /tmp/cognigraph-terra-new-control
python3 -B fixtures/semantic-neurons/terra-luna-low-semeval-2026-09-09/evaluate.py /tmp/cognigraph-terra-new-control --output /tmp/cognigraph-terra-new-control-results.json
```

`--mode live` makes paid OpenAI requests and must use a fresh output directory.
The runner verifies pinned source and binary hashes before dispatch. The old
transport and exact scoring functions are reused; this extension adds the two
low-effort configurations and cache-write accounting only.

Offline result replay and verification make no API requests:

```bash
python3 -B fixtures/semantic-neurons/terra-luna-low-semeval-2026-09-09/verify.py --output /tmp/cognigraph-terra-reverification.json
python3 -B fixtures/semantic-neurons/terra-luna-low-semeval-2026-09-09/check_cost_bounds.py --output /tmp/cognigraph-terra-cost-reverification.json
```

The [gold-control summary](controls.json), [control validation](control-validation.json),
[verifier control](verifier-control.json), and [cost-bound controls](cost-bound-controls.json)
record the checks performed before final interpretation. The live [manifest](live/manifest.json)
and [provider calls](live/provider-calls.json) preserve all 202 requests.
