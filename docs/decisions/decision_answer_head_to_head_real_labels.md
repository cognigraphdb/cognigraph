# Decision: vector-RAG vs graph head-to-head on REAL labels — the fixture result does not reproduce

**Status:** Findings recorded (2026-07-21). Reverses the D3 priority in
`decision_pilot_clinical_graph.md` and sets the next construction goal.
Owner: skitsanos. A negative result, kept in full.

## Context

`fixtures/semantic-neurons/vector-control-results-2026-07-07.md` measured the
head-to-head on **five fixture kits with hand-authored ontologies and
hand-authored ground truth**: graph 86% answer recall vs vector 36%, restraint
0/54 vs 4/54. That result underwrites the positioning dossier.

This reruns the **same methodology and the same runner**
(`examples/vector_rag_control.rs`, `gpt-5.4-mini`, `text-embedding-3-small`,
vector top-8 vs graph 48-edge trace) on the real 100-label DailyMed corpus with
the **LLM-drafted, 85%-precision** `dailymed_pilot_v4` ontology. The question:
does the advantage survive real text and an imperfect graph?

**Ground truth came from the LABEL TEXT, never from the graph.** Deriving
expected facts from the graph would hand the graph arm a win by construction. An
independent model (Gemini — not the drafter, not the answering model) read each
label and wrote the facts a complete answer must contain, restricted to the
ontology's vocabulary so scoring is exact. 12 labels → 24 questions → 149
expected and 77 forbidden facts. Facts the text supports but the graph never
built are scored as real misses: the graph arm only gets credit for a fact that
is **in its trace**.

The inherited fairness asymmetries all still favour the vector arm: it receives
the answer vocabulary, its recall is an **upper bound** (no fabrication guard is
possible over raw passages), and 8 passages carry several times the token budget
of a 48-line trace.

## Result — the graph arm loses

| | vector | graph |
|---|---|---|
| clinical recall, 1-pass | 46/140 | 31/140 |
| **clinical recall, 2-pass** | **51/140 (36%)** | 33/140 (24%) |
| administrative recall, 2-pass | 3/9 | 0/9 |
| **combined, 2-pass** | **54/149 (36%)** | 33/149 (22%) |
| forbidden asserted, 2-pass | 3/77 | 2/77 |

On fixtures the graph won 86% to 36%. On real labels it loses, 24% to 36%. The
restraint advantage also narrowed to nothing meaningful (2 vs 3 leaks).

## Diagnosis — it is not the substrate, and not retrieval

### F1 — The answering step is already at its ceiling
Only **34 of the 140** expected clinical facts exist in the graph at all. The
graph arm asserted **33 of those 34 — 97% of everything achievable.** Trace
construction, ranking and answering are not the bottleneck; there is essentially
nothing left to win there.

### F2 — Grounding is not the bottleneck either
Of the 106 clinical facts the graph did not build, **0 had a rule that failed to
ground**. Every miss is a rule that was never drafted. Grounding does exactly
what it is asked to do.

### F3 — 43 of the missing 76 points are RELATION-LABEL mismatch, not missing knowledge
The decisive split. For the expected facts the graph "missed":

| | share of 140 |
|---|---|
| exact triple present | 34 (24%) |
| **entity pair connected, different relation label** | **60 (43%)** |
| entity pair not connected at all | 46 (33%) |

The knowledge is present for 43% of expected facts under a different label:
`AVOID_IN` vs `use_with_caution_in`, `HAS_WARNING` vs `adverse_effect`,
`CONTRAINDICATED_IN_HYPERSENSITIVITY_TO` vs `contraindicated_with`. **True
coverage is 67%, not 24%** — the exact-match metric is measuring vocabulary
fragmentation as if it were ignorance.

Not all of these are innocent synonyms: `AVOID_IN` and `use_with_caution_in`
differ in clinical strength, and a graph that cannot distinguish them is a
different problem from one that merely spells things two ways. Both need the
same fix first: a bounded relation vocabulary.

### F4 — Every missing entity was extracted; only the relation was never proposed
Of the 106 un-drafted facts, **0 had an unknown source and 0 an unknown target**.
Stage-1 entity extraction — the bottleneck fixed earlier today — is now doing its
job. The gap has moved one layer up, to **stage-2 rule drafting**: the entities
sit in the catalogue and no rule connects them. ~1231 rules over 100 labels is
~12 relations per label, and a real FDA label asserts many times that.

### F5 — The administrative kit is the measured cost of D1
The graph scored 0/9 there, exactly as designed: D1 stopped drafting
administrative relations, so "who manufactures this?" is now unanswerable from
the graph. That bought +13.5 points of fact precision. The trade is real,
now quantified, and worth revisiting only if users ask such questions.

## Decisions

### D1 — Relation-vocabulary normalization: ATTEMPTED, MEASURED, REJECTED

The original text of this decision promoted vocabulary normalization to the top
construction priority and claimed it was worth **+43 points**. Both the estimate
and the intervention were then tested. **The estimate was wrong and the
intervention made the product worse.** Kept in full, because the cheapest thing
in this project is a wrong number that nobody checked.

#### The +43 estimate was an artefact of how the misses were counted
"43% of expected facts are in the graph under a different label" counted any
expected fact whose entity PAIR was connected by any other relation. Splitting
those 60 cases:

| | count |
|---|---|
| shares a head token with a graph label (true spelling variation) | **10** |
| head tokens differ entirely (semantic framing difference) | **50** |

A conservative lexical fold — same head token, refusing to merge across polarity
(`WITHOUT`) or antonym (`ACTIVE`/`INACTIVE`) markers — moved coverage
**24.3% → 25.7%**, i.e. **+1.4 points, not +43**. A hand-written *semantic*
equivalence map (`HAS_INACTIVE_INGREDIENT` ≡ `CONTAINS_INACTIVE_INGREDIENT`,
`HAS_ADVERSE_REACTION` ≡ `CAUSES`, while deliberately keeping `AVOID_IN`,
`USE_WITH_CAUTION_IN` and `CONTRAINDICATED_IN` apart) reached **47.1% with zero
forbidden-fact collisions**. So label-neutral knowledge coverage was ~47% all
along; the honest ceiling for naming work was **~+23 points**, and only via
semantics, never lexical folding.

That map is hand-curated pharma vocabulary and could not ship in
`cognigraph-construct`, which is domain-general — and no domain-general rule
separates the safe folds from the dangerous one, since merging
`CONTRAINDICATED_IN` with `CONTRAINDICATED_WITH` is harmless while merging
`AVOID_IN` into `USE_WITH_CAUTION_IN` changes what the graph claims clinically.

#### The domain-general intervention: show each document the vocabulary so far
Implemented as `draft_space_type_with_vocabulary` — each per-document rule draft
sees the labels the corpus has already used and is asked to reuse one where it
fits, with reuse a hint rather than a constraint so a genuinely new relation
stays expressible. Re-drafted the full 100-label corpus (`dailymed_pilot_v5`).

**It achieved its stated goal and failed its acceptance test.**

| | v4 (no hint) | v5 (vocabulary reuse) |
|---|---|---|
| distinct relation labels | 301 | **236 (−22%)** |
| labels per 100 rules | 24.5 | **19.3** |
| facts built | 1575 | 1594 |
| expected-fact coverage, exact | 24.3% | 13.6% |
| **expected-fact coverage, label-neutral** | **47.1%** | **23.6%** |
| expected entity PAIR present | 67% | **51%** |
| expected TARGET entities present anywhere | 124/140 | **100/140** |

The label-neutral row is the one that matters: it applies the same semantic
buckets to both graphs, so naming cannot explain it. **v5 covers roughly half as
much of the knowledge answers need, while building slightly MORE facts.** The
loss is concentrated in target entities — the rule stage simply stopped
connecting 24 of the entities the questions ask about.

Most likely mechanism: prompt dilution. The label list grows to ~236 entries by
the last document, and a long "reuse one of these" inventory competes for
attention with the actual task of reading the excerpts exhaustively. The rules
that survive fit the offered vocabulary rather than covering the document.

**Reverted.** Vocabulary convergence is not worth halving answer coverage. The
caveat kept honest: v4 and v5 are separate LLM runs, so some variance is
expected — but a 23-point label-neutral swing is far outside the run-to-run
variation seen elsewhere, and the target-entity collapse gives it a mechanism.

#### What remains true
Naming inconsistency still costs ~23 points against a label-neutral ceiling. But
the lever is NOT drafting-time reuse, and it is not a lexical fold. If it is
attempted again, the candidate is a **fixed, small, domain-supplied relation
vocabulary** provided as part of the space-type request (the operator's
vocabulary, not one the drafter accumulates), so the list stays short and stable
instead of growing into noise — and it must clear this same head-to-head before
shipping.

### D2 — Rule-drafting density is now the leading candidate, not the follow-up
The residual 33% is genuinely un-drafted relations between entities the drafter
already extracted. This is the stage-1 starvation pattern one layer up, and the
fix that worked there should be tried here: more density per document (per
section rather than per label), or an explicit second rule pass over entity pairs
that co-occur but have no rule. Measure with the same head-to-head.

### D3 — This head-to-head becomes the standing answer-quality gate
Construction metrics (precision 85%, clean lane 92.2%) proved the right edges are
trustworthy; they said nothing about whether enough edges exist. Only this
measurement did. Any future construction change claims an answer number from this
runner, on text-derived ground truth, or it does not claim one.

## Assessment

**The honest headline: on real text with a drafted ontology, the graph does not
yet beat vector RAG on answer recall.** The fixture result was real but its
conditions — an authored ontology, ground truth authored alongside it — do not
hold here, and the positioning dossier should not be read as covering this case
until the head-to-head is re-run after D1.

The encouraging half is that the failure is precisely located and none of it is
architectural: retrieval extracts 97% of what the graph holds, grounding never
misfires, and entity extraction is fixed. What is missing is **relation
coverage** — a bounded vocabulary and denser rule drafting — both of which are
ordinary work on a stage that has never been optimized. Today's precision gains
(71.5% → 85.0%) are not in tension with this: precision was measured on facts
built, recall on facts needed, and the project had simply never measured the
second until now.
