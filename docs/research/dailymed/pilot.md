# DailyMed 10k-document pilot corpus

Current example scratch output uses ignored `data/dailymed/`. Historical
`fixtures/semantic-neurons/dailymed/` paths below identify sealed private captures;
see the [artifact catalog](../../evidence/research-dailymed.md). Prepare inputs
under the current [evidence policy](../../operations/evidence-policy.md) before a
new experiment. The dated results below have not been rerun for this move.

This workflow builds the real-corpus input for the first operational
Semantic Neurons pilot. It is a corpus acquisition step, not a clinical
application, and its output must not be used as medical advice.

## Corpus contract

The default run accepts exactly:

- 9,000 current "HUMAN PRESCRIPTION DRUG LABEL" SPL documents
  (DailyMed document type "34391-3");
- 1,000 current "HUMAN OTC DRUG LABEL" SPL documents
  (DailyMed document type "34390-5").

The source is the official
[DailyMed API v2](https://dailymed.nlm.nih.gov/dailymed/app-support-web-services.cfm).
DailyMed provides current SPL XML by Set ID and a filtered "/spls" catalog.
One Set ID identifies the version family; the catalog records the current
version used by this baseline.

The collector ranks the complete prescription and OTC catalogs independently
with SHA-256 over the fixed seed, label kind, and Set ID. A document is accepted
only when its XML declares the expected type and its normalized label contains
at least two non-empty narrative sections and 500 characters. Rejected
candidates are recorded, and the collector continues in deterministic rank
order until each quota is full.

This is intentionally a document-level corpus. Do not compare its size directly
with the existing 10,000-**chunk** construction dry run.

## Generate the corpus

From the repository root:

~~~bash
cargo run --release -p cognigraph-construct --bin dailymed-pilot --
~~~

The explicit equivalent is:

~~~bash
cargo run --release -p cognigraph-construct --bin dailymed-pilot -- \
  --output data/dailymed-pilot \
  --rx 9000 \
  --otc 1000 \
  --seed cognigraph-dailymed-pilot-v1 \
  --concurrency 8
~~~

No Hugging Face credential is used. The source is DailyMed, and the collector
does not read ".env".

The first run enumerates the two filtered DailyMed catalogs. Every catalog page
is checkpointed before the final catalog is assembled, so an interrupted run
can resume without repeating successful page requests. Raw XML is also cached
per Set ID. Use "--refresh-catalog" only when deliberately creating a baseline
from a newer DailyMed database snapshot.

For a metadata-only preflight:

~~~bash
cargo run --release -p cognigraph-construct --bin dailymed-pilot -- \
  --catalog-only
~~~

Verify every manifest entry, raw checksum, normalized-text checksum, and
document field without making a network request:

~~~bash
cargo run --release -p cognigraph-construct --bin dailymed-pilot -- \
  --output data/dailymed-pilot \
  --verify-only
~~~

## Output

The generated directory is ignored by Git because a complete corpus can occupy
multiple gigabytes:

~~~text
data/dailymed-pilot/
├── catalog/
│   ├── rx.json
│   ├── otc.json
│   ├── pages-rx/
│   └── pages-otc/
├── raw/
│   └── <set-id>.xml
├── documents/
│   └── <set-id>.json
├── manifest-rx.jsonl
├── manifest-otc.jsonl
├── rejections-rx.jsonl
├── rejections-otc.jsonl
├── request.json
└── run.json
~~~

Each normalized document preserves:

- Set ID, SPL version, publication date, title, and document type;
- source URL and retrieval timestamp;
- SHA-256 checksums for both raw XML and normalized text;
- section codes, titles, and narrative text;
- a combined text representation for downstream chunking.

The manifest repeats operational fields without duplicating the document text.
Keep "run.json", both catalogs, manifests, and rejection logs with every
published result. They define the baseline population and selection process.
The collector refuses to reuse an output directory when its "request.json"
has a different seed, quota, quality threshold, or catalog snapshot.
The raw directory may contain a few candidates prefetched by the final bounded
request batch but not selected into the manifest. Verification requires every
normalized document to be manifested and permits those inert raw-cache extras.

## Recorded baseline acquisition (2026-07-13)

The first full acquisition completed against the DailyMed database snapshot
published July 10, 2026 at 08:06:22 PM EST:

| population | accepted | candidates examined | rejected | mean characters | mean sections |
|---|---:|---:|---:|---:|---:|
| Human prescription | 9,000 | 10,176 | 1,176 | 19,734 | 13.59 |
| Human OTC | 1,000 | 1,086 | 86 | 1,497 | 9.81 |

The output occupies 1.9 GB: 1.5 GB of cached source XML, 378 MB of normalized
documents, and 72 MB of catalog checkpoints. There are 10,000 unique manifested
Set IDs with no overlap between the prescription and OTC populations. These are
acquisition facts only; no construction, reviewer-cost, or answer-quality claim
is implied until the staged pilot runs are completed.

## Reproducibility and restart rules

- Reusing the same completed catalog and seed produces the same selection
  order.
- Cached raw XML makes document downloads resumable.
- A catalog database date is checked across every API page. If DailyMed changes
  during enumeration, the run fails instead of silently mixing snapshots.
- Do not use "--refresh-catalog" during a measured pilot.
- Changing quality thresholds or the seed defines a different corpus and must
  produce a separately named baseline.

The "retrieved_at" value records collection time, while content identity is
anchored by the raw and normalized SHA-256 checksums.

## Pilot staging

Use the generated documents in three gates:

1. Run 100 documents to validate section-aware chunking and the bounded
   ontology.
2. Run 1,000 documents to calibrate proposal limits, reviewer thresholds, and
   cost instrumentation.
3. Freeze those policies before processing the full 10,000-document baseline.

After the baseline, choose a fixed cohort of Set IDs with multiple versions and
run the separate drift experiment. Historical versions are not added to the
10,000-document baseline count.

## Gate 1 results (2026-07-13)

Runner: `cargo run --release -p cognigraph-construct --example
dailymed_gate1` (deterministic cohort: first 90 rx + 10 OTC by
`selection_rank`; `--space FILE` reanalyzes without re-drafting).

**Section-aware chunking — validated.** 100 documents, 1,362 sections →
1,674 chunks (101 sub-40-char stubs skipped, 151 oversized sections
split at sentence boundaries). Sizes: p50 902 / mean 1,023 / p95 1,990 /
max 2,781 chars — well inside grounding and proposal windows.

**Bounded ontology — the drafter behaves exactly as positioned
(vocabulary bootstrap, not coverage).** From a 24-chunk sample
(~1.4% of the cohort) draft-policy-v1 produced 47 entities and 20
relation rules, one closure drop, zero verbatim failures. Every rule
grounds on the full cohort (20/20 fired; 60 grounding events → 20
distinct facts across 14 relation labels) but coverage is inherently
sample-bound: 13 of 1,674 chunks carry evidence. Per-product rules
drafted from one label cannot cover a 100-product corpus.

**The advisor found the domain's restraint signature immediately.**
13 of 20 rules flagged; the repeated pattern — an endpoint like "oral
contraceptives" appearing in NO licensing sentence — is the
drug-interaction-section version of cross-company leakage: a rule
drafted from one product's label will fire on other products'
interaction sections that mention the same co-medication. Gates (or
`{source}`/`{target}` template triggers) are mandatory before gate 2.

Draft artifacts for review: `fixtures/semantic-neurons/dailymed/`
(`gate1-draft-space.json`, `gate1-draft-report.json`,
`gate1-analysis.json`). Gate-2 precondition: an authored, domain-shaped
ontology (typed endpoints, template triggers, sentence gates on
interaction-prone relations) with the draft as vocabulary input.

## Gate 1.5 — mechanical pass with a ground-truth oracle (2026-07-13)

Design: `docs/decisions/decision_dailymed_ontology.md` (D1–D6). Runner:
`cargo run --release -p cognigraph-construct --example dailymed_mechanical`
(deterministic, offline, zero LLM, zero embeddings). The pass constructs
only the mechanically-checkable relations and scores narrative grounding
against the SPL structured data — the first ground-truth-verified
measurement in the programme (every prior eval scored against
hand-authored specs).

On the gate-1 cohort (90 rx + 10 OTC): 100 products, a typed vocabulary
of 157 ingredients / 15 routes / 24 forms / 77 labelers, 27,300 candidate
rules (oracle-blind cross-product, D4), subject-scoped ingest (D5) in
1.6 s → 229 distinct narrative facts against 509 structured oracle facts.

| relation | recall | precision |
|---|---:|---:|
| CONTAINS_INGREDIENT | 139/189 | 91% |
| ADMINISTERED_VIA | 66/120 | 89% |
| HAS_DOSAGE_FORM | 1/100 | 100% |
| MARKETED_BY | 2/100 | 100% |

`HAS_DOSAGE_FORM` and `MARKETED_BY` recall is near-zero exactly as D1
predicted: SPL display forms ("TABLET, FILM COATED") and labeler legal
names rarely appear verbatim in prose. Recall on the two prose-carried
relations is the real signal, and the 21 out-of-oracle facts are a
clean **findings taxonomy**, not fabrications:

- **Oracle-scope (real, not a fabrication):** excipients grounded from
  prose ("SODIUM CHLORIDE, usp" — confirmed `IACT`; povidone) that an
  active-ingredient-only oracle correctly omits. The graph is right; the
  oracle is narrow. A gate-2 fix is a two-tier oracle (active vs
  inactive).
- **Trigger over-breadth:** `of {target}` fires on drug-interaction and
  comparison prose (amitriptyline "of TOPIRAMATE/CIMETIDINE", naproxen
  "of ASPIRIN") — the template catches co-mentioned drugs. This is the
  precise target list for gate 2's relation_hint + gate-advisor loop.
- **Within-document subject leak:** an oral product's label discussing
  the IV form ("INTRAVENOUS administration") grounds `ADMINISTERED_VIA
  intravenous`. Subject scoping (D5) blocks cross-document leakage but
  not a product's own prose about other forms; a sentence/section gate
  is the lever.
- **Name granularity:** salt vs base ("LIDOCAINE HYDROCHLORIDE" grounded,
  oracle substance "LIDOCAINE") — an alias/active-moiety join fixes it.

Results and the full inspection list:
`fixtures/semantic-neurons/dailymed/mechanical-pass-results.json`. These
four categories, each with an identified lever, are what gate 2
calibrates — now with recall/precision measured against truth rather
than estimated.

## Gate 2 — calibration and the first oracle-scored judge measurement (2026-07-13)

Runner: `cargo run --release -p cognigraph-construct --example dailymed_gate2`
(`--dry` validates the plan and prints the LLM call budget without spending;
run it first). Cohort: 900 rx + 100 OTC. Two oracle-grounded experiments,
both scored against the SPL structured data (active **and** inactive
ingredients, routes — the full-ingredient oracle, cleaner than gate 1.5's
active-only view).

**Construction at 1,000-document scale — precision falls with vocabulary.**
Deterministic subject-scoped construction produced 2,993 grounded facts in
~110 s, **2,050 true / 943 false** against the oracle — 69% precision,
down from ~90% at 100 documents. The cause is exactly the gate-1.5 finding
scaled up: a larger ingredient vocabulary gives the `of {target}` template
far more co-mentioned drugs to catch. This 943-false-positive population is
what the judge must clean up.

**Experiment A — the judge as an oracle-scored false-positive filter (the
crown jewel).** 40 true + 40 false grounded facts, each turned into a
relation_hint carrying its grounding trigger and put through the two-stage
judge; each verdict scored against oracle truth. This is the first
ground-truth-verified measurement of judge quality in the programme (every
prior eval scored against hand-authored specs). Result (stable across three
independent runs at 74–76% oracle agreement):

| | accepted | rejected | needs_human |
|---|---:|---:|---:|
| oracle-true | 27 | 13 | 0 |
| oracle-false | 7 | 33 | 0 |

The judge is a **strong precision filter**: it rejected 33 of 40 false
facts (≈82%), lifting the accepted set's precision to ~79% from the
deterministic layer's 69%, at a recall cost (it kept 27 of 40 true facts,
68%). The 13 rejected-true are not judge errors on inspection — they are
cases where the grounding trigger fired on a warnings or clinical section
rather than a composition statement, so the shown evidence does not plainly
assert the fact. The judge scores **evidence support**, the oracle scores
**world truth**; the gap between them is itself the signal (a recall
problem in the evidence, not a reasoning failure). Oracle caveat: the
salt/base name granularity from gate 1.5 means a few "false" labels are
name mismatches, so the judge's measured precision is a lower bound.

**Experiment B — the full propose → judge loop end to end.** 30 active
-ingredient recall gaps run through `propose_neurons_via_backend` and
judged: 18 proposed (12 declined by proposer restraint — "no explicit
supporting sentence"), 8 judge-accepted → **27% net gap closure** on a hard
relation (product composition, which prose states inconsistently).
Proposer restraint declining 40% of gaps for lack of compositional
evidence is the loop working as designed — no hallucinated proposals
survived to the graph.

**Calibration:** ~214 LLM calls, ~6 s per judgment, ~10 min wall-clock for
the whole gate at mini-class pricing. Full results and every verdict with
its reasoning: `fixtures/semantic-neurons/dailymed/gate2-results.json`.

Harness notes (found and fixed while building the gate, recorded for
honesty): the judge schema's verdict enum is `accept`/`reject` (an early
run compared against `accepted`/`rejected` and mis-bucketed every verdict);
the proposer validates a hint's relation against the space's declared
relation vocabulary (it must be passed relation_rules); and product
entities need their drug-name alias for compositional evidence to be
retrievable. All three are fixed in the runner.

## Gate 3 — frozen baseline over the full 10,000 documents (2026-07-13)

Policy: `docs/decisions/decision_gate3_baseline.md`, frozen as
`gate3-policy-v1` (`fixtures/semantic-neurons/dailymed/gate3-policy-v1.json`).
Runner: `cargo run --release -p cognigraph-construct --example dailymed_gate3
-- --rx 9000 --otc 1000`. The trigger freeze was chosen by measurement
(`--calibrate`), per relation, on the 1,000-doc cohort.

**Grounding cost at scale — the QW7 optimization was required.** The naive
full cross-product (every document grounding against the whole ~7,000-name
ingredient vocabulary) did not finish in over two CPU-hours at 10k — the
scaling wall `decision_dailymed_ontology.md` D4 pre-registered. A per-
document mention prefilter (build rules only for vocabulary surfaces that
occur in the document; substring-exact, so provably lossless — verified
identical output at 1,000 docs) brought the full run to ~25 min (6.6
docs/s). A proper Aho-Corasick multi-pattern pass is the recorded next
optimization if the baseline is re-run often.

**The production graph: 129,091 facts at 100% precision, zero leakage.**
Structured-agreement gate (D3) over 10,000 products:

| bucket | facts | role |
|---|---:|---|
| agreed (narrative + structured) | 12,236 | auto-accepted, evidence-bound |
| structured-only (imported) | 116,855 | oracle facts prose did not state |
| **production graph (agreed + structured-only)** | **129,091** | **100% oracle-backed** |
| narrative-only | 4,384 | the review queue |

Subject scoping held structurally: **0 cross-product leaks** across all
16,620 narrative facts. Per relation: `CONTAINS_INGREDIENT` narrative 8,327,
agreed 5,534 (active-recall 24% of 21,279 active-ingredient facts);
`ADMINISTERED_VIA` narrative 8,293, agreed 6,702 (recall 62% of 10,753).

**Acceptance: zero-leakage PASS; narrative-precision 74% — below the 80%
proxy target (FAIL), and this is the informative result.** Narrative
precision fell 85%→74% from 1k to 10k: a larger ingredient vocabulary gives
the composition templates more spurious substring matches. Critically, this
does **not** touch the production graph, which is 100% precise by
construction — the structured-agreement gate absorbs the imprecision exactly
as designed (D3). The only cost is a larger review queue (4,384 narrative-
only facts). The 80% bar was a narrative-layer proxy; the load-bearing
acceptance — a 100%-precise production graph with zero leakage, reproducible
from the pinned snapshot — holds.

**Certified judge quality ON the baseline** (D6; 80 facts sampled from the
agreed and narrative-only buckets, scored against the structured oracle):
accept 38 true / 10 false, reject 2 true / 30 false, **oracle agreement
68/80 = 85%**. The judge held up at 10k scale (85%, vs 91% at 1k and 75% in
gate 2); its false-accepts rose with the noisier narrative layer, consistent
with the precision drop. This is the third independent confirmation that the
judge is a strong precision filter whose residual errors are confident and
so cannot be threshold-gated — the measurement that drove D3/D4.

Results: `fixtures/semantic-neurons/dailymed/gate3-baseline-results.json`.
The drift experiment (fixed multi-version Set-ID cohort) remains the recorded
post-baseline step.

## Drift — graph maintenance under revision (2026-07-13)

Policy: `docs/decisions/decision_dailymed_drift.md`. **No clean historical
structured oracle exists** — the DailyMed v2 API, `downloadzipfile.cfm`
(by set-id+version and by per-version document GUID), the openFDA
drug-label API, and the monthly bulk archives were all probed and every
one serves only current state. So drift is measured by **Method B —
synthetic controlled drift**: real baseline documents carry a clean
current XML oracle; inject a known delta and check whether the frozen
`gate3-policy-v1` narrative grounding tracks it. Ground truth is exact
because the delta is authored. (Method A — real revisions via the
versioned HTML ingredient table, a name-based oracle — is the recorded
external-validity follow-up.)

Runner: `cargo run --release -p cognigraph-construct --example
dailymed_drift`. One controlled edit per document over 90 documents,
`CONTAINS_INGREDIENT`:

| edit | result | what it shows |
|---|---:|---|
| **removal** (delete a grounded ingredient's name from the prose) | **15/15** | grounding drops the fact when the ingredient leaves the label |
| **addition** (inject a composition sentence for a foreign ingredient) | **30/30** | grounding picks the fact up when an ingredient is added |
| **administrative** (append boilerplate, no composition change) | **30/30** | no spurious drift on a no-op revision (restraint) |

Deterministic grounding tracks composition change exactly and shows
restraint on no-op revisions; combined with the structured-agreement gate
(production graph = structured, so it mirrors any structured delta by
construction), **graph maintenance under revision is deterministic and
correct for the structured-backed relation** — confirming D4 on real
documents with a clean injected oracle. The self-healing machinery stays
reserved for pass-2 clinical relations that have no structured truth to
re-gate against.

Honesty note: a first run scored addition 28/30; both misses were the
same ingredient landing on a fixed-size chunk boundary that split the
trigger phrase — a chunking artifact, not a grounding failure (whole
-section chunking cannot split it). Injecting the sentence boundary-safe
gave 30/30. Results:
`fixtures/semantic-neurons/dailymed/drift-synthetic-results.json`.

**Method A — real revisions (external-validity follow-up, 2026-07-13).**
`cargo run --release -p cognigraph-construct --example dailymed_drift_real`
fetches real version pairs (versioned HTML, cached, hard-capped and
polite) and parses the ingredient *table* for a name-based active-
ingredient oracle — lower rigor than the XML gates, stated as such. Over
25 real multi-version labels:
- **Real revisions are overwhelmingly administrative: 23 of 25** carried
  no active-ingredient change; only 2 were reformulations. A useful
  external-validity fact about how drug labels actually get revised.
- **Restraint held on 20 of 23 administrative revisions.** Unlike the
  synthetic no-op edit, a real "administrative" revision genuinely
  rewords prose, so a few grounding shifts are real rather than spurious;
  20/23 is the honest real-world number against the synthetic 30/30.
- The narrative layer tracked 0 of 11 structured removals — expected and
  uninformative here: narrative recall is low (~25%, so most removed
  ingredients never grounded), and the name-based HTML oracle is noisier
  than XML across differently-formatted versions. The rigorous
  add/remove tracking result stays with Method B; Method A's value is the
  administrative-vs-reformulation base rate and the real-revision
  restraint check. Results:
  `fixtures/semantic-neurons/dailymed/drift-real-results.json`.

## Pass-2 — clinical relations, restraint without an oracle (2026-07-13)

Policy: `docs/decisions/decision_dailymed_clinical.md`. Clinical relations
(`TREATS`, `CONTRAINDICATED_IN`) have **no structured oracle**, so this
builds only the label-independent parts; clinical *recall* ground truth is
a deferred domain-expert reference (D2). Runner: `cargo run --release -p
cognigraph-construct --example dailymed_clinical`.

**Mechanism-level restraint probe suite — the load-bearing result.**
Author-constructed clinical sentences with known answers (unit tests for
clinical language, not clinical fact labelling), sentence-gated:

| class | probes | result |
|---|---:|---|
| affirmative ("indicated for {condition}", "contraindicated in {condition}") | 4 | **4/4 grounded** |
| negation ("**not** contraindicated in {condition}") | 3 | **3/3 refused** |
| modality ("**may be considered** in…", "not an approved indication") | 2 | **2/2 refused** |
| differential co-mention (relation belongs to another subject; product absent from the sentence) | 2 | **2/2 refused** |
| bare mention (condition named, no relation trigger) | 1 | **1/1 refused** |

Recall 4/4, **restraint 8/8**. The grounding mechanism handles the clinical
patterns that make pass-2 hard: negation of an inherently-negative-polarity
relation ("not contraindicated"), hedged modality, and the pervasive
differential co-mention that the sentence gate refuses because the product
is not in the licensing sentence.

**Real-corpus structural check (200 docs, gated, fixed public condition
vocabulary).** 3 `TREATS` + 0 `CONTRAINDICATED_IN` grounded, **0
cross-product leaks** — subject scoping holds. Volume is deliberately low
(a 30-condition vocabulary with strict triggers), and **recall is not
scored** — that needs the domain-expert reference. What this establishes
without labels: the mechanism's restraint on clinical prose and the
structural guarantee, i.e. that when clinical facts are built they are
attributed only to the document's own product.

Acceptance (no-labels scope): mechanism restraint 8/8 + zero structural
leaks — **PASS**. Results:
`fixtures/semantic-neurons/dailymed/clinical-results.json`.

**Self-healing on a clinical relation without an oracle (D5 capstone).**
`cargo run --release -p cognigraph-construct --example
dailymed_clinical_selfheal`. A revision rewords an indication ("indicated
for the treatment of psoriasis" → "used in the management of psoriasis"),
so the literal trigger dies though the fact is still true. The loop:
detected the dead `TREATS` pathway (`degradation_report`); re-proposed a
trigger from the revised prose (the LLM recovered "used in the management
of psoriasis" verbatim); the judge accepted it and the verdict **cleared the frozen 0.90 policy
threshold, which the example now enforces** (an audit found it had been applying
the repair on any `accept` verdict regardless of confidence — a governance
demonstration that did not enforce its own gate; the observed runs happened to
pass, so the hole never showed in the output) (confidence 0.98 on that run; **the confidence is live
model output and varies between runs — 0.99 was observed on a later run — so it
is not a deterministic property of the pipeline. What IS deterministic: the
verdict must clear the policy threshold before it is applied**) — the judge IS
the gate here, there is no structured truth to auto-accept against; the
pathway was restored, and the co-mentioned forbidden condition (eczema)
was never revived. **Detected → re-proposed → judge-gated → restored,
restraint held throughout** — the governed loop maintaining a clinical
graph where no oracle can. This is exactly the role the positioning names
for the judge and self-healing: load-bearing precisely where structured
truth is absent.

Remaining, and genuinely label-gated: completion of the clinical **recall**
reference and judge-quality-on-clinical measurement. The missing instrument is
now implemented: `dailymed_clinical_reference` prepares a deterministic,
blinded 50-document cohort (10 calibration + 40 held-out), validates two
independent expert files and adjudication, then compiles and scores the frozen
grounder and judge. The workspace holds **334 unique candidates**, copied to each
of the two reviewer files — **668 unreviewed annotation rows = 334 x 2
reviewers**, never 668 candidates. It refuses incomplete or `unreviewed` labels and excludes
calibration from scoring by default. See
`docs/dailymed-clinical-reference.md` for the review protocol and commands.

### Clinical calibration — the vocabulary was the failure (2026-07-14)

The joint calibration review of the 10 calibration documents froze the
interpretation rules (**R1–R7**, `docs/dailymed-clinical-reference.md`), and
they immediately indicted our own design rather than the reference.

**R1, concept preservation, is governing:** a qualifier may be dropped only
when it does not change the concept's extension — *systemic fungal infection*
!= *infection*, *partial-onset seizures* != *seizures*, *hypersensitivity to
pregabalin* != *hypersensitivity*. **R4** is the safeguard: contraindications
are decided on clinical meaning, **never** on phrasing, so the gold is never
shaped around whether our trigger happens to fire.

Scored against R1–R7, only **3 of 24** candidates the hand-picked 30-term
condition vocabulary surfaced were TRUE; ~20 required concepts were never
surfaced, and 2 of 10 documents produced none. The mechanism builds
`TREATS(pregabalin, pain)` where the truth is *neuropathic pain associated
with diabetic peripheral neuropathy* — a lossy over-claim. **The generic
vocabulary, not the reference, was the defect.** A gold shaped around our own
vocabulary (or auto-labeled by a model) would have scored the mechanism
against its own blind spot and reported a flattering number.

**Measurement correction.** The earlier global concept ceiling and attainment
figures (2.1%, 88.7%, 0.4%, 0.8%) are retracted. They deduplicated concept names
across documents, merged relation vocabularies, used extractor candidates rather
than gold, and included bare-list noise. The replacement counts fact instances
as `(set_id, relation, concept)` and keeps relation vocabularies separate.

The project owner accepted a 27-case calibration artifact for this
third-party-data development showcase: 19 TRUE and 8 FALSE cases, exact evidence,
R1–R7 attribution, and a content digest. This is engineering calibration, not
external clinical validation or the two-reviewer held-out reference. With the
corrected R6 treatment of both dexamethasone list items, the corpus vocabulary
covers **17/19** accepted TRUE cases; the matcher asserts **13/19** when every
accepted concept is offered and **11/19** end to end. All **8/8** accepted FALSE
cases and **14/14** synthetic negative probes are refused. The six semantic
misses are conservative cue-less contraindication lists reserved for phase 2.

**Two fixes, both load-bearing:**
- **The vocabulary is now corpus-derived.** Built from the corpus's own
  indication/contraindication wording — 4,047 condition-typed relation-specific entries (1,421 TREATS + 2,626 CONTRAINDICATED_IN = 4,017 unique condition strings, 30 valid for both relations) from 8,765 labels,
  content-hashed and frozen before scoring, derived from label TEXT (never
  gold labels), with all 50 reference documents **excluded** so held-out
  evaluation is not scored on concepts lifted from its own documents. It
  covers **17/19** accepted showcase-calibration TRUE concepts despite never
  seeing the reference documents, and
  lifts real-corpus clinical grounding from 3 facts across 3 documents to
  **59 across 33** (18 TREATS + 41 CONTRAINDICATED_IN), still at zero
  cross-product leaks. The count fell from an earlier 66 once the vocabulary was
  condition-TYPED. The seven removed facts were checked individually: `known`
  (x5), `moderate`, `severe` — **all prose modifiers, no drug names among them**
  (a drug name never grounded through the old *template* triggers in the first
  place). The drop is still an improvement — seven bare-modifier groundings the
  typing now correctly refuses — but it must not be described as removing
  drug-name facts. Labels reuse concept
  phrasing, so the bootstrap generalizes.
- **Grounding is scored in three explicit lanes.** An oracle leak was caught
  in review before it produced a number: the scorer had been building its
  condition vocabulary from the *adjudicated gold*, handing the grounder the
  answers. Lanes are now **frozen-generic** (end-to-end; the shipped
  mechanism, and the lane that reports the design gap), **corpus-derived**
  (end-to-end; improved system), and **oracle-vocabulary** (**diagnostic
  only** — concept discovery removed from the problem; never end-to-end
  performance). Every score records its lane.

### Phase 1 — clinical assertion matcher (`clinical-matcher-v1`, 2026-07-14)

**Not a recall result. No number below may be described as a recall improvement
until the held-out expert reference is complete.**

**Metric correction first.** The earlier "attainment" figures (0.4% / 0.8%) are
**retracted**: they globally deduplicated concept strings across documents and
used a merged vocabulary. Losses are now counted as fact **instances**
(`set_id, relation, concept`) against the vocabulary **for that relation**.

**Scope, deliberately isolated.** `clinical-matcher-v1` runs ONLY over the two
selected clinical sections. It does **not** touch `ground_chunk`, the generic
trigger semantics, or the frozen gate-1/2/3 policies. It exists because template
triggers cannot see how labels actually assert: they enumerate (`indicated for
the treatment of A, B, and C` — a template only ever matches `A`), bullet
(`• Management of fibromyalgia`), and prohibit in prose (`should not be given to
patients with ...`).

It preserves, non-negotiably: exact evidence spans; product scoping (a unit whose
relation belongs to another subject — "Unlike corticosteroids indicated for
eczema…", "Agents contraindicated in asthma include…" — is refused); negation
(reusing `affirms_phrase`, read but never modified); R1 qualifiers (a generic
term is never substituted for a qualified concept); and governed vocabulary (it
can only assert terms in the relation's frozen list, exactly as neurons cannot
introduce vocabulary).

**Phase 1 is conservative on bare lists by design.** A contraindication must be
introduced by a cue; cue-less prose in a CONTRAINDICATIONS section is NOT
asserted, because the extractor's bare-list path still emits drug names
(`doxazosin`, `ketorolac tromethamine`) and prose fragments (`severe`, `rarely
fatal`) as if they were conditions. Accepting recall misses is preferable to
manufacturing drug-name facts. Condition typing is phase 2.

**Validation, on calibration only — frozen before the 40 held-out labels were
opened** (`fixtures/semantic-neurons/dailymed/clinical-matcher-v1.json`):

| check | result |
|---|---|
| accepted TRUE cases asserted, all concepts offered | **13/19** |
| accepted TRUE cases asserted end to end | **11/19** (17/19 in corpus vocabulary) |
| accepted FALSE cases refused | **8/8** |
| synthetic R1–R7 negative/broad probes refused | **14/14** |

All six misses are `CONTRAINDICATED_IN` **bare lists** (phendimetrazine's list,
dexamethasone's "Systemic fungal infections", hydrochlorothiazide's "Anuria") —
i.e. exactly the accepted cost of the phase-1 bare-list restraint, not a defect
of the matcher. Every TREATS target (enumerations, bullets, adjunctive phrasing)
asserted. The matcher refuses to freeze if any negative probe leaks.

The accepted artifact is
`fixtures/semantic-neurons/dailymed/clinical-calibration-v1.json`. Its limitation
is explicit: project-owner acceptance for a development showcase, not external
clinical validation. R6 is resolved consistently: plain `atopic dermatitis`
and `serum sickness` are FALSE for that restricted dexamethasone sentence, and
their severity/refractory-qualified concepts are TRUE.

**The three losses, reported separately** (never summed — they are distinct
bugs; `clinical-losses.json`), over 270 candidate fact instances in the 40
held-out labels (denominator is *extractor candidates*, not gold):

| loss | instances |
|---|---:|
| 1. vocabulary loss (concept absent from the relation's vocabulary) | **29** |
| 2. trigger/structure loss (expressible, but no assertion matched) | **130** |
| 3. extractor/type noise (not a condition at all — drug names, prose) | **9** |
| asserted by `clinical-matcher-v1` | 94 |

### Phase 2 — condition typing and the bare-list path (`clinical-matcher-v2`)

**Still not a recall result.** No number here may be described as a recall
improvement until the held-out expert reference is complete.

Phase 1 refused every cue-less contraindication list, at the cost of six real
contraindications. Labels write most contraindications as bare lists ("Anuria.",
"Advanced arteriosclerosis, ..., and glaucoma."), and R4 says those ARE
contraindications — so the restraint was correct but expensive. Phase 2 admits
them through **condition typing**, and the key move is that the type test is not
a hand-written deny-list:

- **A drug is not a condition — and the corpus already knows which is which.**
  A drug lexicon is built from DailyMed's *structured* ingredient data (the same
  UNII-anchored block gates 1–3 trusted as ground truth): **18,517 substances**. `doxazosin` and `ketorolac tromethamine` are
  refused because they **are** drugs, not because someone listed them.
  **The corpus-bounded hole is now closed.** `dronedarone` and `aliskiren` are
  named in contraindication prose but are never active ingredients of a *sampled*
  label, so the structured data alone could not type them. The FULL DailyMed
  catalogs — **151,759 titles**, enumerated during collection — know every
  *marketed* drug, and their titles carry the generic in parentheses: "MULTAQ
  (DRONEDARONE)", "TEKTURNA (ALISKIREN HEMIFUMARATE)". Mining those, with
  salt/hydrate suffixes stripped so both the moiety (*aliskiren*) and the salt
  (*aliskiren hemifumarate*) type as drugs, lifts the lexicon to **18,517
  substances** and closes the gap. Still no hand-written deny-list.

  **A trap worth recording, because the accepted calibration caught it
  instantly.** Mining the *brand* segment of a title poisons the lexicon:
  homeopathic products are named after the condition they claim to treat —
  "GLAUCOMA (ACONITUM NAP., ...)", "ASTHMA THERAPY (...)", "PSORIASIS SYMPTOM
  RELIEF (...)". A first cut typed **glaucoma, asthma, psoriasis, diabetes and
  pain as DRUGS**, which would have silently deleted real conditions from every
  contraindication list — and it broke an accepted TRUE case (`glaucoma`) on the
  spot. The rule that survives: mine **only the parenthesized generic**, never
  the brand; require it to *differ* from the brand (a "NEKVNRO PSORIASIS
  (PSORIASIS)" echo is not a generic name); and never split on hyphens
  (`pain-relieving` -> `pain`). The asymmetry is deliberate: **a false drug
  destroys true facts, while a missed drug only risks a junk candidate the other
  rules still have to clear.** Precision wins. Regression tests pin all three
  traps.
- **A clause is not a condition** (finite verbs, reporting language), and **a
  bare modifier is not a condition** (`severe`, `rarely fatal` — no head noun).

Typing is applied at BOTH ends: the vocabulary build (so junk never enters the
vocabulary) and the matcher's bare-list path (so a typed-out term can never be
asserted). INDICATIONS get **no** bare-list path — a cue-less sentence there is
prose, not an indication.

**Two real bugs surfaced while doing this, both fixed:**
- **An arbitrary 90-character cap was deleting exactly the qualifiers R1 exists
  to preserve.** The R6 concept "severe or incapacitating atopic dermatitis
  intractable to adequate trials of conventional treatment" is 99 characters, so
  it could never enter the vocabulary. This alone accounted for the 2 missing
  vocabulary terms.
- **R6 lived only in the matcher, not the extractor that builds the
  vocabulary** — so the matcher could distribute a shared refractory restriction
  but the vocabulary never contained the result, and the matcher may only assert
  vocabulary it was given. R6 is now implemented independently in both (the two
  modules deliberately do not share a parser).

**Validation against the project-owner-accepted 27-case artifact, frozen before
the held-out labels were opened:**

| check | phase 1 | phase 2 |
|---|---|---|
| accepted TRUE asserted | 13/19 | **19/19** |
| corpus vocabulary coverage | 17/19 | **19/19** |
| end-to-end | 11/19 | **19/19** |
| accepted FALSE refused | 8/8 | **8/8** |
| R1–R7 negative/broad probes refused | 14/14 | **14/14** |

Restraint held completely: admitting bare lists cost nothing on any negative
case. No calibration-specific allowlist exists — nothing in the typing module
names a calibration concept.

**The three losses** (a true PARTITION of the 270 candidate fact instances, so
each unasserted candidate is attributed to exactly one cause; denominator is
extractor candidates, **not gold**):

| bucket | instances |
|---|---:|
| asserted candidates | 168 |
| 1. vocabulary loss (a condition the vocabulary lacks) | 32 |
| 2. trigger/structure loss (in vocabulary, no assertion matched) | 53 |
| 3. extractor/type noise (not a condition at all) | 17 |

These four numbers **partition** the 270 candidate instances exactly
(168 + 32 + 53 + 17), so every unasserted candidate is attributed to exactly one
cause. The three losses therefore *do* sum — to the **102 unasserted
candidates** — and that is what a partition means. What must NOT happen is
reading that sum as a performance score, or adding it to the matcher's total
assertions. The three are distinct **bugs with distinct fixes**, so a single
combined figure hides which one is costing you.

`clinical-matcher-v2` asserts **174 facts in total** — 6 of them found *outside*
the candidate list, because the matcher and the reference extractor genuinely do
not share a parser. **That total is not a partition member.**

## Source and reuse caution

DailyMed distributes labeling submitted to the FDA. Preserve source URLs and
the DailyMed snapshot metadata, and review the applicable source terms before
redistributing the raw corpus. The generated corpus is for system evaluation;
its presence in CogniGraph does not validate, approve, or clinically interpret
the labeling.
