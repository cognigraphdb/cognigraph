# Side views as a gap source: measurement under trigger ablation

**Date:** 2026-09-23
**Issue:** [CG-97](../../issues/CG-97.md), measuring the source built in
[CG-88](../../issues/CG-88.md) under the
[side-view gap detector decision](../../decisions/decision_sideview_gap_detector.md).
**Verdict up front:** on two small public kits, side views nominated every
missing expected fact in every repetition. They also nominated a negated,
forbidden fact as the strongest candidate each time. The proposer declined
that fact, so restraint held, but it held downstream of the detector, not in
it.

## Setup

- **Kits:** the public forensics kits `forensics_case_alpha` (3 documents, 6
  expected facts, no forbidden fact) and `forensics_case_bravo` (4 documents,
  9 expected facts, 4 forbidden facts), from `fixtures/semantic-neurons/`.
- **Why ablation:** both kits already build every expected fact, so there is
  no gap to find. In a disposable copy of the `digital_forensics` space type
  the harness removes the triggers of exactly the rules behind each kit's
  expected facts (6 rules for alpha, 9 for bravo). The rules themselves stay,
  so baseline recall drops to 0/6 and 0/9.
- **Protocol per kit and repetition:** a fresh Enterprise 2.7.37 server with
  a temporary store; ground the chunks; store them as documents; generate 12
  side views per document; run the detector as a dry run at `min_support` 1
  and 2; propose from side views at `min_support` 1.
- **Review:** evaluation-backed. Each stored proposal is accepted alone, the
  chunks are re-grounded and the kit is re-evaluated. The proposal survives
  when it adds an expected fact and builds no forbidden fact. Then all
  survivors are accepted together for the repaired recall.
- **Models:** `gpt-5.6-luna` for side-view generation and proposals, as
  authorized by the owner; Gemini embeddings as configured locally. Three
  repetitions.
- **Harness:** [`sideview-gap-measurement.py`](../../verification/harnesses/sideview-gap-measurement.py),
  with a provider-free `--preflight` mode.

## Results

Values are per repetition. Recall is distinct expected facts built.

| Kit | Side views | Baseline | Candidates, support 1 | Missing expected / forbidden among them | Candidates, support 2 | Proposals stored | Survived review | Recall after repair | Forbidden built |
|---|---|---|---|---|---|---|---|---|---|
| alpha | 36, 36, 36 | 0/6 | 6, 6, 6 | 6 / 0 | 6, 6, 6 | 5, 5, 5 | 5, 5, 5 | 5/6, 5/6, 5/6 | 0 |
| bravo | 48, 48, 48 | 0/9 | 10, 10, 10 | 9 / 1 | 9, 9, 10 | 8, 8, 7 | 8, 8, 7 | 8/9, 8/9, 7/9 | 0 |

Other skips at support 1:
- `no_matching_rule` pairs numbered 6 to 12 per run.
- `endpoints_connected` removed 6 pairs in every alpha run. Those pairs are grounded by rules outside alpha's evaluation.
- No pair was `ambiguous_relation`, because each entity pair in this ontology has at most one rule.

## Findings

1. **Detector recall was complete.** Every missing expected fact was a
   candidate in all six runs at support 1. No candidate fell outside the
   kits' evaluations.
2. **The detector nominates negated facts.** Bravo's forbidden
   `iPhone 14 --RUNS_APPLICATION--> Signal` had support 7 of 48 in every
   run. The kit's text says it is not true that the iPhone 14 contained
   Signal. Side views answer questions about that sentence and name both
   entities, and the detector reads mentions, not polarity. Raising
   `min_support` to 2 did not remove it.
3. **Restraint held at the proposer.** The proposer declined the forbidden
   candidate on both attempts in every run, so it was never stored. No
   stored proposal built a forbidden fact, alone or combined.
4. **The proposer also declined four expected facts across the runs:**
   - `iPhone 14 --RUNS_APPLICATION--> Signal` in alpha;
   - `Samsung Galaxy S21 --RUNS_APPLICATION--> WhatsApp` in bravo, in all runs;
   - `Case Alpha --INVESTIGATES--> Alex Morgan` in one bravo run.

   The ontology's own triggers read "contained" as running an application.
   The model did not accept that reading, and it gave the same reason every
   time: no explicit supporting sentence. These losses belong to the
   proposal step, not the detector. They cap recall after repair at 5/6 and
   7 to 8/9.
5. **Support 2 cost recall without adding restraint.** In two bravo runs,
   the only side view naming `Case Alpha` and `Alex Morgan` together was a
   single one, so support 2 dropped that expected fact.

## Consequences

- The default `min_support` stays 1. On these kits, support was not a
  restraint lever: the forbidden candidate had the highest support.
- The detector remains a candidate generator. Restraint depends on the
  proposer's verbatim and decline checks and on review, as the decision
  already states. The proposer's refusal of the negated fact is the
  evidence that this layering works here.

## Spend

The server does not meter calls. The protocol implies:
- 21 side-view generation calls;
- between 58 and 96 proposal completions, because each gap gets up to two attempts;
- 21 embedding batches.

A first attempt stopped at the embedding step after 1 generation call.

## Limitations

- Two small synthetic kits (7 documents in total). Their rules name entity
  instances, one rule per pair, and ablation keeps the rules. This is the
  easiest case for the detector. A real corpus with sparser ontologies will
  show more `no_matching_rule` and less recall.
- Side-view texts were not exported before the disposable stores were
  deleted; the records keep ids, supports and counts.
- Evaluation-backed review sees only the kits' expected and forbidden facts.
  A relation hint extends only its own rule, so it cannot build another
  triple.
- One model, three repetitions, no human review. Model output varies between
  runs, as the bravo counts show.

The raw records (every HTTP exchange, provider log lines, per-proposal
reviews, the failed first attempt and the harness as run) are sealed with a
SHA-256 manifest in the private evidence repository.
