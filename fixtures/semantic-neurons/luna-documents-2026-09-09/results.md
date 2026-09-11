# Luna-low document baseline — development results

**The document path needs further work before holdout evaluation.** All 40
development calls completed, but Luna proposed only 55 relations against 683
published in-scope reference triples. CogniGraph stored 32 occurrences; 16
matched the reference. This is a diagnostic baseline, not a customer-accuracy
estimate or a reason to change the selected model.

The 80 holdout documents remain unrun. The corpus, labels, taxonomy, selection,
production prompt/schema, binary and source hashes were frozen before the calls.
No Rust implementation or model-default change was made during the trial.

## Results at distinct document/triple grain

| Measure | Before gates | Stored after gates |
|---|---:|---:|
| Predicted distinct relations | 55 | 32 |
| Matches to published reference | 21 | 16 |
| Unmatched predictions | 34 | 16 |
| Missed reference relations | 662 | 667 |
| Annotation-reference precision | 38.18% | 50.00% |
| Annotation-reference recall | 3.07% | 2.34% |
| Annotation-reference micro F1 | 5.69% | 4.48% |
| Macro per-relation reference F1 | 14.53% | 11.00% |
| Documents with no output | 13 / 40 | 23 / 40 |

The gates removed five reference matches and eighteen unmatched predictions.
The 23 rejection reasons were seven quote mismatches, five missing affirmed
vocabulary matches, five unknown chunk IDs, four absent sources and two absent
targets. This describes the frozen initial taxonomy and current runtime; it
does not show that every gate removal was semantically correct or incorrect.

The model discovered 54 of 320 in-scope reference endpoint entities before
gates and 39 after gates. This is relation-endpoint coverage, not standalone
named-entity recognition. The one development document without any in-scope
source annotation received an empty output. A single unlabelled document cannot
establish an abstention rate against independently verified negatives.

Mean HTTP latency was **4.126 seconds**, median 3.785 and maximum 13.120. There
were no provider/HTTP failures or retries. Usage was 35,563 input tokens and
10,781 output tokens, including 5,907 reasoning tokens. The frozen standard
tariff estimate, including reported cache writes, is **$0.0201583**; all forty
calls supplied usage. This is a token-cost estimate, not an invoice.

## Interpretation and source limits

These scores compare predictions with the published reference, not independently
exhaustive truth. Country and administrative-location relations account for
616 of 683 development references. For 417 references the endpoints have no
co-sentence annotated mentions, and 358 lack source evidence-sentence IDs.
The source includes document-level inference; the runtime asks for explicit
quoted assertions. Ten references cannot be uniquely identified through their
annotated surface aliases. All remain in the reported denominator.

The cohort contains whole annotated benchmark documents, typically about 200
tokens, rather than full Wikipedia articles or long customer reports. Source
annotations include entity-granularity and alias limitations: for example, the
reference splits `Mecklenburg - Vorpommern` into separate endpoint entities,
while Luna nominates the complete place name. Such disagreement cannot be
reported as a proven false fact. The [source audit](data-quality.json) and
[independent-review guide](review/ANNOTATION_GUIDE.md) describe the remaining
qualification work. Eighty preselected near-miss candidates remain unreviewed;
no synthetic or model-generated labels were substituted for negative gold.

This task differs from the earlier supplied-pair SemEval comparison in document
scope, taxonomy, entity discovery, matching and inference policy. Its scores
must not be presented as a comparable drop from that trial's 70.08% accuracy.

## Two actionable runtime findings

1. [CG-39](../../../docs/issues/CG-39.md): endpoint checks accept a name inside
   another word. A separate real release-server probe persisted `Ann OWNS Acme`
   from `Joanne owns Acme.` The document trial also accepted `South Africa`
   from a `South African` mention. This violates the endpoint evidence contract.
2. [CG-40](../../../docs/issues/CG-40.md): the strict provider schema does not
   bind chunk IDs or relation names to the request. Five Luna proposals in two
   documents cited `chunk development-...`; the supplied IDs lacked that prefix.
   The gates rejected them correctly, leaving an avoidable generation-contract
   gap. Constrain the schema without heuristically repairing invalid citations.

The [three-case release reproduction](../../../docs/issues/evidence/directed-document-contracts-2026-09-09.json)
uses only synthetic provider responses and a fresh database per case. It
includes a correct complete-name positive control. Neither finding was fixed
inside this frozen experiment.

## Scoring correction and verification

The original frozen scorer stopped because it asserted that every proposal
cited a supplied chunk ID. The [original failure](original-scorer-failure.txt)
is preserved. [Scorer V2](score_v2.py) counts those five invalid citations as
unmatched and gives them no reference credit; it does not repair or omit them.
The [amendment](scoring-amendment-v2.json) records the exact code hashes and a
verifier tuple/JSON-array comparison correction. No gold, prompt, runtime, or
model output was changed, and no extra provider calls were made for scoring.

Ten regression tests passed. Before live generation, all 40 gold-control
requests passed through the real release server: all 673 references with
unique endpoint aliases scored correctly before gates, with the ten ambiguous
references retained as misses. The controls verified 647 stored occurrences.
After live generation, [verification](verification.json) reconstructed the
120-document selection from 1,000 validated source documents, checked 525
source/dependency/helper files and eighteen original package files, reproduced
scores and costs, matched all forty live prompts to their pre-run controls, and
verified every one of the 32 live stored evidence spans and attributions.
Five [integrity probes](integrity-probes.json) rejected corrupt captures,
missing attempts, changed effort, altered evidence and cross-document inputs.

**Next:** fix CG-39, then CG-40, and compare a separately frozen candidate on
development data. Independently adjudicate the reference/evidence-policy gaps
before making precision or restraint claims or running the holdout. Keep Luna
low as the model baseline and preserve every historical package.
