# Decision: the ontology drafter (D1–D6)

**Status:** Decided 2026-07-06 (user approved all six); implementation
same day.

## Context

The largest remaining human cost in the Semantic Neurons loop is
authoring the space type at all — reading a corpus and writing the
entities and relation rules the loop then measures and repairs. The
paper's boundary is load-bearing: "new vocabulary is a separate,
deliberate act of ontology design, not a side effect of extraction"
(§6), and neurons deliberately cannot create vocabulary. The drafter
assists that act without eroding it: an LLM-per-purpose pipeline
proposes a DRAFT ontology from sample chunks; a human reads the whole
artifact and accepts it explicitly. The drafter changes who types the
first version, not who authorizes it.

## Decisions (owner: user, 2026-07-06 design session)

**D1 — Drafts live in a separate `space_type_drafts` collection;
inertness is structural.** `load_space` reads `space_types` from six
routes; a status flag would need every consumer to check it (the
set-but-ignored failure mode). A separate collection means a draft
CANNOT ground anything by construction. Acceptance is an explicit copy
to `space_types` stamping `drafted_by: draft:<model>@<DRAFT_REV>`,
`accepted_by`, and timestamps — QW1 attribution applied to vocabulary.

**D2 — Two purpose-bound stages with symbolic self-checks.** Stage 1
discovers entities (every surface — name and each alias — must occur
verbatim in the corpus or is dropped with a visible skip). Stage 2
discovers rules given the CHECKED entity list (endpoints must reference
stage-1 entities — closure; every trigger must occur verbatim AND
affirmed, negation-aware, in some chunk, recorded with its chunk id).
Same discipline as the propose verbatim check; the draft is reviewable
as evidence, not just structure.

**D3 — v1 drafts NEW spaces only.** The drafter refuses an id that
already exists in `space_types`. Extension drafting needs diff/merge
semantics and reopens "who may alter matching semantics" — a future
design session. Recall-gap extensions already have a governed pathway
(relation-hint neurons).

**D4 — The drafter never authors precision config; the advisor
annotates.** Gates and templates are the precision-critical class Lane
A refuses to auto-accept; drafts are always naive (LLM-emitted
gates/templates are stripped with a note). The draft response attaches
the deterministic gate-advisor report against the sample corpus, so the
human applies gates while editing the draft, before acceptance —
composing the existing tools instead of opening a second authoring
channel.

**D5 — Measurement gate before productizing.** Run the drafter on the
five reference kits (author ontologies withheld), ground each corpus
with the draft, score against the kit's eval spec. Scoring is
label-lenient and direction-respecting (drafted names/labels won't
string-match the author's): an expected fact counts as covered when a
drafted edge connects surface-matching endpoints in the right
direction, any relation label; violations counted the same way (which
is CONSERVATIVE for restraint). No pass threshold declared in advance;
the honest number ships whatever it is.

**D6 — Surface and ordering.** Library + example runner + D5
measurement first; then `POST /api/construct/draft` (write scope, inline
chunks, provider required) and `POST /api/construct/draft/{id}/accept`,
OpenAPI, CLI `draft SPACE FILE.jsonl` / `draft accept SPACE`, guide 04
section. `DRAFT_REV` versions the prompt for attribution — WITHOUT the
judge's mandatory regression-harness rule, because the drafter holds no
authority: a human reads every word before anything can ground.

## Boundaries kept

- Vocabulary enters the system only through explicit, attributed human
  acceptance of an artifact readable whole.
- Neurons still cannot create vocabulary; neither can the drafter — it
  can only ask.
- Drafts cannot ground, evaluate, or be extended by neurons (they are
  invisible to every space-loading path).
- The LLM never emits `require_in_sentence` or template triggers.

## Outcome (2026-07-06, same day)

Landed per D1–D6: `cognigraph_construct::draft` (DRAFT_REV,
two-stage prompts, verbatim/closure/affirmation self-checks, advisor
annotations), `POST /api/construct/draft` + `/api/construct/draft/{id}/accept`,
CLI `draft SPACE FILE.jsonl` / `draft accept SPACE`, structural
inertness pinned by test (a draft cannot be ingested; acceptance
carries drafted_by/accepted_by).

**D5 measurement (three full runs, fixtures/semantic-neurons/
draft-eval-results-2026-07-06.md): the drafter is a VOCABULARY
BOOTSTRAP, not a rule author.** Expected-pair coverage 7/96, 7/96,
5/96 (~6%, direction reversals endemic — the third independent
sighting of the direction weakness); author-entity coverage 95/187
(51%; 77% where the sample covers the corpus); forbidden-pair
connections **0/45 in all three runs**. Productized with exactly that
positioning: the drafter makes the repair loop startable on a fresh
corpus (entities are what proposals validate against); the loop
remains the measured rule author. Per-kit rule coverage varies widely
run to run — one-shot drafting has a wide output distribution, so the
draft is a starting point to edit, never a deliverable.
