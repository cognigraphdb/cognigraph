# CG-27 WebNLG status and artifact reconciliation — 2026-09-09

[CG-27](CG-27.md) is resolved. Current guidance now records the completed
July 17–20 experiment sequence and links its retained artifacts correctly.
CG-24 was committed locally as `b90a28a`, including the corrected paper and
its existing publication assets. CG-27 was committed locally as `44e8890`;
nothing was pushed or published, and no model benchmark was run.

## Documentation changes

The [pilot guide](../research/webnlg/pilot.md),
[positioning dossier](../../../docs/research/semantic-neurons/positioning.md),
[roadmap](../plans/archive/roadmap-2026-h2.md), and
[implementation plan](../implementation-plan.md) distinguish:

- July 17: supervised deterministic mining, validation-informed tuning, and the
  frozen one-shot test result, 7.0% recall / 68.8% precision.
- July 20: fuzzy matching tried and rejected; expanded-corpus mining without a
  recall improvement; a live `gpt-5.4-mini` proposal snapshot; automatic
  disambiguation; and a separate pruned candidate snapshot.
- Remaining work: entity discovery, hard-negative restraint, full runtime
  governed-review evaluation, and generalization on a newly reserved set.

The [decision ledger](../decisions/decision_webnlg_scoring.md) keeps its original
chronology, tables, unsuccessful outcomes, and dated interpretations. A new
current-reading note explains that “future work” statements describe earlier
checkpoints, and qualifies the old “net-positive/full loop” interpretation.
Only its two broken artifact references were replaced within the historical
body. Current links resolve to the three retained JSON files under
`crates/cognigraph-construct/fixtures/webnlg/`.

The LLM experiment selected target predicates from validation-oracle frequencies
before prompting with train examples. Its scores are therefore development-set
measurements, not a fresh untouched holdout estimate. Every lane assumes
oracle-provided entity surfaces, and the diagnostic lane additionally receives
predicate labels. Exact-triple precision measures agreement with WebNLG's
supplied generating triples; it is not independent truth adjudication or a
hard-negative test.

The ledger's “human prune” label records offline candidate editing. Neither
candidate JSON contains reviewer identities or signed attestations. The CLI's
`disambiguated` result automatically removes template collisions; it is not a
server review action. The pruned artifact adds eight correct triples while
precision falls from 76.6% to 76.3%, a small tradeoff rather than strict
improvement on both metrics. No runtime `Neuron` lifecycle or production judge
qualification is established by these measurements.

## Artifact inventory and frozen-data verification

| Retained artifact | Verified content | SHA-256 |
|---|---|---|
| [neuron-ruleset.mined.json](../../crates/cognigraph-construct/fixtures/webnlg/neuron-ruleset.mined.json) | `neuron-authored-mined-v3`; 303 predicates, 1,842 templates | `32b89c813853ca3b17dd8d8ae66f4dea8ce0baaa544cacd248422d8c262db1c5` |
| [llm-proposals.json](../../crates/cognigraph-construct/fixtures/webnlg/llm-proposals.json) | 25 predicates, 248 entries; 247 unique within predicates | `d5cf0963e61232fea0f7eb2e1035f137d59a58087e49c09a0f6706e2187933dc` |
| [llm-proposals-pruned.json](../../crates/cognigraph-construct/fixtures/webnlg/llm-proposals-pruned.json) | Same 25 predicates, 202 entries; 46 entries removed, no additions | `7f8839f035eca45cfdb682720c96908868211c221c6b4c253ece88190c858d68` |

The current release `webnlg-pilot --verify-only` verified the retained frozen
revision, 38,872 documents, 115,279 triples, 411 predicates, and 19 categories.
It reparses cached inputs and verifies the source/document/oracle/manifests;
this includes integrity checks on retained test files, not scoring them.
All **401 prepared files** and all three candidate/ruleset files matched their
pre-run hashes afterward.

`webnlg-mine-rules` then regenerated the 303-predicate / 1,842-template artifact
into a temporary file. Its bytes exactly matched the retained frozen ruleset.
It consumed only train+validation, with no test files in its input root.

## Current release replay

The [offline harness](../evidence/engineering-historical-checks.md#artifact-e3ef687898a673fad37a) built on the existing
release binaries executes five commands: cached-corpus verification, temporary
mining, the validation scorer, and two explicit `webnlg-llm-run --load` runs.
Every command exited successfully. Scoring/mining use a temporary root with
only the four train/validation document/oracle files; test files are absent.
The commands run outside the repository with a minimal environment and no
provider credentials. `--load` bypasses provider construction and proposal
writes. The corpus-verification branch returns before any network setup.

All scoring rows below use **1,667 validation documents and 4,841 oracle
triples**. They reproduce retained-candidate behavior on the development set;
they do not constitute another model experiment.

| Scoring variant | Correct / constructed | Recall | Precision |
|---|---:|---:|---:|
| Generic | 51 / 88 | 1.1% | 58.0% |
| Exact train-mined baseline | 601 / 785 | 12.4% | 76.6% |
| Exact CorpusProposer | 591 / 756 | 12.2% | 78.2% |
| Oracle-diagnostic, CorpusProposer | 591 / 645 | 12.2% | 91.6% |
| Fuzzy gap 0 | 703 / 1,720 | 14.5% | 40.9% |
| Fuzzy gap 1 | 881 / 3,054 | 18.2% | 28.8% |
| Fuzzy gap 2 | 936 / 6,093 | 19.3% | 15.4% |
| Fuzzy gap 3 | 975 / 7,431 | 20.1% | 13.1% |
| Raw saved proposals merged with baseline | 630 / 1,185 | 13.0% | 53.2% |
| Raw saved proposals, merged and disambiguated | 612 / 854 | 12.6% | 71.7% |
| Pruned saved proposals merged with baseline | 609 / 800 | 12.6% | 76.1% |
| Pruned saved proposals, merged and disambiguated | 609 / 798 | 12.6% | 76.3% |

The eight scorer rows plus three rows from each proposal run give **14 lane
observations**, including two additional repeated baseline checks. The current
CLI's diagnostic row uses the CorpusProposer ruleset; it is distinct from the
historical July 17 baseline diagnostic row (12.4% / 90.1%). The unbounded fuzzy
row (28.5% / 8.3%) remains historical evidence: the current CLI sweeps only gaps
0–3. No `webnlg-test` command was executed and no rule was tuned during replay.

The [replay artifact](../evidence/engineering-historical-checks.md#artifact-88580d96a54c6a89cc22) records all
command outputs, binary/harness hashes, file digests, and input boundaries.
The [validation artifact](../evidence/engineering-historical-checks.md#artifact-299831a7964800966d4f)
checks exact counts, reported rates, links, registry state, and preservation.

## Reproduction and checks

With `data/webnlg-pilot/` already present:

```bash
cargo build --release -p cognigraph-construct --bin webnlg-pilot \
  --bin webnlg-mine-rules --bin webnlg-score --bin webnlg-llm-run
python3 docs/issues/evidence/webnlg-status-replay.py --output /tmp/cg27-replay.json
```

This procedure neither downloads fresh data nor makes model calls. The guide
also documents the difference between `--load` replay and the separate live
proposal mode, whose default output can overwrite the raw candidate snapshot.

The release build, corpus verification, artifact regeneration, and all scorer
and proposal replays passed on their first attempts. All relevant local links
resolve. Original decision tables and frozen artifacts are preserved; no Rust
source or dependency changed. Accordingly, full Rust gates were not rerun for
this documentation correction. The latest full gates are CG-36's passing
formatting, strict Clippy, and 961 reported tests, including eight Arango early
returns and two live embedding tests at that checkpoint.

All six pre-existing user-owned paths are unchanged, including the shared
research-index edits and existing `CLAUDE.md` deletion. The CG-24 paper and its
publication bundle are unchanged; their frozen test result and statement that
runtime review evaluation remains future work still hold. No PDF rebuild or
publication was required.

The registry has **35 Resolved and 3 Open** issues: CG-25, CG-26, and CG-30.
Next bounded task is CG-30, the decision-log index. Model benchmarking remains
deferred until the original ticket list is closed, with Luna as the economical
baseline.
