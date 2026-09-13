# Decision: relation_blocker — extraction-time veto without a precedence engine

**Date:** 2026-07-03
**Owner:** skitsanos (approved all five), Claude (design + implementation)

## Context

The research roadmap's second neuron type: "when this wording appears, do
NOT infer this relation" — critical for pharma/legal/forensic contexts where
co-occurrence wording legitimately contains a trigger phrase but does not
assert the fact (allegations, investigations, speculation). The research
explicitly warns this is where the config model can degrade: "blockers +
scopes turn today's additive, order-independent config into a rule-precedence
engine." Bounding lesson from Case Bravo: when the *matcher* is wrong
(negation-blindness), fix the matcher — blockers are for when the matcher is
correct but the inference is still wrong.

## Decision (all five approved 2026-07-03)

1. **Triple-scoped shape**, mirroring `relation_hint`:
   `{"type": "relation_blocker", source, relation, target, when_any: [veto phrases]}`
   (`when_any` is a serde alias for the shared `triggers` field). Relation-
   scoped blocking deferred to a future scopes discussion.
2. **Veto matching is plain casefold presence — deliberately NOT
   negation-aware.** Avoids the double-negation rabbit hole, and the costs
   are asymmetric: an over-eager veto loses recall on one chunk (the fact can
   ground elsewhere); an over-timid one loses restraint.
3. **Unconditional "veto wins":** grounding per triple per chunk is
   `(any trigger affirmed) AND NOT (any accepted veto phrase present)`.
   Boolean, commutative — the config stays order-independent, and no
   precedence engine exists to grow. "This hint should override that
   blocker" is answered by retiring the blocker, not by adding a tier.
4. **Accepted hint + accepted blocker on the same triple is a hard
   validation error** (`HintBlockerConflict`) — the engine's semantics would
   not be ambiguous, but the pair is a review mistake at our n; a human must
   retire one. Legal while one side is merely proposed. Blocker + base rule
   is explicitly fine — vetoing a base rule's overreach is the point.
5. **Space-type scope only; review-authored in v1** (no LLM proposing —
   there is no real violation corpus yet); acceptance test manufactures its
   own violation and proves it closes with recall held plus ablation/blocker
   report attribution.

Implementation notes: `effective_config` ignores blockers (they cannot alter
the space type); `effective_vetoes(neurons)` extracts accepted vetoes;
`ground_chunk`/`ingest_chunks` take the veto slice; `blocker_report` is the
restraint counterpart of `ablation_report` (NeverFired / Suppressed /
StillViolated with per-blocker attribution).

## Outcome

- Acceptance suite (`tests/blockers.rs`, deterministic): allegation wording
  ("Meridian is under investigation over claims of Meridian supplying
  Compound X") grounds the forbidden fact on the base config (1/1 violation,
  recall 1/1); a proposed blocker is inert; the accepted blocker closes the
  violation (0/1) with recall held (1/1); `blocker_report` attributes the
  suppression; vetoes are chunk-local (a clean assertion elsewhere still
  grounds — restraint is evidence discipline, not triple erasure); the
  same-triple accepted pair fails validation.
- Benchmarked (see docs/benchmarks.md, grounding section): veto checking
  costs +4% at 8 vetoes; the shared-casefold optimization landed alongside
  (ground_chunk 9.5 → 5.8 µs/chunk with 0 vetoes, 1.6×). At 64 vetoes/space
  cost is 17.1 µs/chunk — fine at realistic veto counts (handfuls); if
  spaces ever carry hundreds of vetoes, multi-pattern matching
  (Aho-Corasick-style, custom per project preference) is the identified fix.
- All 301 workspace tests green; existing fixtures unchanged (parity suite
  passes with empty veto sets).

## Outcome addendum — coverage-guided iteration (2026-07-06)

The blocker-repair experiment (see decision_grounding_gates.md context)
measured one-shot violation-directed blocker proposals at 1/9 closed on
the hostile corpus — chunk-local vetoes vs ~45 differently-phrased
violating chunks, with the coverage simulation disclosing the
incompleteness at proposal time. `propose_blockers_covering` closes the
gap the simulation exposed: per violated fact it proposes, simulates,
and re-proposes against ONLY the still-uncovered chunks (prior phrases
shown to the proposer) until covered, dry (no-progress rounds are
dropped — a useless veto is pure risk), failed, or capped. Measured on
the hostile corpus with gates stripped and a same-day one-shot control:
**one-shot 1/6, iterated 6/6** (2–3 rounds per fact, 15 proposals),
IF-ACCEPTED restraint 6/13 → 0/13 with recall untouched (37/37).
Framing unchanged: iteration makes the neuron-space option COMPLETE,
not structural — authored gates/templates remain the fix for systematic
leakage (0/13 at zero LLM cost); everything still emits
`status: proposed` and blockers never auto-accept (D1/D6,
decision_review_policy.md).
