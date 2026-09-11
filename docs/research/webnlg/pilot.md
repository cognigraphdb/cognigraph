# WebNLG Semantic Neurons pilot

## Purpose

WebNLG is a completed non-clinical Semantic Neurons pilot, with deterministic
scoring recorded on July 17, 2026 and recall experiments on July 20. Each English example
contains natural-language text and the DBpedia triples from which it was
lexicalized. Those triples provide a mechanical external reference, so this
pilot can measure relation construction without clinical adjudication.

This does **not** replace DailyMed. It adds a complementary, automatically
scorable benchmark. WebNLG is controlled data-to-text material, not evidence
that recall transfers to arbitrary documents, and the supplied triples should
not be described as independently expert-reviewed facts.

## Frozen source

- Dataset: [`GEM/web_nlg`](https://huggingface.co/datasets/GEM/web_nlg)
- Configuration: `en`
- Hub revision: `1d41f28b06efb62d39cc83a0c00b231e825720fe`
- License declared by the dataset: `CC-BY-NC-4.0`
- Included splits: `train`, `validation`, `test`
- Excluded derived challenge splits: `challenge_train_sample`,
  `challenge_validation_sample`, `challenge_test_scramble`, and
  `challenge_test_numbers`

The challenge splits overlap or transform the canonical material and are kept
out to avoid accidental duplication. The dataset is public; `HF_TOKEN` is not
required for preparation.

## Prepare and verify

```bash
cargo run --release -p cognigraph-construct --bin webnlg-pilot
target/release/webnlg-pilot --verify-only
```

The default output is the ignored directory `data/webnlg-pilot/`. Downloads
are page-cached and resumable. `--refresh` deliberately replaces the cache;
the command refuses to run if the current Hub revision differs from the frozen
revision.

The output keeps construction input and evaluation truth physically separate:

```text
data/webnlg-pilot/
├── raw/{split}/page-*.json       # pinned Dataset Viewer responses
├── documents/{split}.jsonl       # text visible to construction
├── oracle/{split}.jsonl          # triples visible only to evaluation
├── manifest-{split}.jsonl        # row alignment and text digests
├── request.json                  # frozen acquisition request
└── run.json                      # counts and per-artifact SHA-256 digests
```

This is an **entities-provided relation-construction** benchmark: every scored
lane receives entity surfaces from that document's oracle. The generic and
neuron-authored constructors do not receive its predicate labels or assembled
triples; `oracle-diagnostic` additionally receives the document's predicates.
Rule authoring is explicitly supervised:
`webnlg-mine-rules` consumes the train oracle (and, for the final frozen
artifact, train+validation) to mine candidate relation templates. That makes
this a supervised template-mining benchmark, not a text-only proposal run. The
test oracle never entered mining or tuning. The later LLM experiment chose
its top-25 target predicates using validation-oracle frequencies, then prompted
with train examples. Validation therefore informed selection as well as tuning;
its later results are development-set measurements, not a new untouched holdout.
Entity and predicate strings are
preserved exactly as supplied; normalization belongs in the versioned scoring
policy, not data preparation.

## Prepared baseline (2026-07-16)

| split | documents | triples | predicates | categories | document SHA-256 | oracle SHA-256 |
|---|---:|---:|---:|---:|---|---|
| train | 35,426 | 104,799 | 372 | 16 | `aeff78d125926c72ec3fb568076609df684db730e5545c81773468bb54a9ecac` | `7c4c1a3ee7a0af7d5afa81365d8d0298ca342723de2c9838fd37e91a3f008213` |
| validation | 1,667 | 4,841 | 290 | 16 | `06539b5b9fcd0c886803862e581e6816202003c696417290c921b1322fdc6ad6` | `042257ae01514a74736539c3f2438120ea06b3f2961d5c21e9a7b147483f6e62` |
| test | 1,779 | 5,639 | 220 | 19 | `57340a06a3c1dad5dea136a006dd2e3313dd44a7df109b0771b5f15f56c16fb4` | `3bfc9dfdb68f63467e50fa227ac7f53ab880d2020b0c7309bec6625b8c223368` |
| **total** | **38,872** | **115,279** | **411 unique overall** | **19 unique overall** | | |

The July 16 local output occupied 70 MB. An immediate cached rebuild produced
the same digest over every file except `run.json`; that file intentionally
records the new `generated_at` timestamp. Offline verification reparses every
cached source page and checks raw, document, oracle, and manifest digests,
sequential row alignment, IDs, text lengths, triple counts, split summaries,
and aggregate totals.

## Scoring policy and the neuron-authored ruleset

The scoring policy is frozen in
[`decision_webnlg_scoring.md`](../../decisions/decision_webnlg_scoring.md) (W1–W7) and
implemented by the pure `cognigraph_construct::webnlg` scorer: three lanes
(`generic`, `neuron-authored`, `oracle-diagnostic`), exact-triple recall +
precision-as-restraint, over provided entities.

Here “precision-as-restraint” means exact agreement with the supplied
generating triples. It is not an independent truth assessment and does not
replace a forbidden-fact/hard-negative restraint suite.

The **`neuron-authored` candidate ruleset** is mined deterministically from the
authoring corpus (train+validation only — never `test`, W7) by
`webnlg-mine-rules`:

```bash
cargo run --release -p cognigraph-construct --bin webnlg-mine-rules -- \
  --out /tmp/webnlg-mined-recheck.json
```

For each single-triple training document where both entity surfaces appear
verbatim, the sentence is templatized (subject → `{source}`, object →
`{target}`); templates are frequency-ranked per predicate, and cross-predicate
collisions are disambiguated (each template assigned to the predicate it most
frequently lexicalizes — see the disambiguation addendum in the decision doc).
This is an offline analog of the "propose" step; the committed
[mined artifact](../../../crates/cognigraph-construct/fixtures/webnlg/neuron-ruleset.mined.json)
(`neuron-authored-mined-v3`) is a **reviewable candidate set** — a human prunes
it further before it can be treated as reviewed. The frozen scoring artifact
did not pass through the runtime `Neuron` proposal/review/accept lifecycle;
`neuron-authored` is the established lane name, not a claim that lifecycle was
exercised. Mining is deterministic (byte-reproducible, digest-recorded).

Baseline mine (2026-07-17, defaults `min_count=1`, `max_per_predicate=20`,
disambiguation on): **303 predicates / 1,842 templates** from 37,093 authoring
documents.

### Held-out validation scoring (`webnlg-score`)

`webnlg-score` mines the neuron ruleset from `train` only and scores it on
`validation` (`test` untouched). Validation was used for the tuning described
below; these July 17 development-set scores are not the frozen test result.
Per-lane, exact-triple recall / precision:

| lane | recall | precision |
|---|---:|---:|
| `generic` (4 predicates) | 1.1% | 58.0% |
| `neuron-authored` | **12.4%** | **76.6%** |
| `oracle-diagnostic` (ceiling) | 12.4% | 90.1% |

Two data-driven tuning steps got here (both A/B'd on held-out validation; see the
disambiguation and recall addenda in the decision doc): **disambiguation** of
cross-predicate template collisions (46% → 76% precision, false edges 559 → 139),
then **recall tuning** (keep long-tail phrasings: 8.9% → 12.4% recall at held
precision). Neuron recall equals the oracle-diagnostic ceiling, so it is capped by
trigger rigidity — exact-phrase templates only fire on phrasings seen in
training, not by predicate confusion.

### One-shot `test` result (`webnlg-test`)

The held-out `test` oracle was opened **once** against the frozen
`neuron-authored-mined-v3` ruleset (test never fed mining; validation informed
only the hyperparameters). This remains the July 17 frozen test result — see the
[Outcome](../../decisions/decision_webnlg_scoring.md#outcome-2026-07-17-one-shot-test-result).

| lane | recall | precision |
|---|---:|---:|
| `generic` | 0.4% | 52.6% |
| `neuron-authored` (frozen) | **7.0%** | **68.8%** |
| `oracle-diagnostic` (ceiling) | 7.0% | 90.1% |

Both are lower than the validation estimates, as expected for a truly-unseen
split — that gap is why `test` is held out. Governance still beats naive
(precision 68.8% vs generic 52.6%), and recall (7.0%) equals the ceiling,
confirming the cap is trigger rigidity, not predicate confusion.

## Completed recall experiments (2026-07-20)

The [later decision addenda](../../decisions/decision_webnlg_scoring.md#addendum-2026-07-20-the-recall-lever--fuzzy-matching-is-the-wrong-one)
record the following validation-set experiments. None reopens or replaces the
frozen test result above. Exact matching remains the construction default.

| Experiment | Recall | Precision | Recorded outcome |
|---|---:|---:|---|
| Exact train-mined baseline | 12.4% | 76.6% | 601 correct / 785 constructed |
| Fuzzy, `max_gap=0` | 14.5% | 40.9% | Rejected: precision loss for a small recall gain |
| Fuzzy, `max_gap=1` / `2` / `3` | 18.2% / 19.3% / 20.1% | 28.8% / 15.4% / 13.1% | Rejected across the sweep |
| Fuzzy, unbounded (historical addendum) | 28.5% | 8.3% | Rejected; current CLI sweeps gaps 0–3 only |
| Exact `CorpusProposer`, including attributable multi-triple examples | 12.2% | 78.2% | More corpus did not improve recall |
| Mined baseline + raw LLM proposals | 13.0% | 53.2% | 630 / 1,185; more correct triples with substantial precision loss |
| LLM proposals + automatic disambiguation | 12.6% | 71.7% | 612 / 854; residual precision loss |
| Pruned LLM candidates + automatic disambiguation | 12.6% | 76.3% | 609 correct; eight more than baseline, precision lower by 0.3 percentage points |

The live proposal run used `gpt-5.4-mini` on 25 predicates selected by validation
frequency, with examples from train. It produced a retained non-deterministic
snapshot, not a new frozen test ruleset. The follow-up pruning removed 46
template entries, leaving 202. The raw file has 248 entries, including one
duplicate within a predicate; the pruned file has 202 distinct entries.

The ledger calls the last step “human prune” and “full loop.” In this experiment
those labels describe an offline candidate-editing and scoring sequence. The
JSON files do not contain reviewer identities or attestations, and the CLI's
`disambiguated` result performs automatic collision handling. None of these
experiments exercised the runtime `Neuron` proposal/review/accept lifecycle or
qualified a production judge. The small recall gain and slight precision loss
are a measured tradeoff, not a strict improvement on both metrics.

## Retained artifacts and offline replay

The repository paths are under `crates/cognigraph-construct/fixtures/webnlg/`:

| Artifact | Role |
|---|---|
| [neuron-ruleset.mined.json](../../../crates/cognigraph-construct/fixtures/webnlg/neuron-ruleset.mined.json) | Frozen `neuron-authored-mined-v3`: 303 predicates, 1,842 templates, mined from train+validation |
| [llm-proposals.json](../../../crates/cognigraph-construct/fixtures/webnlg/llm-proposals.json) | Raw July 20 candidate snapshot: 25 predicates, 248 entries |
| [llm-proposals-pruned.json](../../../crates/cognigraph-construct/fixtures/webnlg/llm-proposals-pruned.json) | Pruned candidate snapshot: 25 predicates, 202 entries; not a replacement for the frozen test artifact |

With the prepared corpus already present, replay its verification and the
validation experiments without making model calls or changing the retained
artifacts:

```bash
cargo build --release -p cognigraph-construct --bin webnlg-pilot \
  --bin webnlg-mine-rules --bin webnlg-score --bin webnlg-llm-run
python3 docs/issues/evidence/webnlg-status-replay.py --output /tmp/cg27-replay.json
```

The [replay harness](../../issues/evidence/webnlg-status-replay.py) verifies the cached
corpus, reproduces the mined artifact into a temporary file, and invokes
`webnlg-score` plus `webnlg-llm-run --load` for both stored proposal files.
Scoring gets a separate temporary root containing only train and validation;
the test files are absent. `--load` bypasses provider construction and does not
write proposals. Without `--load`, `webnlg-llm-run` performs new model calls and
its default output can overwrite the raw candidate file; that is a separate
experiment, not this replay procedure.

## Current status (2026-09-09, CG-27)

Data preparation, frozen deterministic scoring, fuzzy rejection, expanded-corpus
mining, live LLM proposals, and offline candidate pruning are completed and
recorded. The [CG-27 reconciliation](../../issues/webnlg-status-2026-09-09.md) separates
historical observations from current offline replay evidence. The test result
remains 7.0% recall / 68.8% precision for its frozen entity-provided ruleset.

Open work is entity-discovered scoring, hard-negative restraint, full runtime
governed-review evaluation, and generalization on a newly reserved evaluation
set. The consumed test oracle must not be reused to tune another ruleset.
