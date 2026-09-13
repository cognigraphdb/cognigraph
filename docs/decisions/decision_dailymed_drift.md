# Decision: DailyMed pilot drift experiment (D1–D6)

**Status:** Decided 2026-07-13 (user approved all six). The post-baseline
drift step from `docs/dailymed-pilot.md`; runs the frozen
`gate3-policy-v1` against real label revisions.

## Context

DailyMed's structured data exists for each *version* of a label, so we
have ground truth for what a graph *should* change when a label is
revised — the first ground-truth-scored measurement of graph
*maintenance* (not just construction) in the programme, and the
real-data successor to the synthetic self-healing experiment
(`docs/self-healing-experiment.md`). Probe (2026-07-13): 6 of 8 sampled
baseline Set IDs are multi-version (up to 19 versions spanning years) —
abundant signal.

**Feasibility resolution (the D1 flag) — no clean historical oracle
exists.** Probed every structured path (2026-07-13, evidence in the
session): the DailyMed v2 API, `downloadzipfile.cfm` (by set-id+version
and by per-version document GUID), the openFDA drug-label API, and the
monthly bulk archives **all serve only current state** — DailyMed and
openFDA do not publish historical structured data in any queryable form.
The versioned HTML page (`lookup.cfm?setid=<id>&version=<n>`) is the sole
historical source; its ingredient data is a structured HTML table
(`formTable` markup, ingredient-name cells) but UNII codes are present
only inconsistently, so a historical oracle from it is name-based and
scrape-dependent.

**Two-method plan (user-approved 2026-07-13):**
- **Method B — synthetic controlled drift (primary, run first).**
  Real baseline documents carry a clean current XML oracle; inject KNOWN
  deltas (remove a grounded active ingredient from the prose; add a
  foreign ingredient via a composition sentence; reword with no
  composition change = administrative-only) and measure whether the
  frozen `gate3-policy-v1` narrative grounding tracks the injected change.
  Ground truth is exact because we author the delta — this preserves the
  pilot's oracle-rigor bar. Trades real-revision provenance for airtight
  measurement (the self-healing experiment's method, on the pilot corpus).
- **Method A — real revisions via the HTML ingredient table (follow-up).**
  Parse the versioned page's ingredient table for a name-based per-version
  oracle over real label revisions; external-validity check, lower oracle
  rigor, hard-capped cohort. Deferred to a later session.

Drift centers on `CONTAINS_INGREDIENT` (the flagship formulation-drift
relation) in both methods; route/form drift stays out of scope.

## Decisions (owner: user, 2026-07-13)

**D1 — Drift cohort + version source.** A bounded, deterministic cohort
of baseline Set IDs with ≥2 versions (via `/spls/{setid}/history.json`),
fetching an older version alongside the pinned current from the versioned
HTML page (cached under `data/dailymed-drift/`, git-ignored).
Administrative-only revisions are included deliberately — they are the
drift restraint test.

**D2 — Drift = the versioned fact diff under the frozen policy.** For
each Set ID, construct v_old and v_current under `gate3-policy-v1`
(unchanged — drift tests the frozen policy, it does not re-tune it) and
compute the fact delta (added / removed / persisted) for the narrative
and structured layers separately. Facts carry a version stamp.

**D3 — Oracle-scored drift correctness (the crown jewel).** The
structured UNII delta between versions is ground truth for what should
change. Score whether the pipeline's delta matches: did the graph add
facts that became true, remove facts that became false, and preserve the
unchanged.

**D4 — Deterministic re-construction for the structured-backed relation;
self-healing reserved for pass-2.** For `CONTAINS_INGREDIENT`, drift
handling is deterministic: re-ground v_new and re-gate against v_new's
structured oracle — the production graph mirrors the structured delta by
construction, so drift correctness of the production graph is exact. The
narrative-evidence layer is where the empirical question lives: when a
formulation changes, does the prose track it? The self-healing machinery
(degradation_report, gap-directed rewiring) is reserved for pass-2
clinical relations with no structured oracle, where a revision can
dead-end a neuron pathway with nothing to re-gate against.

**D5 — Version provenance.** Facts carry a version stamp (extends the
`provenance` field, `decision_dailymed_ontology.md` D2); a per-Set-ID
drift record captures the delta with per-version provenance. Back-compat:
single-version facts unaffected.

**D6 — Deliverable, acceptance, taxonomy.** Produces: per-cohort drift
statistics, oracle-scored drift correctness (structured delta vs graph
delta), narrative-evidence drift (does prose track formulation change),
and a taxonomy of revision types (reformulation vs administrative-only).
Acceptance: production-graph drift matches the structured version delta;
administrative-only revisions yield ~zero fact delta (restraint);
reproducible from the pinned version pairs.

## Boundaries kept

- Pass-2 clinical drift and self-healing rewiring stay out of scope — no
  structured oracle for them yet.
- Route/form drift is out of scope (not cleanly HTML-available).
- The frozen `gate3-policy-v1` is not re-tuned by this experiment.

## Outcome

No clean historical structured oracle could be recovered from the available
DailyMed/openFDA/versioned sources, so the primary test used controlled deltas
on real baseline documents. The frozen policy tracked removal 15/15, addition
30/30, and administrative no-op 30/30. A real-revision HTML follow-up found
23/25 sampled revisions administrative and held restraint on 20/23 of those.
This supports deterministic maintenance for the structured-backed relation; it
does not establish automatic re-ingestion repair for ordinary stored graphs or
pass-2 clinical relations.
