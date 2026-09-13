# Terra low and Luna low — captured SemEval results

> Public reading copy; original research captures and scripts are private.
> Original path: `fixtures/semantic-neurons/terra-luna-low-semeval-2026-09-09/results.md`.
> Original SHA-256: `b5d92aaa59b9d8445eafb0363ec92a06459ae47674dbf6c75c2163dea213a21a` (9814 bytes).
> Navigation changed; dated results and limitations retain their original scope.


**Terra low has the highest observed raw accuracy: 72.42%.** This is supplied-pair classification on a public benchmark. Production defaults remain unchanged.

The useful next comparison is **Luna low versus GLM Flash on representative documents**, with Terra low retained as a stronger reference. Terra gains 2.33 percentage points over Luna low (paired interval +0.42 to +4.25), but costs about 10.4 times as much and takes about 42% longer per batch here. That small quality margin does not justify promoting Terra to the economical baseline on this evidence.

Luna low is the main finding: it gains 27.42 points over the historical Luna none capture, reduces direction errors from 136 to 1, and reduces repeat disagreements from 181 to 71. Its measured token estimate is about 1.74 times the corrected historical Luna none estimate, with 2.27 times the HTTP latency. This historical contrast strongly motivates qualification of low effort, but is not a randomized causal experiment. Luna low leads GLM by 4.33 observed points, with an interval spanning −1.83 to +10.00; that comparison does not establish a clear winner. GLM remains cheaper on reported usage. All three candidates assert a relation on more than half of the repeated Other observations, so restraint remains a qualification blocker.

The two new configurations completed the frozen 600-case, two-repetition schedule through real release-build CogniGraph servers. Each configuration has 1,200 observations over 600 distinct sentences. [Protocol](../../../evidence/research-terra-luna-low-semeval-2026-09-09.md#artifact-1edd191d9505caf3a602), [scope and provenance](README.md), [full new scores](../../../evidence/research-terra-luna-low-semeval-2026-09-09.md#artifact-b60685f7beb93d424d7a), [four-configuration comparison](../../../evidence/research-terra-luna-low-semeval-2026-09-09.md#artifact-119d895b6744247ec708), and [verification](../../../evidence/research-terra-luna-low-semeval-2026-09-09.md#artifact-10b0f0db91571ee267c4) retain the evidence.

| Configuration | Raw accuracy run 1 / run 2 | Pooled raw accuracy | Nine-relation macro F1 | False assertions on Other / 220 | Direction errors |
|---|---:|---:|---:|---:|---:|
| Luna none (historical) | 42.83% / 42.50% | 42.67% | 46.48% | 175 | 136 |
| GLM Flash low (historical) | 65.17% / 66.33% | 65.75% | 71.24% | 113 | 44 |
| Luna low | 70.33% / 69.83% | 70.08% | 76.32% | 120 | 1 |
| Terra low | 71.50% / 73.33% | 72.42% | 77.96% | 117 | 5 |

Other counts concern 110 negative sentences repeated twice. Failed cases are always wrong for primary accuracy and are listed separately; they do not count as either correct abstentions or false assertions. Endpoint direction must match the original human label. No labels, prompts, gates or scoring rules were tuned.

## Paired accuracy contrasts

| Contrast (right minus left) | Raw difference, percentage points | Paired 95% interval | Capture relationship |
|---|---:|---:|---|
| Terra low − Luna low | +2.33 | [+0.42, +4.25] | Interleaved primary |
| Luna low − Luna none (historical) | +27.42 | [+22.75, +32.50] | Historical secondary |
| Terra low − GLM Flash low (historical) | +6.67 | [+0.33, +12.42] | Historical secondary |
| Luna low − GLM Flash low (historical) | +4.33 | [-1.83, +10.00] | Historical secondary |

Intervals use 10,000 bootstrap resamples of the 50 paired batches, keeping both models and repetitions together. Two repetitions do not double independent sample size. The primary comparison is Terra low versus Luna low. Historical contrasts are exploratory, without adjustment for multiple comparisons; capture time, serving conditions and cache history differ. They do not isolate a randomized causal effect of reasoning effort.

## Reliability, latency and costs

| Configuration | Successful measured calls / 100 | FAILED cases / 1,200 | Repeat disagreements / 600 | Mean / p95 HTTP per 12-case batch | Known measured cost at regular tariffs |
|---|---:|---:|---:|---:|---:|
| Luna none (historical) | 96 | 48 | 181 | 5.372s / 7.325s | $0.088354–$0.088601 |
| GLM Flash low (historical) | 98 | 24 | 186 | 10.211s / 22.678s | $0.059692 + 1 unknown call(s) |
| Luna low | 96 | 48 | 71 | 12.180s / 16.810s | $0.153621–$0.153868 |
| Terra low | 96 | 48 | 62 | 17.340s / 28.568s | $1.593414–$1.595879 |

The **new 202-call schedule**, including both preflights, has a known token estimate of **$1.764058–$1.766769**; 0 call(s) have no usage record. The conservative reservation was $13.587759, below the $20 cap. This is not a billing invoice. The [accounting ledger](../../../evidence/research-terra-luna-low-semeval-2026-09-09.md#artifact-1a82737c7ae88f508944) preserves raw usage counters, pricing calculations, cache uncertainty and historical corrections. Latency includes the real construct HTTP path and failed requests; it is an observed batch measurement, not a single-sentence or production throughput forecast.

New execution failures: luna-low run 1 batch 23 (server 500, recorder 200, finish content_filter); terra-low run 1 batch 23 (server 500, recorder 200, finish content_filter); terra-low run 1 batch 42 (server 500, recorder 200, finish content_filter); luna-low run 1 batch 42 (server 500, recorder 200, finish content_filter); terra-low run 2 batch 23 (server 500, recorder 200, finish content_filter); luna-low run 2 batch 23 (server 500, recorder 200, finish content_filter); luna-low run 2 batch 42 (server 500, recorder 200, finish content_filter); terra-low run 2 batch 42 (server 500, recorder 200, finish content_filter). See the [failure ledger](../../../evidence/research-terra-luna-low-semeval-2026-09-09.md#artifact-5202caa13afa1474e95c). No retry or completion repair was performed. Recorder 502, if present, denotes a local transport failure without an upstream response. The earlier GLM/Luna schedule retains its separate four Luna content-filter failures, one GLM schema failure and one GLM timeout.

OpenAI costs now include cache-write tokens at the published 1.25× ordinary-input tariff. Input, cache-read and cache-write buckets are disjoint; reasoning tokens are already included in output usage. Missing cache counters produce a bounded estimate rather than invented zero usage. The historical Luna report used an older helper that omitted the write premium; its original files remain unchanged. GLM is repriced at regular rather than temporary promotional rates, using the same observed usage; its timeout remains unknown. These are observed-cache estimates, not cold-cache forecasts. [OpenAI pricing](https://developers.openai.com/api/docs/pricing), [cache accounting](https://developers.openai.com/api/docs/guides/prompt-caching), and [frozen GLM tariff evidence](../../../evidence/research-glm-luna-semeval-2026-09-09.md#artifact-374c44e808cb051c0fd6) document the assumptions.

## Token usage

| Configuration | Known input tokens | Known output tokens, including reasoning | Known reasoning tokens | Known cached-input tokens | Known cache-write tokens |
|---|---:|---:|---:|---:|---:|
| Luna none (historical) | 118,710 | 60,210 | 0 | 57,888 | 55,604 (+ 4 unreported calls) |
| GLM Flash low (historical) | 126,213 (+ 1 unreported calls) | 83,332 (+ 1 unreported calls) | 24,924 (+ 1 unreported calls) | 7,552 (+ 1 unreported calls) | 0 (+ 100 unreported calls) |
| Luna low | 118,710 | 114,380 | 55,125 | 56,746 | 56,746 (+ 4 unreported calls) |
| Terra low | 118,710 | 119,147 | 58,756 | 56,746 | 56,746 (+ 4 unreported calls) |

These are measured-call totals only. Unreported counters are not measured zeroes. GLM cache-write counts are not exposed in these captures and are not a separate component of its frozen tariff. Reasoning tokens must not be added to output usage a second time.

## Gates and matched-success sensitivity

| Configuration | Post-gate accuracy | Post-gate macro F1 | Post-gate false assertions on Other / 220 | Accepted occurrences |
|---|---:|---:|---:|---:|
| Luna none (historical) | 44.58% | 46.46% | 131 | 868 |
| GLM Flash low (historical) | 64.50% | 69.40% | 88 | 837 |
| Luna low | 68.67% | 74.83% | 101 | 882 |
| Terra low | 70.75% | 76.84% | 98 | 890 |

The finite vocabulary gates were frozen in the previous experiment. A correct full-excerpt gold-proposal control loses 34 of 490 positive facts per repetition, leaving 94.33% overall accuracy for that evidence form. This is a control reference, not a universal maximum or proof of semantic correctness. No gate was widened after seeing results.

The prespecified four-way matched-success sensitivity removes 48 sentences with any FAILED observation, leaving 552 matched sentences in both repetitions. Raw accuracies are Luna none (historical) 44.47%; GLM Flash low (historical) 66.67%; Luna low 72.55%; Terra low 75.09%. This conditional analysis does not replace the primary results that include failures.

The [illustrative errors](../../../evidence/research-terra-luna-low-semeval-2026-09-09.md#artifact-a27cb213494663450bcd) select the first run-one case in each fixed category: Terra correct/Luna low wrong, the converse, Terra correct/GLM wrong, all four wrong on complete responses, and a Terra false assertion on Other. These are explanatory examples, not a new scoring subset or corrected labels.

## Verification and limits

Ten unit tests and 202 gold-proposal controls passed. Exact new and historical replay, all 404 outgoing prompt comparisons, 600 source-gold checks, deterministic resampling, eight new official Perl scorer comparisons, 1772 new evidence-span/attribution checks and five tamper-rejection probes passed. All 545 pinned code/harness files, the original release binary and all 248 inherited package files match their hashes. The comparison makes no Rust source changes; earlier Rust validation remains tied to that binary and source checkpoint.

Published human labels provide an independent reference, but the public 2010 corpus may have appeared in training. The task supplies two common-noun arguments and a benchmark-only pair instruction. It does not establish entity discovery, exhaustive document extraction, unseen customer-domain accuracy or judge qualification. Keep the economical Luna baseline until representative, independently checked documents establish an acceptable quality/cost tradeoff. DeepSeek V4 Flash remains retired.
