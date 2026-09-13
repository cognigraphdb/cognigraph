# Decision: DailyMed pilot ontology, mechanical pass (D1–D6)

**Status:** Decided 2026-07-13 (user approved all six). Applies to the
DailyMed pilot (docs/dailymed-pilot.md) from gate 1.5 onward.

## Context

Gate 1 validated section-aware chunking and confirmed the drafter's
honest scope (vocabulary bootstrap, not coverage), and the gate advisor
immediately surfaced this domain's restraint signature: rules drafted
from one product's label fire on other products' interaction sections
(the drug-label form of cross-company leakage). The user's design prior
for the first pass: bound the ontology to relationships whose truth can
be checked mechanically, and exclude `TREATS`/`CAUSES`/clinical-effect
relationships — they require interpretation and carry safety-sensitive
ambiguity.

That prior does something bigger than bound scope: DailyMed's SPL
structured data is a **mechanical oracle**. Every prior eval in this
programme scored against hand-authored specs; the mechanical pass can
score precision/recall against ground truth — and in gate 2, judge
verdicts themselves become measurable against truth.

Measured on the gate-1 cohort (100 docs, cached raw SPL XML, same
pinned snapshot): 99 unique active ingredients (UNII-coded), 12 routes,
11 dosage forms, 77 labelers. Narrative presence of structured values:
ingredients ~every doc, routes 88/100, labelers 43/100, dosage-form
display names 10/100 (surface mismatch: "TABLET, FILM COATED" vs
"film-coated tablets"). Pharmacologic class and RxNorm appear in label
XML zero times (separate DailyMed API families, not collected).

## Decisions (owner: user, 2026-07-13 design session)

**D1 — Relation scope v1, split by evidence channel.**
Narrative-groundable (the governed loop's subject):
`CONTAINS_INGREDIENT`, `ADMINISTERED_VIA`, and — expected-weak recall,
measured anyway — `HAS_DOSAGE_FORM`, `MARKETED_BY`. Not narratively
evidenced: `HAS_PHARMACOLOGIC_CLASS`, `HAS_RXNORM_CONCEPT` (no verbatim
surface exists in prose for an RxCUI); these enter the graph only as
imported structured facts and need a collector extension (two more API
families) — deferred to gate 2. Clinical-effect relations (`TREATS`,
`CAUSES`, interactions) excluded from pass 1; recorded trigger: only
after the mechanical pass has calibrated reviewer cost and restraint
on this corpus.

**D2 — Two fact provenances, explicit on the edge.**
`provenance: "narrative"` (evidence chunk + trigger — the pilot's
subject) vs `provenance: "structured"` (SPL structured-data element,
pinned snapshot). Never mixed; the structured facts double as the
oracle. Back-compat: absent field = narrative.

**D3 — Typed entities, name-keyed, code-anchored.** Entity types
`product | ingredient | route | dosage_form | labeler` (+ `class`,
`rxnorm_concept` when imported). Identity stays name-keyed; UNII/RxCUI/
codes ride as entity properties for oracle joins and never act as
grounding surfaces (they do not occur in prose). Names + aliases remain
the only grounding surfaces.

**D4 — Candidate rules must not leak the answer.** Rules are generated
from the typed vocabulary cross-product (every product × every corpus
ingredient/route/form/labeler) with authored template triggers that
always reference `{target}` — never from per-document oracle pairs,
which would encode the answers into the rule set and leave the
experiment measuring nothing. ~11k candidate rules at 100-doc scale;
grounding cost is measured (QW7 Aho-Corasick has its trigger if slow).

**D5 — Subject-scoped grounding: structural restraint.** A chunk can
only evidence relations whose source is the product its own document
is about — doc→product is document metadata, not the answer. Sentence
gates fight the leakage weakly here ("each tablet contains X" rarely
names the product in-sentence); subject-scoping makes cross-product
leakage impossible by construction, the repo's standing principle.
Implementation is free: `ingest_chunks` takes the config per call, so
ingest runs per document with that product's rule subset. Accepted
recall ceiling, chosen deliberately: a label discussing a co-packaged
or comparator product's composition can never produce that fact
(hearsay refusal).

**D6 — Oracle scoring.** Per-relation precision/recall of
narrative-grounded facts against the structured XML; every narrative
fact absent from the oracle is individually inspected (extraction
error vs oracle gap — both are findings). Gate 2 adds judge
accept/reject decisions scored against ground truth — the first
oracle-verified measurement of judge quality in the programme.

## Consequences

- The gate-1 draft becomes vocabulary input only; its per-product rules
  are superseded by D4's typed generation, and its clinical-ish
  relation labels are exactly the pass-2 material deferred by D1.
- Trigger quality becomes measurable: oracle recall gaps are the
  target list for gate 2's relation_hint proposal loop — the
  calibration story with ground truth attached.

## Boundaries kept

- No clinical-effect relation is constructed in pass 1.
- No oracle information flows into rule generation (vocabulary is
  corpus-wide and public; per-document pairs are the answers).
- Structured and narrative facts never share an edge.

## Outcome

The typed-vocabulary, subject-scoped construction design shipped and became the
input to Gate 3. It prevented cross-product leakage across the 10,000-label run
and supported the lossless mention prefilter needed to finish grounding at that
scale. The production result and its narrative-layer caveat are recorded in
`decision_gate3_baseline.md`; this decision establishes the ontology and
provenance contract, not a clinical-effect recall claim.
