# 03 — Retrieval

Five search routes, one shared idea: the graph and the vectors work
together, and the cache sits in front of the embedding-based ones. All
routes are POST.

## Which route when

| Route | Input | Use when |
|---|---|---|
| `/api/search/vector` | a raw vector | you computed the query embedding yourself |
| `/api/search/text` | text | plain BM25 keyword search; the only text route that needs **no embedding provider** |
| `/api/search/semantic` | text | the standard "find by meaning" call |
| `/api/search/hybrid` | text | terminology matters as much as meaning (BM25 + vector, RRF-fused) |
| `/api/search/graph-augmented` | text | you want neighbors and **graph facts** alongside the hits |

## Vector and semantic

```sh
curl -s -X POST $COGNIGRAPH_URL/api/search/vector -H "content-type: application/json" \
  -d '{"collection": "chunks_raw", "vector": [0.12, -0.03, ...], "limit": 10, "threshold": 0.7}'

curl -s -X POST $COGNIGRAPH_URL/api/search/semantic -H "content-type: application/json" \
  -d '{"collection": "chunks_raw", "query": "data platform migration results", "limit": 10, "threshold": 0.7}'
```

- `threshold` is cosine similarity (default 0.7). If you get zero
  results on data you *know* is there, lower it before suspecting the
  data — short chunks and cross-domain phrasing commonly score 0.5–0.7.
- `collection` defaults to `embeddings` on the text routes — pass yours
  explicitly if you use self-contained documents.

## Plain text (BM25)

```sh
curl -s -X POST $COGNIGRAPH_URL/api/search/text -H "content-type: application/json" \
  -d '{"collection": "chunks_raw", "query": "migration", "fields": ["title", "content"], "limit": 10}'
```

- Word-level BM25 over the string fields you name (default
  `["content", "title"]`); scores are BM25, not cosine — there is no
  `threshold`.
- Needs no embedding provider, so it works on an offline deployment
  where the semantic routes answer 400.
- First query on a collection builds the tantivy index (persisted next
  to the store when the backend is durable); subsequent queries are
  milliseconds. The console's document browser uses this route for its
  search box.

**How hits resolve** (both conventions from guide 01 work): if a hit
carries a `document_id` pointer (`"docs/a1"`), the route fetches and
returns that source document; otherwise the hit *is* the document and is
returned as-is. Mixed collections are fine.

## Hybrid

```sh
curl -s -X POST $COGNIGRAPH_URL/api/search/hybrid -H "content-type: application/json" -d '{
  "collection": "chunks_raw",
  "embeddings_collection": "chunks_raw",
  "query": "GxP validation requirements",
  "limit": 10, "threshold": 0.4
}'
```

BM25 excels exactly where embeddings blur: exact terms, acronyms, product
names, codes. The two result lists fuse by reciprocal rank (tunable
`bm25_weight` / `vector_weight` / `rrf_k`). On backends without full-text
search the BM25 leg degrades gracefully and the response says so.

## Graph-augmented

```sh
curl -s -X POST $COGNIGRAPH_URL/api/search/graph-augmented -H "content-type: application/json" -d '{
  "query": "who supplies the data platform?",
  "collection": "chunks_raw",
  "edge_collection": "document_relations",
  "threshold": 0.5, "seed_limit": 5, "max_depth": 2
}'
```

Semantic seeds → graph expansion along `edge_collection` → plus a ranked
**`graph_facts`** array drawn from the `facts` collection (guide 04):
compact `A --REL--> B` lines, reweighted by any accepted
`relation_rank_hint` neurons. `graph_facts` is what you feed an LLM as a
grounded, ontology-governed context block — that is the GraphRAG payload.

### The answer prompt (use this one)

Prompt wording around `graph_facts` is worth real recall: on the blind
evaluation kits, moving from a "list the facts that answer" phrasing to
the exhaustive one below took answer-level recall from 47% to 78% with
zero restraint cost. Hand your answering model this shape:

```text
System:
You answer questions STRICTLY from the provided graph facts.
Never invent facts; never reword them.

User:
Graph facts (the ONLY knowledge you may use):
{graph_facts, one per line}

Question: {question}

List EVERY fact from the list above that is relevant to answering the
question, copied verbatim — do not stop at the most salient ones;
include each fact that belongs in a complete answer. Never include a
fact that is not in the list. Then answer in prose using only those
facts.
```

Two properties to preserve if you adapt it: **exhaustive selection**
("EVERY … do not stop at the most salient") — models under-select on
broad questions without it — and **verbatim copying**, which lets you
symbolically verify that each cited fact really is in the trace
(anything not in the list is a fabrication; drop it before use).

## The query cache

With `COGNIGRAPH_QUERY_CACHE_ENABLED=true`, the embedding-based text routes (semantic, hybrid, graph-augmented) get:

- an **embedding cache** (query text → vector): repeated queries skip the
  provider call entirely;
- a **result cache** with *similarity-aware* hits: a query similar (≥0.97
  by default) to a cached one returns directly; moderately similar ones
  blend the cached signal into the fresh search (RRF).

Two operational facts to internalize:

1. **Writes invalidate.** Any document write in a collection drops that
   collection's cached results — you never serve stale hits after a load.
   (This also means bulk loads followed by heavy search see cold caches;
   that's correct, not broken.)
2. `COGNIGRAPH_QUERY_CACHE_BACKEND=persistent` +
   `COGNIGRAPH_QUERY_CACHE_PATH=/data/qcache.redb` makes the *embedding*
   level survive restarts — every survivor is a provider call you don't
   pay again. Results deliberately stay in-memory (they'd be stale or
   expired after a restart anyway).

Observe and reset:

```sh
cognigraph cache stats     # GET  /cache/stats — direct/assisted hits, evictions
cognigraph cache clear     # POST /api/cache/clear
```

## Retrieval checklist when results look wrong

1. `FOR c IN chunks_raw FILTER c.embedding == null RETURN c._key` — any
   unembedded chunks? (guide 02)
2. Lower `threshold` to 0.3 once; if results appear, it's a similarity
   calibration issue, not missing data.
3. Same query through `/api/search/hybrid` — if hybrid finds it and semantic
   doesn't, your query is terminology-bound; keep hybrid for that
   surface.
4. Check `/api/cache/stats` — a high `hits_direct` count during debugging
   means you're re-reading cached results; `cache clear` and retry.

Next: [04 — Semantic Neurons](04-semantic-neurons.md).
