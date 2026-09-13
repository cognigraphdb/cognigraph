# Decision: Port Semantic Neurons as the CogniGraph construction layer

Date: 2026-07-03 · Status: ACCEPTED, v1 delivered (M14)

## Context
The founder's Semantic Neurons research (paper + HugeGraph repo) established
that LLMs extract entities but not relations, that false edges corrupt
silently, and that the fix is a governed, evidence-bound, reviewable
edge-construction loop measuring both recall and restraint. The paper is
explicitly substrate-independent; CogniGraph already covered the research
repo's infrastructure gap list (durable reviews, vector lifecycle, atomic
version publishes, provenance).

## Decision (owner: skitsanos — five explicit calls, all "yes")
1. v1 = alias + relation_hint neurons, negation-aware grounding, recall +
   restraint eval, ablation/redundancy reports; `relation_rank_hint` next.
2. Parity with the research evals is the acceptance test.
3. Exit criterion: the generalization experiment (harness vs a messy
   third-party document nobody authored evals for).
4. `CompletionProvider` trait (OpenAI + Gemini) drives gap-directed
   proposals — proposals are always `proposed`, never auto-accepted.
5. Keep the name "Semantic Neuron".

## Outcome
`cognigraph-construct` shipped with faithful ports of the config schema,
the `_affirms_phrase` negation semantics (Case Bravo as a unit regression),
and the lifecycle. Parity achieved on first run: SOTU full recall with
accepted neurons, case:alpha and study:px-101 on base ontology, Case Bravo
full recall with 0/4 forbidden, and the redundancy/graduation finding
reproduced. The closed loop (measure gap → propose → validate → accept →
re-measure) passes deterministically and live against both OpenAI and
Gemini. Decision 3 was subsequently completed on the CrowdStrike document,
followed by hostile, blind/gap, DailyMed, and WebNLG programmes. The current
evidence boundaries and corrected distinct-fact counts live in
`docs/semantic-neurons/positioning.md`; this original decision is no longer the
status page for generalization work.
