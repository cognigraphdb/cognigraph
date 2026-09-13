# Decision: Retire CUAD and establish a captured Luna baseline

- Date: 2026-09-09
- Status: Research
- Runtime policy: Active Luna-low baseline (2026-09-09)
- Scope: Directed construction evaluation and shared Luna-low runtime; judge qualification unchanged

**Current selection — user decision, 2026-09-09:** use `gpt-5.6-luna` with
reasoning `low` as the economical baseline. Model selection is settled for
the next product-quality phase; further GLM/Terra comparisons are optional
references, not the next required work. The captured experiments below remain
historical evidence with their original settings and conclusions.

**Runtime status:** the shared Rust provider now sends `reasoning_effort: "low"`
for Luna in [`completion.rs`](../../crates/cognigraph-embeddings/src/completion.rs).
This applies to default and explicit Luna selections, including inherited
side-views. Other OpenAI model overrides keep their provider defaults. The
production JSON-schema format and schema-specific strictness are unchanged.
Formatting, strict Clippy, all 962 reported Rust tests, the release build, and
the [live runtime verification](../issues/luna-low-runtime-2026-09-09.md) passed.
The live OpenAI smoke sent the actual production request bytes unchanged, with
`json_schema`, `strict: true`, and `reasoning_effort: "low"`, for both directed
construction and inherited side-views. This decision does not qualify Luna as
an automatic judge or establish document-extraction accuracy.

**Reproducibility checkpoint:** local commit `8b8cceb` preserves the completed
CG-26 refactor and all four frozen benchmark packages before runtime adoption.
Their source and release-binary hashes refer to that earlier state. Replay them
against the checkpoint and matching binary; do not rewrite their pins to match
the changed provider.

The user retired CUAD from active evaluation and selected Luna as the ongoing
economical baseline, with a bounded look at a stronger model. The existing
[CUAD recovery](../issues/cuad-recovery-2026-09-09.md) preserves corrected
historical diagnostics but cannot reconstruct missing execution evidence.
[CG-25](../issues/CG-25.md) is therefore **Closed without change** on that
remaining recovery scope, rather than marked reproduced or resolved.

## Evaluation decision

Start with the [captured synthetic baseline](../research/experiments/luna-baseline-2026-09-09/README.md):
48 fictional excerpts, four relations, 32 gold triples, 20 negative excerpts,
and two runs per model. Freeze the corpus, labels, taxonomy, prompt/schema,
source and binary hashes, generation settings, and scoring policy before
execution. Preserve all provider attempts and server results, including rejected
proposals and full accepted occurrences. Replaying stored output must reproduce
the counts; repeating model generation is allowed to differ.

The initial experiment used `gpt-5.6-luna` with reasoning `none` and
`gpt-6-astra` with explicit reasoning `medium` as a reference configuration.
The latter is a capability comparison candidate, not presumed superior on
CogniGraph data. Official documentation identifies
[Astra](https://developers.openai.com/api/docs/models/gpt-6-astra) as OpenAI's
most capable model and [Luna](https://developers.openai.com/api/docs/models/gpt-5.6-luna)
as intended for cost-sensitive workloads. On 2026-09-09,
[standard short-context prices](https://developers.openai.com/api/docs/pricing)
per million tokens are $0.20 input/$1.20 output for Luna and $10/$50 for Astra.
The harness admits requests within a conservative $5 token-cost reservation.

Scoring uses exact normalized, directed triples within a chunk, with consistent
gold-triple false-negative counts. Inspect pre-gate and post-gate results,
false positives on negative cases, source offsets, schema failures, latency,
and cost. Model and reasoning effort both differ; this is a comparison of
configurations rather than an isolated test of model weights.

## Limits and next qualification boundary

The corpus and labels were authored together by Codex, without independent
annotation. Shared templates and tiny excerpts make this a regression and
capture baseline, not a customer accuracy estimate. Repeated observations
measure stability on the same cases, not additional independent samples.

Keep Luna as the default pending stronger evidence. Expand next with
representative documents, independently checked gold labels, ambiguous
relations, longer contexts, and counterexamples. Keep construction, retrieval
utility, and automatic judge qualification distinct. DeepSeek, GLM and other
models remain recorded follow-up work; the original ticket-list prerequisite
is now satisfied, but this bounded two-model experiment does not perform that
broader comparison.

The historical CUAD package and already qualified publication remain historical
evidence. This synthetic result neither replaces their labels nor restores
their missing trace. No new paper-quality claims follow from it.

## Observed outcome

All sixteen real provider/server requests completed without errors or retries.
Luna scored TP/FP/FN 27/1/5 and 26/1/6; Astra scored 32/0/0 in both runs.
Both abstained on all 20 negative excerpts. Luna reversed one passive dependency
and missed four facts following distractor sentences in both runs; Astra
recovered them. All 119 accepted occurrences passed evidence-byte and attribution
checks. Gates rejected none of the live nominations, including the wrong
direction, so the gates are not a semantic truth guarantee.

Estimated costs were $0.004202 for Luna and $0.203370 for Astra; mean batch
latencies were 3.725 s and 6.734 s. Astra's benefit on this sample supports its
use as a reference, with Luna retained as the economical default. Six scorer
tests, sixteen synthetic control requests, a rejection/duplicate capture
control, exact score replay, integrity probes, and navigation checks passed.
The real release binary and all Rust sources match the already validated
CG-26 state; this evaluation changed no Rust behavior. See the
[full result and limitations](../research/experiments/luna-baseline-2026-09-09/results.md).

## Cross-provider extension — 2026-09-09

The user requested DeepSeek V4 Pro/Flash and Z.ai GLM-5.3/Flash, identifying
`DEEPSEEK_API_KEY` and `ZHIPU_API_KEY` in the existing local environment file.
The [separate frozen comparison](../research/experiments/cross-provider-baseline-2026-09-09/README.md)
uses those official providers through a loopback evaluation adapter. The
original Luna/Astra package, Rust sources, binary, and production defaults
remain unchanged.

Because these APIs document JSON-object mode, all four new candidates and a
fresh Luna reference receive that format and an identical schema appendix to
the original system prompt. Excerpts, taxonomy, labels, matching policy, and
writer/gates are unchanged. GLM-5.3/Flash require thinking; low effort is
explicit for both GLM and DeepSeek, with Luna retaining none. These are
configuration comparisons, not equivalent-compute experiments.

The protocol freezes two repetitions per arm and one separate transport
preflight, a $5 reservation budget, failure retention without retries, and
provider-specific time-dependent pricing. DeepSeek peak/off-peak windows and
GLM Flash's promotion expiry at 2026-09-09 16:00 UTC must remain visible in any
cost comparison. An incomplete arm receives no full-corpus accuracy score.
The [completed comparison](../research/experiments/cross-provider-baseline-2026-09-09/results.md)
made 38 provider calls: four complete arms, five successful preflights, and a
retained DeepSeek Flash failure on its first measured batch. Luna scored
31/0/1 and 31/1/1 TP/FP/FN; DeepSeek Pro, GLM Flash, and GLM-5.3 each scored
32/0/0 twice. Flash used 3,586 reasoning tokens within its 4,096-token allowance
and produced truncated JSON, which the server rejected. It is unranked on
full-corpus quality; no repair or retry was performed.

GLM Flash averaged 4.165 seconds per batch versus Luna's 3.710, with estimated
measured cost $0.00123481 promotional or $0.00246962 at regular rates, versus
$0.00467520 for Luna. DeepSeek Pro and GLM-5.3 were slower and more expensive
on this sample. Total estimated cost including failures/preflights was
$0.075562071. Six tests, 45 synthetic controls, exact offline replay, 255
accepted occurrence checks, integrity probes, source/binary and preservation
checks passed. No Rust behavior or production default changed.

**Current candidate policy (user decision, 2026-09-09):** drop DeepSeek V4
Flash entirely from future candidate runs. Its failed historical capture remains
intact; no thinking or output-capacity follow-up is planned. Evaluation covers
GLM-5.3-Flash and Luna, with the Terra low reference documented below. Broader representative-data and judge qualification
remain separate.

## Independently labelled supplied-pair trial — 2026-09-09

The user authorized a larger GLM Flash–Luna comparison if the data was suitable.
[SemEval-2010 Task 8](https://aclanthology.org/S10-1006/) supplies published human
annotations, with independent annotators and disagreement resolution. The data
release declares CC BY 3.0; attribution and exact mirrored source bytes are
retained in the [trial package](../research/experiments/glm-luna-semeval-2026-09-09/README.md).
This is an independent public-label benchmark, not a newly commissioned human
review or a representative customer-document evaluation.

The frozen protocol selects 600 distinct sentences after label-independent
checks for duplicates, train/test text overlap and identical nominal strings.
There are 490 positive pairs across nine relations and 110 Other cases. Both
models receive only text, candidate nominal pairs and the shared task/schema;
no gold labels or source annotation comments enter API requests. Two repetitions
use 12-case batches, alternating model order, 8,192 output tokens, a $5 request
reservation cap and no retries. Luna uses reasoning none; GLM uses thinking low.
Raw pair classification is primary; the unchanged Rust writer and finite frozen
vocabulary gates are a separate diagnostic. Scores include direction, restraint,
repeat stability and paired uncertainty.

**Captured outcome:** the [results](../research/experiments/glm-luna-semeval-2026-09-09/results.md)
show GLM Flash at 65.75% pooled raw accuracy against Luna's 42.67%, a +23.08
percentage-point lead with paired 95% interval [+17.67, +28.75]. Nine-relation
macro F1 was 71.24% versus 46.48%. On 110 Other sentences repeated twice,
GLM made 113 false assertions and Luna 175; the gates reduced these to 88
and 131. Accepted accuracy was 64.50% and 44.58%, respectively. These restraint
errors prevent treating the synthetic perfect scores as general extraction
accuracy. GLM is the stronger candidate for a representative domain trial;
Luna remains the product baseline, with no native GLM integration or judge
qualification implied.

Mean 12-case HTTP latency was 10.211 seconds for GLM and 5.372 for Luna.
Known measured token estimates were $0.029846 for GLM ($0.059692 at regular
rates) and $0.085574 for Luna. GLM's measured timeout has unknown usage, so
its cost estimate is incomplete. The completed schedule had 202 requests:
Luna completed 96/100 measured calls, with four content-filtered batches;
GLM completed 98/100, with one missing `target_type` response and one timeout.
All six failed batches committed no facts. A descriptive sensitivity check
removing the 48 affected sentences symmetrically still gives GLM 66.67%
versus Luna 44.47% on 552 matched sentences; the preregistered primary score
retains every failure.

An earlier five-call attempt stopped when a GLM timeout exposed a recorder
bug querying absent collections. Its original protocol, runner, captures and
lost-observation limitation are retained. The recorder was corrected and the
full schedule restarted once with unchanged data, prompts, models and scoring;
no quality output was used to tune the restart. Combined known usage is
estimated at $0.119233, with two timeout calls of unknown usage. The total
scheduled reservation was below $1.79, within the $5 cap.

Six scoring/adapter tests, 202 amended gold-proposal HTTP controls, two explicit
failure controls, exact replay, eight official-scorer comparisons, 1,705 accepted
evidence checks, 600 source-gold checks and five negative integrity probes
passed. The earlier runner also passed 202 gold controls before exposing its
failure-capture defect. All 538 pinned source/harness files and the release
binary matched their recorded hashes. Historical benchmark packages, user-owned
files and `.env` were preserved. This trial edits no Rust source.

**Next evaluation boundary:** use representative domain documents with
independently checked, exhaustive facts and near-miss negatives. Freeze the
scope and restraint policy before further candidate comparisons.
SemEval supplies common-noun pairs, a public 2010 corpus and a benchmark-specific
pair instruction; it does not establish entity discovery, exhaustive document
extraction or production accuracy.

## Low-effort extension — 2026-09-09

The user authorized `gpt-5.6-terra` low and a `gpt-5.6-luna` low control on the
same 600 sentences and two repetitions. The [separate package](../research/experiments/terra-luna-low-semeval-2026-09-09/README.md)
preserves the prior trial. Corpus bytes, human labels, supplied pairs, prompts,
JSON mode, 8,192-token cap, taxonomy gates and release binary are identical.
The primary Terra low–Luna low contrast interleaves calls; contrasts to the
earlier Luna none and GLM Flash low captures are historical, with potentially
different serving and cache conditions. No prompts, labels or scoring rules
were tuned to the new output.

The [verified results](../research/experiments/terra-luna-low-semeval-2026-09-09/results.md)
give Terra low **72.42%** pooled raw accuracy and Luna low **70.08%**. Terra's
advantage is **+2.33 percentage points**, paired 95% interval **[+0.42, +4.25]**.
Macro F1 is 77.96% and 76.32%, respectively. Terra costs about 10.4 times as
much in this trial and averages 17.340 seconds per batch versus Luna low's
12.180 seconds. That measured margin supports retaining Terra as a stronger
reference; it does not justify making it the economical baseline.

Luna low is the main practical finding: **+27.42 points** over historical Luna
none, interval **[+22.75, +32.50]**. Direction errors fall from 136 to **1** and
repeat disagreements from 181 to **71** across 600 repeated sentences. Terra
has five direction errors and 62 disagreements. Luna low's estimate is about
1.74 times the corrected Luna none cost, with 2.27 times the HTTP latency.
Against historical GLM Flash, Luna low's observed +4.33-point advantage has
interval **[-1.83, +10.00]**, so this trial does not establish a clear winner
between those economical candidates. Historical contrasts are exploratory;
they are not a randomized causal estimate of reasoning effort.

Restraint remains unresolved: Luna low and Terra assert relations on **120**
and **117** of 220 Other observations, versus GLM's 113. Post-gate accuracy is
68.67% and 70.75%, with 101 and 98 false assertions on Other. All four-way
matched-success results retain 552 sentences: Luna none 44.47%, GLM Flash
66.67%, Luna low 72.55%, Terra low 75.09%. This prespecified sensitivity removes
any sentence with a failed observation in any configuration or repetition;
full-denominator scores remain primary. Public-dataset and supplied-pair limits
still apply; none of these results qualifies exhaustive document extraction.

All **202 new calls** were captured, with no retry, repair or restart. Each
model completed **96/100** measured requests. Batches 23 and 42 were content
filtered for both models in both repetitions; each failed request stored no
facts. The [failure ledger](../evidence/research-terra-luna-low-semeval-2026-09-09.md#artifact-5202caa13afa1474e95c)
retains the raw outcome. New measured costs at standard tariffs are
**$0.153621–$0.153868** for Luna low and **$1.593414–$1.595879** for Terra.
Including preflights, the schedule estimate is **$1.764058–$1.766769**. All
requests report usage; eight filtered calls omit cache-write counters, which
are bounded rather than treated as zero. The conservative reservation is
$13.587759, below the frozen $20 cap. These figures are token estimates, not
invoices.

The [new accounting](../evidence/research-terra-luna-low-semeval-2026-09-09.md#artifact-1a82737c7ae88f508944)
adds OpenAI's cache-write premium and bounds missing cache counters. The earlier
helper omitted that premium. Historical Luna none's corrected measured estimate
is **$0.088354–$0.088601**, replacing $0.085574 for comparison purposes here;
the historical files and their originally reported estimates remain intact.
GLM's regular-tariff estimate remains $0.059692 plus one call of unknown usage.
See [official pricing](https://developers.openai.com/api/docs/pricing) and
[cache accounting](https://developers.openai.com/api/docs/guides/prompt-caching).

Ten unit tests, 202 gold-proposal real-server controls, 4,000 cost-bound checks,
exact new/historical replay, 404 identical-prompt comparisons, eight new official
Perl scorer comparisons, 1,772 accepted evidence-span/attribution checks and five
negative integrity probes passed. All 545 pinned source/harness files, the
original release binary and all 248 inherited package files match their hashes.
No Rust source, product default, provider integration or judge qualification
changed.

**Recommendation at the trial's conclusion, before the user selected Luna low:** representative documents with independently
checked, exhaustive labels and near-miss negatives, comparing **Luna low and
GLM Flash**, with Terra low as the stronger reference. Keep Luna as the
economical product baseline while qualifying effort and restraint on the target
domain. DeepSeek V4 Flash remains retired.

## Document development trial — 2026-09-09

The shared runtime adoption is complete. The user chose general factual
materials for the [first document trial](../research/experiments/luna-documents-2026-09-09/results.md).
It freezes 40 development and 80 unrun holdout documents from the authors'
[human-reviewed Re-DocRED release](https://github.com/tonytan48/Re-DocRED), with
twelve predeclared relations. All benchmark sentences are retained; these short
documents do not represent full Wikipedia articles or long customer reports.
No supplied entity list, pair hints or gold evidence enters Luna's prompts.

All forty live calls completed without retries. Luna proposed 55 relations and
the writer retained 32; the published-reference matches are 21 and 16,
respectively, against 683 reference triples. Estimated token cost is $0.0201583.
These low reference scores expose a large document-task gap, but source omissions,
entity granularity and inferred geographic relations make them unsuitable as a
customer accuracy estimate. The desired exhaustive human reference and verified
near-miss negatives remain a qualification prerequisite, not a completed claim.

The scorer's initial assumption that all chunk citations were valid failed on
five nominations. The preserved V2 amendment counts them as unmatched without
repairing citations, changing gold or rerunning generation. Ten tests, forty
real-server controls, complete score/source replay, prompt parity and integrity
probes passed. A separate minimal release HTTP reproduction confirmed
[CG-39](../issues/CG-39.md), which admits `Ann` from `Joanne`, and
[CG-40](../issues/CG-40.md), which leaves request-bound chunk IDs unconstrained
in the completion schema. Runtime code was unchanged during this experiment.

This original trial was committed locally as `a0e4b24` before the fixes below.

## Directed contract candidate — 2026-09-09

[CG-39](../issues/CG-39.md) and [CG-40](../issues/CG-40.md) are now resolved.
[Policy v2](decision_directed_extraction_contracts.md) adds Unicode endpoint token
boundaries and exact request-derived chunk-ID/relation enums. Formatting, strict
Clippy, all 967 reported Rust tests, the release build and 34 OpenAI/Gemini HTTP
cases passed. Eight Arango test entries returned early without the exported
password; the live contract checks used disposable Native databases.

The [frozen candidate](../research/experiments/luna-directed-v2-2026-09-09/results.md)
first replayed the original 55 proposals without model calls. It removed one
invalid endpoint occurrence and retained all 16 stored reference matches.
A fresh 40-document Luna-low pass produced zero invalid chunk citations, versus
five previously, and stored 35 occurrences with 20 reference matches (57.14%
reference precision, 2.93% reference recall). Estimated token cost was
$0.0194147. The prompt, model, taxonomy, cohort and scorer matching policy were
unchanged; provider schemas added only the exact enums. Capture reconstruction
and five tamper probes passed.

**Next:** complete independent reference/evidence-policy review before making
precision/abstention claims or running the holdout. The fresh pass is one
development observation with incomplete source labels, not proof of an accuracy
gain attributable to schema constraints alone. Keep Luna low as the model
baseline, preserve all captured experiments and leave the holdout unrun while
development decisions change.
