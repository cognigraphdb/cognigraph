# GLM Flash versus Luna — captured independent-label results

> Public reading copy; original research captures and scripts are private.
> Original path: `fixtures/semantic-neurons/glm-luna-semeval-2026-09-09/results.md`.
> Original SHA-256: `58c72d6810889c8752b76014ac9c4b26623b61aa306b68c0a24624519dd67896` (7962 bytes).
> Navigation changed; dated results and limitations retain their original scope.


GLM Flash leads Luna on this supplied-pair benchmark; the paired accuracy interval stays above zero.
Keep Luna as the production baseline pending representative document qualification. DeepSeek V4 Flash is retired.

GLM is the stronger candidate for the next domain trial. Its remaining restraint errors matter: it asserted a relation on 113 of 220 Other observations; Luna did so on 175. After gating, those counts were still 88 and 131. These counts concern 110 distinct negative sentences repeated twice. GLM also took longer per batch. The result supports candidate prioritization, not unattended fact acceptance.

The trial uses 600 distinct SemEval-2010 sentences with published human labels: 490 positive nominal pairs and 110 Other pairs. Two repetitions produce 1,200 observations per model, not 1,200 independent examples. The [protocol](../../../evidence/research-glm-luna-semeval-2026-09-09.md#artifact-374c44e808cb051c0fd6), [source and limits](README.md), [full results](../../../evidence/research-glm-luna-semeval-2026-09-09.md#artifact-3bb9a30ac92c7466b0e2), [data-quality notebook](../../../evidence/research-glm-luna-semeval-2026-09-09.md#artifact-fa75cdbc5a5ee50af3da) and [verification](../../../evidence/research-glm-luna-semeval-2026-09-09.md#artifact-bee453befc9233f206e7) make the comparison reproducible.

| Model/configuration | Raw accuracy, run 1 / run 2 | Pooled nine-relation macro F1 | False assertions on Other, run 1 / run 2 | Mean / p95 batch HTTP | Known measured token cost | Known usage at regular prices |
|---|---:|---:|---:|---:|---:|---:|
| gpt-5.6-luna (none) | 42.83% / 42.50% | 46.48% | 90 / 85 of 110 | 5.372s / 7.325s | $0.085574 | $0.085574 |
| glm-5.3-flash (low) | 65.17% / 66.33% | 71.24% | 56 / 57 of 110 | 10.211s / 22.678s | $0.029846 | $0.059692 |

GLM minus Luna raw accuracy: **+23.08 percentage points**, paired 95% percentile interval **[+17.67, +28.75]**. Ten thousand bootstrap samples resample the 50 paired batches while carrying both repetitions and both models together. GLM alone was correct on 360 observations; Luna alone on 83. This is sample uncertainty, not a bound on unseen domains or pretraining contamination.

| Model | Raw correct / 1,200 | Direction errors | Raw repeat disagreements / 600 | Accepted accuracy | Accepted macro F1 | Nominations / accepted occurrences |
|---|---:|---:|---:|---:|---:|---:|
| gpt-5.6-luna | 512 | 136 | 181 | 44.58% | 46.46% | 975 / 868 |
| glm-5.3-flash | 789 | 44 | 186 | 64.50% | 69.40% | 921 / 837 |

After gates, GLM minus Luna accuracy is +19.92 points, interval [+14.67, +25.33]. The finite vocabulary policy rejected 34 of 490 correct relations in the gold-proposal control, leaving 94.33% overall accuracy for that exact full-excerpt evidence form. This is a gold-proposal reference, not a proven maximum over every possible evidence quote. These losses are a separate policy limitation; no vocabulary was tuned on measured output. Evidence-span validation does not establish relation correctness.

Three [illustrative errors](../../../evidence/research-glm-luna-semeval-2026-09-09.md#artifact-8915f6c7c0b67b927432) retain the first run-one example in each explicitly named category: GLM-only correct, both wrong, and a GLM false assertion on Other. They are explanatory examples, not a new scoring subset or relabelled gold.

## Relation breakdown

| Relation | Unique gold pairs | Luna raw F1 | GLM Flash raw F1 |
|---|---:|---:|---:|
| Cause-Effect | 69 | 45.65% | 79.85% |
| Instrument-Agency | 40 | 37.04% | 66.67% |
| Product-Producer | 53 | 41.30% | 73.20% |
| Content-Container | 45 | 60.09% | 83.67% |
| Entity-Origin | 41 | 33.10% | 58.75% |
| Entity-Destination | 62 | 60.99% | 76.47% |
| Component-Whole | 75 | 58.22% | 71.83% |
| Member-Collection | 47 | 28.57% | 53.63% |
| Message-Topic | 58 | 53.33% | 77.12% |

## Operational evidence and cost

Recorded execution failures: luna-json run 1 batch 23: recorder HTTP 200, finish reason content_filter, server HTTP 500; glm-flash run 1 batch 29: recorder HTTP 200, finish reason stop, server HTTP 500; luna-json run 1 batch 42: recorder HTTP 200, finish reason content_filter, server HTTP 500; glm-flash run 2 batch 12: recorder HTTP 502, finish reason None, server HTTP 500; luna-json run 2 batch 23: recorder HTTP 200, finish reason content_filter, server HTTP 500; luna-json run 2 batch 42: recorder HTTP 200, finish reason content_filter, server HTTP 500. See the [failure ledger](../../../evidence/research-glm-luna-semeval-2026-09-09.md#artifact-37cb9caa0d0f0f3ece71). Luna had four content-filtered batches; GLM had one schema-invalid response missing target_type and one transport timeout. CogniGraph committed no facts for any failed batch.

A descriptive [sensitivity check](../../../evidence/research-glm-luna-semeval-2026-09-09.md#artifact-838559ed3f477aeef0ef), added after observing an execution failure, removes all 48 affected sentences from both models and both repetitions. On the remaining 552 matched sentences, raw accuracy is gpt-5.6-luna 44.47%; glm-5.3-flash 66.67%. This is not the preregistered primary endpoint and does not replace the full-denominator results above.

The completed schedule contains 202 calls: two train-only preflights and 200 measured requests. Success counts: gpt-5.6-luna 96/100 measured calls; glm-5.3-flash 98/100 measured calls. Failed or malformed cases count as FAILED, never successful abstention. No per-call retry or completion repair was used.

An earlier attempt stopped after five provider calls. Four completed observations were captured; the fifth was a GLM transport timeout after 110 seconds, and a recorder bug then lost the server HTTP observation while querying absent collections. The untouched [first attempt](../../../evidence/research-glm-luna-semeval-2026-09-09.md#artifact-b735778122c5f09facc5), original protocol, runner and progress log are retained. The recorder was corrected to persist HTTP before store queries. The full trial was restarted once with identical data, prompts, models and scoring; no quality outputs were inspected or used for tuning before restart. The amendment is recorded in the current protocol.

Known token estimates: completed schedule **$0.116577** including its preflights; stopped attempt **$0.002656**; combined known usage **$0.119233**. **2 call(s) have unknown usage**, so the combined amount is incomplete, not an exact total or evidence those calls were free. The conservative maximum reservation for both schedules was $1.786226, below the $5 cap.

GLM promotional pricing expires September 9, 2026 at 16:00 UTC. The regular-price column recalculates identical observed tokens at the ordinary tariff, including reported cached input; it is not a cold-cache forecast. The stopped attempt may have warmed the preflight and earliest batch prompts. Costs come from provider token counters and published rates, not invoices, and omit taxes, cache-storage charges and unreported adjustments. See [Z.ai pricing](https://docs.z.ai/guides/overview/pricing) and [Luna pricing](https://developers.openai.com/api/docs/models/gpt-5.6-luna).

## Verification and interpretation

Six scoring/adapter tests passed. The amended harness passed 202 gold-proposal HTTP controls plus two explicit timeout/missing-collection controls; the earlier harness also completed 202 gold controls before exposing that failure path. The local scorer agrees with SemEval's original Perl scorer on gold, post-gate, failed-case controls, and all eight measured model/repetition/stage combinations. Exact replay, 1705 stored evidence-span/attribution checks, 538 pinned code files, 600 source-gold row checks and five negative integrity probes passed. The source and release binary match the earlier validated Rust checkpoint; this trial changes no Rust source.

The [original dataset paper](https://aclanthology.org/S10-1006/) describes independent human annotation and adjudication. This trial retains those labels without an LLM judge. It supplies the two nominal arguments, covers a public 2010 benchmark and uses a benchmark-specific pair instruction. It therefore supports a model comparison on this task; it does not establish entity discovery, exhaustive extraction, customer-document accuracy, judge qualification or production readiness. A new representative document holdout with independently checked, exhaustive labels remains necessary before changing the product default.
