# Decision: pilot-grade governed clinical graph from 100 FDA labels — measured precision

**Status:** Results recorded (2026-07-21). Sets the next construction priorities.
Owner: skitsanos. Supersedes nothing; extends
`decision_neurons_real_label_construction.md` (the 50-label run) with a
precision-measured build at 2× the corpus.

## Context

The 50-label run established that the governed pipeline works and localized the
recall cap to stage-1 entity extraction (D1 there). This run asks the next
question: **build the graph at pilot scale and measure how right it is.** Same
dev `dailymed` tenant, non-production data, free to modify for evals.

## What we ran

- **Corpus:** 100 real DailyMed FDA labels → chunked by LOINC `sections`
  (≤2000 chars, ≥2 sections per label) → **659 chunks**, `title` = the label, so
  per-document drafting groups one document per drug.
- **Draft:** `POST /api/construct/draft` with `per_document: true, async: true` —
  the **durable background job** (M16/M17), 100 documents, ~200 completions,
  **1024 s**, checkpointing every 25 documents. `gpt-5.4-mini` drafting,
  `gemini-embedding-2` embeddings.
- **Accept → ingest:** 659 chunks → **1746 facts in 10.7 s** (deterministic, no LLM).
- **Advisor:** `/api/construct/advise` over all 1327 rules (deterministic, no LLM).
- **Precision:** 200 facts sampled with a fixed seed from the 1746 and judged by
  **Gemini `gemini-flash-latest` — a different provider from the drafter** —
  under a strict response schema. The judge was calibrated **6/6** on a
  two-sided set before use (accepts qualified truths, rejects boilerplate,
  cross-label leakage, negation, and plausible-but-unstated relations).

## Results

### The graph

| metric | value |
|---|---|
| facts (edges) | **1746** |
| entities | 1384 |
| chunks | 659 |
| labels contributing ≥1 fact | **100 of 100** |
| distinct relation types | 366 (168 used exactly once) |
| drafting wall time | 1024 s (background job) |
| ingest wall time | 10.7 s |

### Precision (n=200, independent judge, seeded sample)

| lane | share of graph | evidence-supported | label-correct |
|---|---|---|---|
| **whole graph** | 100% (1746) | **71.5%** | 82.5% |
| **clinical relations** | 89% (1554) | **81.1%** | **87.6%** |
| administrative / document-structure | 11% (192) | **19.4%** | — |
| core clinical (`INDICATED_FOR`, `TREATS`, `CONTRAINDICATED_*`, `INTERACTS_WITH`, `CAUSES`) | 482 facts | 78.8% | — |

Failure modes across the sample: `wrong_relation` 22, `boilerplate` 18,
`wrong_entity` 15, `negated` 1, `other` 1.

### It answers clinical questions, with evidence attached

Every edge carries the sentence fragment that licensed it. Real output:

- `Metformin --INTERACTS_WITH--> cimetidine` — *"drugs that reduce metformin
  clearance (such as ranolazine, vandetanib, dolutegravir, and cimetidine)"*
- `Metformin --INCREASES_RISK_OF--> lactic acidosis` — *"carbonic anhydrase
  inhibitors may increase risk of lactic acidosis"*
- `Bupivacaine Hydrochloride Injection --CONTRAINDICATED_IN--> intravenous
  regional anesthesia` — *"intravenous regional anesthesia (bier block)"*
- `TRUVADA --INDICATED_FOR--> HIV-1 pre-exposure prophylaxis` — *"hiv-1 prep"*

## Findings

### F1 — Per-document drafting scales, and the entity starvation is gone
The 50-label broad draft extracted **0** condition entities and **0**
drug→condition rules. At 100 labels, per-document drafting produced **435
condition-like entities and 511 drug→condition rules**, and **all 100 labels**
contributed at least one fact. D1 of the previous decision is confirmed at scale,
not just in the 15-drug A/B.

### F2 — Per-document drafting had a latent identity bug; found only at scale
63 case-variant entity groups made the accepted ontology un-ingestable. Fixed
and documented as the addendum to D3 in
`decision_neurons_real_label_construction.md`. **The pilot's value was largely
this:** a defect invisible on 15 drugs and fatal on 100.

### F3 — Administrative relations are where the precision goes to die
The drafter proposes relations for regulatory furniture as readily as for
pharmacology — `REPORT_ADVERSE_REACTIONS_TO`, `CONTAINS_INFO_ON`,
`MANUFACTURED_BY`, `HAS_WARNING`-as-section-pointer. These are **11% of the
graph at 19.4% precision**, and they contribute 32% of all judged failures
(`REPORT_ADVERSE_REACTIONS_TO` 10, `CONTAINS_INFO_ON` 8). They are not knowledge;
they are page furniture. Excluding them lifts precision **71.5% → 81.1% while
keeping 89% of the facts**.

### F4 — The gate advisor no longer discriminates, because the failure class changed
On 50 labels the advisor flagged **every** bad fact. At 100 labels it flags 566
of 1327 rules and those flags carry **no signal about correctness**:

| lane | evidence-supported |
|---|---|
| advisor-clean | 70.4% (57% of sample) |
| advisor-flagged | 72.9% (42% of sample) |

This is not a regression in the advisor — it is a **change in what fails**. The
advisor detects one signature: *an endpoint that never appears in any licensing
sentence*, i.e. cross-document leakage. Per-document drafting plus the D2
boilerplate filter largely eliminated that class. What remains is
`wrong_relation` / `wrong_entity` — the trigger sentence is about the right
drug but does not assert *that relation*, e.g.
`benzodiazepines --AVOID_CONCOMITANT_USE--> opioids` licensed by a sentence that
describes the *risks* of concomitant use without advising avoidance. The advisor
is structurally blind to this: it checks endpoint **presence**, never relation
**semantics**.

### F5 — The relation vocabulary is unnormalized
366 distinct relation types for 1746 facts, **168 used exactly once**;
`CONTRAINDICATED_IN` / `_WITH` / `_FOR` coexist, as do `CAUSES` /
`ADVERSE_REACTION` / `CAUSES_ADVERSE_REACTION`. Each per-document draft invents
its own labels and nothing reconciles them. This fragments traversal (a query
for contraindications must know all three spellings) and, more subtly, makes
`wrong_relation` easy: an over-specific invented label like `TREATED_WITH` or
`USED_FOR` asserts more than the sentence does.

### F6 — Two hypotheses tested and rejected (recorded so they are not re-tried)
- *"Failures come from the table-of-contents section each label opens with."*
  **Rejected**: ToC-shaped evidence appears in 9/57 failures but 24/143
  successes — it is more common among correct facts.
- *"Failures involve corpus-ubiquitous entities."* **Rejected**: at a ≥80%
  document-frequency threshold only substring artifacts (`MI`, `USP`) qualify;
  the split moves precision 71.5% → 72.1% while discarding nothing meaningful.

## Decisions

### D1 — The drafter must not propose administrative / document-structure relations — IMPLEMENTED AND RE-MEASURED

Implemented in `finalize_draft` (`drop_administrative_relations`) and the graph
rebuilt end to end through the real pipeline (`dailymed_pilot_v4`): same 100
labels, same 659 chunks, same drafting model, filter active.

**Keyed on entity type, not relation name.** The obvious implementation is a
relation-name pattern; the data rejected it. On the v3 sample, endpoint *type*
identifies these facts at **4.5%** precision versus **19.4%** for a relation-name
pattern — and relation names are an unbounded invented vocabulary (366 types for
1746 facts) while entity types are the smaller structured axis the drafter
already commits to. Types are matched **exactly, never as substrings**: the
drafter emits `anatomical site` and `parasite`, both of which a substring match
on "site" would silently destroy. An exact list under-matches on invented
synonyms, which is the safe direction — an unlisted type leaves an
administrative rule standing, it never drops a clinical one. `person` is
deliberately excluded because it holds the patient populations (`pregnant
woman`, `Elderly patients`, `children`) that `CONTRAINDICATED_IN` depends on.

#### Measured result (fresh seeded 200-fact sample from the rebuilt graph)

| metric | v3 — no filter | v4 — filtered |
|---|---|---|
| facts in graph | 1746 | 1575 (**−10%**) |
| **evidence-supported** | 71.5% | **85.0%** (**+13.5**) |
| **label-correct** | 82.5% | **92.5%** (**+10.0**) |
| clinical-lane precision | 81.3% | 85.1% |
| administrative facts in sample | 17% | 2% |

Failure modes, v3 → v4: `boilerplate` **18 → 0**, `wrong_entity` **15 → 8**,
`wrong_relation` 22 → 21, `negated` 1 → 0.

Three things worth noting beyond the headline:

1. **It beat its own prediction** (83.4% predicted from subsetting the frozen v3
   ontology, 85.0% measured on the rebuild). Subsetting only models the removal
   of bad facts; the rebuild also removed the administrative *entities* from the
   catalogue, which took a class of entity confusion with them — `wrong_entity`
   nearly halved. The clinical lane itself improved 81.3% → 85.1%, which pure
   subsetting cannot explain.
2. **Boilerplate failures went to zero.** Combined with the D2 trigger filter of
   the previous decision, the entire leakage/boilerplate class is now closed.
3. **`wrong_relation` did not move** (22 → 21) and is now **70% of all remaining
   failures**. This is exactly what D2 below predicts and this filter was never
   going to touch it.

Residual: 23 of 1575 facts (1.5%) still match an administrative relation-name
pattern, almost all `HAS_WARNING` (19) — whose objects are *conditions*
(`Quetiapine --HAS_WARNING--> Hyperprolactinemia`), so the type filter correctly
left them alone. Whether `HAS_WARNING` is clinical knowledge or document
structure is a vocabulary question, and belongs to D3.

### D2 — A second detector for relation-semantics mismatch — IMPLEMENTED AND MEASURED

After D1, `wrong_relation` was 21 of 30 remaining failures (70%) and had not
moved at all when the administrative mass was removed. Implemented as
`relation_semantics_signals` in the advisor and reported per rule as
`semantics_suspect` + `semantics_samples`. Deterministic, no LLM, **advisory
only — nothing is dropped**, so the ground-then-advise contract of
`decision_cross_chunk_grounding.md` is untouched.

The three shipped signals: the **target is absent** from the licensing sentence;
the endpoints are **merely co-listed** (`myopathy and rhabdomyolysis` — the
sentence enumerates, it does not relate); the sentence **never uses the
relation's own vocabulary** (`atorvastatin --TREATS--> MI` off "indicated to
reduce the risk of MI", which is prevention).

#### Designed by measurement, on a design/holdout split
400 judged facts were available: 200 from the v3 ontology, 200 from v4. Every
candidate was designed against v3 and confirmed on v4 — a **different ontology
over a different fact set**, so surviving it is a real generalization test. Six
candidates were tried; **three were rejected by the holdout**:

| candidate | design (v3) | holdout (v4) | verdict |
|---|---|---|---|
| target absent from licensing sentence | +36.0% | **+34.2%** | kept |
| endpoints merely co-listed | +57.3% | **+61.2%** | kept |
| relation vocabulary absent | +16.5% | **+15.3%** | kept |
| source absent from licensing sentence | −6.0% | −7.1% | **rejected** |
| section-index ("table of contents") shaped | +8.6% | −5.9% | **rejected** |
| union of all six | +24.9% | **−0.8%** | **rejected** |

(separation = unflagged-lane precision − flagged-lane precision)

Two rejections matter beyond this feature:

- **Source absence is ANTI-correlated with error.** A label section names its
  drug once and refers to it implicitly thereafter, so a licensing sentence
  without the source is ordinary prose, not leakage. This explains F4: the
  original endpoint-presence advisor treats source and target alike, so its two
  halves cancel and it separates nothing on a real corpus.
- **The union of everything scored +24.9% on design and −0.8% on holdout.**
  Shipped on design-set evidence alone it would have been worthless while
  looking like a success. This is the entire argument for the split.

#### Measured result, shipped code on the real corpus
`/api/construct/advise` over the 1231-rule v4 ontology flags 756 of 1575
groundings (48%); cross-tabulated against the judged sample:

| lane | precision |
|---|---|
| advisor-clean (57%) | **91.8%** |
| advisor-flagged (43%) | 76.7% |
| **separation** | **+15.1%** (old detector: −2.5%) |
| errors caught | 68% |

#### Per-fact verdicts, persisted as a quarantined sidecar

`ingest_chunks` now writes a verdict per grounded fact into `fact_semantics`,
keyed by the fact's own key, in the same atomic batch as the facts themselves.
**Deliberately a sidecar, not fields on the fact edge:** M26 attests that the
fact projection is byte-for-byte what the governed configuration produces, and
this detector is a heuristic that is expected to keep improving — folding it into
the attested record would make every detector revision invalidate previously
signed artifacts. Same reasoning that quarantines `side_views`. Only suspect
facts get a row, so the collection *is* the review queue; because rows are
written with the facts they describe, "no row" means "analyzed and clean", never
"not yet analyzed". A stale verdict cannot outlive its evidence (re-ingest
rebuilds both together) and the sidecar is publicly readable but never publicly
writable, so a forged "clean" verdict cannot launder a suspect fact.

Measured on the rebuilt v4 graph: 756 of 1575 facts carry a verdict (48%),
**clean lane 91.8%, flagged 76.7%, separation +15.2%, 70% of errors caught**.

**Correction — the earlier "per-rule aggregation costs ~2 points" claim was
wrong.** Scored on the same 200 judged facts, the prototype and the shipped
implementation agree on **192 of 200**; per-fact granularity is worth ~0.1 points,
not 2. The prototype's apparent 93.9% came from its own crude sentence
extraction, not from its granularity — a measurement-harness artifact that
disappeared once production's `sentence_bounds` was used. The per-fact sidecar is
still the right shape (persisted, unambiguous, joinable without re-running a
10-second advisor pass), but it did not buy the precision that was claimed for it.

**The real residual, diagnosed and then FIXED: abbreviation periods.** All 8
disagreements traced to sentence splitting. `sentence_bounds` ended a sentence at
any period followed by whitespace, so an **abbreviation period truncated the
sentence mid-entity-name**: `St. John's Wort`, `D C Yellow No. 10 Aluminum Lake`
and `Rifampin` (after "St.") all looked target-absent. All 6 such flags in the
sample were correct facts.

`is_sentence_boundary` now treats a dot closing a known abbreviation as
non-terminal. **The list is deliberately narrow, because the two mistakes are not
symmetric:** failing to split MERGES sentences, and a `require_in_sentence` gate
then judges a longer span and admits more groundings — a restraint regression —
whereas splitting too eagerly merely truncates a sentence. So it holds only
tokens essentially never sentence-final in label prose, and pointedly excludes
`etc.` and the corporate suffixes `Inc.`/`Ltd.`/`Corp.`, which routinely end the
manufacturer line of an FDA label. Every entry comes from a measured false flag,
not from a general-purpose abbreviation list.

Measured after the fix, same 200 judged facts:

| | before fix | after fix |
|---|---|---|
| clean-lane precision | 91.8% | **92.2%** |
| separation | +15.2% | **+17.2%** |
| errors caught | 70% | **70%** (none lost) |
| facts flagged | 756 (48%) | 742 (47%) |
| facts grounded | 1575 | **1575** (unchanged) |

The two safety checks both held: the full suite — which exercises the gate,
restraint and the reference kits — stayed green, and the pilot's grounded-fact
count was byte-identical, confirming no grounding moved. Every false flag in the
sample cleared and no true error escaped with them.

### D3 — Normalize the relation vocabulary across per-document drafts
`finalize_draft` already exists as the whole-corpus pass and is the natural
place. Fold near-duplicate relation labels onto a canonical form and record the
folds in `skips` like every other draft-time decision.

**Revised after D2 shipped:** this was assumed to be a *prerequisite* for the
semantics detector, on the theory that lexical cues need a bounded vocabulary.
That turned out to be false — the shipped detector derives its cue from the
relation's own name at match time, so an unbounded invented vocabulary costs it
nothing and it works today. D3 is now a graph-usability item (a traversal for
contraindications should not have to know three spellings), not a precision
blocker.

**REVERSED AGAIN (2026-07-21) — and then that reversal was itself tested and
withdrawn.** The head-to-head briefly appeared to show fragmentation costing 43
points, which promoted this to top priority. Measuring it properly showed the
real figure is **~23 points and only via semantic equivalence** (a lexical fold
buys +1.4), and the domain-general intervention — showing each document the
vocabulary so far — **converged the labels 22% while halving label-neutral answer
coverage, and was reverted**. See `decision_answer_head_to_head_real_labels.md`
D1 for the full numbers. Operative status: naming work is real but is NOT the top
lever, the top lever is rule-drafting density, and any future attempt needs a
fixed operator-supplied vocabulary rather than an accumulated one.

### D4 — Judge calibration is part of the measurement, not overhead
The first judge revision rejected `Entresto CONTRAINDICATED_WITH aliskiren`
because the triple did not restate the sentence's qualifier ("in patients with
diabetes"). Every fact here is an **occurrence-level pointer at its own
sentence** (`construction_schema: occurrence-v1`), not a universally quantified
claim, so that judge would have under-reported precision on nearly every
qualified clinical statement. Any future precision number in this project must
ship with its calibration set; an uncalibrated judge is an unmeasured
instrument.

## Assessment

**85.0% evidence-supported / 92.5% label-correct after D1** — up from 71.5% /
82.5%, for a 10% reduction in graph size. Still short of the ≥95% a clinical
decision-support surface needs, but the remaining gap is now a *single* named
cause: `wrong_relation` is 70% of what is left, and D2 is the work that attacks
it. The governance thesis itself held: every fact is evidence-bound, ingest is
deterministic and sub-second, ingest fails closed on identity conflicts rather
than silently merging, and the whole 100-label build is reproducible from a
durable, checkpointed job.

## Operational notes

- Four space types exist from this run: `dailymed_pilot` (un-ingestable, pre-fix),
  `dailymed_pilot_v2` (accepted, blocked by legacy `demo_meds` entity identities),
  `dailymed_pilot_v3` (the un-filtered baseline), `dailymed_pilot_v4` (**the
  current graph**). `space_types` is managed, so none can be deleted through the
  public API — host-admin or redb cleanup.
- **Global entity identity is real friction on repeat runs.** The four legacy
  `demo_meds` entities collide on any run; after v3 was ingested, its 1384
  entities became global, so v4 needed **435 identity alignments (93 renames)**
  against the stored graph before it could be accepted — the second run over the
  same corpus inherits the first run's type choices wherever the model wavered
  (`sacubitril` substance→ingredient, `pregnancy` medical condition→condition).
  This is the D3 identity contract working as designed, but a pilot operator
  needs it as a supported "align draft to stored identities" step, not a script.
- CGQL (`/api/query`) is only mounted when `cgql_mutations_enabled`; the pilot
  queries ran through `/api/graph/relationships`.
