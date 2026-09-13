# Decision: grounding semantics for systematic leakage (D1–D4)

**Status:** Decided 2026-07-06 (user approved all four); landed same day.

## Context

The QW6 hostile-scale eval (1,120 chunks, author-curated adversarial
near-misses) held recall at 37/37 but violated **9 of 13 distinct
forbidden facts**: literal trigger matching grounds a licensed triple
wherever its phrase appears affirmed — including chunks about a
*different* company (cross-company leakage) and reversed-direction
transaction phrasings. The governed repair loop could not fix it:
blockers closed 1/9 (chunk-local vetoes vs ~45 differently-phrased
violating chunks per fact), with the coverage simulation disclosing the
incompleteness at proposal time. Neuron-space was measured out; the
grounding contract itself had to move.

**Every candidate was simulated before the discussion**
(`examples/endpoint_gate_sim.rs`, deterministic, run on the hostile
corpus AND all four blind kits):

| gate | hostile violations (distinct) | hostile recall | honest-kit recall damage |
|---|---|---|---|
| none (then-current) | 9/13 | 37/37 | — |
| chunk-level both-endpoints | 8/13 | 36/37 | procurement −5, market-size −4 |
| sentence-scoped source | 4/13 | 30/37 | market-size −7, procurement −4 |
| sentence-scoped both | 3/13 | 29/37 | market-size 1/15 — catastrophic |

Two facts fell out: the sentence-scoped mechanism is RIGHT (9→4), and
every GLOBAL application of it is wrong (honest-kit recall collapses,
because legitimate triggers often sit in sentences whose subject is
implicit or phrased outside the alias list). On the hostile corpus the
violated rules and the recall-carrying rules are disjoint, so
author-scoped gating achieves the reduction at zero recall cost.

## Decisions (owner: user, 2026-07-06 design session)

**D1 — Sentence-scoped endpoint gating: per-rule opt-in, never global.**
`RelationRule.require_in_sentence: Vec<EndpointRef>` (`source`/`target`;
an enum so typos fail loudly at deserialization — set-but-ignored
restraint config is the worst failure mode). When set, the affirmed
trigger occurrence's SENTENCE (coarser bounds than the negation clause:
`.` `!` `?` newline) must contain each listed endpoint's surface (name
or alias). Checked PER OCCURRENCE: a later occurrence in a satisfying
sentence grounds when an earlier one fails, mirroring negation lookback.
Relation-hint neurons extend a rule's triggers and inherit its gate; a
hint that creates a new rule carries no gate.

**D2 — Template triggers for direction and subject-binding.** A trigger
containing `{source}`/`{target}` expands against the endpoints' surfaces
at grounding time and matches only direction-faithful phrasings. Plain
triggers are unchanged; the matched EXPANSION is recorded as the
grounded trigger (with its span), so provenance stays verbatim. This is
the only mechanism among the candidates that closes transaction-
direction traps — both endpoints legitimately share every acquisition
sentence, so no presence gate can separate directions.

**D3 — Absence claims: no change, verified adequate.** The 4 distinct
forbidden facts that HELD on the hostile corpus are the
absence/boundary family — the existing negation-aware matcher already
refuses them. Question closed with evidence rather than code.

**D4 — Eval counting: distinct facts.** `evaluate()`'s `Vec::dedup` was
adjacent-only, silently making totals per-mention across questions
(hostile "14/18" = 9/13 distinct). Now order-preserving distinct.
Re-measured baselines (all conclusions unchanged): hostile 37/37 +
9/13; blind kits 13/13, 17/17, 14/14, 15/15 with 0/8, 0/8, 0/6, 0/10.
Historical results files carry dated annotation notes; per-question
numbers keep mention-level granularity.

## Boundaries deliberately kept

- **Neurons cannot set or propose gates or templates.** Both are
  authored ontology config, visible in the space type a reviewer reads.
  The "neurons cannot alter matching semantics" governance line stays.
- **Veto matching stays plain casefold presence** (no gates, no
  templates): the asymmetric-cost rationale from
  decision_relation_blocker.md is unchanged — an over-eager veto costs
  one chunk's recall; an over-timid one costs restraint.
- Default behavior without opt-ins is byte-identical to before; the
  entire corpus of prior results remains valid.

## Outcome

Landed 2026-07-06: `EndpointRef` + `require_in_sentence` on rules,
template expansion in `ground_chunk`, per-occurrence gate via
`affirming_offset_where`, distinct eval counting, tests pinning
wrong-subject refusal, per-occurrence gating, alias surfaces, loud
deserialization failure, direction-faithful and negation-aware template
matching. Guides (dataops 04, blind TEMPLATE) document both tools with
the "gate the risky rules, not everything" guidance.

Applied to the hostile fixture after user approval: the author-owned ontology
gated/templated the leakage-prone rules and re-ran at **37/37 recall, 0/13
restraint violations**. The remaining answer-layer failure is recall/selectivity
(`recall_ok=false`, `restraint_ok=true`), not forbidden construction leakage.

## Outcome addendum — the gate advisor (2026-07-06)

The "gate the risky rules, not everything" guidance is now a measured
tool instead of author intuition: `POST /api/construct/advise` /
`cognigraph advise SPACE` / `examples/gate_advisor.rs` ground the corpus
under each candidate gate (via `ground_chunk` itself — production
semantics) and report a SAFE suggestion where a gate blocks off-subject
groundings at zero corpus recall cost, plus a REVIEW flag where an
endpoint appears in NO licensing sentence — the hostile-leakage
signature, deliberately never auto-suggested because it is ambiguous
with legitimately cross-sentence evidence (sample sentences attached;
the author decides). Validation: with this fixture's authored gates
stripped, the advisor rediscovered **6/6 gates on the exact endpoints**
(five source, one target). The advisor also exposed a production gate
bug: `sentence_bounds` split sentences at decimal points ("$3.09B"),
mis-scoping D1 gates on numeric-endpoint rules — fixed, regression
test added, all kit baselines re-verified byte-identical. The
"neurons cannot set gates" boundary is untouched; the advisor reads,
never writes.
