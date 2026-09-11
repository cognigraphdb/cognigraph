# Luna-low document development trial

This package advances the selected Luna-low baseline from supplied entity pairs
to whole, multi-sentence general factual documents. It freezes **40 development
documents and 80 holdout documents** across twelve relations before inference.
The model receives document text and a fixed taxonomy, with no supplied entity
list, entity pair, gold label, or gold evidence. The holdout is prepared but not
run; the included inference runner permits development only.

The original scorer and its failed attribution assumption remain frozen; use
[Scorer V2](score_v2.py) and its [recorded amendment](scoring-amendment-v2.json).

See [results](results.md), [data-quality audit](data-quality.json), the
[executed quality notebook](data-quality.ipynb), and [protocol](protocol.json).

## Source and annotation provenance

The source is the authors' [Re-DocRED release](https://github.com/tonytan48/Re-DocRED)
at revision `ccfb54f5ddf5836027c87badda10f6dfc56efaac`. Both complete source
splits, the README and MIT license are preserved byte for byte under `source/`,
with URLs and SHA-256 values in [source-manifest.json](source-manifest.json).
The original DocRED documents derive from Wikipedia and Wikidata.

[Tan et al. (EMNLP 2022)](https://aclanthology.org/2022.emnlp-main.580/)
describe machine-generated candidate triples checked by two human annotators,
with a third resolving disagreements. These labels were created independently
of CogniGraph and this trial. We introduce no model-authored gold labels.

“Whole document” means every sentence in the released annotated benchmark
document. These are short encyclopedia documents, **not complete Wikipedia
articles, long reports, or a sample of customer traffic**. Development documents
range from 131 to 436 tokens under the source's space-separated tokenization.

Re-DocRED is not guaranteed exhaustive. The later
[Re²-DocRED study](https://aclanthology.org/2026.eacl-long.213/)
identifies remaining omissions and adds both human-verified proposals and
relations inferred through rules. We retain the earlier human-reviewed source
for a transparent initial reference; rule-expanded labels would introduce a
different inference policy. Neither source can establish that every absent
annotation is a true negative.

## Frozen design

- Selection: the first 40 upstream development and 80 upstream test documents
  under a seeded SHA-256 title ordering. No selection by extraction results,
  relation presence or convenient evidence. The seed and exact rules are in
  [settings.json](settings.json).
- Scope: country, administrative location, headquarters, birthplace, death
  place, father, mother, spouse, child, founder, owner and publisher. Every
  published label in that scope is retained. Spouse is symmetric; all other
  relation directions remain distinct.
- Text: join the original tokens with spaces, retain every source sentence in
  order with newline boundaries, and normalize NFC. One whole document becomes
  one chunk and one completion request. No sentence selection or context crop.
- Inputs and gold occupy separate directories. Annotation aliases are derived
  from the source token spans and used only by the offline scorer. Original
  annotation-name discrepancies remain recorded rather than silently repaired.
- Model: shared runtime `gpt-5.6-luna`, reasoning `low`, original production
  prompt and strict JSON schema. A recorder adds only an 8,192-output-token cap
  for bounded measurement. It does not add entity hints or rewrite the prompt,
  format, effort, or schema.
- Execution: one development repetition, 40 calls, no retries or replacement
  documents, at most $1 reserved using uncached/cache-write-aware token bounds.
  Each document uses a fresh disposable Native database. Embeddings are local
  deterministic controls. No existing local database is reset.

## What the metrics mean

The primary counts use unique document-local `(head entity, relation, tail
entity)` triples. Exact normalized source aliases identify entities; ambiguous
or unknown names are not resolved by guessing the gold relation. Alternative
names for the same entity do not create extra credit, and repeated occurrences
do not enlarge the recall denominator. Scores are reported before and after
the real writer/gates. Failed requests retain their reference facts as misses.

Precision, recall and F1 are explicitly **agreement with the published
annotation reference**. An unmatched prediction is not an independently proven
false assertion. Preserve those predictions for review instead of using their
count to claim actual false-positive or abstention rates. A document without
an in-scope reference relation is likewise not a verified negative document.

Relation-endpoint discovery measures entities participating in these relations;
it is not standalone named-entity recognition. Macro per-relation F1 and each
relation's counts accompany micro scores because geographic relations dominate
the sample. Source labels include document-level inference, while the runtime
asks for explicit quoted evidence; this policy difference remains visible in
the full denominator. Cross-sentence endpoint counts describe the source, not
proof that a relation is unsupported by a contiguous evidence span.

The [80 lexical near-miss review candidates](review/development-negative-candidates.json)
were chosen before model generation. They contain co-sentence entities and
relation vocabulary but no corresponding source annotation. Every verdict is
`unreviewed`; these candidates are not negative gold. An independent reviewer
must inspect the full document and add missing facts, not merely vote on model
outputs, before claiming exhaustive truth or measured restraint.

## Reproduction

No Rust implementation changes are part of this experiment. The protocol pins
525 Rust/dependency/helper files and the previously validated release binary at
commit `b9a97f9`. Python requires only the standard library.

```bash
python3 -B -m unittest discover -s fixtures/semantic-neurons/luna-documents-2026-09-09 -p 'test_*.py' -v
python3 -B fixtures/semantic-neurons/luna-documents-2026-09-09/verify_v2.py --output /tmp/luna-documents-verification.json
```

`prepare.build()` deterministically reconstructs and validates the cohort from
the pinned source; the notebook executes that reconstruction. Existing packages
and result directories must remain unchanged. For a separately authorized fresh
development rerun, use a new output directory:

```bash
python3 -B fixtures/semantic-neurons/luna-documents-2026-09-09/run.py --mode live --output /tmp/luna-documents-new-attempt
python3 -B fixtures/semantic-neurons/luna-documents-2026-09-09/score_v2.py /tmp/luna-documents-new-attempt --output /tmp/luna-documents-new-score.json
```

The runner reuses the existing OpenAI key without logging credentials. It hashes
all frozen package files for integrity but never parses gold during live
generation. Recorded live prompts must equal the prompts from the pre-run
release-server gold controls. The final verifier reconstructs source selection,
checks all pins and captures, validates stored evidence bytes, and replays scores
and token-cost accounting without making provider calls.

Before any holdout execution, freeze the chosen development candidate and
resolve the scope of independent label review. Public benchmark exposure during
model training is unknown. This trial does not qualify automatic judges,
signed semantic repair, production extraction accuracy, or long-document use.
