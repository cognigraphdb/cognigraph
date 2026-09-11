# Vector-RAG control arm — 2026-07-07, `gpt-5.4-mini` + `text-embedding-3-small`

The head-to-head the positioning dossier was missing: same five kits,
same questions, same answering model, same one-pass/two-pass technique —
the only variable is the substrate. Vector arm: top-8 chunks by cosine
(question vs chunk embeddings), raw passages to the model. Graph arm:
the shipped pipeline (authored ontology → grounded facts → ranked
48-edge trace). Runner: `examples/vector_rag_control.rs`.

Fairness asymmetries, all favoring the VECTOR arm, disclosed up front:
it receives the answer vocabulary (entity names + relation labels — the
same vocabulary the graph trace exposes implicitly); its recall is an
upper bound (no fabrication guard is possible over raw passages); K=8
passages carry several times the token budget of the 48-line trace.

## Result

| combined (105 expected, 54 forbidden) | vector | graph |
|---|---|---|
| one-pass recall | 30/105 (29%) | **85/105 (81%)** |
| two-pass recall | 38/105 (36%) | **90/105 (86%)** |
| forbidden asserted, one-pass | 2/54 | **0/54** |
| forbidden asserted, two-pass | **4/54** | **0/54** |

Per kit (recall two-pass, vector vs graph): vendor 5/19 vs 18/19;
digital-transformation 4/15 vs 14/15; procurement **9/17 vs 8/17**
(the one exception — see finding 3); market-size 12/17 vs 17/17;
hostile 8/37 vs 33/37.

## Findings

1. **The recall gap is the paper's §1 claim, measured: dispersed-
   evidence assembly is where vector retrieval starves.** A question
   whose complete answer needs 5–7 facts scattered across many chunks
   gets top-8 passages by similarity to the QUESTION — most of the
   evidence never reaches the model (worst on the 1,120-chunk hostile
   corpus: 8/37 vs 33/37). The graph trace aggregates facts from the
   whole corpus by entity seeding and ranked traversal; the substrate,
   not the answering model, is the difference (same model, same
   prompts).
2. **The vector arm leaked forbidden facts, and the completeness pass
   made it WORSE (2/54 → 4/54)** — the additions-only critic pushes
   toward inclusion, and with no trace check between passages and
   assertions there is nothing to catch it. On the graph arm the same
   two-pass technique stayed 0/54 in both modes, because every
   addition is still checked against constructed, restraint-gated
   facts. The leaks are exactly the boundary-question traps
   ("publishes a canonical vendor roster/architecture") the kits were
   built to set. This is the sharpest single result: **the same
   answering technique is safe on one substrate and unsafe on the
   other.**
3. **One honest exception: the procurement kit's meta-questions**,
   where vector two-pass (9/17) edged the graph arm (8/17) — the same
   questions behind the graph arm's known 14/105 residue. Deep
   process/meta questions benefit from raw prose; fact-line traces
   compress away the connective tissue. This localizes the residue:
   it is a trace-representation limit, not an answering-model limit.
4. Graph one-pass measured 85/105 here vs 77/105 in the two-pass
   experiment two days ago (same config) — answer-layer run-to-run
   variance is real and now has a second data point; two-pass is
   stabler (90 vs 91).

## Prediction scorecard (recorded before the run)

- "Recall will be competitive on single-hop questions" — **WRONG**:
  recall collapsed 2.4× even with every asymmetry favoring the vector
  arm. The eval questions are enumerations over dispersed evidence,
  and that is the honest shape of real questions in these domains.
- "The hostile corpus will be brutal for the vector arm on restraint"
  — **directionally right, magnitude wrong**: it leaked (4/54 across
  kits incl. one hostile trap) rather than hemorrhaged — a capable
  model reading full passages avoids many cross-company traps that
  fool literal matchers. The restraint advantage is real but its
  headline is finding 2 (two-pass unsafe without a trace check), not
  raw leak volume.

## Caveats

Single run, single model family, K=8 (untuned); the vector arm is a
plain-cosine baseline, not a tuned hybrid-rerank pipeline — this
measures the substrate argument, not the best possible vector system.
