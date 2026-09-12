# 01 — Data preparation

Goal: source material on disk → clean collections in CogniGraph, with
embeddings, verified. Everything here is copy-pasteable against a local
server.

## 1. Model your collections

Collections are created on first write; there is no schema step. A useful
baseline layout:

| Collection | Holds | Notes |
|---|---|---|
| `docs` | source documents (or metadata about them) | one per document |
| `chunks_raw` | pre-chunked text ready for embedding | one per chunk, carries `text` |
| `embeddings` | *optional* pointer-style chunks (see below) | only if you use the chunk→source convention |

Keys: pass `_key` explicitly when you want stable, meaningful ids
(`"acme-q3-report"`); omit it for a generated UUID. Explicit keys make
everything downstream — CGQL, provenance, snapshots — easier to read.

Two conventions for making documents searchable:

- **Self-contained** (simplest): the document carries its own `embedding`
  field. Search hits return the document itself.
- **Chunk→source pointers**: an `embeddings` collection holds small chunk
  docs, each with an `embedding` and a `document_id` field like
  `"docs/acme-q3-report"`. Search hits resolve the pointer and return the
  *source* document. Use this when documents are too large to embed whole.

Both work with every search route; you can mix them.

## 2. Chunk your source material

Grounding and embedding both operate on chunks. Rules of thumb:

- Paragraph-sized chunks (200–1200 characters) work well; grounding
  evidence and vector similarity both degrade on multi-page chunks.
- Keep chunk ids stable across re-runs (`"report-p3-c2"`), because fact
  edges record `evidence_chunk_id` — stable ids keep provenance meaningful.
- Never split mid-sentence: negation-aware grounding reads clause context.
- **Overlap is the chunking-side answer to evidence near boundaries** —
  a sentence or two of overlap between adjacent chunks keeps a trigger
  and its subject together without any grounding-semantics cost.
  Grounding itself stays deliberately chunk-local (measured: chunk
  locality cost ZERO recall across the five reference corpora even at
  single-sentence chunking — decision_cross_chunk_grounding.md), so
  boundary hygiene belongs here, in the chunker.

Tooling options, in order of least effort:

- **cognigraph-chunker** (companion REST service) — POST text, get chunks.
- **The repo's paragraph packer** — deterministic, offline:

  ```sh
  cargo run --release -p cognigraph-construct --example chunk_text -- input.md > chunks.jsonl
  ```

- **Your own script** — anything that emits JSONL, one object per line:

  ```json
  {"_key": "report-p3-c2", "text": "In Q3, Acme migrated its data platform to ..."}
  ```

### Governed M23 preparation is a separate evaluation path

The advice above is for ordinary operational ingestion, embedding, and
retrieval. M23 does not silently change that pipeline or turn `/api/documents`
into a document converter. It defines a fresh, reproducible evaluation-input
contract for exact UTF-8 plain-text bytes. In particular, M23 deliberately uses
**no overlap** and may fall back to a whitespace or UTF-8 boundary when an
overlong paragraph has no fitting sentence boundary. Do not mix those pinned
semantics with a retrieval chunker whose overlap and sizing are product choices.
The normative details are in the
[M23 decision](../decisions/decision_m23_reproducible_raw_document_prepared_corpus_processing.md),
the [checked plan](../examples/m23-preparation-plan.json), and the
[`fixtures/m23/` authoring guide](../../fixtures/m23).

For an M23 evaluation, extraction happens first and outside CogniGraph. Convert
PDF, HTML, office, archive, image, or scanned material to strict UTF-8 text with
your governed extraction/OCR process; CogniGraph neither performs nor attests
that conversion. Then build canonical `documents.json` schema version 1 with:

- the target `space_type`, `corpus_revision_id`, and exact preparation-plan
  digest;
- document rows sorted by unique non-blank NFC/control-free `id`;
- per row, exactly that `id` and an NFC/control-free `title`, each at most 1,024
  UTF-8 bytes, media type
  `text/plain; charset=utf-8`, exact byte length, SHA-256 digest, and canonical
  unpadded base64url of the exact text bytes.

Use the checked [M23 plan](../examples/m23-preparation-plan.json) to reproduce
canonical `corpus.json`. The pinned preparer freezes Unicode 17.0.0 for NFC and
whitespace classification, strips exactly one leading UTF-8 BOM when present,
normalizes CRLF/bare CR and NFC, rejects unsupported controls, and applies the
per-document normalized-byte cap at this post-newline/NFC stage before
whitespace collapse. It trims Unicode whitespace from line edges, collapses
interior runs to one ASCII space, joins adjacent non-blank lines, uses blank
lines as paragraph boundaries, rejects a document with no non-blank paragraph,
and applies the aggregate normalized-byte cap after full collapse. It packs
non-overlapping chunks to 8 KiB, splitting an overlong paragraph after the
latest fitting `.`, `!`, or `?` only when followed by whitespace or paragraph
end, then at whitespace, then the largest fitting UTF-8 boundary.
Chunk ids have the exact form
`d-<full lowercase sha256 of NFC document-id UTF-8>-c<zero-based eight-digit ordinal>`.

Attest `corpus.json` and `documents.json` together, in that sorted logical-path
order, as the two non-executable `application/json` entries of
`cognigraph.reproducible-prepared-chunk-corpus.v1`, then stage both content
addresses in the tenant-incarnation local CAS. A context-v6/job-v4 evaluation
reconstructs and compares the prepared corpus before running M22 derivation;
clients do not submit the resulting receipt. The server caps the package at
100,000 documents, 4 MiB per raw document/64 MiB total raw bytes, 8 MiB per
post-newline/NFC document before whitespace collapse, 64 MiB total
prepared-normalized bytes after full collapse, 100,000 chunks, 8 KiB per chunk,
48 MiB total prepared text, 96 MiB for `documents.json`, and 64 MiB for
`corpus.json`.

This path produces a verified evaluation input only. It does not insert
`docs`, `chunks_raw`, entities, mentions, embeddings, indexes, or a complete
operational graph. Preparation receipts and Native snapshots retain addresses,
not another copy of the external bytes, so preserve and back up the CAS if the
evaluation must be replayable.

## 3. Load documents

One at a time:

```sh
curl -s -X POST $COGNIGRAPH_URL/api/documents \
  -H "content-type: application/json" \
  -d '{"collection": "docs", "_key": "acme-q3-report",
       "title": "Acme Q3 Report", "year": 2026, "status": "final"}'
```

Many at once, **atomically** (one storage transaction — on a persistent
backend this is the fast path, ~50× the single-write rate):

```sh
curl -s -X POST $COGNIGRAPH_URL/api/batch -H "content-type: application/json" -d '{
  "ops": [
    {"op": "insert", "collection": "docs", "doc": {"_key": "a1", "title": "..."}},
    {"op": "insert", "collection": "docs", "doc": {"_key": "a2", "title": "..."}},
    {"op": "update", "collection": "docs", "key": "a1", "merge": {"status": "final"}}
  ]
}'
```

A key conflict anywhere in the batch stores **nothing** — there are no
partial ingest states to clean up.

## 4. Embed server-side (the batch pipeline)

`POST /api/documents/embed` takes pre-chunked items, embeds each item's text
in provider batches (64 texts per API call, four calls in flight), and
stores everything in one atomic transaction:

```sh
curl -s -X POST $COGNIGRAPH_URL/api/documents/embed -H "content-type: application/json" -d '{
  "collection": "chunks_raw",
  "items": [
    {"_key": "report-p3-c2", "text": "In Q3, Acme migrated its data platform to ..."},
    {"_key": "report-p4-c1", "text": "The migration reduced reporting latency by ..."}
  ],
  "text_field": "text",
  "embedding_field": "embedding",
  "batch_size": 64
}'
# → {"stored": 2, "keys": ["report-p3-c2","report-p4-c1"], "model": "...", "batches": 1}
```

Or from a JSONL file via the CLI:

```sh
cognigraph embed chunks_raw chunks.jsonl --batch-size 64
```

Semantics worth relying on:

- **All-or-nothing.** An item without non-empty text fails the whole
  request *before* the first provider call (no spend on a doomed batch);
  a key conflict stores nothing.
- `text_field` defaults to `text`, `embedding_field` to `embedding` —
  which is the field every vector search reads.
- Chunking stays your job; the pipeline will not split text for you.

If you compute embeddings yourself, just store them as a JSON array in
the `embedding` field — the pipeline is a convenience, not a requirement.

## 5. Relationships (plain graph edges)

Independent of Semantic Neurons, you can relate any two documents:

```sh
curl -s -X POST $COGNIGRAPH_URL/api/graph/relationships -H "content-type: application/json" -d '{
  "collection": "document_relations",
  "from": "docs/a1", "to": "docs/a2",
  "relation_type": "SUPERSEDES",
  "confidence": 0.9,
  "metadata": {"source": "manual"}
}'
```

and traverse them (`POST /api/graph/traverse`) or query them in CGQL (guide
02). Fact edges built by grounding (guide 04) live in their own `facts`
collection with a stricter contract.

## 6. Verify what you loaded (CGQL)

`POST /api/search/query` runs read-only CGQL:

```sh
curl -s -X POST $COGNIGRAPH_URL/api/search/query -H "content-type: application/json" \
  -d '{"query": "FOR d IN docs COLLECT status = d.status WITH COUNT INTO n RETURN {status: status, n: n}"}'
```

Spot-check the embedding pipeline actually wrote vectors:

```cgql
FOR c IN chunks_raw
  FILTER c.embedding == null
  RETURN c._key
```

(empty result = every chunk has a vector), and eyeball dimensions:

```cgql
FOR c IN chunks_raw
  LIMIT 3
  RETURN { key: c._key, dim: LENGTH(c.embedding) }
```

## 7. Snapshots

Before bulk operations, take a hot backup; after, take another:

```sh
cognigraph export --out pre-load.json      # GET  /api/admin/export
cognigraph import pre-load.json            # POST /api/admin/import (restore)
```

Snapshots are the migration path between in-memory and persistent modes,
and between machines for backend state. M21-M23 external artifact bytes are not
included. M24 derives a bounded evidence recovery plan and creates/verifies an
offline tenant-incarnation CAS bundle; restore that bundle separately from the
Native database snapshot. Database records, CAS bytes and external
configuration/trust/secrets must form one recovery plan.

Next: [02 — CGQL for DataOps](02-cgql-for-dataops.md).
