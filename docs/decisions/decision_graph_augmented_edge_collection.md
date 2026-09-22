# Decision: graph-augmented search keeps `document_relations` as its default edge collection and warns when rank hints are inert

Status: accepted and implemented 2026-09-22 for [CG-89](../issues/CG-89.md).

## Context

`POST /api/search/graph-augmented` traverses one edge collection from the
vector seeds and ranks the traversed edges into the `graph_facts` trace.
Governed construction writes its facts into the managed `facts` edge
collection, and accepted `relation_rank_hint` neurons reweight relations in
that ranking. The request default, `document_relations`, is a documented
user convention for application-written edges; no code creates it.

With the default, neuron-built facts never enter the trace and an accepted
rank hint has no observable effect. Before this decision that was silent.

## Decision

1. **The default stays `document_relations` in both editions.** It is the
   collection an application writes through `/api/graph/relationships`, it
   exists without any construction having run, and changing it would break
   the live editions harness and every caller relying on the documented
   default. `facts` exists only after `/api/construct/ingest`, so a `facts`
   default would fail closed for every Enterprise instance that has not
   built a graph yet.
2. **Enterprise responses carry a warning when rank hints are inert.** When
   the neurons collection holds at least one accepted `relation_rank_hint`
   and `edge_collection` is not exactly `facts`, the response includes
   `warnings: [{code: "inert_rank_hints", message, accepted_rank_hints,
   edge_collection}]`. The check reuses the neurons read the ranking already
   performs; it adds no backend call. Collection names are exact identities,
   so `Facts` or `facts/` warn like any other name.
3. **Warnings describe live state and are never cached.** Fresh and
   cache-assisted responses compute them from the neurons read in the same
   request. A strong cache hit returns the cached results and trace without
   a `warnings` field, because that path performs no backend read by design.
4. **Community never emits the warning.** Community has no neuron semantics;
   a rank-hint document that happens to exist there is data.
5. **Unreadable or malformed neuron documents do not warn and do not
   fail.** They are skipped exactly as the ranking skips them.

Callers who want construct-built facts in the trace pass
`edge_collection: "facts"`. Traversing both collections in one request
remains a possible later contract and is not part of this decision.

## Consequences

- `docs/reference/http-api.md` documents `edge_collection` and the warning;
  the OpenAPI description lists the response field.
- Route tests cover the fresh, cache-assisted, strong-hit, `facts`,
  lifecycle-state, unreadable-collection, malformed-document, zero-limit and
  missing-edge-collection cases; unit tests cover exact-identity names.
- The live editions harness asserts the warning on Enterprise and its
  absence on Community.
- The cache key already serialises the whole request, so a cached
  `document_relations` trace is never served for a `facts` request.
