# Generalization Experiment — Deep Assessment

**Date:** 2026-07-03
**Subject:** Semantic Neurons harness (`cognigraph-construct`) applied to a
third-party document it was never tuned on.
**Verdict up front:** the control loop generalizes — measured gap 2/6 → 6/6
recall with restraint held at 0/3 — but the run's most valuable output was
not the recall number. It was two governance catches: a human review catching
LLM over-inference, and that catch being converted into a mechanical check
the proposer now runs on itself.

---

## 1. Setup

- **Document:** Wikipedia's account of the July 2024 CrowdStrike outage,
  40 chunks (`fixtures/semantic-neurons/generalization/crowdstrike_2024.chunks.jsonl`).
  Third-party prose, not written for this system, never seen by the harness.
- **Ontology:** a fresh `incident_analysis` space type (8 entities, 6 relation
  rules) authored *before* reading grounding results, with deliberately naive
  triggers — the same "first draft an analyst would write" posture the
  original research used.
- **Eval:** 6 expected facts, 3 forbidden facts. The forbidden set includes a
  deliberate trap: `Microsoft --CAUSED--> Global IT outage`. The document
  mentions Microsoft and Windows constantly; the outage was CrowdStrike's
  fault. A co-occurrence-driven extractor is expected to blame Microsoft.
- **Metric:** construction-level — does the fact edge exist in the graph with
  evidence provenance (`fact_exists`: relation + space + evidence chunk).
  This is *not* a retrieval-quality or answer-quality metric (see §6).

## 2. Results

| Stage | Recall | Restraint violations |
|---|---|---|
| Baseline (naive ontology) | 2/6 | 0/3 |
| After proposals accepted (dry-run) | **6/6** | **0/3** |

- **Baseline misses (4):** Falcon Sensor RUNS_ON Windows, outage AFFECTS
  Delta, Kurtz LEADS CrowdStrike, CrowdStrike RESPONDED_WITH faulty update.
  All four are trigger-phrasing gaps, not entity gaps — the naive rules used
  phrases like "runs on" that the encyclopedia register never uses verbatim.
- **The Microsoft trap did not fire.** Important honesty: this is largely
  *restraint by construction*. The ontology has no rule licensing a
  Microsoft-sourced CAUSED edge, so the system cannot manufacture one no
  matter how often Microsoft co-occurs with outage language. That is the
  design working as intended — unlicensed edges are unrepresentable — but it
  is a weaker claim than "negation-aware grounding rejected a tempting
  surface match." The Case Bravo suite covers the latter; this experiment
  covers the former. Both matter; they should not be conflated.
- **Proposer (after the reliability fix):** 4/4 gaps yielded valid proposals,
  every trigger verified verbatim against the document, and accepting all
  four closes recall completely without opening a single violation.

## 3. The review pass — the experiment's real finding

The first proposer run produced a Delta neuron whose rationale read:

> evidence "implies that Delta Air Lines, as an airline, was affected"

— grounded on the generic sentence *"Many industries were affected—airlines,
airports, banks…"*, which never mentions Delta. This is textbook
category-membership inference: exactly the over-reach class that produces
plausible-but-unsupported edges in ungoverned GraphRAG pipelines, and it was
caught at the **proposal boundary** by the human review pass before touching
the graph. The artifact is preserved as
`crowdstrike_2024.neurons.reviewed.json`.

This is the moat mechanism demonstrating itself end-to-end on untuned input:

1. The LLM over-reached (as the paper predicts it will).
2. The over-reach arrived as `status: proposed` — structurally inert.
3. A human reviewer caught the "implies" rationale.
4. The catch was then **mechanized** (§4): the proposer prompt now forbids
   category-membership triggers, and a symbolic self-check rejects triggers
   that don't occur verbatim in the document.

The research-direction doc's own criterion was: *"If it catches real
over-firing on an untuned document, the moat hypothesis earns its first real
evidence."* The trap-fact half of that is only structurally satisfied, but
the Delta catch is a genuine instance of the governance layer stopping real
over-inference on a real document. Combined verdict: the moat hypothesis has
its first field evidence, with the caveat in §2 stated plainly.

## 4. Proposer reliability: before and after

**Before:** 3 of 4 gaps disappeared silently — `else { continue }` on
completion errors and parse failures, plus whole-set validation where one
bad neuron failed the entire batch. A governance loop whose actuator drops
work invisibly is not governed; "no proposal" and "proposal failed" were
indistinguishable.

**After** (`propose.rs`, commit 9cce3ea):

- `propose_neurons_report` returns `ProposalReport { set, skipped }` — every
  gap ends as either a neuron or a `ProposalSkip { fact, reason }`. Zero
  silent paths remain.
- Per-gap retry (2 attempts) on completion, parse, and validation errors;
  per-neuron validation so one malformed proposal cannot sink the batch;
  tolerant parsing of `{"neurons":[...]}` wrapper shapes.
- **Anti-inference rule in the prompt:** triggers must be copied verbatim
  from sentences that explicitly assert the relation; category membership or
  co-occurrence is named and forbidden; an empty trigger array is a
  legitimate decline, reported as such.
- **Symbolic self-check:** triggers that never occur verbatim in the document
  are filtered (they could never ground); an all-paraphrased proposal is a
  retry, not a shipped dud. This check caught real paraphrases in the final
  run — the Delta neuron shipped with 1 verified trigger out of 4 proposed.

**A second-order finding fell out of the fix.** With drops visible, the next
run showed the model *declining* the Kurtz and Delta gaps ("no explicit
supporting sentence") even though the document states both facts outright
("CrowdStrike CEO George Kurtz confirmed…", "Delta, the most affected of the
US major airlines"). The failure was in **evidence selection**, not the
model: `candidate_chunks` took the first 4 chunks mentioning *either*
endpoint, and popular endpoints (CrowdStrike, the outage) flooded the window
with chunks that never mention the rare endpoint. Ranking both-endpoint
chunks first (take 6) fixed it — 4/4 proposals immediately after. Lesson:
in a gap-directed proposer, retrieval quality bounds proposal recall, and
skip reasons are what make that diagnosable. Silent drops would have
attributed this to "the LLM being unreliable."

## 5. Reliability of the loop as a whole

- Closed-loop tests (manufactured gap → propose → inert → accept → closed)
  pass against the scripted proposer, live OpenAI, and live Gemini.
- The verbatim self-check also fixed a live-test flake: a paraphrased trigger
  passed schema validation and human-plausibility but silently failed to
  ground, leaving the gap open after acceptance. That failure mode is now
  caught before the proposal ships.
- The `IF ACCEPTED` dry-run in the example re-ingests under
  `effective_config` without persisting acceptance — the review remains the
  human's; nothing in the pipeline can promote a neuron.

## 6. Honest limits

1. **Not fully blind.** The same agent authored the space type, the eval, and
   the system under test. The naive triggers were written in good faith
   before grounding, but this is a weaker protocol than an independent
   author. A real test needs a second person's document + eval.
2. **Construction-level metric.** "Fact edge exists with provenance" is the
   right metric for this layer, but it says nothing about downstream answer
   quality. The research's claim chain (better edges → better GraphRAG
   answers) still needs an end-to-end retrieval eval.
3. **Restraint evidence is partly structural** (§2). The trap held because
   the ontology never licensed the edge, not because grounding rejected a
   near-miss. Add forbidden facts that *are* licensed by a rule but only
   supported by negated/speculative sentences to test the grounding half on
   third-party prose.
4. **Verbatim substring grounding is brittle by design.** Typographic quotes,
   whitespace, and paraphrase all break matching. The self-check turns this
   brittleness from a silent failure into a visible one, but smarter
   (still-symbolic) matching — normalization, sentence-level anchors — is the
   real fix.
5. **Single document, single register.** One encyclopedia article. Different
   registers (chat logs, contracts, clinical notes) will stress the trigger
   model differently.
6. **LLM nondeterminism.** Run-to-run variance in proposals was observed
   (one run proposed 2, the next 4 with different triggers). The loop
   tolerates this — proposals are reviewed, not trusted — but any published
   number should be a multi-run distribution, not a single sample.

## 7. What this earns / next steps

- **Earned:** the port is not a re-implementation of a demo; the loop
  measures, repairs, and holds restraint on input it never saw, and its
  governance boundary caught real over-inference. The proposer is now
  observable end-to-end.
- **Next, in rough order of leverage:**
  1. Evidence retrieval for `candidate_chunks` via the native BM25/vector
     search instead of surface matching — the experiment showed retrieval is
     the proposer's bottleneck, and the backend already has both.
  2. Licensed-but-negated forbidden facts on third-party prose (close the
     structural-restraint caveat).
  3. `relation_rank_hint` neuron type — the research's own lowest-risk next
     step.
  4. An independent-author blind eval (second document, second person).

---

**Post-assessment update (2026-07-03):** the review accepted all four
proposals. The accepted set lives in
`crowdstrike_2024.neurons.accepted.json`, and the closed-loop result
(6/6 recall, 0/3 violations) is pinned as a deterministic regression test:
`crowdstrike_accepted_neurons_close_the_generalization_loop` in
`crates/cognigraph-construct/tests/parity.rs`.
