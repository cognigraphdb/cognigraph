# Directed policy v2: development results

CG-39 and CG-40 are resolved on the tested contracts. Replaying the original
proposals removed one invalid endpoint occurrence without losing any published
reference matches. A fresh Luna-low pass produced no invalid chunk citations
and improved reference agreement on the same 40 development documents. This
single pass does not qualify extraction accuracy or establish a causal accuracy
gain from the schema alone.

The [frozen protocol](protocol.json), [paired comparison](comparison.json),
[fixed-output replay](replay-validation.json), and [fresh capture](live/manifest.json)
retain the exact evidence. Protocol SHA-256:
`0cb301aae6fd1413bf1d744d4fec2b00c48c1ab2adb454aed33430d8feb5ced1`.
The baseline checkpoint is `a0e4b24`; the candidate source diff, added Rust tests,
binary and helper hashes are separately pinned. No prior fixture was edited.

## Same cohort, different measurements

All columns use the same 40 complete Re-DocRED development documents and 683
published reference relations. No entity pairs are supplied. Precision, recall
and F1 below measure agreement with those annotations, whose incompleteness,
inferred relations and alias limitations remain unchanged.

| Measurement | Original v1 baseline | Original proposals, v2 gates | Fresh v2 Luna-low pass |
|---|---:|---:|---:|
| New external model calls | 40 historical | 0 | 40 |
| Raw proposals | 55 | 55 | 51 |
| Stored occurrences | 32 | 31 | 35 |
| Stored reference matches | 16 | 16 | 20 |
| Stored unmatched predictions | 16 | 15 | 15 |
| Stored reference precision | 50.00% | 51.61% | 57.14% |
| Stored reference recall | 2.34% | 2.34% | 2.93% |
| Stored reference F1 | 4.48% | 4.48% | 5.57% |
| Raw reference matches | 21 | 21 | 25 |
| Invalid chunk citations | 5 | 5, rejected | 0 |
| Gate skips | 23 | 24 | 16 |
| HTTP/provider failures | 0 | 0 | 0 |
| Estimated token cost | $0.0201583 | $0 | $0.0194147 |

The replay retained the original provider choices verbatim. Its sole removed
occurrence was `African National Congress COUNTRY South Africa`, for which the
quote contained `South African`. All 16 stored reference matches survived.
The replay's five bad chunk IDs remain bad and are still rejected: no citation
repair was introduced.

The fresh pass used the actual release server's default `gpt-5.6-luna`,
`reasoning_effort: low`, closed strict JSON schema and unchanged prompt. All 40
wire requests matched the old requests after adding only the expected chunk-ID
and relation-name enums. The recorder added only the frozen 8192 completion-token
cap, as in the baseline. There were no retries or changes to the cohort, taxonomy,
labels or model. All provider completions ended with `stop`.

There were zero invalid relations and zero invalid chunk citations in the fresh
pass. Of its 16 skipped proposals, five failed the source endpoint gate, three
the target gate, four quote matching and four affirmed-vocabulary matching.
Provider usage reported 38,312 prompt tokens, 9,350 completion tokens and 4,854
reasoning tokens (included in completion usage). Cache-aware accounting has no
unknown-usage calls. Mean directed HTTP latency was 3.253 seconds, median 2.802,
maximum 7.318; baseline mean was 4.126 seconds. These cost and latency differences
are observations from one pass, not performance guarantees.

## Verification and limits

The release server passed [34 OpenAI/Gemini loopback HTTP cases](../../../docs/issues/evidence/directed-contract-fixes-2026-09-09.json).
These include both endpoint positions, Unicode/NFC offsets, later valid mentions,
opaque IDs, exact relation identity, ignored enums, malformed responses preserving
facts and explicit-empty reconciliation. Formatting, strict Clippy, the full Rust
suite and release build passed. The suite reported 967 passing tests; its eight
Arango integration entries returned early without an exported password.

The preserved baseline's ten scorer/data tests and the candidate's nine scorer
tests passed. Capture verification checked all 40 replay and 40 live requests,
all 31 replay and 35 live stored evidence spans, original-proposal equality,
exact score/cost reconstruction and the historical package seal.
[Five tamper probes](../../../docs/issues/evidence/directed-candidate-integrity-2026-09-09.json)
detected altered digests, missing attempts, fabricated stored quotes, changed
document input and an effort override, using disposable copies only.

The original 80-document holdout has had **zero inference calls**. Public dataset
contamination remains possible. Unmatched predictions are unreviewed, not proven
false facts; near-miss candidates remain unlabelled. Very low reference recall
and incomplete source labels prevent production-quality conclusions. Next,
complete independent reference review and settle the explicit-evidence policy on
development material before freezing qualification criteria and using holdout
data. Luna low remains the selected baseline.
