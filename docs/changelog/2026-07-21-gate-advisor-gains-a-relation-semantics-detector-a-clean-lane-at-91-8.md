# Gate advisor gains a relation-semantics detector — a clean lane at 91.8%

- Date: 2026-07-21
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:705-723` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Gate advisor gains a relation-semantics detector — a clean lane at 91.8%.**
  The existing endpoint-presence detector separated nothing on the pilot (70.4%
  clean vs 72.9% flagged); after the administrative fix, `wrong_relation` was 70%
  of every remaining failure and it is structurally blind to that. `advise_gates`
  now also asks whether each licensing sentence **asserts** the relation, via
  three deterministic signals: the target is absent from the sentence, the
  endpoints are merely co-listed (`myopathy and rhabdomyolysis` enumerates, it
  does not relate), or the sentence never uses the relation's own vocabulary
  (`atorvastatin --TREATS--> MI` off "reduce the risk of MI"). Reported per rule
  as `semantics_suspect` + `semantics_samples`; **advisory only, nothing is
  dropped**, so ground-then-advise is untouched. Measured on the shipped code:
  **clean lane 91.8% vs flagged 76.7%, separation +15.1%** (old detector: −2.5%),
  catching 68% of all errors. Designed on 200 judged facts from one ontology and
  confirmed on 200 from another; **three of six candidates were rejected by that
  holdout**, including the union of everything (+24.9% on design, −0.8% held out)
  and "source absent from the sentence", which is *anti*-correlated with error —
  a label names its drug once and refers to it implicitly after, which is exactly
  why the symmetric endpoint-presence check cancels itself out.
