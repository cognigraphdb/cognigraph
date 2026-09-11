# Decision: WebNLG pilot scoring policy (W1–W7)

**Status:** Decided 2026-07-17 (user approved the lane structure, exact-triple
matching, entity-provided handling, and precision-as-headline). Freezes the
relation mapping and scoring semantics for the WebNLG pilot on train+validation,
before the held-out `test` oracle is opened — the gate named in
`docs/webnlg-pilot.md`. **The one-shot `test` run has now been executed — see
the July 17 Outcome section below.**

## Current reading (2026-09-09, CG-27)

The dated sections below are a chronological research ledger. Statements that
fuzzy matching or live proposals are future work describe their own checkpoint;
the July 20 addenda record their execution, including unsuccessful results.
The [current pilot guide](../research/webnlg/pilot.md) consolidates those outcomes.

The frozen July 17 test score remains **7.0% recall / 68.8% precision** for the
entity-provided, supervised mined candidate artifact. The July 20 fuzzy sweep
was rejected; expanded deterministic corpus mining did not improve recall;
live LLM proposals added correct triples but lowered precision; pruning reached
609 correct versus 601 baseline, at 76.3% versus 76.6% precision. Thus the
historical “net-positive” wording denotes a small recall gain with a slight
precision cost, not strict dominance on the precision-first metric.

The LLM run used validation-oracle frequencies to select its 25 target
predicates, then prompted from train examples. Its validation scores are
development measurements, not an untouched holdout estimate. “Human prune” and
“full loop” below refer to editing a candidate JSON artifact and rescoring it;
the retained files have no reviewer identity/attestation and do not demonstrate
the runtime `Neuron` proposal/review/accept lifecycle. Automatic disambiguation
in `webnlg-llm-run` is separate from that pruning step and from governed review.

The retained artifacts are the
[frozen mined ruleset](../../crates/cognigraph-construct/fixtures/webnlg/neuron-ruleset.mined.json),
[raw proposals](../../crates/cognigraph-construct/fixtures/webnlg/llm-proposals.json),
and [pruned proposals](../../crates/cognigraph-construct/fixtures/webnlg/llm-proposals-pruned.json).
The [CG-27 report](../issues/webnlg-status-2026-09-09.md) records current offline
verification and separates it from historical model and test-split evidence.
Five release commands passed with 14 scoring observations, unchanged hashes for
401 prepared files, and byte-identical frozen mining. No model call or test
scoring was performed.
New generalization claims require a newly reserved evaluation set; the spent
test oracle must not become a tuning source.

## Context

The WebNLG pilot (`docs/webnlg-pilot.md`, prepared 2026-07-16) is the
automatically scorable, non-clinical complement to the DailyMed clinical
reference. Each English document is natural-language text lexicalized from a set
of DBpedia triples `(subject, predicate, object)`; those triples are a
mechanical external oracle, physically separated into `oracle/` from the
`documents/` text the construction run may read. 38,872 documents, 115,279
triples, 411 predicates across 19 categories.

Unlike DailyMed clinical relations, WebNLG has a structured oracle, so it needs
**no domain-expert adjudication** — the bottleneck that gates DailyMed recall.
That removes the human from the scoring loop, but it makes the scoring *policy*
itself the load-bearing artifact: how a constructed CogniGraph edge is judged
to have recovered an oracle triple, and what is (and is not) claimed from the
result. This document freezes that policy so the `test` split can be opened
exactly once, against a policy authored without it.

The design deliberately mirrors `clinical_reference::score`
(`decision_dailymed_clinical.md`): explicit measurement lanes, honest labels
that never overclaim end-to-end, and a structural no-leak guarantee between the
authoring path and the oracle.

## Decisions (owner: user, 2026-07-17)

**W1 — Three lanes now, two deferred; the label carries its own asterisk.**
Each lane is scored independently and reported with a label that states exactly
what it assumes, so no number can be read as more than it is.

*Built this iteration:*
- **`generic`** — a small, frozen, hand-authored predicate-rule set (the naive
  first pass over the most common DBpedia predicates). The honest low baseline;
  the WebNLG analog of clinical's frozen-generic vocabulary, and expected, like
  it, to expose how little a generic first guess covers an open domain.
- **`neuron-authored`** — specified as Semantic-Neuron rules proposed over
  **train+validation only**, human-reviewed, and frozen before scoring. The
  delivered pilot diverged at the review step: it froze a deterministically
  mined candidate artifact without running the runtime human-review lifecycle
  (see Outcome). The lane name is retained for result compatibility.
- **`oracle-diagnostic`** — the gold predicate for the document is handed to the
  grounder. **DIAGNOSTIC ONLY**, never end-to-end; it isolates trigger/relation
  quality by removing predicate discovery from the problem, exactly as clinical's
  oracle-vocabulary lane does.

*Spec'd, deferred (not built this iteration):*
- **`entity-discovered`** — the full text→graph pipeline, extracting entities
  from text rather than receiving them (see W4). Quantifies the
  entity-recognition cost the headline lanes exclude.
- **`hard-negative`** — injected negation/entity-swap negatives, the analog of
  clinical's curated forbidden-facts restraint set (see W5).

**W2 — Entities are provided in every lane; lanes vary only relation knowledge.**
The grounder receives the oracle subject/object **surface forms** in all lanes.
This is **not** an oracle leak: the pilot's own thesis (and the standard
relation-extraction evaluation setup, e.g. TACRED/DocRED) is that entity
extraction is the easy, surface-anchored half and relation extraction is the
hard asymmetry a semantic neuron exists to solve — by converting open generation
into constrained selection over an ontology-bound entity set. Providing entities
models the real deployment (a neuron may only assert entities already in the
schema) and isolates the score on the relation, which is the thing on trial.
The leak boundary is the **predicate** and the **triple**: only
`oracle-diagnostic` receives the predicate; no lane receives the assembled
triple.

**W3 — Honest labels: "relation-end-to-end (entities provided)", never bare
"end-to-end".** Because entities are supplied, the `generic` and
`neuron-authored` lanes are end-to-end over *relation construction*, not over the
full text→graph pipeline. They are labeled `relation-end-to-end (entities
provided)` and every reported score carries the "entities assumed" caveat —
the same discipline that makes clinical stamp "construction only" and
"oracle-diagnostic — NOT end-to-end". The truly end-to-end pipeline is the
deferred `entity-discovered` lane. No lane in this document may be described as a
full text-to-knowledge-graph result.

**W4 — Matching: exact-triple headline, partial-credit diagnostics.** A
constructed edge recovers an oracle triple when **all three** of subject,
predicate, and object match after canonical normalization (W6). The headline
recall and precision use this `correct` set only. Two partial-credit figures are
reported alongside as diagnostics, never in the headline:
- **`mislabeled`** — subject ∧ object match, predicate wrong ("found the link,
  wrong relation").
- **`entity-pair-only`** — the subject/object pair is linked by some edge but
  neither the predicate nor necessarily the direction matches.

These separate three failure modes the headline conflates: missed entirely,
linked-but-mislabeled, and wrong-pair.

**W5 — Metrics: recall and precision per lane and per predicate; precision is
the headline restraint figure.** For each lane, and broken down per DBpedia
predicate:
- **Recall** = `correct / oracle_total` — did the loop build the facts?
- **Precision** = `correct / constructed_total` — the false-edge / restraint
  metric. Because each text is lexicalized from exactly its oracle triples
  (treated as a closed generating set), any constructed triple **not** in the
  oracle is a **scoring false positive**. That means “not among the supplied
  generating triples,” not independently adjudicated falsehood; the oracle may
  not enumerate every relation a reader could infer. Precision is reported as
  the headline restraint proxy; no separate restraint figure is defined for the
  built lanes. The curated-negative
  restraint metric (TN/(TN+FP) over injected negatives) is deferred with the
  `hard-negative` lane.

Honesty caveat recorded with every WebNLG result: WebNLG is clean,
crowd-sourced, near-affirmative data-to-text material. It is **light on natural
negation and modality**, so it does **not** exercise the polarity/modality
discipline that is clinical's headline strength. WebNLG precision measures
false-edge restraint against clean text; it is not evidence that negation
handling transfers. That differentiator is the point of the deferred
`hard-negative` lane.

**W6 — Normalization is a single frozen function.** Before comparison, subjects,
predicates, and objects are normalized by: replace `_` with space; trim; Unicode
case-fold. Objects that both parse as numbers compare **numerically**
(`2702.0` == `2702`, `507` == `507.0`); otherwise string-equal after the above.
Entity and predicate strings in `oracle/` are preserved exactly as supplied by
the source; normalization lives **only** in this scoring function, never in data
preparation (which stays byte-reproducible per `decision_rebuildable_derivatives.md`).
Inverse predicates and multiword/alias surface forms beyond this normalization
are **not** silently credited in v1; if a predicate is commonly expressed in the
inverse direction, that is handled explicitly in a `neuron-authored` rule (with
its own reviewed direction), not by a global matcher relaxation. The normalization
function, the `generic` rule set, and the `neuron-authored` rule set are each
**frozen on train+validation and digest-recorded** in the run metadata.

**W7 — The test oracle opens once, structurally walled from authoring.** Rule
authoring (both lanes) and normalization are frozen using `train` and
`validation` only. The `test` oracle is loaded behind a separate call that the
authoring path cannot reach — the code-level analog of the `documents/` vs
`oracle/` physical split the data already enforces — so no rule set can be tuned
against `test`. Opening `test` produces the reported figures once; it never feeds
back into rule authoring. Reusing `test` to iterate a rule set invalidates the
result and must be recorded as such.

## Module shape

New `crates/cognigraph-construct/src/webnlg/`, parallel to `clinical_reference/`:
- `model.rs` — loaded record types (`ScoredTriple`, lane enum, per-lane and
  per-predicate score structs).
- `normalize.rs` — the single frozen W6 normalization + numeric-literal compare;
  pure, exhaustively unit-tested.
- `rules.rs` — the `generic` and `neuron-authored` predicate-rule sets as frozen,
  digest-recorded artifacts; construction reuses `ground_chunk` + `SpaceType`.
- `score.rs` — pure/synchronous scoring over loaded records (recall, precision,
  partial-credit diagnostics, per-predicate breakdown), unit-testable without a
  backend, exactly like `clinical_reference::score`.
- `mod.rs` — lane orchestration + the W7 train/val-vs-test load boundary.

Scoring produces a `WebnlgScore` per lane carrying its honest label (W3), the
headline recall/precision (W4/W5), the diagnostic partial-credit counts, the
per-predicate table, and the frozen-artifact digests.

## Boundaries kept

- No oracle predicate or assembled triple reaches any built lane; only
  `oracle-diagnostic` receives the predicate, and it never claims end-to-end.
- No lane in this iteration is a full text→graph result — entities are provided
  (W2/W3); the end-to-end pipeline is the deferred `entity-discovered` lane.
- The `test` split is opened once, against a policy frozen without it (W7).
- No claim that WebNLG precision demonstrates negation/modality handling; that
  is deferred to the `hard-negative` lane (W5).
- Data preparation stays byte-reproducible; all normalization is scoring-side
  only (W6).
- No construction score is claimed by this document — it freezes the *policy*;
  the numbers come from a later scored run.

## Addendum (2026-07-17): implementation reconciliations

Two points where the shipped `cognigraph_construct::webnlg` module and the
frozen text above needed to be made to read identically (caught in final
review):

- **W6 numeric canonicalization applies to all three fields, not only objects.**
  W6's headline wording scopes the numeric compare to *objects*. The
  implementation's `canonical_field` applies the numeric branch uniformly to
  subject, predicate, and object — simpler (one canonicalization rule, no
  per-field variant) and harmless for this corpus, since DBpedia predicate
  surfaces are never bare numbers and subject surfaces essentially never are. A
  side effect, accepted here: bare-numeric tokens and non-finite float literals
  (`"007"` → `"7"`, `"nan"`/`"inf"`) are absorbed by the numeric branch. This is
  the deliberate, blessed behavior of the frozen function; it does not change any
  object comparison W6 specified.
- **W7 is a procedural, greppable guard — not a compile-time wall.** The prose
  "structurally walled" / "a separate call the authoring path cannot reach"
  overstates the mechanism. What ships is a single `load_corpus` choke point
  keyed on a freely-constructible `Corpus` enum: an `Authoring` request never
  reads `test`, and every test-reading path must visibly name
  `Corpus::Evaluation` (greppable). The "opened once" discipline is procedural,
  enforced by making violations explicit rather than by the type system. A true
  structural wall (gating `Evaluation` behind a separate module/feature) was
  judged unnecessary for this pilot.

## Addendum (2026-07-17): date-literal aliasing (W2/W6)

W6 flagged literals as a known gap. The held-out validation run surfaced the
concrete cost: ISO-date oracle objects (`1982-07-23`) never grounded, because a
trigger's `{target}` expanded to the ISO token while the prose renders the date
("born on July 23, 1982") — `birthDate` scored 0 correct out of 46 constructed.

Fix (`rules::date_aliases`, wired into `build_space`): an entity token that
parses as `YYYY-MM-DD` carries its common prose renderings as **aliases**
("July 23, 1982", "23 July 1982", "23rd July 1982", comma/ordinal variants), so
the grounder matches the date in text. The `EntityDef.name` and the
`RelationRule` endpoints stay the ISO token, so the constructed edge still
carries `1982-07-23` and compares correctly against the oracle (W6 unchanged).
This extends the same aliasing already used for the underscore→space readable
form (W2: entities are provided; aliases are additional surfaces for the *same*
provided entity, never new entities). It changes construction, not scoring or
matching.

Effect on held-out validation: `birthDate` moved from the worst-precision
offender (0/46) to a healthy diagnostic-ceiling `11/12`; neuron-authored recall
edged 9.7% → 9.9% (+12 correct triples). The aggregate lift is small because
ISO-date predicates are a minority of triples; the remaining large gaps are
bare-year over-matching and template ambiguity (e.g. "born in {target}" firing
on place/year entities), which are separate from date literals.

## Addendum (2026-07-17): cross-predicate template disambiguation

The held-out run's precision was dominated by **cross-predicate collision
templates** — 17 templates mined under more than one predicate (e.g.
`{source} is a {target}` under course/occupation/profession/type;
`{source} was born in {target}` under birthDate/birthPlace/birthYear/origin;
`{source} is in {target}` under five predicates). A template claimed by N
predicates is correct for at most one on any given text match; instantiated for
the other N-1 it manufactures false edges.

Fix (`mining::disambiguate`, on by default via `MineOpts::disambiguate`): each
template is assigned to the single predicate that most frequently lexicalizes it
(highest mining count; lexical tie-break) and removed from the rest —
deterministic. This is the mechanical form of the policy's "human review" step
(W1); a human can still prune further.

Held-out validation A/B (neuron-authored, raw vs disambiguated):

| | recall | precision | false edges | constructed |
|---|---:|---:|---:|---:|
| raw | 9.9% | 46.2% | 559 | 1040 |
| disambiguated | 8.9% | **75.5%** | **139** | 568 |

Precision +29pp for a −1pp recall cost; false edges cut from 559 to 139. Since
precision is the false-edge/restraint metric (W5) — the point of neuron
governance — this is a strongly favorable trade, so disambiguation is the
default. The recall cost is the honest downside: a template that legitimately
served a minority predicate (e.g. "born in {year}" for birthYear) is lost when
assigned to its dominant owner (birthPlace). Remaining precision offenders are
inherently over-general single-predicate templates (`occupation`
"{source} is a {target}").

## Addendum (2026-07-17): recall tuning (keep the long tail)

Recall was capped near 9% because the default mining dropped every phrasing seen
only once (`min_count = 2`) and kept few per predicate (`max_per_predicate = 8`).
WebNLG texts are clean and phrasing-diverse, so the discarded singletons were
mostly *correct* long-tail phrasings, not noise. A held-out sweep confirmed it:

| mining opts | recall | precision | correct |
|---|---:|---:|---:|
| min_count=2, cap=8 (old default) | 8.9% | 75.5% | 429 |
| min_count=1, cap=20 (**new default**) | **12.4%** | **76.6%** | 601 |
| min_count=1, cap=40 | 13.0% | 70.1% | 629 |

`min_count=1, cap=20` lifts recall ~40% relative (+172 correct triples) with
precision *unchanged* — the long-tail phrasings add coverage and disambiguation
still guards collisions. `cap=40` buys little more recall at a real precision
cost, so `cap=20` is the chosen default. The committed artifact is regenerated as
`neuron-authored-mined-v3` (303 predicates / 1,842 templates on train+validation).

Recall still equals the oracle-diagnostic ceiling (12.4%), so it remains capped
by **trigger rigidity** — exact-phrase templates only fire on phrasings seen in
training. Lifting it further needs looser matching or the LLM-propose loop, not
more of the same deterministic templates; that is the next frontier, and it
trades against the precision that is the pilot's current strength.

## Outcome (2026-07-17): one-shot `test` result

The held-out `test` oracle was opened **once** (`webnlg-test`, the only binary
that names `Corpus::Evaluation`), scoring the frozen committed ruleset
`neuron-authored-mined-v3` (mined on train+validation, loaded — not re-mined).
`test` never fed mining; validation informed only the mining hyperparameters
(standard dev-set tuning). Per W7 this number is final; re-running against a
changed ruleset would invalidate it.

The delivered `neuron-authored-mined-v3` artifact is a deterministic,
supervised candidate ruleset mined from authoring triples. It was frozen and
auditable, but it was not human-pruned and did not pass through the runtime
`Neuron` proposal/review/accept lifecycle described in W1. The lane name is
retained for result compatibility; the result supports the grounding/template
mechanism, not the complete governed lifecycle.

**Test split: 1,779 documents, 5,639 gold triples.**

| lane | recall | precision | correct / constructed |
|---|---:|---:|---|
| `generic` (4 predicates) | 0.4% | 52.6% | 20 / 38 |
| `neuron-authored` (frozen) | **7.0%** | **68.8%** | 393 / 571 |
| `oracle-diagnostic` (ceiling) | 7.0% | 90.1% | 393 / 436 |

**Honest reading.** Both figures are lower than the validation estimates (12.4%
recall / 76.6% precision). That drop is expected and is exactly why `test` is
held out: the validation numbers benefited from hyperparameters chosen on
validation, and `test` is genuinely unseen — so **7.0% / 68.8% is the unbiased
headline**, not the validation figures.

- The governance still clearly beats naive on unseen data: neuron precision
  68.8% vs generic 52.6%, and the disambiguation lifts it toward the 90.1%
  oracle-diagnostic ceiling. On this corpus a constructed edge is right about
  two times in three, with intrinsic evidence for each.
- Recall is low (7.0%) and equals the oracle-diagnostic ceiling, confirming on
  truly-unseen data that the cap is **trigger rigidity**, not predicate
  confusion. The deterministic exact-phrase method has reached its floor; higher
  recall needs fuzzy matching or an LLM-propose loop.

No construction-quality claim is made beyond this table. The deterministic pilot
is complete; the recall frontier is future work and is recorded, not spun.

## Addendum (2026-07-20): the recall lever — fuzzy matching is the wrong one

The Outcome flagged two candidate recall levers: looser/fuzzy matching, or an
LLM-propose loop. The fuzzy lever was built and measured (`webnlg::fuzzy`, a
webnlg-only matcher that does NOT touch the shared `ground_chunk`): a template
fires when its connective tokens appear contiguously, in template order, within
one sentence containing both entities in their roles, with at most `max_gap`
tokens between parts. Measured on **held-out validation** only — the `test`
split stays spent (W7), so the 7.0%/68.8% headline above is unchanged.

Held-out validation A/B (neuron-authored), exact vs fuzzy across the gap knob:

| matcher | recall | precision | constructed |
|---|---:|---:|---:|
| exact (current) | 12.4% | **76.6%** | 785 |
| fuzzy `max_gap=0` | 14.5% | 40.9% | 1,720 |
| fuzzy `max_gap=1` | 18.2% | 28.8% | 3,054 |
| fuzzy `max_gap=2` | 19.3% | 15.4% | 6,093 |
| fuzzy `max_gap=3` | 20.1% | 13.1% | 7,431 |
| fuzzy unbounded | 28.5% | 8.3% | 16,751 |

**Decision: keep exact matching; the fuzzy lever is rejected.** The frontier is
steep and unfavorable — even the tightest setting (`max_gap=0`, connective
directly adjacent to the entities) roughly halves precision (77%→41%) for +2pp
recall, and there is no operating point that beats exact on the precision-first
metric. WebNLG documents pack several entities and the mined templates share
connective words (`is a`, `born in`, `located in`), so any looseness fires a
connective across many wrong entity pairs — the exact matcher's rigidity *is*
its precision. Since precision is the false-edge/restraint metric that is the
entire Semantic Neurons thesis, trading it away for recall is the wrong move.

The favorable recall lever is therefore **not looser matching but better
templates**: an LLM-propose loop that proposes richer *precise* phrasings for
unseen wordings (each firing exactly, high precision) with human review — the
governed-construction path, not a matcher relaxation. `webnlg::fuzzy` and the
`webnlg-score` gap sweep are retained as the reproducible evidence behind this
rejection, not as a shipped construction path.

## Addendum (2026-07-20): the template-proposal loop — the cap is unseen phrasings

The favorable lever (better *precise* templates, not looser matching) was built
as a proposal loop: `webnlg::propose` gathers per-predicate examples from the
authoring corpus, a `TemplateProposer` turns them into candidate
`{source}`/`{target}` templates, and they are validated, frequency-ranked, and
disambiguated exactly like the miner, then reviewed before freezing (W1). The
loop is LLM-pluggable — the `llm_prompt`/`parse_proposed_templates` plumbing is
built and mock-tested — but the LLM proposer is **gated**: a live model call is
outward-facing, paid, non-deterministic, and would break the pilot's byte
reproducibility, so it is not run here.

To measure the lever deterministically, the `CorpusProposer` was run: it
templatizes single-triple documents PLUS multi-triple sentences that carry an
unambiguously-attributable triple — **9,715 extra attributed examples, more than
doubling the mining pool** (292 → 308 predicates). Held-out validation, exact
matcher:

| ruleset | recall | precision |
|---|---:|---:|
| baseline (single-triple mine) | 12.4% | 76.6% |
| CorpusProposer (+ multi-triple) | 12.2% | 78.2% |

**Recall is flat.** More training data — even doubling it — does not lift
recall, and the oracle-diagnostic ceiling is unchanged. This sharpens the
diagnosis: the cap is **not too few training examples**, it is that validation
(and test) contain phrasings absent from training *at any volume*. Deterministic
mining can only reproduce phrasings it has already seen.

That is precisely why the lever needs an **LLM, not more corpus**: a model can
*generalize* — propose precise phrasings for wordings never seen in training —
which no amount of mining can. `webnlg::propose` is that loop, ready to run
against a live proposer. The remaining, deliberate step is a live LLM run
(needs a provider; accepts cost, non-determinism, and a human-review pass, and
records the resulting ruleset as reviewed-then-frozen rather than
byte-reproducible). The loop + the `CorpusProposer` measurement are retained as
the evidence; the committed frozen artifact is unchanged.

## Addendum (2026-07-20): the live LLM proposal run

The loop was run against a live model (gpt-5.4-mini via `webnlg-llm-run`,
OpenAI, `.env`-provided key) — outward, paid, non-deterministic, so a separate
opt-in binary. It proposed precise templates for the 25 highest-frequency
validation predicates from their `train` examples, merged them onto the mined
baseline, and measured on held-out `validation` (exact matcher; `test`
untouched, W7). The raw proposals are saved for review at
[the raw proposal artifact](../../crates/cognigraph-construct/fixtures/webnlg/llm-proposals.json)
(a non-deterministic snapshot).

| ruleset | recall | precision | correct / constructed |
|---|---:|---:|---|
| baseline (mined) | 12.4% | 76.6% | 601 / 785 |
| + LLM proposals, raw (no review) | 13.0% | 53.2% | 630 / 1185 |
| + LLM proposals, disambiguated (the loop's review) | 12.6% | 71.7% | 612 / 854 |

**Honest reading.** The LLM *does* surface real recall headroom — it proposes
genuinely novel matching phrasings (e.g. `{source} is found in {target}`,
`{source}'s leader name is {target}`) that mining never produced, worth +29
correct triples raw. But raw proposals also add over-general and
cross-predicate-colliding templates that crater precision (77%→53%). The loop's
cross-predicate **disambiguation recovers most of it** (53%→72%) while keeping
part of the gain (+11 correct) — yet the augmented set still lands *slightly
below* baseline on the precision-first metric (recall +0.2pp, precision
−4.9pp). The residual loss is **intrinsically over-general** proposals (e.g.
`{source} is in {target}` assigned to one predicate but still firing widely)
that disambiguation cannot fix — only a human reviewer pruning the candidate
set can, which is the loop's designed but manual final step.

**Conclusion.** On WebNLG the LLM lever is real but its automated payoff is
small and net-negative on precision; realizing it would require the manual
human-review/prune pass on the saved candidates. The frozen `test` headline
(7.0% / 68.8%) is unchanged and remains the claim. Across all three levers —
fuzzy matching (rejected), more corpus (no effect), LLM proposals (small,
review-gated) — the WebNLG recall frontier is genuinely hard, and the pilot's
precision-first exact matcher remains the right default.

## Addendum (2026-07-20): the human prune — the loop closes net-positive

Acting as the reviewer, 46 over-general / malformed templates were pruned from
the 25-predicate LLM proposals (bare spatial/copula phrases like
`{source} is in {target}`, the catastrophic `{source} is {target}` under
`nationality`, an unfilled-`{ordinal}`/`{month}` template, and sentence
fragments), leaving 202. Saved in
[the pruned artifact](../../crates/cognigraph-construct/fixtures/webnlg/llm-proposals-pruned.json);
re-measured with `webnlg-llm-run --load`.

| ruleset | recall | precision | correct |
|---|---:|---:|---:|
| baseline | 12.4% | 76.6% | 601 |
| LLM raw (unreviewed) | 13.0% | 53.2% | 630 |
| LLM + auto-disambiguation | 12.6% | 71.7% | 612 |
| **LLM + human prune (full loop)** | **12.6%** | **76.3%** | **609** |

The full loop — propose → disambiguate → human prune — is the first lever to
close **net-positive**: +8 correct triples with precision held (76.6% → 76.3%).
The gain is small on WebNLG, but it is real, and it validates the loop's design:
the LLM surfaces novel matching phrasings, and the human review (removing the
over-general ones) is the load-bearing, non-optional step that keeps precision —
which is exactly the Semantic Neurons thesis (governed, human-in-the-loop
construction; the LLM proposes, it does not decide). The frozen `test` headline
(7.0% / 68.8%) is still unchanged; the human-reviewed ruleset is a reviewed
candidate, not a re-frozen artifact.
