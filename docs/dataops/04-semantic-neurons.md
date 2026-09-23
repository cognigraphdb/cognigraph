# 04 — Semantic Neurons: governed graph construction

This is the layer that separates CogniGraph's GraphRAG from "ask an LLM
to extract triples": facts are built **only** where document evidence
affirms an ontology trigger, every repair arrives inert, and both *recall*
(expected facts built) and *restraint* (forbidden facts NOT built) are
measured. The authoring lifecycle is **measure → miss → repair → prove**.
M25 adds the authority boundary around its result: a PolicyAuthor signs one
exact typed candidate, an independent PolicyApprover approves or rejects it,
and governed construction resolves only when the existing promotion head
selects that same digest. M26 can then build a verified complete occurrence
generation and deploy it through a separate Promoter signature and Native
atomic switch. Every action remains explicit; selection is not automatic
self-healing or deployment.

## 1. Author a space type (the ontology)

A space type declares the vocabulary facts may use — entities and
relation rules with **trigger phrases**. Neurons can never add
vocabulary; that boundary is what makes the graph governable.

```json
{
  "id": "acme_supply",
  "name": "Acme supply relationships",
  "version": 1,
  "entities": [
    {"name": "Acme", "type": "org", "aliases": ["Acme Corp"]},
    {"name": "DataCloud", "type": "platform", "aliases": []},
    {"name": "Nimbus", "type": "vendor", "aliases": ["Nimbus Inc"]}
  ],
  "relation_rules": [
    {"source": "Nimbus", "relation": "SUPPLIES", "target": "DataCloud",
     "when_any": ["nimbus supplies the datacloud platform",
                  "datacloud is built on nimbus"]},
    {"source": "Acme", "relation": "OPERATES", "target": "DataCloud",
     "when_any": ["acme operates datacloud"]}
  ]
}
```

Authoring rules that hold up in practice:

- **Triggers are verbatim evidence phrases**, lowercase, matched
  casefolded and *negation-aware*: a trigger inside a negated clause
  ("is **not** built on nimbus") does not ground. Write triggers you
  expect to literally appear in the text.
- A rule licenses exactly one `source --RELATION--> target` triple.
  There is no wildcard rule — that is deliberate.
- Start naive. Missing triggers are what the repair loop is *for*;
  over-broad triggers are what it cannot fix.

Two opt-in precision tools for leakage-prone rules
(decision_grounding_gates.md — both measured on a 1,120-chunk hostile
corpus):

- **Sentence-scoped endpoint gate.** `"require_in_sentence": ["source"]`
  (or `["target"]`, or both) makes the rule ground only when the
  affirmed trigger's SENTENCE also names that endpoint (name or alias).
  Use it on rules whose trigger phrasing could appear in text about a
  DIFFERENT company/entity — the measured cross-company leakage class.
  Do not gate everything: applied globally it costs real recall (also
  measured); gate the risky rules.
- **Template triggers.** `"when_any": ["{source} acquired {target}",
  "{target} was acquired by {source}"]` expands against the endpoints'
  names and aliases at grounding time and matches only
  direction-faithful phrasings — the only fix for transaction-direction
  traps, where both endpoints legitimately share every sentence. The
  matched expansion is recorded verbatim (with its span), so provenance
  is unchanged.

You do not have to find the risky rules by intuition — ask the **gate
advisor** (deterministic, no API key, runs the real gate semantics
against your corpus):

```sh
curl -s -X POST $COGNIGRAPH_URL/api/construct/advise \
  -H "content-type: application/json" \
  -d '{"space_type": "acme_supply"}'          # or: cognigraph advise acme_supply
```

Per rule it reports what each candidate gate would keep and refuse, a
**safe suggestion** where a gate blocks off-subject groundings at zero
recall cost on your corpus, and a **REVIEW flag** where an endpoint
appears in *no* licensing sentence — that is either cross-company
leakage (gate it; on the hostile corpus the advisor rediscovered all
6/6 author-applied gates this way) or legitimately cross-sentence
evidence (don't) — the sample sentences attached to each flag are how
you tell. The advisor never edits the ontology; you do.

**Starting from a blank page?** The ontology drafter writes the first
version for you to edit — measured honestly: it is a **vocabulary
bootstrap, not a rule author** (it discovers about half an expert's
entity catalogue, all surface-checked against your corpus, but only
~6% of an expert's expected relation pairs, with direction reversals
common — the repair loop below is the measured rule author, and it
needs entities to exist first):

```sh
cognigraph draft acme_supply chunks.jsonl
# or: curl -s -X POST $COGNIGRAPH_URL/api/construct/draft \
#   -d '{"space_type": "acme_supply", "chunks": [...]}'
```

The draft lands in `space_type_drafts` — a collection nothing can
ground from — with every entity and trigger symbolically checked
(verbatim, negation-aware), every drop listed in `skips`, and the gate
advisor's annotations attached. Review and edit it via the documents
API (delete bad rules freely; the loop will re-propose better ones),
then make it vocabulary with the explicit, attributed act:

```sh
cognigraph draft accept acme_supply
```

The drafter never writes gates or templates (author-only, D4), and
refuses ids that already exist — it creates NEW spaces only.

M25 makes `space_types` read-only through generic document, batch, Lua, graph,
embedding, and CGQL mutation surfaces. `draft accept` remains a dedicated typed
legacy-authoring path, but direct `POST /api/documents` writes to `space_types`
now return HTTP 403. For prospective governed construction, place the reviewed
space under the exact M22 `candidate.base_space_type` shape and follow the
signed revision flow in §5. M25 does not copy that candidate back into
`space_types` automatically.

## 2. Ground chunks into facts

Grounding walks every chunk, and for each relation rule whose trigger is
**affirmed** (present, casefolded, not negated, not vetoed) writes an
evidence-bound fact edge. It is deterministic and involves no model.

The legacy authoring-workspace route reads its space type from `space_types`
and applies that space's mutable **accepted** neurons (hints extend triggers,
blockers veto):

```sh
curl -s -X POST $COGNIGRAPH_URL/api/construct/ingest -H "content-type: application/json" -d '{
  "space_type": "acme_supply",
  "chunks": [
    {"id": "report-p3-c2", "text": "Everyone knows DataCloud runs on Nimbus these days."},
    {"id": "report-p4-c1", "text": "It is not true that Acme operates DataCloud."}
  ]
}'
# → {"space_type": "acme_supply", "chunks": 2, "facts_grounded": 1, "accepted_neurons": 0}
```

For an M25 target, prefer the governed route. It resolves the existing
promotion head, requires the selected candidate to have an independently
approved signed revision for that exact target, and grounds directly from the
candidate embedded in that immutable revision:

```sh
curl -s -X POST $COGNIGRAPH_URL/api/construct/governed-ingest \
  -H "content-type: application/json" -d '{
  "target": {"space_type": "acme_supply", "channel": "m25-production"},
  "chunks": [
    {"id": "report-p3-c2", "text": "Everyone knows DataCloud runs on Nimbus these days."}
  ]
}'
```

This is an explicit construction invocation and still requires an atomic-batch
backend. One call admits at most 1,000 chunks and holds the process-local
promotion lock through that batch. Multiple calls are not one graph-generation
transaction; a head may change between calls, so retain and compare the
authority projection returned by each response. Changing the selected head
does not call this route, retain another materialized generation, or switch readers. The legacy `/construct/ingest` path remains for
pre-existing authoring state, but its mutable collections are not M25 authority.

For a complete, reproducible target projection, use the M26 flow in §5 after
the M22/M23 evaluation, M25 approval, and promotion are current. It reads the
verified prepared corpus from CAS and replaces the target space once through a
separately signed Native deployment; it is not multiple governed-ingest calls
wrapped together.

Note the second chunk grounded nothing: its trigger sits in a negated clause.
Re-ingesting supplied chunk ids is an **atomic replacement** on the native
backend: the stored text, mentions, and narrative fact occurrences are
reconciled together. Re-POSTing identical content is idempotent; rewording or
deleting the licensing sentence retracts that chunk's old occurrence. An
identical fact grounded in another chunk or space remains available through
its independent occurrence.

Ingestion still runs no degradation detection, proposal, or repair. Replacing
the stored facts makes the graph truthful about the new text; it does not invent
a new pathway for an expected fact whose old pathway disappeared.

Handling drift is a **deliberate, invoked** sequence after truthful
reconciliation: use the library/example `degradation_report` against revised
chunks as a lead, propose repairs, and gate them through review.
`examples/self_healing.rs` and
`examples/dailymed_clinical_selfheal.rs` show the loop end to end. Treat
semantic repair as an operational procedure you schedule; stale-edge cleanup
itself is now an ingestion invariant.

`degradation_report` is not currently an HTTP/CLI operation and is not an exact
production-grounding diff. It checks literal authored triggers for affirmed
presence; it does not expand templates, reproduce sentence endpoint gates, or
attribute blocker effects. An unrelated occurrence can hide a dead gated
pathway, and a literal template can look dead. Treat its output as triage, then
prove a proposed candidate through exact M22/M23 derivation/evaluation and the
M25 signed selection flow. A generation-to-generation causal drift job remains
future work.

Grounding produces four collections:

| Collection | Kind | Shape |
|---|---|---|
| `entities` | document | global `_key` = sanitized name, `{name, entity_type, aliases}`; canonical name/type must agree across spaces |
| `chunks` | document | `_key` = `<space>-<chunk-id>`, `{chunk_id, text, content_hash, space_id, construction_schema}` — text is NFC before grounding; content_hash is SHA-256 of the stored UTF-8 bytes |
| `mentions` | edge | deterministic occurrence per `(space, chunk, entity)`, with `evidence_chunk_id` |
| `facts` | edge | deterministic occurrence per `(space, chunk, triple, trigger span)`, `{relation_type, space_id, evidence_chunk_id, trigger, trigger_start, trigger_end, construction_schema}` — the span is the UTF-8 byte range of the affirmed trigger in the chunk's stored NFC text |

`facts` remains the canonical query/evaluation collection: consumers still
deduplicate by `(source, relation, target)` when they need distinct facts, while
auditors can enumerate every evidence occurrence. A first re-ingest removes a
matching legacy edge that has `space_id` and `evidence_chunk_id` even when it
lacks the new occurrence metadata. Provenance overwritten before this migration
cannot be reconstructed automatically; re-ingest the source chunks to rebuild
it.

Readable sanitized keys are not collision-free. Ingestion validates each
stored chunk's raw `space_id` and `chunk_id`, rejects colliding chunk or entity
identities, and rejects identifiers that sanitize to an empty key. A legacy
chunk without raw `chunk_id` cannot be distinguished safely from a collision:
rebuild the derived `chunks`, `mentions`, and `facts` collections before
re-ingesting that key.

Atomic reconciliation uses Native batches. Capability checks precede
collection creation and entity writes so an unsupported implementation cannot
partially apply a revision.

M25 rejects hand-written `facts` edges and generic writes to all four derived
collections (`entities`, `chunks`, `mentions`, `facts`). This closes the old
bypass in which an ordinary graph/document operation could manufacture a row
that looked grounded. Use `/api/construct/governed-ingest` for governed state,
the legacy typed `/api/construct/ingest` route for an authoring workspace, or
the shipped `blind_eval` runner for an offline measurement loop.

## 3. Measure: the eval spec

Ground truth lives beside the data as questions with **expected** and
**forbidden** facts (`eval.json`):

```json
{
  "space_id": "acme_supply",
  "questions": [{
    "id": "q1_supply_chain",
    "question": "Which vendors supply Acme's data platform?",
    "expected_facts": ["Nimbus --SUPPLIES--> DataCloud"],
    "forbidden_facts": ["Acme --SUPPLIES--> DataCloud"]
  }]
}
```

Forbidden facts are the **restraint traps**: plausible over-inferences a
sloppy extractor would make. Write them from real temptations in the
text (an org mentioned near a product it doesn't supply, a denied
allegation, a competitor comparison). A trap no rule licenses is only
structurally safe; the valuable traps are ones the text nearly supports.

Measure over HTTP by passing the spec inline. The call is a POST but read-only
in effect (read scope), so it is suitable for a periodic degradation check:

```sh
curl -s -X POST $COGNIGRAPH_URL/api/construct/evaluate -H "content-type: application/json" \
  -d "$(jq -n --slurpfile eval eval.json \
        '{space_type: "acme_supply", eval: $eval[0]}')"
# → {"recall": {"found": 3, "total": 9}, "restraint": {"violations": 0, "total": 4},
#    "recall_ok": false, "restraint_ok": true,
#    "missing": ["Nimbus --SUPPLIES--> DataCloud", ...], "violations": []}
```

M25 makes `eval_specs` read-only through generic mutation APIs. A pre-existing
legacy stored spec remains readable, so `{"space_type":"acme_supply"}` still
resolves it; no signature or approval is synthesized for that historical row.
For a new prospective workflow, keep the oracle outside mutable tenant state,
pass it inline for authoring measurements, and use the unchanged M18-M24
evaluation/promotion chain for selection authority.

Alternatively, with three files in a directory — `space_type.json`,
`chunks.jsonl` (`{"id": "...", "text": "..."}` per line), `eval.json` —
the packaged runners do the same offline:

```sh
cargo run --release -p cognigraph-construct --example blind_validate -- ./myspace
# typo-catcher: parses + cross-references, never grounds

cargo run --release -p cognigraph-construct --example blind_eval -- ./myspace
# BASELINE   recall 3/9  restraint violations 0/4
#   MISS  Nimbus --SUPPLIES--> DataCloud
#   ...
```

Baseline recall below 100% is normal and healthy — it is the measured
gap list the repair stage consumes.

The ordinary live-graph evaluator is an authoring diagnostic, not promotion
authority. Restraint covers only the forbidden facts the operator enumerated,
not precision over every constructed edge. Validate fixture structure with
`blind_validate`; M18-M23 promotion uses stricter frozen context, positive
denominators, independent replay, exact verified artifacts, and signed gates.

## 4. Repair: propose neurons for the gaps

With `OPENAI_API_KEY` (or `GEMINI_API_KEY`) set on the server — model via
`COGNIGRAPH_COMPLETION_MODEL` — repair is one call. Gaps are measured
server-side from the stored eval spec (or pass `"gaps": ["A --REL--> B"]`
explicitly):

```sh
curl -s -X POST $COGNIGRAPH_URL/api/construct/propose -H "content-type: application/json" \
  -d '{"space_type": "acme_supply"}'
# → {"gaps": 1, "stored": 1,
#    "proposed": [{"id": "acme-operates-datacloud",
#                  "fact": "Acme --OPERATES--> DataCloud",
#                  "triggers": ["operates the DataCloud platform"]}],
#    "skipped": [], "proposed_by": "vera",
#    "note": "proposals are inert until accepted via POST /api/neurons/{key}/accept"}
```

For each missing fact the server retrieves evidence chunks via BM25,
asks the model for **verbatim** trigger phrases, and symbolically
self-checks every proposal — a phrase that does not occur verbatim somewhere
in the space's stored corpus is refused, and the gap is reported in `skipped`
rather than filled with a paraphrase. That floor proves literal corpus presence,
not that the phrase occurs in the model's selected excerpts, supports the
claimed endpoints/direction, or avoids collateral firings; review and
re-evaluation remain load-bearing. Everything stored lands as
`status: proposed` with authorship, straight into the review queue of
section 5; the graph does not change until review accepts — human by
default, or the policy-gated judge lane at volume.

### Side views as a gap source

Stored side views can nominate gaps too
([decision](../decisions/decision_sideview_gap_detector.md), CG-88). Select
the side views of one source collection, optionally narrowed to some of its
documents, and preview the candidates first; a dry run needs no provider and
writes nothing:

```sh
curl -s -X POST $COGNIGRAPH_URL/api/construct/propose -H "content-type: application/json" \
  -d '{"space_type": "acme_supply",
       "side_views": {"collection": "notes", "min_support": 2, "dry_run": true}}'
# → {"source": "side_views", "dry_run": true, "gaps": 1,
#    "side_views": {"collection": "notes", "scanned": 40, "min_support": 2,
#      "candidates": [{"fact": "Acme --OPERATES--> DataCloud", "support": 3,
#                      "side_views": ["sv-12", "sv-19", "sv-31"]}],
#      "skipped": [{"reason": "endpoints_connected", ...}, ...]}}
```

A pair of ontology entities that one side view names together (by name or
alias, in its question or answer) becomes a candidate only when exactly one
relation rule of the space type joins them, in one direction, and no fact
edge of the space connects them either way. Everything else is reported with
a reason: `no_matching_rule`, `ambiguous_relation` (the fitting facts are in
`detail`), `endpoints_connected`, `below_min_support` or
`over_max_candidates` (default cap 20, at most 100). Without `dry_run` the
candidates go through the same proposal path as measured gaps, including the
verbatim self-check, and each stored proposal records
`"proposed_from": "side_views"` and `side_view_source` (collection, side view
keys, support). `proposed_by` still names the caller. Side views stay a
retrieval aid: the path reads `side_views` and `facts` and writes only
proposals and refusal rows.

The detector has not been measured on a real corpus yet; that run is
tracked separately (see the decision).

The packaged `blind_eval` runner does the same offline (plus the
answer-level scorecard):

Proposals land in `./myspace/proposed.neurons.json` with
`"status": "proposed"` — structurally inert. Nothing changed in your
graph yet. Preview what acceptance *would* do, deterministically and
offline:

```sh
cargo run --release -p cognigraph-construct --example blind_recheck -- ./myspace
# recall 8/9  restraint violations 0/4
#   STILL MISSING  ...   (with the proposed triggers that failed)
```

## 5. Review: the neuron lifecycle

Neurons live in the `neurons` collection and move through
`proposed → accepted | rejected`, later `retired`. Author or submit one:

```sh
curl -s -X POST $COGNIGRAPH_URL/api/neurons -H "content-type: application/json" -d '{
  "space_type": "acme_supply",
  "id": "hint-nimbus-datacloud-1",
  "type": "relation_hint",
  "confidence": 0.9,
  "evidence": ["p.3: DataCloud runs on Nimbus infrastructure"],
  "source": "Nimbus", "relation": "SUPPLIES", "target": "DataCloud",
  "triggers": ["datacloud runs on nimbus infrastructure"],
  "rationale": "phrasing found on p.3 the ontology missed"
}'
```

`evidence` is required and at least one entry must be non-blank, but the current
validator treats it as a provenance claim rather than proving that it contains
the trigger or supports the triple. A neuron without such a claim is rejected;
the reviewer must inspect the actual matched chunks shown by `neuron show`.
M25 freezes the claim inside the exact signed candidate but does not make the
claim semantically true.

Submissions are validated against the stored space type (unknown
entities/relations are rejected) and **always stored as `proposed`** —
acceptance is its own transition, re-validated against the space's
already-accepted set so conflicting states cannot be assembled stepwise:

```sh
cognigraph neuron pending acme_supply     # the review queue, digested
cognigraph neuron show hint-nimbus-datacloud-1
# → the neuron PLUS every chunk its triggers match (verbatim text), and
#   any trigger matching no chunk at all — a dead trigger to question
#   before accepting. No second tool needed to see what you approve.
cognigraph neuron accept hint-nimbus-datacloud-1 --note "verified against p.3"
cognigraph neuron reject some-other-key              # POST /api/neurons/{key}/reject
```

Every transition is attributed: the neuron records `proposed_by`/
`proposed_at` at creation and `reviewed_by`/`reviewed_at` (plus your
optional `--note`) at accept/reject/retire — the reviewer is the
authenticated user, or `"anonymous"` when auth is disabled. The audit
fields show up in `GET /api/neurons` and on graduation candidates.

This legacy row stores the **latest** review attribution; it is not an
append-only transition ledger, and the legacy route does not enforce a distinct
proposer/reviewer principal. A later transition overwrites those latest-review
fields. M25 does not pretend otherwise: its immutable signed candidate revision
and independent final review provide separation and history at the exact
candidate boundary, not a retroactive event log for every mutable neuron row.

**At volume: the risk-tiered review policy**
(decision_review_policy.md). For a pre-existing legacy
`review_policies/{space}` document, `POST /api/construct/review` (or
`cognigraph neuron review SPACE`) judges pending proposals with the two-stage,
schema-constrained LLM reviewer. The only automatic lane is now an eligible
`relation_hint` at high judge confidence on a non-precision-critical existing
rule, with full attribution (`reviewed_by: judge:<model>@<policy-rev>`).
Aliases need a kind-specific judge packet and therefore queue, as do blockers,
rank hints, new triples, and precision-critical rules. The judge never
auto-rejects.

Both stages fail closed. A taint-screener response with missing/wrongly typed
fields, an out-of-range confidence, or empty reasoning is treated as tainted.
A quality response with an unknown verdict, malformed concerns, out-of-range
confidence, missing fields, or empty reasoning becomes `needs_human` at zero
confidence. OpenAI structured output uses strict schema mode, but the server
still validates the returned value rather than treating provider conformance as
authority. The default deterministic audit sampling rate is `0.1` when the
field is omitted.

The historical policy shape is shown below for inspection; generic mutation of
`review_policies` is blocked by M25, so this is not a request body to POST:

```json
{
  "_key": "acme_supply",
  "auto_accept": {
    "kinds": ["relation_hint"],
    "min_confidence": 0.9,
    "qualified_judges": ["gpt-5.4-mini"]
  },
  "sampling_rate": 0.1,
  "injection_suite_passed": true
}
```

`qualified_judges` records which models the historical replay/injection
harnesses measured; it is an ordinary legacy policy field, **not** an M25 signed
judge-qualification record. An unlisted model may produce triage, but all its
proposals queue. The route also does not require the proposal model/provider to
differ from the judge; M25's principal separation occurs later at signed
candidate author/review/promotion. No policy document means fully human review. Existing-policy
operations remain:

```sh
cognigraph neuron review acme_supply    # judge + apply relation-hint lane
cognigraph neuron audit acme_supply     # sampled auto-accepts for human spot-review
```

At volume, drain the queue in bounded slices: `POST /api/construct/review`
takes `limit` (judge at most N *unjudged* proposals, oldest first — a
judged-and-queued item is awaiting a human, so limited calls always
make progress) and returns `reviewed` / `pending_remaining` /
`judge_calls` / `elapsed_ms` for cron dashboards; `rejudge: true`
re-judges everything after a policy revision. `cognigraph neuron
pending SPACE --limit N` pages the queue the same way.

Audit sampling is deterministic FNV selection over an author-controlled neuron
id. That makes routine spot checks reproducible, but not unpredictable or
tamper-resistant; do not treat the sample as an adversarial fraud-control
mechanism. A keyed or persisted-unpredictable audit sampler remains open work.

Optionally, an **agreement lane (Lane A+)** extends auto-accept to the
direction-critical class Lane A excludes — hints on templated/gated
EXISTING rules — when TWO judges with measured-disjoint blind spots
concur (decision_agreement_lane.md). Add to `auto_accept`:

```json
"agreement": { "kinds": ["relation_hint"],
               "judges": ["gpt-5.4-mini", "gpt-5.4"],
               "min_confidence": 0.9,
               "concordance_measured": true }
```

`concordance_measured` attests the pair showed ZERO concordant
false-accepts at threshold in the offline instrument
(`examples/concordance_sim.rs`). The server runs the partner via
`COGNIGRAPH_JUDGE_PARTNER_MODEL`; if the running pair doesn't match the
attested pair, Lane A+ degrades to queue (never to a single judge for
this class), and disagreements queue with BOTH verdicts attached.

The four neuron types, and what accepting each means:

| Type | Effect when accepted |
|---|---|
| `relation_hint` | adds triggers to ONE licensed triple — repairs recall |
| `relation_blocker` | veto phrases (`triggers`/`when_any`) that un-ground a triple in chunks matching the veto — repairs restraint (e.g. allegation language) |
| `relation_rank_hint` | additive `boost` (may be negative) to a relation's retrieval rank — no construction effect |
| `alias` | additional surface forms for one entity |

A hint and a blocker for the same triple cannot both be accepted — the
conflict is rejected at transition time.

### Authorize one exact repair candidate (M25)

The mutable neuron lifecycle above is an authoring workspace. A proposed,
human-accepted, or legacy judge-accepted neuron is not by itself governed M25
authority. Build one exact unchanged M22 schema-v1 candidate from the reviewed
base space plus accepted alias, relation-hint, and relation-blocker neurons;
`relation_rank_hint` is intentionally not part of that construction artifact.
Then use three independent acts:

```sh
# PolicyAuthor: submit a statement signed outside CogniGraph.
cognigraph semantic-repair revision submit @semantic-revision.request.json \
  --idempotency-key acme-semantic-revision-1

# Different stable PolicyApprover principal: approve or reject that exact revision.
cognigraph semantic-repair review submit REVISION_ID \
  @semantic-review.request.json --idempotency-key acme-semantic-review-1

# After the unchanged M23 evidence/promotion flow selects the same candidate digest:
cognigraph semantic-repair current acme_supply m25-production
```

The templates in [`fixtures/m25/`](../../fixtures/m25) are deliberately
unsigned placeholders. The server recomputes the canonical candidate digest,
validates both purpose-bound signatures and stable-principal separation, and
requires both statements to bind the observed
`base_promotion_head_decision_id` (explicit JSON `null` for a fresh target;
omission is invalid). It stores immutable revision/review records. Approve and
reject share one natural review id; changing the decision requires a new
revision, not an overwrite.

Approval does not promote, and promotion does not run construction. The
existing promotion head remains the sole selector. Once `current` resolves,
invoke `/api/construct/governed-ingest` explicitly as shown in §2. M25 does not
schedule degradation detection, re-ingest old corpus bytes, preserve parallel
materialized graph generations, or switch query consumers. M26 adds the
separate explicit generation/deployment option below without changing M25
authority.

### Build and deploy one verified graph generation (M26)

M26 accepts only a current M22 or M23 promotion whose exact candidate resolves
through the approved M25 revision above. As an authenticated Promoter, bind the
exact current promotion decision and build synchronously:

```sh
cognigraph semantic-repair generation build \
  --target-space acme_supply \
  --channel m26-production \
  --expected-promotion-head PROMOTION_DECISION_ID \
  --idempotency-key acme-generation-1
cognigraph semantic-repair generation show GENERATION_ID
```

The Native-only builder requires the tenant-incarnation read-only local CAS.
It verifies the canonical prepared `corpus.json`, reruns the pinned
deterministic grounder from the exact signed candidate, and requires its sorted
semantic facts to equal both original and replay derivation receipts. The
immutable generation contains the complete entity/chunk/mention/fact
occurrence projection and exact candidate-versus-baseline added, removed, and
unchanged fact sets. A stale head or failed CAS/derivation check stores no
generation and changes no graph rows.

Building is inert. Inspect its projection and impact, complete the public
template in [`fixtures/m26/`](../../fixtures/m26), and have an active
root-certified Promoter sign the exact
`cognigraph.semantic-repair-deployment-intent.v1` statement outside
CogniGraph:

```sh
cognigraph semantic-repair generation deploy GENERATION_ID \
  @deployment-intent.request.json \
  --idempotency-key acme-deployment-1
cognigraph semantic-repair deployment current acme_supply
```

One Native transaction replaces the target space's chunks, mentions, and facts
and commits the immutable deployment decision/head. Compatible missing shared
entities may be inserted; entities and other spaces are never deleted. Only
one channel's generation is deployed per space because the logical graph rows
have `space_id` but no channel.

For rollback, first complete the ordinary signed promotion rollback so the
prior candidate is current, then sign a fresh M26 `rollback` statement naming
the retained one-step-prior generation and current deployed decision. Both
nullable deployment fields must be present: use explicit JSON `null` where the
M26 contract requires it; omission is invalid.

This is invoked verified repair, not an automatic controller. Promotion does
not build, build does not deploy, and M26 adds no drift scheduler, durable
materialization job, background retry, LLM/prompt qualification, physical
per-generation query routing, generation deletion/GC, distributed writer, or
HA. M26 requires Native atomic batch execution.

## 6. Prove: re-measure, attribute, prune

Re-run the eval with accepted neurons applied (the runner does this in
its "IF ACCEPTED" stage; `blind_recheck` does it offline). Recall must
close *without* opening violations — restraint is re-measured on every
run, not assumed.

Then keep the neuron library honest over time:

```sh
cognigraph neuron graduation acme_supply     # GET /api/neurons/graduation
```

Graduation is a leave-one-out analysis: a hint whose fact now grounds
*without* it is flagged `covered_by_base` (the ontology or another neuron
took over); a blocker whose forbidden fact no longer grounds even
unvetoed is flagged inert. Flags are advisories — **retirement stays a
human decision** (`cognigraph neuron retire KEY`).

## 6b. Trace: what the gates refused

Every rejection from `POST /api/construct/directed` and
`POST /api/construct/propose` is recorded in the generated
`construction_refusals` collection in the same request that produced it
([decision](../decisions/decision_construction_refusal_ledger.md), CG-90).
Rows carry `origin` (`directed` or `propose`), `space_type`, `gate`, the
nominated `source`/`relation`/`target`, `chunk_id` and `evidence` (directed
only), the human-readable `reason`, `policy`, `attribution`, `actor` and
`recorded_at`. Keys are deterministic, so re-submitting the same nomination
records once. The collection is readable through generic reads, CGQL and
`cognigraph export`, and publicly write-protected like `side_views`.

Refusals per gate for one space:

```text
FOR r IN construction_refusals
  FILTER r.space_type == "acme"
  COLLECT gate = r.gate WITH COUNT INTO n
  RETURN { gate, n }
```

The nominations one gate refused, with their evidence:

```text
FOR r IN construction_refusals
  FILTER r.space_type == "acme" AND r.gate == "vocabulary_not_affirmed"
  RETURN { fact: CONCAT(r.source, " --", r.relation, "--> ", r.target),
           chunk: r.chunk_id, evidence: r.evidence, by: r.attribution }
```

Directed gate codes, in gate order: `relation_not_in_taxonomy`,
`chunk_not_in_request`, `empty_endpoint`, `unusable_endpoint_identity`,
`evidence_not_verbatim`, `source_not_in_sentence`, `target_not_in_sentence`,
`vocabulary_not_affirmed`. Proposal codes: `proposal_rejected`,
`forbidden_fact_not_grounded`, `blocker_rejected`, `blocker_round_failed`,
`blocker_round_dry`, `validation_failed`, `duplicate_id`.

Each tenant store keeps at most 10,000 rows. At the cap, new refusals are
still returned in the response (`refusals[].stored: false`,
`refusals_dropped`) but not written; there is no retention sweeper yet, so
clearing the ledger is an operator action through snapshot tooling. A
ledger write failure is reported as `refusals_error` and never fails the
construction that already happened.

## 7. Answer-level checks

Construction metrics prove the right edges exist; the answer layer
measures whether they carry through a ranked trace into model answers —
reporting per-question recall, restraint, and any **fabrications**
(asserted facts absent from the trace; they are flagged and never
credited). Restraint at this level is behavioral: the model *could*
assert a constructed-but-forbidden fact, and the score says whether it
did.

```sh
curl -s -X POST $COGNIGRAPH_URL/api/construct/answer-eval \
  -H "content-type: application/json" \
  -d '{"space_type": "acme_supply", "two_pass": true}'
```

Uses the stored eval spec (or an inline `eval`) and needs the
completion provider. `two_pass: true` adds a completeness-critic second
pass per question — additions only, trace-checked — which recovered the
measured selectivity residue (answer recall 77/105 → 91/105 across the
five reference kits) with restraint clean (0/54) and zero fabrications
in both modes, at 2× the completion calls. `evidence_sentences: true`
augments each trace fact line with its licensing sentence (stored
provenance) — measured best for meta/process-heavy evals (procurement
11/17 → 15/17, the kit's best-ever) while restraint stayed 0/54; keep
it off (the default) for entity-dense or adversarial corpora, where it
distracts, and note the critic pass may then produce guard-caught
fabrications.

Useful CGQL while operating a space:

These parsed CGQL examples remain valid because M25 preserves generic reads of
managed collections. `INSERT`, `UPDATE`, `REPLACE`, `REMOVE`, and `UPSERT`
against them are rejected. Opaque backend-native query text cannot safely be
classified as read-only and therefore cannot reference managed collections at
all; use parsed CGQL or typed GET routes for inspection.

```cgql
// every fact with its evidence, for spot review
FOR f IN facts
  FILTER f.space_id == "acme_supply"
  RETURN { fact: CONCAT(f._from, " --", f.relation_type, "--> ", f._to),
           evidence: f.evidence_chunk_id }
```

```cgql
// accepted neurons per type
FOR n IN neurons
  FILTER n.space_type == "acme_supply" AND n.status == "accepted"
  COLLECT kind = n.type WITH COUNT INTO n_of
  RETURN { type: kind, n: n_of }
```

Next: [05 — Recurring routines](05-recurring-routines.md).
