# Retrieval cache

## Semantic Query Cache

The cache is not a simple key-value store. It is a **continuous retrieval signal** that accumulates knowledge from previous queries.

### How It Works

1. **Embedding cache**: Maps (query text, model) → embedding vector, avoiding redundant API calls.
2. **Result cache**: Maps (collection, search_mode, parameters, normalized query)
   → search results. Similarity lookup requires exact parameter identity.
   Hybrid, semantic, and graph-augmented search use versioned JSON parameters,
   preserving collection/view names, ordered field arrays, and optional values
   (CG-31/CG-32). An omitted/null semantic model filter accepts every model;
   an empty string matches only that literal model name and has a distinct key.
3. **Continuous influence**: Cache weight follows a cubic curve based on cosine similarity between query embeddings:
   - `weight = ((similarity - floor) / (1 - floor))³`
   - High similarity (0.97+) → return cached results directly (fast path)
   - Above-floor lower similarity → merge cached signal with fresh results via RRF, weighted by the curve
   - At or below the configured floor → weight 0, fresh retrieval only
   - Per-document rank decay ensures top cached results influence more than tail results
4. **Continuous when enabled**: The cache participates between the configured floor and strong-hit threshold instead of acting only as a binary hit/miss layer. The cache itself is disabled by default.
5. **Invalidation**: Managed document, relationship, construction,
   neuron-transition, batch, read-write CGQL, and write-capable Lua routes
   invalidate result caches conservatively. An
   invalidation generation prevents an older in-flight search from repopulating
   stale results. Direct storage writes outside the server require explicit
   clearing.

### Cache-Assisted Retrieval (Semantic Search)

```
Query → Embed → Check Cache → Best Match Found?
                                  │
                    ┌──────────── YES ──────────────┐
                    │                               │
              sim >= 0.97?                    weight = cubic(sim)
                    │                               │
                   YES                    Run fresh vector search
                    │                               │
            Return cached               RRF merge: cached (weight)
            results directly            + fresh (1.0 - weight)
                                                    │
                                             Return merged results
```

---
