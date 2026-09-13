# CGQL Mutations — Design Proposal (for discussion)

This is the dated design discussion. Use the [current CGQL specification](../../reference/cgql.md#mutations)
for implemented mutation rules and the [Native-only decision](../../decisions/decision_native_only.md)
for current storage. Earlier backend comparisons below are historical.

Status: ACCEPTED 2026-07-02. Decisions:
1. v1 = INSERT / UPDATE / REPLACE / REMOVE. **UPDATE is a partial update
   (merge); REPLACE swaps the whole document.** UPSERT delivered 2026-07-03
   (`_key` fast path + all-fields match, key order).
2. New `POST /api/query` endpoint for ReadWrite mode.
3. Lua `graph.query()` stays read-only **until RBAC is available**, then it
   is lifted.
4. Per-document atomicity for CGQL mutations; batch transactions delivered
   2026-07-03 as `GraphBackend::execute_batch` (capability pattern, native
   backend all-or-nothing in one redb transaction, `POST /api/batch`).
5. Mutations ship before Phase 8 auth, gated by
   `COGNIGRAPH_CGQL_MUTATIONS_ENABLED=false` by default.

The remainder of this document is the original proposal and decision prompt.
Future-tense and deferred statements below are preserved as design history;
the status block above and `cgql-v1.md` describe the delivered behavior.

## Proposed syntax (AQL-style, matching CGQL's heritage)

```cgql
INSERT { title: "New", category: @cat } INTO documents
UPDATE "key123" WITH { reviewed: true } IN documents
REPLACE "key123" WITH { title: "Rewritten" } IN documents
REMOVE "key123" IN documents

// Bulk, FOR-driven:
FOR d IN documents
FILTER d.category == "stale"
REMOVE d._key IN documents

// Optional result projection:
INSERT { ... } INTO documents RETURN NEW
UPDATE "k" WITH { ... } IN documents RETURN { before: OLD, after: NEW }
```

- `NEW`/`OLD` become reserved pseudo-variables, in scope only after a
  mutation clause.
- A query has at most one mutation clause; mutations and `COLLECT` are
  mutually exclusive (as in AQL v1 restrictions).
- `UPSERT` is deferred to a second phase (its match-semantics deserve their
  own discussion; `upsert_edge` already covers the main product need).

## Execution model

- Mutations map 1:1 onto the existing `GraphBackend` ops
  (`create_document`, `update_document`, `replace_document`,
  `delete_document`) — **no trait changes**. Conflicts surface as
  `DocumentConflict` (409), missing docs as `DocumentNotFound`.
- **Atomicity is per document**, not per query. A bulk `FOR ... REMOVE`
  that fails midway leaves earlier removals applied. This is honest about
  what both backends can do through the trait today (redb could batch, the
  Arango HTTP API cannot) and must be stated in the spec. A future native
  "transactional batch" capability could tighten this behind a trait
  method, same pattern as `text_search`.

## Access control (the reason this needs discussion)

CGQL today is read-only, so `query()` is safely exposed via
`/api/search/query` and Lua's `graph.query()`. Mutations change that.

Proposal:
1. The executor gains a `QueryMode { ReadOnly, ReadWrite }` parameter.
   Parsing a mutation in `ReadOnly` mode is a validation error
   ("mutations are not allowed on this endpoint").
2. `/api/search/query` and Lua `graph.query()` stay **ReadOnly** — no
   behavior change for existing consumers.
3. A new `POST /api/query` endpoint runs ReadWrite, and is the natural
   attachment point for Phase 8 RBAC scopes (`documents:write`,
   `graph:write`). Until auth lands, it can be disabled by default
   (`CGQL_MUTATIONS_ENABLED=false`).
4. Cache invalidation: the ReadWrite executor must call
   `invalidate_collection()` like the REST write routes do.

## Open decisions (need your call)

1. **Scope of v1**: INSERT/UPDATE/REPLACE/REMOVE only, UPSERT deferred —
   agreed?
2. **Endpoint shape**: new `POST /api/query` (ReadWrite) vs a `mode` flag on
   `/api/search/query`? I recommend the new endpoint — cleaner RBAC story.
3. **Lua**: keep `graph.query()` read-only permanently (mutations via the
   existing `graph.create_document`/etc. bindings), or lift it once RBAC
   exists? I recommend permanently read-only — one auditable write path
   per surface.
4. **Per-document atomicity**: acceptable for v1, with the future
   batch-transaction capability noted? Or is query-level atomicity on the
   native backend worth a trait extension now?
5. **Sequencing**: implement mutations before Phase 8 auth (gated by the
   env flag), or land auth first? I recommend mutations first behind the
   flag — it unblocks CLI/tooling use immediately while auth is designed.
