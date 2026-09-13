# Decision: DailyMed pilot pass-2 — clinical relations (D1–D6)

**Status:** Decided 2026-07-13 (user approved all six). The pass-2
experiment named as the frontier by gate 3 and the positioning dossier:
governed construction of clinical relations that have **no structured
oracle**.

## Context

Gates 1–3 and drift all rested on DailyMed's structured data as ground
truth. Clinical relations — `TREATS`, `CONTRAINDICATED_IN`,
interactions — are free prose with no coded per-label truth, and external
clinical databases are license-gated or discontinued (RxNav's DDI API was
retired). This is the real-world condition the thesis targets: governed
graphs where edges cannot be verified against an oracle, and where the
judge and the self-healing loop stop being optional.

## Decisions (owner: user, 2026-07-13)

**D1 — Two clinical relations first.** `TREATS` (product → condition,
from INDICATIONS) and `CONTRAINDICATED_IN` (product → condition, from
CONTRAINDICATIONS). Interactions and warnings-derived relations (highest
leakage) are a later sub-pass.

**D2 — Evaluation without an oracle: restraint-first + a small
domain-expert recall reference.** Restraint is the load-bearing,
scalable metric (adversarial forbidden facts must not be built) and is
measurable **without labels**. Recall needs real clinical ground truth,
which is a **domain-expert (user) responsibility** — a model labeling
clinical facts to score a model is circular and, for safety-sensitive
clinical claims, inappropriate. **No LLM-as-oracle.**

*Buildable now (no labels) vs deferred (needs the user's labels):*
- **Now:** the clinical ontology and grounding; a **mechanism-level
  restraint probe suite** (author-constructed clinical sentences with
  known should/should-not-ground answers — unit tests for clinical
  negation/modality/co-mention, not clinical fact labeling); real-corpus
  construction volume and the structural no-leak guarantee; the
  self-healing demonstration on a synthetic revision.
- **Deferred to the user's labels:** real-corpus **recall** (did the loop
  build the right `TREATS`/`CONTRAINDICATED_IN` facts) — a ~30–50 doc
  domain-expert reference set.

**D3 — Leakage control is the core mechanism.** Clinical prose is
leakage-maximal, so all three restraint tools are mandatory:
subject-scoping (fact source = the doc's product), negation-and-modality
-aware grounding, and sentence gates on these relations. Clinical
subtlety recorded: contraindication prose expresses the relation with
affirmative triggers (`contraindicated in {target}`); the negation
lookback must fire on genuine negations (`is not contraindicated in
{target}`) but not on the inherently-negative-polarity relation word — so
triggers are chosen affirmative and clean.

**D4 — The judge is the production gate.** With no structured-agreement
gate to fall back on, the clinical production path is propose → judge →
human-for-low-confidence, at the 0.90 policy threshold (gate 3 D4's
"where structured truth is absent" clause). Judge quality on clinical
facts is measured against the D2 recall reference plus injection
resistance — deferred with the labels.

**D5 — Self-healing becomes load-bearing.** Clinical relations are where
drift cannot be handled deterministically (no oracle to re-gate against),
so a revision that dead-ends a clinical fact's evidence must be DETECTED
(`degradation_report`) and RE-PROPOSED (gap-directed rewiring).
Demonstrated on a synthetic controlled `TREATS` revision — the real-data
successor to `docs/self-healing-experiment.md`.

**D6 — Deliverable and honest framing.** Clinical construction for the
two relations with restraint measured at the mechanism level and at
corpus scale (structural), a self-healing demonstration, and the recall
reference explicitly deferred to domain-expert labeling. Framed
throughout as operating without an external oracle — restraint
load-bearing, recall small-N and user-authored.

## Domain-expert reference tooling (implemented 2026-07-14)

The label-gated work now has a reproducible instrument:
`dailymed_clinical_reference`. It deterministically prepares a blinded 50-label
cohort (10 calibration, 40 held-out evaluation), creates two independent
annotation templates, validates completeness and exact evidence provenance,
compares reviewers, produces an explicit adjudication queue, compiles the
existing `EvalSpec`, scores the frozen grounding mechanism, and measures the
judge against the adjudicated cases at the 0.90 policy threshold.

The tooling does not relax D2. Mechanical candidate generation can reduce
review effort but cannot assign a verdict; validation refuses incomplete or
`unreviewed` files, and neither grounding nor judge scoring runs until the
human-authored reference is complete. Calibration documents are excluded from
reported scores by default. The complete procedure and commands are in
`docs/dailymed-clinical-reference.md`.

Live instrument check, 2026-07-14: the release binary prepared six real
DailyMed packets (two calibration, four evaluation; 15 mechanical candidates)
and the validator correctly refused the untouched expert template as
incomplete. Positive validation/scoring paths are covered by Rust tests; the
real recall and judge measurements remain intentionally blocked until the two
experts and adjudicator complete the reference.

## Outcome: calibration (joint review, 2026-07-14) — and what it exposed

The 10 calibration documents were reviewed jointly and the interpretation
rules frozen as **R1–R7** (`docs/dailymed-clinical-reference.md`). The
governing rule is **concept preservation (R1)**: a qualifier may be dropped
only when it does not change the concept's extension; subtype, severity,
anatomy, timing, causative drug, and treatment combination are essential.
So *systemic fungal infection* != *infection*, *partial-onset seizures* !=
*seizures*, *hypersensitivity to pregabalin* != *hypersensitivity*.

**The calibration produced a negative result about our own design, and it is
the most valuable thing to come out of pass-2.** Scored against R1–R7, only
**3 of the 24** candidates the generic 30-term condition vocabulary surfaced
were TRUE; roughly **20 required concepts were never surfaced at all**, and 2
of 10 documents produced no candidates. The generic vocabulary cannot express
a concept-preserving clinical gold.

**Calibration and measurement correction (2026-07-14).** The earlier global
ceiling and attainment percentages are retracted: they deduplicated concept
names across documents, merged relation vocabularies, used extractor candidates
rather than gold, and included bare-list noise. The replacement counts fact
instances as `(set_id, relation, concept)` with relation-specific vocabularies.

For this third-party-data development showcase, the project owner explicitly
accepted a content-hashed calibration artifact containing 19 TRUE and 8 FALSE
cases, exact evidence, and R1–R7 attribution. This is engineering calibration,
not external clinical validation, medical advice, or the two-reviewer held-out
reference. The corrected R6 ruling makes both plain `atopic dermatitis` and
plain `serum sickness` FALSE in the restricted dexamethasone sentence, with
their severity/refractory-qualified concepts TRUE.

Two isolated matcher phases were validated against the accepted artifact, each
frozen before the 40 held-out labels were opened:

| check | phase 1 (`clinical-matcher-v1`) | phase 2 (`clinical-matcher-v2`) |
|---|---|---|
| accepted TRUE asserted | 13/19 | **19/19** |
| corpus vocabulary coverage | 17/19 | **19/19** |
| end-to-end | 11/19 | **19/19** |
| accepted FALSE refused | 8/8 | **8/8** |
| R1–R7 synthetic negative probes refused | 14/14 | **14/14** |

**Phase 1** made the matcher enumeration-, bullet-, adjunctive- and
prohibition-aware, but deliberately refused every cue-less bare contraindication
list — accepting six misses rather than manufacturing facts from drug names and
prose fragments. **Phase 2** closed all six **without costing a single negative
case**: restraint did not trade against coverage. Bare lists are admitted through
condition typing (`typing::is_condition`), and the discriminator is mined from
data already held rather than authored — the structured UNII ingredient block
plus the full DailyMed catalogs enumerated during collection (151,759 titles)
give an **18,517-substance lexicon**, so `doxazosin` is refused as a condition
*because it is a drug*, not because anyone deny-listed it.

Neither phase alters `ground_chunk`, the generic trigger semantics, or the frozen
gate-1/2/3 policies; the matcher runs only over the two selected clinical
sections.

The release-mode `lexicon`, `vocabulary`, `matcher` and `losses` commands
reproduced these results on 2026-07-14. This live validation freezes engineering
behavior only; **clinical recall and judge quality remain unmeasured** until the
two-reviewer held-out reference is complete, and no number above may be described
as a recall improvement.

**A fourth failure, exposed by adversarial verification rather than by the
target (2026-07-14) — and it was a PIPELINE gap, not merely a reporting one.**
Phase 2 changed the *extractor* (R6 distribution, the concept length cap), but
the prepared workspace on disk was never regenerated, so every downstream
artifact was computed against a **stale candidate set** (314 candidates where the
code produces 334) while being reported as "rebuilt from nothing". It would be
self-serving to file this under bad reporting: **the pipeline had no mechanism
that could detect a workspace its own extractor would no longer produce.** The
tooling made the wrong claim reachable, and any operator re-running it would have
hit the same wall silently. The guard below is the actual fix; correcting the
prose alone would have left the failure mode intact. The workspace is now regenerated (334 candidates / 668
rows; the accepted 27 calibration cases survive unchanged — every evidence span
is still an exact substring and still in the calibration split), and
`assert_workspace_fresh` now **refuses to run** `matcher` or `losses` against a
workspace the current extractor would not reproduce (exit 1). The guard is what
makes the reproducibility claim checkable instead of asserted.

**Three failures the accepted target exposed, none visible without it:**
1. An arbitrary **90-character cap** silently deleted exactly the qualifiers R1
   exists to preserve (the accepted R6 concept is 99 characters). It alone
   accounted for both missing vocabulary terms.
2. **R6 was implemented only in the matcher**, not in the extractor that *builds*
   the vocabulary — so the matcher could distribute a shared refractory
   restriction the vocabulary could never contain, and the matcher may only
   assert vocabulary it was given.
3. Mining a catalog title's **brand segment** typed *glaucoma, asthma, psoriasis,
   diabetes* and *pain* as **drugs** — homeopathic products are named after the
   condition they claim to treat. That regression *removes* true facts, so it
   hides inside an aggregate; the accepted calibration caught it in one run by
   breaking the TRUE case `glaucoma`. Surviving rule: parenthesized generic only,
   must differ from the brand, never split hyphens. **A false drug destroys true
   facts; a missed drug only risks a junk candidate the other rules still clear.
   Precision wins.**

Consequences, recorded honestly:
- **The original generic clinical vocabulary is inadequate.** It
  grounds facts like `TREATS(product, pain)`; the accepted calibration says
  that is FALSE and
  the truth is *neuropathic pain associated with diabetic peripheral
  neuropathy / postherpetic neuralgia / fibromyalgia*. Against the reference,
  several facts it builds are false positives at the accepted granularity. The
  corpus-derived vocabulary alone was NOT a fix: it repaired coverage (17/19) but
  end-to-end assertion stayed at 11/19, because the trigger templates could not
  see how labels actually assert. Only the clinical matcher (phases 1–2) closed
  that, reaching 19/19 on the showcase while holding every negative case. The
  mechanism-level restraint and self-healing results are unaffected; they never
  depended on vocabulary coverage.
- **The reference instrument was rebuilt** (D2 unchanged): candidate
  generation no longer matches a generic vocabulary. It now surfaces the
  label's OWN concept phrases, deterministically (cue-anchored spans,
  coordinated lists split, bare contraindication lists honored per R4,
  population/purpose clauses trimmed) — **no LLM**, because an LLM proposer
  would shape the reference by omission. Candidates surface, they never label.
  The 50-document workspace was regenerated: **334 unique candidates** (was
  106) — copied to both reviewer files, i.e. **668 unreviewed annotation rows**,
  which is 334 candidates x 2 independent reviewers and must not be reported as
  668 candidates. The
  concepts the gold actually needs now appear.

This is the value of insisting the reference be human and concept-preserving:
a model-labeled or vocabulary-shaped gold would have quietly scored the
mechanism against its own blind spot and reported a flattering number.

## Boundaries kept

- No LLM-as-oracle; no auto-generated clinical fact labels.
- Interactions/warnings relations deferred.
- Discontinued/license-gated external clinical databases not used.
- Recall claims wait on domain-expert labels; nothing in the buildable-now
  scope asserts clinical recall.
