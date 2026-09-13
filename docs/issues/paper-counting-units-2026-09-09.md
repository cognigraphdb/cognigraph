# CG-24 paper counting-unit correction — 2026-09-09

> Data removal, 2026-09-11: Historical aggregate results and source hashes affected by repository data removal are withdrawn. Retained records are a subset, not a rerun or a qualified benchmark. See the repository data-removal record for the current boundary.

[CG-24](CG-24.md) is resolved. Rev2 reports construction facts distinctly within
each kit, labels answer scores as per-question mentions, and carries the same
correction in its generated HTML and searchable PDF. CG-36 was committed
locally as `57d65af`; this correction was subsequently committed as `b90a28a`,
including the paper and its existing publication build assets. The shared
research index was staged only for the CG-24 clarification, preserving its
other edits. Nothing was pushed or published. The evidence below records
verification before that commit.

## Exact units and sources

The [D4 decision](../decisions/decision_grounding_gates.md) and current
[evaluator](../../crates/cognigraph-construct/src/eval.rs) count parsed
`(source, relation, target)` triples once within a kit, even when repeated
across questions. The combined score sums four separate scoring spaces; it
does not assert that all 59 triples are globally unique across those spaces.
The [corrected fixture ledger](../evidence/research-blind.md#artifact-f05cc4248bac68b6e807)
records the distinct baselines and the July 17 stored-proposal recheck.

| Counting scheme, summed over four kits | Expected denominator | Forbidden denominator |
|---|---:|---:|
| Raw fact mentions across 16 questions | 68 | 36 |
| Historical construction: consecutive duplicates removed only | 67 | 36 |
| Current construction: all duplicates removed within each kit | 59 | 32 |

The old evaluator's `Vec::dedup` removed one consecutive expected duplicate.
Consequently, calling 67 the raw mention count is also imprecise. The paper's
historical note now spells out the partial deduplication, while reporting the
corrected distinct-fact scores as its construction result.

| Runtime replay | Distinct expected facts found | Distinct forbidden facts constructed |
|---|---:|---:|
| Four cold authored kits | 59/59 | 0/32 |
| Four kits with grounding pathways stripped | 0/59 | 0/32 |
| Four stripped kits with stored proposals treated as accepted | 54/59 (92%) | 0/32 |

Per-kit repaired numerators are 13, 15, 14, and 12 against denominators 13, 17,
14, and 15. Per-kit forbidden denominators are 8, 8, 6, and 10. The
[machine-readable recount](../evidence/engineering-historical-checks.md#artifact-2a5e42fb3ed383ca2fbd) records
all input hashes, the exact kit identities, the binary/harness hashes, and
all twelve replay outcomes. Reapplying the historical counting scheme to these
results recovers 67/67 cold and 62/67 repaired; neither is a distinct-fact score.

The other affected scores remain historical measurements from the same
fixture ledger, rather than new model results:

| Paper location | Retained score | Unit and source section |
|---|---|---|
| §9.4 trace-to-answer improvement | 32/68 → 53/68 | Expected fact mentions across questions in four kits; ledger “Trace-to-answer improvement” |
| §9.4 additions-only second pass | 77/105 → 91/105; 0/54 forbidden | Expected/forbidden mentions across four kits plus the hostile kit; ledger “Two-pass answering” |
| §8 and §9.4 repaired versus authored graphs | 31/68 versus 32/68; 0/36 forbidden in both | Expected/forbidden question mentions; ledger “Gap-bearing runs” and “Second run” |

The last comparison supports similar aggregate recall, not identical answers
or statistical equivalence. Both paper occurrences and the current positioning
text now say so. The four kits are described as assistant-authored and
author-curated outside the implementation session, not independent external
evaluation. Acceptance of stored repairs is explicitly simulated.

## Changes and preservation

The [rev2 source](../../../docs/research/semantic-neurons/semantic-neurons-paper-rev2.md) changes
only the relevant §8 paragraph, §9.1, §9.4, and a dated version-history entry.
A September 9 counting note preserves the August 4 original revision date.
The abstract, unrelated studies, reviewer-decision denominator 67, and clinical
coverage percentage 67% retain their original text. There are no authored-kit
score tables elsewhere in this paper to update; the numerical narrative and
both exports were checked. This does not substantiate the separate CUAD or
WebNLG claims whose reproduction/reporting issues remain open.

The [positioning dossier](../../../docs/research/semantic-neurons/positioning.md),
[research index](../../../docs/research/semantic-neurons/README.md), fixture ledger addendum, and
[publication runbook](../../../docs/research/semantic-neurons/publish/PUBLISHING.md) use the same
counting distinction. Historical result tables, rev1, the research-direction
note, and the edit pack remain intact.

The existing [builder](../../../docs/research/semantic-neurons/publish/build.py) regenerated
[index.html](../../../docs/research/semantic-neurons/publish/index.html) and
[the PDF](../../../docs/research/semantic-neurons/publish/semantic-neurons-rev2.pdf) from Markdown;
build code, template, diagram, fonts, and BibTeX were not changed. Original
user-owned files were snapshotted outside the checkout before editing. Five
of the 24 pre-existing paths received the required targeted correction or
export regeneration; the other 19 match their original hashes. Paragraph-level
checks preserve the rest of the existing draft and the pre-existing README
and runbook content.

## Verification

- Built the existing release `blind_recheck` example and passed twelve offline
  executions with current Rust construction/evaluation, using disposable
  in-memory stores. No model calls, proposal generation, or fixture-data writes.
- Recounted 181 chunks and 16 questions from the retained inputs. Checked all
  source fixture hashes again after replay.
- Rebuilt the publication bundle successfully with its existing Pandoc and
  Chromium workflow. The resulting PDF has 23 searchable A4 pages.
- Served the local bundle over real HTTP and checked 1440px and 390px browser
  widths: page/PDF HTTP 200, fonts loaded, corrected construction and answer
  text, no horizontal overflow, broken internal anchors, browser errors, or
  external resource requests. See [browser evidence](../evidence/engineering-historical-checks.md#artifact-ea63c2a7bb3ac7895ca4).
- Rendered all 23 PDF pages, visually inspected page overviews and the affected
  pages at readable resolution, and inspected desktop/mobile section captures.
  Text is legible and the correction is present without clipped or overlapping
  content. Text extraction separately checks scores and paragraph preservation.
- Checked local documentation links, registry statuses/counts, whitespace,
  source/export parity, and preservation. See the
  [validation artifact](../evidence/engineering-historical-checks.md#artifact-32e592f8a5a2a903d379).

The first browser-check attempt looked for “per question” while the corrected
text said “each question.” Its assertion was corrected; the final desktop and
mobile checks passed. This was a harness expectation error, not a failed
routing or counting result. Construction replays and publication builds passed.
The preservation validator also needed to ignore paragraph-separator newlines
introduced by appending the version note; the complete text comparison then
confirmed that all other paper paragraphs were preserved.

No Rust source or dependency changed, so full Rust gates were not rerun for
this documentation correction. CG-36's passing formatting, strict Clippy, and
961 reported workspace tests remain the latest full suite; eight Arango entries
early-returned and two live embedding tests ran at that checkpoint.

## Reproduction and remaining work

Public-distribution amendment, 2026-09-11: the remaining market-research inputs
are retained privately. Supply their authorized external directory explicitly;
the public checkout does not include them. The dated results above are not a
claim of reproduction from public files alone. Publication assets now live in
the separate private product-docs checkout.

```bash
cargo build --release -p cognigraph-construct --example blind_recheck
python3 docs/issues/evidence/paper-counting-units.py \
  --fixture-root /path/to/private/blind --output /tmp/cg24-counts.json
```

This verifies counting semantics and the symbolic outcome of retained proposals.
It does not rerun historical answer models, validate corpus independence,
qualify a judge, or provide a public reproduction package. At this checkpoint,
CG-25 and CG-27 remained open and the registry had **34 Resolved and 4 Open**
issues. The next bounded task, CG-27, was subsequently completed in the
[WebNLG reconciliation](webnlg-status-2026-09-09.md). Broader model benchmarking
remains deferred, with Luna as the economical baseline.
