# CG-25 CUAD evidence recovery — 2026-09-09

**Later user decision (2026-09-09):** CUAD is retired from active evaluation.
CG-25 is closed without change to its missing execution evidence; the status
counts below describe this report's earlier checkpoint. See the
[Luna baseline decision](../decisions/decision_luna_baseline.md) and current
[registry](README.md).

[CG-25](CG-25.md) is **partially remediated and remains open**. Its original
scoring package was found outside the moved checkout and recovered into
[fixtures](../research/experiments/cuad-2026-07-22/README.md). The
headline reproduces arithmetically, but the scorer mixes counting units. The
paper, HTML/PDF exports, and pilot one-pager now qualify the experiment and
withdraw the old recall/F1 as quality claims. Missing original execution
artifacts still prevent satisfying the issue's full acceptance criteria.

CG-30 was committed locally as `e7ea755`; this CG-25 recovery was subsequently
committed as `3fa7db7`. Nothing was pushed, published, or sent to a model provider.

## Recovered evidence and grain

The original directory survives at
`/Users/skitsanos/FTP/Projects/rust/cognigraph-evals/cuad`. Ten files were
copied byte for byte: the preparation, scoring, and execution scripts; original
status report; split IDs; gold labels; design/holdout chunks; and accepted
evidence projections. Their original bytes are hashed in the package's
[provenance manifest](../evidence/research-cuad-2026-07-22.md#artifact-91829b71e18bcb6a6c03).
The source files were not modified. The full corpus archive stays outside Git;
the package contains a pinned, licensed fetch/preparation recipe.

The experiment used a custom stratified sample of 10 design and 40 holdout
contracts, selected from CUAD v1 using gold category coverage. It covers eight
categories, rather than the official 41-category benchmark protocol. The
holdout contains 1,705 chunks, 330 gold annotation entries, and 170 positive
contract/category pairs. Its 164 retained predictions span 36 contracts; four
have no retained predictions. Each prediction has only `evidence` and
`suspect`. No exact duplicate gold entry occurs within a contract/category.

## Findings and corrected interpretation

| Finding | Evidence and impact | Assessment |
|---|---|---|
| Mixed counting units | Legacy TP counts matched gold entries; FN counts at most one wholly missed contract/category. Partially matched categories lose their remaining FN. The 192-unit recall denominator has no consistent grain. | P2, high confidence; corrected reporting and a separate consistent-count replay are included. |
| Incomplete extraction provenance | The runner projected away endpoints, offsets, chunk/fact keys, and attribution. Raw completion responses/nominations and resolved provider/model/settings are absent from the recovered directory; its old runtime configuration is gone. | P2, high confidence for the recovered scope; full historical execution remains unreconstructable from these files. |
| Holdout restart omitted from headline | Original tool outputs show the initial holdout failed with HTTP 400, then the gate was fixed in `e0ff466` and the holdout restarted. | P2, high confidence; “single pass” is now qualified as one completed scored run after failure, with prior holdout exposure. |
| Different design revision retained | The design projections reproduce the third calibration round (legacy F1 0.579); holdout used the restored round-1 taxonomy. | Provenance limitation; those design outputs must not be presented as a same-taxonomy control. |

The original matcher greedily assigns each prediction to the first unused gold
entry sharing a normalized containment or 12-word run. The corrected diagnostic
preserves those exact assignments and order, changing only FN to count all
unmatched gold entries. It does not add semantic adjudication, use an official
CUAD metric, or establish legal truth.

| Holdout lane / counting policy | TP | FP | FN | P | R | F1 |
|---|---:|---:|---:|---:|---:|---:|
| All predictions, legacy mixed units | 122 | 42 | 70 | 0.744 | 0.635 | 0.685 |
| All predictions, consistent gold entries | 122 | 42 | 208 | 0.744 | 0.370 | 0.494 |
| Semantics-filtered, legacy mixed units | 72 | 25 | 113 | 0.742 | 0.389 | 0.511 |
| Semantics-filtered, consistent gold entries | 72 | 25 | 258 | 0.742 | 0.218 | 0.337 |

The weak-category precision findings remain in the original evidence. The
semantics filter still leaves precision roughly flat while reducing matches,
but the literal “recall halved” claim is replaced by count-level observations.
Comparing these outputs with free drafting on another task does not establish
that governance, rather than model capability, caused the difference. That
causal language was removed from the affected paper passages.

## Verification

- A fresh download of the versioned CUAD archive and the recovered local
  archive independently passed the pinned size/SHA-256 check. Both regenerated
  all four prepared files byte for byte from the CSV and 510 text members.
- Offline replay reproduced the historical per-category and aggregate counts,
  then produced consistent counts and a full assignment ledger for both splits
  and both semantics-filter lanes. The [notebook](../evidence/research-cuad-2026-07-22.md#artifact-d63e0fc54a456a2b2a91)
  executed all four code cells, including count assertions.
- Seven scoring/input regression tests passed, including partial-category
  misses, duplicate predictions, empty predictions, matcher boundaries, and six
  malformed-input cases. Archive verification rejects a corrupted archive
  before preparation.
- The paper rebuilt successfully. Real HTTP/browser checks passed at 1440 and
  390 pixels with no paper overflow; the pilot one-pager also served and rendered.
  PDF text and visual checks confirmed the corrected metrics and disclosures.
  The paper is 23 pages and the pilot one-pager is one A4 page.
- Navigation, registry, original-data and six user-owned-path preservation
  checks passed. Rust sources, Cargo manifests/lockfile, and the React UI are
  unchanged. No Rust gates were rerun for this documentation/evidence-tooling
  change; the prior CG-36 runtime/gate evidence remains dated to that work.

The initial PDF inspection found a split footer, corrected with compact print
spacing and keeping the footer together. The one-pager's existing mobile media
query also applied to print and produced two pages; limiting that query to
screen media and tightening print-only spacing restored its one-page print
layout. Initial tooling setup needed
the bundled Pillow runtime and a browser-directory filter. A first browser
check blocked the one-pager's existing Google Fonts dependency; the final check
loaded its declared font assets. The web reader received a Zenodo 429, while
the actual pinned archive download succeeded. These were corrected verification
attempts, not failed final data replays.

See [validation evidence](../evidence/engineering-historical-checks.md#artifact-d03477ea2db63f872c5a) for
hashes, commands, counts, and preservation checks. The package README provides
commands usable from a clean checkout. The full extraction runner is retained
as historical source and was not run against a server.

## Remaining acceptance boundary

CG-25 stays open for the original resolved model/provider/settings, complete
completion attempts and raw nominations, and full accepted fact records. No
current default or newly generated output has been substituted for missing
history. Recovery of those artifacts would permit a fuller audit. If they no
longer exist, historical provenance needs an explicit closure decision; a new
experiment must use a new reserved split and preserve its complete trace.

The registry remains **36 Resolved / 2 Open**. CG-26 modularity can proceed
independently. Broader DeepSeek/GLM/other-model benchmarking remains deferred
under the existing decision, with Luna the economical baseline.
