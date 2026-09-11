# DailyMed operational pilot — governed construction on 10,000 real FDA drug labels, with the programme's first external ground-truth oracle

- Date: 2026-07-14
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:1332-1464` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **DailyMed operational pilot — governed construction on 10,000 real FDA
  drug labels, with the programme's first external ground-truth oracle.**
  The Semantic Neurons loop, run cold on a pinned public NLM snapshot
  whose structured data (UNII ingredients, routes, forms, labelers) is
  used as mechanical ground truth. Staged as gates plus drift and a
  clinical pass, each with its own decision record under `docs/decisions/`
  and honest results under `fixtures/semantic-neurons/dailymed/`:
  - **Gate 1 / mechanical pass:** section-aware chunking validated; the
    governed drafter confirmed as vocabulary bootstrap; the gate advisor
    surfaced the interaction-section leakage signature; oracle-scored
    construction of the mechanically-checkable relations.
  - **Gate 2 — first oracle-scored judge measurement:** the two-stage
    judge rejected ~82% of the deterministic layer's false positives
    (precision 69→79%), stable across three samples; **judge confidence
    does not separate its mistakes**, which drove the structured-first
    production design.
  - **Gate 3 — frozen `gate3-policy-v1` baseline over the full 10k:** a
    **structured-agreement gate** yields a **129,091-fact production graph
    at 100% precision with zero cross-product leakage**; per-relation
    trigger freeze chosen by measurement; the pre-registered grounding
    scaling wall hit and was resolved with a provably-lossless mention
    prefilter (2+ CPU-hours → ~25 min).
  - **Drift:** no clean historical structured oracle is published (every
    DailyMed/openFDA path serves current state), so drift was measured by
    synthetic controlled deltas (removal 15/15, addition 30/30,
    administrative 30/30 — deterministic maintenance confirmed) with a
    real-revision HTML follow-up (23/25 revisions administrative;
    restraint 20/23).
  - **Pass-2 clinical (no oracle):** `TREATS`/`CONTRAINDICATED_IN`
    restraint measured at the mechanism level (recall 4/4, restraint 8/8
    across negation, modality, co-mention) with zero structural leaks; a
    **self-healing capstone** where a reworded indication is detected,
    re-proposed, judge-gated, and restored without an oracle. Clinical
    recall remains deferred to a domain-expert reference by design. A new
    blinded reference toolkit prepares the deterministic 10-calibration +
    40-held-out cohort, validates two independent evidence-backed annotation
    files, generates an adjudication queue, and scores grounding plus judge
    quality only after the human reference is complete.
  - **Clinical calibration caught a real construction failure (2026-07-14).**
    Project-owner showcase calibration froze a *concept-preserving* rule (a qualifier
    may be dropped only if it does not change the concept's extension:
    *systemic fungal infection* != *infection*). Scored against it, only **3 of
    24** candidates the hand-picked 30-term condition vocabulary surfaced were
    TRUE — the clinical construction vocabulary, not the reference, was the
    defect. A reference shaped around our own vocabulary would have hidden it.
    Earlier global ceiling/attainment percentages were retracted because they
    deduplicated concepts across documents, merged relation vocabularies, and
    used noisy extractor candidates rather than gold. The corrected instrument
    counts `(set_id, relation, concept)` instances and keeps relations separate.
    A content-hashed, project-owner accepted showcase calibration now records
    19 TRUE and 8 FALSE cases with exact evidence. It is explicitly not external
    clinical validation. Two isolated matcher phases were validated against it,
    each frozen before the held-out labels were opened:

    | | phase 1 | phase 2 |
    |---|---|---|
    | accepted TRUE asserted | 13/19 | **19/19** |
    | corpus vocabulary coverage | 17/19 | **19/19** |
    | end-to-end | 11/19 | **19/19** |
    | accepted FALSE refused | 8/8 | **8/8** |
    | synthetic restraint probes refused | 14/14 | **14/14** |

    **Phase 1** (`clinical-matcher-v1`) made the clinical matcher list- and
    bullet-aware: template triggers of the form `indicated for {target}` cannot
    see how labels actually assert, because labels enumerate ("indicated for the
    treatment of A, B, and C" — a template only ever matches *A*), bullet, and
    prohibit in prose ("should not be given to patients with ..."). It runs ONLY
    over the two selected clinical sections; `ground_chunk` and the frozen gates
    1-3 are untouched. It deliberately refused every cue-less contraindication
    list, accepting six misses rather than manufacturing drug-name facts.
    **Phase 2** (`clinical-matcher-v2`) closed those six **without costing a
    single negative case** — restraint did not trade against coverage. Bare
    contraindication lists are admitted through CONDITION TYPING, and the
    discriminator did not have to be authored: it is mined from data already held.
    The corpus's STRUCTURED ingredient data (the UNII-anchored block gates 1-3
    trusted) plus the FULL DailyMed catalogs enumerated during collection
    (151,759 titles — every MARKETED label, not just the 10k sampled) yield an
    **18,517-substance lexicon**, so `doxazosin` is refused as a condition
    *because it is a drug*, not because anyone deny-listed it. Catalog titles
    carry the generic in parentheses ("MULTAQ (DRONEDARONE)", "TEKTURNA (ALISKIREN
    HEMIFUMARATE)"), and salt/hydrate suffixes are stripped so both the moiety
    (*aliskiren*) and the salt type as drugs — closing the earlier corpus-bounded
    hole, where a drug named in contraindication prose but never an active
    ingredient of a *sampled* label could not be typed. Clauses and bare modifiers
    are refused structurally; INDICATIONS get no bare-list path. Nothing in the
    typing module names a calibration concept.
    **A trap the accepted calibration caught instantly:** mining a title's BRAND
    segment poisons the lexicon, because homeopathic products are named after the
    condition they claim to treat ("GLAUCOMA (ACONITUM NAP., ...)", "PSORIASIS
    SYMPTOM RELIEF (...)"). A first cut typed *glaucoma, asthma, psoriasis,
    diabetes* and *pain* as DRUGS — which would have silently deleted real
    conditions from every contraindication list — and it broke the accepted TRUE
    case `glaucoma` on the spot. The surviving rule: mine only the parenthesized
    generic, require it to DIFFER from the brand (a "NEKVNRO PSORIASIS
    (PSORIASIS)" echo is not a generic name), and never split on hyphens
    ("pain-relieving" -> "pain"). The asymmetry is deliberate: **a false drug
    destroys true facts, while a missed drug only risks a junk candidate the other
    rules still have to clear.** Precision wins; regression tests pin all three
    traps.
    Phase 2 also surfaced **two bugs worth more than the feature**: an arbitrary
    90-character cap was silently deleting exactly the qualifiers R1 exists to
    preserve (the accepted R6 concept is 99 characters), and R6 was implemented
    only in the matcher and not in the extractor that *builds* the vocabulary —
    so the matcher could distribute a shared refractory restriction the
    vocabulary could never contain.
    The three losses are now a true PARTITION of the 270 candidate instances
    (asserted candidates 168 | vocabulary 32 | trigger/structure 53 | type noise
    17 — these sum to exactly 270; the three losses DO sum to the 102 unasserted
    candidates, which is what a partition means — what must not happen is reading
    that sum as a score, or adding it to the matcher's 174 TOTAL assertions, which
    include 6 found outside the candidate list and are NOT a partition member);
    previously typed-out junk was double-counted as "vocabulary loss", conflating
    a typing bug with a coverage bug.
    Release-mode live runs reproduced every result. **None of this is a clinical
    recall claim:** every number is measured against a project-owner development
    showcase, and the two-reviewer 40-document held-out evaluation — the only
    thing that can produce recall — remains pending.
    Fixed: the clinical vocabulary is now **corpus-derived** (4,047 condition-typed terms from
    8,765 labels, content-hashed, built from label text with the 50 reference
    documents excluded; covers 19/19 accepted showcase-calibration TRUE cases (17/19 before the
    length-cap and R6-extractor fixes) and lifts
    real-corpus clinical grounding from 3 facts to 59 — 18 TREATS + 41
    CONTRAINDICATED_IN, zero leaks; down from an earlier 66 once the vocabulary was
    condition-typed, the 7 lost being `known` x5, `moderate` and `severe` —
    all prose modifiers, no drug names — that typing now correctly refuses).
    Grounding is now scored in three explicit lanes — frozen-generic
    (end-to-end headline), corpus-derived (end-to-end improved), and
    oracle-vocabulary (**diagnostic only**; it supplies the gold concepts and
    was previously conflated with end-to-end recall).
  - The evidence dossier (`docs/semantic-neurons/positioning.md`) gains
    items 19–23 and an updated abstract seed. The construct crate now exposes
    the reference-workflow types and checks used by the example; no server or
    HTTP API surface changed.
