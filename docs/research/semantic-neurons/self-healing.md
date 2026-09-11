# Self-Healing Experiment (B7)

**Date:** 2026-07-05
**Question:** the Semantic Neurons paper's origin idea — neural pathways
that fail get rewired through alternative routes — can it run as an actual
mechanism? Concretely: when a document REVISION kills the wording that a
space's grounding pathways depend on, does
*detect degradation → propose alternative grounding → review → prune*
restore capability?

**Answer: yes, end to end, live.** The loop healed a 6/6→1/6 recall
collapse back to 6/6 with restraint held, using only machinery that
already existed for other purposes — which is the finding.

## Setup

- Space: the CrowdStrike generalization fixture (untuned third-party
  article, 40 chunks) with its four human-accepted repair neurons —
  the healthy state is 6/6 recall, 0/3 restraint violations.
- Degradation event: a synthetic *editorial revision* of the article
  (`crowdstrike_2024.chunks.v2.jsonl`): 6 of 40 chunks paraphrased so that
  every accepted-neuron trigger phrase disappears — meaning preserved,
  wording changed. Exactly what happens when a source document gets
  rewritten, retranslated, or re-chunked.

## The loop, measured (live run, gpt-4.1-mini as the rewirer)

| stage | recall | restraint |
|---|---|---|
| v1 healthy | 6/6 | 0/3 |
| v2 after revision | **1/6** | 0/3 |
| v2 after rewiring + review | **6/6** | **0/3** |

1. **Detect** — `degradation_report` (new, trivial on top of the grounding
   machinery): pathways whose triggers no longer affirm anywhere. The
   revision killed NINE: all four accepted neurons AND five naive base
   rules. Degradation is per-pathway, not per-fact — a base rule and a
   neuron on the same triple die independently.
2. **Rewire** — gap-directed proposing over the revised corpus
   (`propose_neurons_via_backend`, B4's BM25 retrieval). The live model
   proposed repairs for all five missing facts, every trigger verbatim
   from the v2 wording, zero skips. Base-rule deaths matter here: the
   loop cannot edit the ontology, so **neurons are the only rewiring
   mechanism for dead base pathways** — the mechanism the paper's
   metaphor actually predicts.
3. **Review** — proposals arrive `proposed`; acceptance was simulated in
   the experiment runner and is a human act in production (B5 endpoints).
4. **Prune** — post-healing, `graduation_report` is the pruning detector:
   a dead original whose fact now grounds via the rewired pathway flags
   `covered_by_others`. The live run produced an organic twist: the
   revision's new wording ("George Kurtz, chief executive of CrowdStrike")
   incidentally satisfied a naive BASE rule that had never fired before —
   so the old Kurtz neuron flagged `covered_by_base`: the document drifted
   back under the ontology, and the loop noticed.

## Deterministic regression

`tests/self_healing.rs` pins the whole cycle with a scripted proposer
(no LLM): healthy → degraded (recall breaks, 4 neuron pathways flagged
dead) → rewired (all gaps, proposals inert until accepted) → healed
(recall restored, restraint held) → pruned (every dead original flags,
no rewired neuron flags).

## Honest limits

1. **The revision is synthetic** (hand-paraphrased passages, occasionally
   ungrammatical at splice points). A real revision history — Wikipedia
   diffs of the same article — is the natural follow-up.
2. **ID collision nuance:** the live model reused three of the original
   kebab ids for its rewired neurons. In the library-level combine this
   breaks leave-one-out attribution (two neurons, one id); the `/api/neurons`
   route is immune (`_key` conflict on create). Reviewers should rename
   on acceptance when a rewired neuron shadows a dead one, or retire the
   dead one first.
3. **Acceptance was simulated** in the live run. The loop's claim is that
   the *mechanism* works; the governance claim (review catches bad
   rewires) rests on the earlier generalization evidence.
4. Restraint never wavered (0/3 at every stage), but this revision was
   recall-targeted; a revision that ADDS misleading wording is the
   restraint-side experiment (blockers + B3 are the machinery for it).

## For the paper

The framing writes itself, and now has numbers behind it: grounding
pathways are axon-like routes from evidence to fact; document drift is
lesion; gap-directed proposing is rewiring through alternative routes;
review is the safety interlock biology doesn't have; graduation is
synaptic pruning. Every step maps to a mechanism that exists in code and
is covered by a deterministic test. Reference the neural self-repair
literature generically as inspiration.
