# DailyMed clinical domain-expert reference

This workflow closes the only label-gated part of the DailyMed pass-2
evaluation: real-corpus recall and clinical judge quality for `TREATS` and
`CONTRAINDICATED_IN`. It does not use an LLM to create, complete, or adjudicate
gold labels.

## Review design

Use two independent reviewers with clinical medication-label expertise and a
third qualified adjudicator (or one of the two reviewers plus a qualified tie
breaker). At least one reviewer should be a pharmacist, physician, or similarly
qualified drug-label specialist. Record reviewer roles separately from the
de-identified annotator IDs stored in the files.

The default 50-document cohort is deterministic and split into:

- 10 calibration documents, reviewed jointly to freeze boundary decisions;
- 40 evaluation documents, labeled independently and scored only after
  adjudication.

The calibration rules additionally have a content-hashed development-showcase
acceptance artifact at
`fixtures/semantic-neurons/dailymed/clinical-calibration-v1.json`: 19 TRUE and
8 FALSE cases with exact evidence. The project owner explicitly accepted this
Codex-assisted calibration because the work is a development showcase on
third-party data. It is not external clinical validation and does not replace
the two clinically qualified reviewers required for the held-out reference or
production claims.

Preparation selects labels by a seeded hash of DailyMed Set ID, not by
CogniGraph output or judge decisions. Only the INDICATIONS AND USAGE
(`34067-9`) and CONTRAINDICATIONS (`34070-3`) sections are exposed. A fixed
public condition vocabulary pre-populates possible review cases, but those
candidates are not labels. Reviewers must decide every candidate and add facts
outside that vocabulary so the reference is exhaustive for the two relations.

## Commands

Create the blinded workspace (the command refuses to overwrite an existing
workspace):

```bash
cargo run --release -p cognigraph-construct \
  --example dailymed_clinical_reference -- prepare
```

Read `data/dailymed-clinical-reference/ANNOTATION_GUIDE.md`. Review calibration
first, record and freeze boundary decisions, then make independent copies of:

- `annotations/expert-a.jsonl`
- `annotations/expert-b.jsonl`

For every document, replace all `unreviewed` verdicts, add any missed facts,
and set `complete` to `true`. Evidence must be copied exactly from the named
section. Validate each file before comparison:

```bash
cargo run --release -p cognigraph-construct \
  --example dailymed_clinical_reference -- validate \
  --annotations data/dailymed-clinical-reference/annotations/expert-a.jsonl

cargo run --release -p cognigraph-construct \
  --example dailymed_clinical_reference -- validate \
  --annotations data/dailymed-clinical-reference/annotations/expert-b.jsonl
```

Create the adjudication file:

```bash
cargo run --release -p cognigraph-construct \
  --example dailymed_clinical_reference -- compare
```

Agreements are copied. Missing facts and verdict disagreements become
`unreviewed` with both reviewer decisions recorded in `notes`. The adjudicator
resolves those rows, verifies the agreed rows, and marks every document
complete. Validate the result, then compile and score it:

```bash
cargo run --release -p cognigraph-construct \
  --example dailymed_clinical_reference -- validate \
  --annotations data/dailymed-clinical-reference/adjudication.jsonl

cargo run --release -p cognigraph-construct \
  --example dailymed_clinical_reference -- compile

cargo run --release -p cognigraph-construct \
  --example dailymed_clinical_reference -- score
```

`score` measures the frozen grounding mechanism with the expert-authored
condition vocabulary. It reports recall, restraint, precision, and confusion
counts overall and per relation. This separates relation grounding from open
entity discovery. Calibration documents are excluded unless
`--include-calibration` is explicitly supplied.

Finally, measure the existing judge against the frozen expert cases:

```bash
cargo run --release -p cognigraph-construct \
  --example dailymed_clinical_reference -- judge
```

The judge sees each expert evidence case only after the reference is complete.
Its accepted-set precision and confusion counts use the frozen production
policy (`accept` with confidence at least 0.90). This is judge quality on the
expert reference cases, not on the complete distribution of future model
proposals. Use `--limit N` only for a costed smoke test; do not report a limited
run as the final measurement.

## Integrity rules

- Never inspect CogniGraph grounding or judge output before both independent
  annotation files are frozen.
- Keep `uncertain` cases in the reference for auditability; scoring excludes
  them rather than silently coercing them.
- Do not report calibration results as held-out evaluation results.
- Preserve `packets.jsonl`, the annotation files, adjudication, generated
  scores, seed, source URLs, Set IDs, and SPL versions together.
- The reference evaluates extraction from labeling; it is not medical advice
  and does not independently establish clinical truth beyond the cited label.

## Frozen calibration rules (joint review, 2026-07-14)

Ratified over the 10 calibration documents (24 surfaced candidates). These
govern the held-out labeling and adjudication.

**R1 — Concept preservation (governing rule).** A qualifier may be dropped
only when it does not change the clinical concept's *extension*. Essential
qualifiers — subtype, severity, anatomy, timing, causative drug, treatment
combination — must be preserved. A lossy broad candidate is FALSE, and the
exact qualified concept is added as TRUE. ("Known" may be dropped: it does
not alter the condition. The allergen may not.)
Therefore: systemic fungal infection != infection; moderate/severe
hypertension != hypertension; supine hypertension != hypertension;
partial-onset seizures != seizures; hypersensitivity to pregabalin !=
hypersensitivity.
Any resulting loss of generic-vocabulary recall exposes a
vocabulary/modeling limitation; it never justifies a broader gold label.

**R2 — Adjunctive therapy counts as treatment,** but the target must retain
its specific subtype (R1 still applies).

**R3 — Treated target, not associated context.** "Adjunctive therapy in
edema associated with congestive heart failure" is TREATS *edema associated
with CHF*, not TREATS *heart failure*.

**R4 — Contraindications are decided on clinical meaning, never on
phrasing.** Bare list items under CONTRAINDICATIONS and "should not be used
in ..." are valid contraindications. The gold is never shaped around whether
CogniGraph's trigger happens to fire; doing so would inflate measured recall
and void the reference.

**R5 — Combination contraindications bind the combination.** "Do not
co-administer aliskiren with olmesartan in patients with diabetes" does NOT
make diabetes a contraindication for olmesartan alone. (Drug-drug-in-
population contraindications belong to the deferred interactions sub-pass.)

**R6 — Restricted/refractory-only indications do not support a plain
TREATS.** Dexamethasone's "severe or incapacitating allergic conditions
intractable to conventional treatment in asthma" does not license
TREATS(asthma); the label's qualified refractory concept is recorded instead.

**R7 — Boilerplate is not an indication.** Antibiotic-stewardship language
("should be used only to treat or prevent infections proven or strongly
suspected...") is not licensing evidence; the reviewer must instead check
the indications section for properly-worded facts.

**Calibration outcome.** The initial generic-vocabulary instrument surfaced
only 3 acceptable candidates out of 24 and omitted roughly 20 required
concepts; that finding triggered the instrument rebuild. The accepted showcase
artifact now freezes 19 TRUE and 8 FALSE instance-level cases. Corpus
vocabulary covers 17/19 TRUE cases; `clinical-matcher-v1` asserts 13/19 when all
concepts are offered and 11/19 end to end, while refusing 8/8 FALSE cases. This
is engineering calibration, not held-out recall or external clinical
validation. Some original generic facts remain false positives at the accepted
granularity.
