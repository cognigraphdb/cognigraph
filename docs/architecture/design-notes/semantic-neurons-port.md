# Semantic Neurons on CogniGraph — Port Design (for discussion)

Status: ACCEPTED & v1 DELIVERED 2026-07-03 (all five recommendations; see decision_semantic_neurons.md). Generalization experiment RUN 2026-07-03 (CrowdStrike-outage Wikipedia article, blind eval, naive triggers): baseline recall 2/6, restraint 0/3 — the Microsoft-blame trap did NOT fire; after the proposer reliability fix (skip reports, retries, verbatim trigger self-check, both-endpoint evidence ranking) the loop proposed 4/4 repairs; accepting them closes recall to 6/6 with restraint still 0/3. The review pass caught one category-inference over-reach (Delta "implied" via a generic industries list) — first field evidence for the governance moat. Full analysis: generalization-assessment.md. All four proposals ACCEPTED on review 2026-07-03 (crowdstrike_2024.neurons.accepted.json); the closed loop is pinned as a deterministic regression test in parity.rs. Original runbook: pick a document none of us has authored evals for; write expected_facts + forbidden_facts blind; ingest with the relevant space type; run `evaluate`; feed misses to `propose_neurons`; review honestly; measure whether the loop catches real over-inference. Source: the Semantic Neurons paper and research
repo (`research-hugegraph`: semantic-neurons-paper.md, semantic-neurons.md,
semantic-neurons-research-direction.md, evaluation-results.md). No
implementation began until the open decisions below were settled; the status
paragraph above records the delivered outcome and the proposal is retained as
design history.

Naming note for readers comparing the proposal to current code: the delivered
reserved collections are `space_types`, `neurons`, and `eval_specs` (without
the proposal's leading underscores). Current request/response shapes are in
`dataops/04-semantic-neurons.md` and the server OpenAPI document.

## Reading of the research (what must survive the port)

The paper's claims, restated as build requirements:

1. **LLMs extract entities, not edges** (SOTU: raw path 4/8, zero edges on
   key chunks; embeddings changed nothing). Construction, not retrieval, is
   the bottleneck — so the port is a *construction layer*, not a retrieval
   feature.
2. **A Semantic Neuron is symbolic config, not a learned unit**: alias
   neurons (surface forms → canonical entity) and relation-hint neurons
   (trigger phrases → pre-approved source/relation/target). Five defining
   properties: ontology-bounded, evidence-bound, reviewable (rules, not
   instances), reversible/staged (proposed → accepted → graduated →
   retired), and linguistically sound (negation/modality-aware — the Case
   Bravo lesson: "it is not true that X contains Y" must not ground
   `X CONTAINS Y`).
3. **The loop is the product**: measure recall AND restraint
   (expected_facts + forbidden_facts), propose gap-directed repairs,
   review, apply, re-measure, and detect graduation (redundant neurons =
   success). The paper is explicitly substrate-independent — the storage
   layer it was proven on (HugeGraph) contributed nothing conceptual.

## Why CogniGraph is a better substrate than HugeGraph was

The research repo's own gap list maps onto things CogniGraph already has:

| Research-repo gap | CogniGraph today |
|---|---|
| "Vector index is unbuilt" (local/transient, no lifecycle) | Persistent, quantized, mmap sidecar; 0.34 ms |
| "Store neuron reviews durably" (hand-edited YAML) | Documents + RBAC users + mutations + timestamps |
| "Provenance fields for proposer/reviewer" | `created_by`, auth identities, stamped writes |
| Accept/reject without editing YAML | CGQL UPSERT / `POST /api/batch` (atomic publishes) |
| Relation ranking preferring semantic over CO_OCCURS_WITH | Traversal confidence + path decay; rankable in CGQL |
| Separate graph + vector + eval stacks | One engine, one query language, one auth model |
| Their SOTU corpus | Already in `fixtures/sotu.txt` with an ingest suite |

## Schema mapping (their HugeGraph model → CogniGraph)

The research repo's own porting checklist, answered:

| Their construct | CogniGraph representation |
|---|---|
| Reified `Fact` vertices (space_id, version_id, subject, predicate, object, evidence_chunk_id) | Documents in a `facts` collection + edges `FACT_SUBJECT`/`FACT_OBJECT`/`EVIDENCED_BY` — or, simpler v1: direct entity→entity edges carrying `space_id`, `version_id`, `evidence_chunk_id`, `neuron_id` properties (their reification exists because HugeGraph edges are awkward to scope; our edges are full JSON documents) |
| Global entities, space-scoped facts | `entities` collection global; fact edges filtered by `space_id`/`version_id` in CGQL |
| `active_version` pointer swap without locks | `execute_batch` — the version publish becomes ONE atomic transaction (they wanted this; we have it) |
| BFS expansion, depth 2, seed sets | `traverse` with the cached adjacency index (O(degree)) |
| Edge ranking (semantic 100 / MENTIONS 20 / CO_OCCURS_WITH 10 + seed + evidence scores), low-signal caps | CGQL: LET score expression + SORT, or traversal `min_confidence`; encode relation weights as data, rankable per space — their `relation_rank_hint` becomes a config document, no engine change |
| Negation lookback bounded by clause punctuation | Port `_affirms_phrase` semantics to Rust verbatim, Case Bravo as regression corpus |
| Query seeding: vector top-k chunks → mentioned entities | `/search/semantic` + `MENTIONS` edges — existing surface |
| Chunking + ingestion jobs (in-process registry) | Our chunker companion project + batch ingestion; durable jobs deferred like theirs |

## Proposed shape (new crate: `cognigraph-construct`)

A construction layer *above* `GraphBackend`, exactly where the paper says
it belongs. Everything is stored as documents in reserved collections, so
the whole control plane is queryable in CGQL and audited by the existing
auth:

- `_space_types` — the ontology: entity definitions (name, type, aliases),
  relation rules (source, relation, target, trigger phrases). One document
  per space type, versioned.
- `_neurons` — one document per neuron: type (`alias` | `relation_hint`;
  later `relation_rank_hint`, `relation_blocker`), status lifecycle,
  confidence, rationale, evidence (text + chunk ref), proposer/reviewer,
  timestamps. Validation on write: unknown entity/relation, empty
  evidence/triggers, kebab-case IDs — the same rejections as the research
  code, enforced at the API.
- `_evals` — expected_facts + forbidden_facts per space; runs produce
  recall + restraint scores and per-fact attribution.
- Grounding engine (Rust, in the crate): chunk text + active config →
  canonical edges with `evidence_chunk`/`evidence_span` properties and
  neuron attribution on each edge. **Negation-aware matching is core Rust**
  (the `_affirms_phrase` lesson), with the Case Bravo probe ported as a
  standing regression test.
- Reports as CGQL over the control-plane collections: raw-vs-grounded,
  per-neuron ablation, redundancy (graduation detection).
- LLM proposal step (gap-directed, JSON-schema constrained) rides the
  existing `cognigraph-embeddings` provider pattern — Apollo/OpenAI/Gemini
  behind one trait; proposals are *never* auto-accepted.

Deliberately out of scope for v1: multi-scope precedence (their roadmap
item 3 — introduce with blockers, not before), vector-space-influencing
neurons (their conceptual fork — stay symbolic), any UI.

## Open decisions

1. **Scope of v1**: alias + relation_hint neurons, grounding engine,
   eval harness (recall + restraint), raw-vs-grounded + redundancy
   reports — matching the research repo's proven surface, with
   `relation_rank_hint` next (their lowest-risk next type)?
2. **Parity target**: port their three eval spaces (SOTU 8/8 on our own
   fixture, case:alpha 6/6, study:px-101 5/5 — configs copied from the
   research repo) as the acceptance test?
3. **The generalization experiment** (their single most informative next
   step): after parity, run the harness on a messy third-party document
   nobody authored evals for — is that the milestone's exit criterion?
4. **LLM proposer**: which provider drives gap-directed proposals in v1
   (their Apollo setup vs our OpenAI/Gemini)? Proposals need chat, not
   embeddings — a small `CompletionProvider` trait alongside
   `EmbeddingProvider`?
5. **Naming**: keep "Semantic Neuron" as the product concept (their
   guidance: yes, rename only on real confusion)?

## relation_rank_hint (added 2026-07-03)

The roadmap's lowest-risk neuron type, implemented per the research
direction doc §4: it cannot create facts — it only reweights
`relation_score` in retrieval-trace edge ranking (`src/rank.rs`, a faithful
port of `graph_facts.py`: 100 for configured relations / 20 MENTIONS / 10
other low-signal, seed-adjacency and evidence tiebreaks, low-signal caps
of 4 global and 2 per entity, deterministic lexicographic ordering).

- Schema: `{"type": "relation_rank_hint", "relation": ..., "boost": f64}` —
  boost is an additive score delta (negative demotes), so multiple hints
  sum and there is **no precedence-conflict problem** by construction.
- Governance is identical to construction neurons: only `accepted` hints
  contribute (`rank_boosts`), and `effective_config` ignores them entirely —
  a rank hint structurally cannot change what gets built.
- Validation: relation must be a configured rule's relation or a built-in
  low-signal relation (MENTIONS / CO_OCCURS_WITH / HAS_CHUNK); boost must be
  finite and non-zero.
- Deliberate scope limits: boosts do not change `is_low_signal`, so the
  low-signal selection caps hold even for heavily boosted relations, and
  the LLM proposer does not emit rank hints — they are review-authored.
- **Wired into the product (2026-07-03):** `POST /api/search/graph-augmented`
  now returns a `graph_facts` array — the traversed edges ranked by
  `select_graph_edges`, reweighted by accepted `relation_rank_hint` neurons
  read from a `neurons` collection (request field `neurons_collection`,
  default "neurons"; `graph_facts_limit` caps the trace). Neuron documents
  are written through the ordinary documents API, so review/acceptance
  rides the existing RBAC write scopes; proposed/rejected hints are inert
  live, proven by an in-route test that flips a stored hint's status and
  watches the trace reorder. Missing collection or malformed neuron
  documents mean "no boosts", never a search failure.

## relation_blocker (added 2026-07-03)

The extraction-time veto, implemented after a five-decision design session
(docs/decisions/decision_relation_blocker.md): triple-scoped `when_any` veto
phrases, plain-presence matching (not negation-aware, by decision), and the
one rule that avoids the research's feared precedence engine — grounding is
`(any trigger affirmed) AND NOT (any accepted veto present)`, boolean and
order-independent, veto wins unconditionally. Accepted hint + accepted
blocker on one triple is a validation error a human must resolve. Vetoes are
chunk-local: a clean assertion elsewhere still grounds the fact.
`blocker_report` is the restraint counterpart of `ablation_report`.
Acceptance suite manufactures the pharma allegation scenario and proves the
violation closes with recall held; benchmarks in docs/benchmarks.md.
