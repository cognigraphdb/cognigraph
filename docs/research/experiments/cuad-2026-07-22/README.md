# CUAD July 22 evidence recovery

> Public reading copy; original research captures and scripts are private.
> Original path: `fixtures/semantic-neurons/cuad-2026-07-22/README.md`.
> Original SHA-256: `71bf00431467bf6ddda8d51293b27cf0957ebd7ef20d5c0338b1a8d360133318` (9133 bytes).
> Navigation changed; dated results and limitations retain their original scope.


> Data removal, 2026-09-11: Historical aggregate results and source hashes affected by repository data removal are withdrawn. Retained records are a subset, not a rerun or a qualified benchmark. See the repository data-removal record for the current boundary.

**Historical package — retired from active evaluation by user decision on
2026-09-09.** The original execution gaps remain. CG-25 was subsequently closed
without completing that recovery; the earlier status below is preserved as a
checkpoint. New work uses the [Luna baseline](../luna-baseline-2026-09-09/README.md).

This package makes the retained CUAD **scoring** experiment replayable. It was
recovered on September 9, 2026 for [CG-25](../../../issues/CG-25.md) from
the evaluation directory outside the relocated CogniGraph checkout. It does
not reconstruct the complete original model/gate/write execution.

The legacy headline reproduces, but its recall/F1 mix counting units and are
withdrawn as quality claims. Holding the original predictions, order, and
matching rule fixed, consistent gold-entry accounting gives:

| Holdout lane | Predictions | Matched gold entries | Unmatched gold entries | Precision | Recall | F1 |
|---|---:|---:|---:|---:|---:|---:|
| All retained accepted evidence | 164 | 122 | 208 | 0.744 | 0.370 | 0.494 |
| Exclude semantics-flagged evidence | 97 | 72 | 258 | 0.742 | 0.218 | 0.337 |

These are post-hoc evidence-match diagnostics over 330 provided gold entries,
not official CUAD leaderboard scores, expert judgments of each fact's truth,
a model comparison, or a new held-out experiment. The matcher is permissive:
ASCII alphanumeric normalization followed by containment or a shared contiguous
12-word run. It greedily pairs each prediction with the first unused matching
gold entry. It does not adjudicate whether the matched wording asserts the
predicted legal relation. Matching/order are deliberately preserved for this
counting correction; alternative matching algorithms require a new protocol.

## Data, license, and preparation

CUAD v1 was curated by The Atticus Project; cite Dan Hendrycks, Collin Burns,
Anya Chen, and Spencer Ball, *CUAD: An Expert-Annotated NLP Dataset for Legal
Contract Review* (2021). The source contains 510 contracts and 41 categories;
this experiment selected eight categories and a custom 10-design/40-holdout
split. Selection is stratified using gold category coverage, seed `20260722`,
and is **not CUAD's official test split**. The 50 IDs are preserved verbatim in
[sample.json](../../../evidence/research-cuad-2026-07-22.md#artifact-f21c9e5d5225f49a2011). No contract crosses our two splits.

Dataset derivatives here (gold excerpts, chunks, and quoted prediction
evidence) are attributed to CUAD under
[CC BY 4.0](https://creativecommons.org/licenses/by/4.0/), as declared by the
[publisher's dataset card](https://huggingface.co/datasets/theatticusproject/cuad/blob/main/README.md).
Changes from upstream: category/contract selection, parsing of label cells,
paragraph chunking with a nominal 1,800-character size and 200-character
overlap, and model-derived evidence projections. Long paragraphs can exceed
the nominal chunk size. Annotation order and gold text are retained. Dataset
licensing does not imply endorsement of CogniGraph or this scoring policy.

The [versioned dataset record](https://zenodo.org/records/4595826) supplies
`CUAD_v1.zip` (105,883,672 bytes). Pinned SHA-256:
`88b694d99007d39777fa44cd72daf8297773d285dc3eab0091ba32078888d18e`.
The archive is fetched separately rather than duplicated in Git. A fresh
download and the recovered archive both reproduced the four prepared files
byte for byte: sample IDs, gold, 702 design chunks, and 1,705 holdout chunks.
See [source verification](../../../evidence/research-cuad-2026-07-22.md#artifact-2f43727a01ef120f585f) and
[download verification](../../../evidence/research-cuad-2026-07-22.md#artifact-0d7e818f64123ac8e635).

From the repository root:

```bash
# Fetch, verify, and prepare only the CSV/text members in a temporary directory.
python3 fixtures/semantic-neurons/cuad-2026-07-22/verify_source.py --download-to /tmp/CUAD_v1.zip

# Or verify an existing archive without modifying it.
python3 fixtures/semantic-neurons/cuad-2026-07-22/verify_source.py --archive /tmp/CUAD_v1.zip

# Offline score replay; the optional ledger output must be a new file.
python3 fixtures/semantic-neurons/cuad-2026-07-22/replay.py --output /tmp/cuad-replay.json
python3 -B fixtures/semantic-neurons/cuad-2026-07-22/test_replay.py
```

Neither verification script calls a model or a running CogniGraph server.
Preparation was checked using Python 3.14.7; the historical run used Python
3.13.14. Exact output comparison catches any future sampling/parser drift.

## What survives

| Artifact | Meaning |
|---|---|
| [Provenance manifest](../../../evidence/research-cuad-2026-07-22.md#artifact-91829b71e18bcb6a6c03) | Original file sizes/digests, pinned dataset, versions, explicit missing fields. |
| [Historical preparation](../../../evidence/research-cuad-2026-07-22.md#artifact-84db9068f793002198cf) | Original sampling, cell parsing, and chunking code, unchanged. |
| [Historical scorer](../../../evidence/research-cuad-2026-07-22.md#artifact-e7ca1bc120ccad60cb8b) | Original algorithm, unchanged; included to reproduce its mixed-unit numbers. |
| [Historical runner](../../../evidence/research-cuad-2026-07-22.md#artifact-83cf8f5261ba2e52dafd) | Original request slicing, taxonomy, extraction projection, and retry behavior. An archival source file, not a recommended current benchmark command. |
| [Frozen taxonomy](../../../evidence/research-cuad-2026-07-22.md#artifact-79a46134ad385a37447e) | Exact round-1 taxonomy restored after three design rounds; extracted from the retained runner. |
| [Holdout projections](../../../evidence/research-cuad-2026-07-22.md#artifact-2774675dad632c9fc4da) | 164 accepted-evidence projections, each carrying only `evidence` and `suspect`, over 36 contracts; four contracts have no retained predictions. |
| [Design projections](../../../evidence/research-cuad-2026-07-22.md#artifact-7eafda4cff964c82b431) | Final round-3 calibration output, **not** an output of the restored round-1 taxonomy. |
| [Original status report](../../../evidence/research-cuad-2026-07-22.md#artifact-3690c5e7c80761b5864b) | Unchanged historical claims; its “once,” “quotable,” and recall interpretations are qualified by this README. |
| [Session excerpts](../../../evidence/research-cuad-2026-07-22.md#artifact-11dd327dccfd9a344b15) | Bounded original tool outputs corroborating failure/restart and the final counts; no credentials or private session context. |
| [Replay implementation](../../../evidence/research-cuad-2026-07-22.md#artifact-48166f648d181188fded), [results](../../../evidence/research-cuad-2026-07-22.md#artifact-174ac575955670f4d176), [analysis notebook](../../../evidence/research-cuad-2026-07-22.md#artifact-d63e0fc54a456a2b2a91) | Inspectable legacy/consistent counts and per-contract/category prediction-to-gold assignments. |

The original scorer counted a true positive for **each matched gold entry**,
but at most one false negative for an entirely missed **contract/category**.
Partially matched categories contributed no false negatives for remaining
entries. Thus its `122 / (122 + 70) = 0.635` recall denominator was neither
the 330 gold entries nor the 170 positive contract/categories. The correction
counts every unmatched gold entry: `122 / 330 = 0.370`. Precision is unchanged
because prediction matching is unchanged. There are no exact duplicate gold
entries within a contract/category in either split.

The historical semantics-filter comparison also used the mixed denominator.
The consistent comparison still supports the narrow negative observation:
precision stays roughly flat while matched entries fall from 122 to 72. It
does not support the old literal claim that recall was halved.

## What cannot be reconstructed

The full completion requests/responses, raw rejected nominations, full accepted
fact rows, and resolved server provider/model/settings are absent from the
recovered evaluation directory. The projection discarded endpoints, chunk/fact
keys, offsets, and `reviewed_by`; the original runner's code shows this loss.
The old runtime configuration file no longer exists. Current defaults cannot
be substituted as evidence of the original model. The gpt-5.4-mini draft job
belongs to a separate design experiment and does not identify the directed
holdout model. These fields remain explicitly unknown in the manifest.

The taxonomy was restored before holdout execution. The initial holdout
attempt failed on symbol-only redacted endpoints; the gate was fixed in
`e0ff466`, then the holdout was restarted and scored. The record therefore
supports **one completed scored run after a failure**, not one untouched
execution. The runtime change followed holdout exposure. Raw requests from the
failed/retried attempts are unavailable, so their exact counts and outputs
cannot be established from the final projections.

The inherited status report's 97% auto-draft precision and causal comparison
against free-form drafting concern different tasks and supervision. Recovering
the directed score does not establish that governance rather than model
capability caused the difference.

CG-25 stays open for the missing original execution artifacts. Its reporting
overclaim is mitigated in the paper and exports. A future experiment needs a
new reserved split and a manifest frozen before execution, including exact
model/settings, all completion attempts, nominations, gate outcomes, full
accepted records, and an explicit scoring grain. It must be labelled a new
experiment; it cannot backfill the July run's missing history. Broader model
benchmarking remains deferred, with Luna the economical baseline.
