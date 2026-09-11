# Decision: batch embedding pipeline shape (H8)

**Status:** Decided and landed 2026-07-06.

## Context

Documents could only get embeddings client-side (compute the vector,
POST it with the doc) — every ingest pipeline reimplemented the same
embed-then-store dance, one document at a time. The `EmbeddingProvider`
trait was already batch-native (`embed(&[&str])`, one API request per
array); what was missing was the pipeline around it.

## Decision (owner: agent, within the approved H8 scope)

1. **One route, `POST /api/documents/embed`:** items in, embedded documents
   stored, keys out. Per item the server reads `text_field` (default
   `text`), writes the vector to `embedding_field` (default
   `embedding` — what vector search reads), and stores the document
   otherwise verbatim.
2. **All-or-nothing.** Validation (every item an object with non-empty
   text) fails the whole request BEFORE the first provider call; any
   failed provider batch fails the request before any write; storage is
   one `execute_batch` transaction (the H4-measured fast path — one
   commit for the whole batch), so a key conflict stores nothing.
   Partial ingest states don't exist.
3. **Provider batches with bounded concurrency.** Texts go to the
   provider in `batch_size` chunks (default 64, capped 2048), at most 4
   requests in flight; order restored by batch index. Concurrency is a
   constant, not a knob — it bounds pressure on the provider API, and
   nobody has demonstrated a workload where 4 is wrong.
4. **Chunking stays upstream.** The route takes pre-chunked items;
   cognigraph-chunker (and construct's chunk_text) own text splitting.
   A server that chunks would need language/tokenizer policy the
   platform deliberately doesn't carry.
5. **No query-cache writes.** Document texts don't repeat the way
   queries do; stuffing thousands of one-shot texts into the embedding
   cache would evict the query entries it exists for.
6. **Fixed in passing — search hits without `document_id`:** all three
   embedding-search routes assumed the chunk→source convention (an
   `embeddings` collection whose entries point at source docs via
   `document_id`) and returned `document: null` — or silently dropped
   the hit from RRF fusion — for self-contained documents, which is
   exactly what this pipeline produces. `result_doc_id` now falls back
   to the hit's own `_id`; the chunk convention is unchanged.

## Outcome

Landed 2026-07-06: route + OpenAPI (drift-test enforced), `cognigraph
embed COLLECTION FILE.jsonl [--text-field F] [--batch-size N]` CLI,
in-module tests (atomicity, validation-before-spend, conflict stores
nothing, order across batches), live verification against OpenAI
(3 items → 2 provider batches → semantic and hybrid both return the
right chunk with full payloads).
