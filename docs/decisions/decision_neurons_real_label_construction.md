# Decision: Semantic Neurons construction on real FDA labels — findings and pipeline priorities

**Status:** Findings recorded (2026-07-20). Sets the priority order for construction
work. Owner: skitsanos. No code changed yet; this is the eval + the decisions it drives.

## Context

Every prior construction eval used either the WebNLG oracle or a 3-sentence toy space
(`demo_meds`). This is the **first governed run on real pharmaceutical text**: 50 real
DailyMed FDA labels through the full pipeline (draft ontology → accept → ingest → facts),
on the dev `dailymed` tenant, with the all-Gemini retrieval stack and `gpt-5.4-mini`
drafting. Goal: see what Semantic Neurons actually builds on production-shaped data, and
find where the leverage is. (Non-production data; free to modify for evals.)

## What we ran

- 50 labels → chunked by LOINC `sections` (≤2400 chars) → **393 chunks**; drug classes
  spanned NSAIDs, ACE/ARB, retinoids, corticosteroids, antineoplastics, antivirals, etc.
- Validate-small probe (5 labels) → full contract chain confirmed → full run.
- Draft `dailymed_v1` (sample_cap 40) → 35 entities / 11 relation rules → accept →
  **ingest 393 chunks → 39 facts (0.23s)** → advisor → propose/accept/re-ingest → **40 facts**.
- **~80s LLM wall time total, < $0.20.** All ingests + the advisor are deterministic and
  sub-second. Every governed step worked (`draft`, `draft/{id}/accept`, `ingest`, `advise`,
  `propose`, `neurons/{key}/accept`). Plain `/ingest` needs only tenant-active +
  mutations-healthy + space-not-materialized — **not** the M18 promotion machinery (that is
  `governed-ingest`, which fails closed without approved authority).

## Findings

### F1 — The governed pipeline works end to end on real labels
It produces correct, well-evidenced facts, e.g.:
- `Captopril --CONTRAINDICATED_WITH--> sacubitril` — *"contraindicated in combination with a neprilysin inhibitor (e.g., sacubitril)"*
- `Captopril --INTERACTS_WITH--> aliskiren` · `--AVOID_WITHIN--> sacubitril/valsartan`
- `AZSTARYS --CONTAINS--> dexmethylphenidate`
- `Betamethasone dipropionate cream --IS_CORTICOSTEROID--> corticosteroids`

### F2 — Restraint is real and the advisor is precise (the thesis holds)
A fact grounds only when **both endpoints are governed entities AND a trigger fires
verbatim/affirmed**. The deterministic gate advisor (`/advise`, no LLM) raised 6 review
flags that pinpointed **every** bad fact, including the `MAXALT→FDA` leak (flagged: source
never in the licensing sentence across 29 chunks). It auto-suggested 0 "safe" gates —
correctly, since silencing those facts is a human decision. The repair loop is
vocabulary-bounded: an in-vocab gap (`Captopril CONTRAINDICATED_WITH aliskiren`) →
proposed (inert) → accepted → re-ingested with full fact→neuron→reviewer provenance; an
out-of-vocab gap (`Amlodipine INDICATED_FOR hypertension`) → refused (unknown entity).
Neurons extend governed triples; they cannot invent entities or relations.

### F3 — THE bottleneck: stage-1 entity extraction, not grounding
On the broad 50-drug corpus, entity extraction **under-extracted condition/class
entities** — so the relations that matter most for a pharma pilot (`INDICATED_FOR`,
`CONTRAINDICATED_IN`) were **proposed and then dropped by endpoint-closure** for lack of a
governed target entity. A focused draft *proposed* a goldmine — `Amlodipine INDICATED_FOR
hypertension`, `Zolpidem INDICATED_FOR insomnia`, `TRUVADA INDICATED_FOR HIV-1`,
`captopril CONTRAINDICATED_WITH pregnancy`, `tramadol CONTRAINDICATED_WITH benzodiazepines`
— but **all ~51 were discarded** because stage-1 extracted 0 disease entities. The 5-label
*dense* probe **did** extract conditions and produced real `INDICATED_FOR` facts.
Reproducible across 3 broad drafts. **The governance is correct; it is being starved of
entities by broad, interleaved sampling.** This localizes the WebNLG "recall is capped"
result to a specific, fixable stage.

### F4 — Precision bug: cross-label leakage
`MAXALT --CONTACT--> FDA` grounded **29 times** off the generic MedWatch boilerplate
(*"contact FDA at 1-800-fda-1088…"*) present in nearly every label — 29 of 39 base edges
were this one leak, first evidenced from the *atropine* label. `TRUVADA --CONTAINS-->
lamivudine` leaked from the *entecavir* label. Root cause: interleaved chunks + **global**
entities + boilerplate trigger phrases let a trigger in label A's chunk attach to an entity
governed elsewhere. The advisor caught them, but grounding should not produce them.

### F5 — Operational gotchas
- **Entities are global** (keyed by sanitized name, no space scope): a validation subset
  that overlaps the full corpus causes entity-type collisions (a probe `Betamethasone
  [product]` vs full-draft `[drug]` → ingest failed; recovered under `dailymed_v1r`).
- **Managed collections are hard read-only via the generic API** (`403 … managed by
  governed Semantic Neuron workflows`): test-run cleanup of facts/entities/mentions/chunks/
  space_types needs host-admin or direct redb access.

## Decisions (priority order)

### D1 — Stage-1 entity extraction is the top construction priority
The pilot-relevant facts (`INDICATED_FOR`, `CONTRAINDICATED_IN`) are lost here, not at
grounding. **Confirmed by A/B (below).** The fix: **draft densely per document/drug**, not
by broad interleaved sampling. Follow-on work: bake per-document drafting into the
construction workflow, and/or prompt-tune the extractor to surface condition/drug-class
entities; then a grounding confirmation (accept a dense ontology → ingest → count
grounded drug→condition facts at held precision).

#### A/B confirmation (2026-07-20, draft-level, 15 drugs)
Isolated the sampling variable: same 15 labels, same `sample_cap` (40), same model
(gpt-5.4-mini). Arm A = one draft over all drugs interleaved; Arm B = one dense draft per
drug, aggregated. Drafts are inert (no grounding/managed-write needed to measure stage-1).

| metric | Arm A (broad) | Arm B (dense per-drug) |
|---|---|---|
| entities extracted | 17 | 191 |
| **condition entities** | **0** | **15** |
| **drug→condition rules (unique triples)** | **0** | **15** |

Broad drafting extracted **zero** condition entities and **zero** drug→condition rules —
the exact starvation seen in the 50-label run. Dense per-drug drafting recovered correct
clinical rules: `Fludrocortisone INDICATED_FOR Addison's disease`, `corticosteroids
CONTRAINDICATED_IN systemic fungal infections`, `Metoclopramide INDICATED_FOR
gastroesophageal reflux`, `Amlodipine TREATS Hypertension`, `LORBRENA INDICATED_FOR
NSCLC`, etc. **Grounding was never the problem — broad drafting starves stage-1
extraction.** Draft density is the single highest-leverage construction lever.

#### Implemented — per-document drafting
`cognigraph_construct::draft_space_type_per_document` drafts each source document
separately (each entity/trigger stays verbatim self-checked within its own chunks)
and unions the catalogues and rules; `POST /api/construct/draft` gains a
`per_document` flag that groups the request's chunks into documents by their
`title`. Live-validated through the endpoint on real labels: `per_document=false`
→ 0 condition entities / 0 drug→condition rules; `per_document=true` → conditions +
drug→condition rules recovered (e.g. `Betamethasone TREATS chronic plaque
psoriasis`), with the response reporting the document count.

**Follow-up (known limit):** drafting runs synchronously inside the request, so a
large corpus (many documents × two completions each) exceeds the server request
timeout (observed: 10 documents → 408). Per-document drafting over big corpora
should move to the durable background job framework (as side-view generation did).
Also surfaced: durable job records for a JobPayload variant a running build lacks
panic job-recovery on startup (a branch/version-skew hazard on shared tenant data).

### D2 — Boilerplate trigger specificity — IMPLEMENTED at draft time
**Revised after investigation.** The first attempt required the licensing chunk to name
both endpoints at GROUNDING time. That was wrong. Ground-then-advise is deliberate: an
endpoint missing from the licensing *sentence* may be legitimate cross-sentence evidence,
so the grounder must not silently drop it — `advise_gates` exists precisely so the system
"must not silently pick a side" (decision_cross_chunk_grounding.md, enforced by
`advisor::tests::flags_endpoints_never_in_any_licensing_sentence_for_review`). The change
broke that test and would have dropped real facts. Reverted.

The actual defect is upstream: a corpus-wide boilerplate phrase became a rule TRIGGER. The
rule prompt already forbids it ("a trigger that could appear in text about OTHER entities
is a bad trigger") but nothing enforced it, and a single-document draft cannot see
corpus-wide boilerplate. `draft_space_type_per_document` is the first place the whole
corpus is held, so the check lives there: **a trigger that fires inside a document which
never names the rule's source is dropped as boilerplate**, and a rule that loses every
trigger is dropped — both recorded visibly in `skips`. This kills the `MAXALT --CONTACT-->
FDA` class at the source. Grounding semantics and the advisor contract are untouched, and
all reference kits still pass (114 construct tests green).

### D3 — Global entity identity: investigated, deliberately NOT changed
**Revised after investigation.** The eval collision (`Betamethasone [product]` vs
`[drug]`) is not a defect. `ingest_chunks` states the intent: "Global entities retain
first-definition-wins aliases, but canonical name/type identity must agree across spaces;
a sanitized-key collision is an error rather than a silent semantic merge." Space-scoping
entity identity would reverse that property — letting one tenant hold two contradictory
identities for the same name — and would change the identity surface that the M26
materialization projection digests. The genuine gap was *when* conflicts surface: only at
ingest. Per-document drafting now surfaces them at DRAFT time ("entity `X`: type `a` and
`b` proposed across documents — kept `a` (review)"), where a human resolves them before
acceptance. No identity-scheme change; the operational lesson is to not overlap eval
subsets with the full corpus.

#### Addendum (2026-07-21) — the draft must RESPECT that identity, not redefine it
Scaling per-document drafting to 100 labels surfaced the other half of D3. The identity
scheme stays as decided; what was broken is that the drafter did not honour it.
`merge_drafted` unions entities by their **literal name**, while the graph keys an entity
on `entity_key` (case-folded). Independent per-document drafts routinely capitalize the
same entity differently — `Epinephrine` in one label, `epinephrine` in another — which are
one entity to the store. Drafting 100 real labels produced **63 colliding groups**
(`Insulin`/`insulin`, `Hypertension`/`hypertension`, `Tardive Dyskinesia`/`tardive
dyskinesia`, …), and because `ingest_chunks` correctly fails closed on a key collision, the
accepted 1445-entity ontology **could not be ingested at all** (`distinct entity
definitions 'Epinephrine' (drug) and 'epinephrine' (drug) map to the same key`).

Fixed in `finalize_draft` (`canonicalize_entity_identity`): case variants collapse to the
first-seen definition with aliases unioned, and every collapse is recorded in `skips` — a
type disagreement between variants is reported for review exactly like the same-name
conflict above. The subtle part is the **rules**: they name endpoints by literal name and
grounding resolves them by exact string, so collapsing entities without rewriting the rules
would have silently orphaned every rule that referenced a dropped variant. Rules are
rewritten to the kept name and any that then collide onto one triple are folded back
together with their triggers unioned; a test fails if a rule is left pointing at a dropped
variant. This is a per-document-drafting bug specifically — one broad draft never produced
it, because a single model call is internally consistent about capitalization.

## Next steps
D1 first (the empirical entity-extraction study), then D2 (leakage fix). D3 is a
correctness cleanup that also unblocks overlapping evals. The advisor + repair-loop layers
are in good shape and need no immediate work.
