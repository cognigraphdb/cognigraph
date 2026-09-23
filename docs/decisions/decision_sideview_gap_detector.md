# Decision: side views as an explicit, conservative gap source

Status: accepted and implemented 2026-09-23 for [CG-88](../issues/CG-88.md).
The live measurement is [CG-97](../issues/CG-97.md). Builds on the
[side-view provider decision](decision_sideviews_provider_and_benchmark.md)
and the [construction refusal ledger](decision_construction_refusal_ledger.md).

## Context

Side views are generated question and answer pairs stored in the
write-protected `side_views` collection as a retrieval aid. The Semantic
Neurons paper names them as a fourth proposal source: a pair that keeps
surfacing knowledge the graph lacks points at a gap. Before this change,
`POST /api/construct/propose` took gaps only as explicit `A --REL--> B` lines
or from a live evaluation, and nothing read side views for construction.

Relation rules in a space type name entity instances (`"source": "Case
Alpha"`), not entity types, and construct ingestion stores fact edges under
`space_id` equal to the space type.

## Decision

1. **Explicit selection (owner choice).** The caller names the side views:
   every side view whose parent document is in one source `collection`,
   optionally narrowed to up to 1,000 `documents`. There is no query log and
   no background scan. A selection over 10,000 side views is refused with a
   request to narrow it.
2. **One matching rule or nothing (owner choice).** Entities are found with
   the grounding surface matcher (name or alias, case-insensitive) over the
   question and answer together. A co-mentioned pair becomes a candidate only
   when exactly one relation rule joins the two entities, in one direction.
   No rule gives `no_matching_rule`. Several rules, or a rule declared both
   ways, give `ambiguous_relation` with the fitting facts listed. The
   detector never guesses a relation.
3. **A gap means unconnected endpoints.** A candidate is dropped as
   `endpoints_connected` when any fact edge of the same `space_id` joins the
   two entities, in either direction and by any relation. This follows the
   ticket's "same `space_id` and endpoints" and keeps the proposer away from
   pairs the graph already relates.
4. **Support and bounds.** Support counts distinct side views. Candidates
   below `min_support` (default 1) are reported as `below_min_support`.
   Candidates are ordered by support, then by fact, and only the first
   `max_candidates` (default 20, at most 100) go to the proposer; the rest
   are reported as `over_max_candidates`.
5. **Same proposal path.** Without `dry_run`, candidates become the gaps of
   the existing proposal path: the same evidence retrieval, verbatim
   self-check, validation, `status: proposed` storage and refusal ledger.
   With `dry_run` the detection report is the whole response, no provider is
   required and nothing is written.
6. **Provenance beside authorship.** The ticket asked for `proposed_by` to
   name the side-view source. `proposed_by` is the authorship field that
   review and audit read as "who asked", so it keeps naming the caller.
   Proposals from side views add `proposed_from: "side_views"` and
   `side_view_source` with the collection, the supporting side view keys and
   the support count. Responses carry `source` (`gaps`, `evaluation` or
   `side_views`).
7. **Quarantine unchanged.** The path reads `side_views` and `facts` and
   writes only proposals and refusal rows. `side_views` and `fact_semantics`
   stay generated collections. A regression test compares `facts`,
   `entities`, `space_types`, `chunks`, `mentions` and `side_views` before
   and after a proposal run.
8. **Build now, measure later (owner choice).** The measured run on a public
   kit needs model calls for side-view generation and proposals, so it is
   split into CG-97 rather than simulated with hand-written side views.

## Consequences

- Operators can preview side-view candidates at no model cost and choose
  `min_support` before spending proposal calls.
- Pairs whose relation is ambiguous in the ontology never reach the proposer;
  a space whose rules are declared both ways gets no side-view candidates for
  those pairs.
- Surface matching is substring matching, as in grounding, so a short alias
  can over-match; that affects which pairs are nominated, not what is stored,
  because every proposal still passes the verbatim self-check and review.
- Revisit the defaults and the either-direction connection rule after CG-97
  reports how many candidates were expected or forbidden facts.
