# Decision: system collections are unreachable through the public API

**Status:** Decided and landed 2026-07-15.

## Context

In single-store mode (no `COGNIGRAPH_DATA_DIR`) the auth control
collections — `_users` (Argon2 password hashes), `_tokens` (token
hashes), `_tenants` — live in the same `GraphBackend` as application
data. The routes never restricted collection names, so any caller with
`DocumentsRead` could read the hashes (`GET
/api/documents?collection=_users`, `FOR u IN _users` via CGQL, Lua's
`graph.find_documents("_users")`), and `DocumentsWrite` could escalate a
role with a single PATCH. Multi-tenant mode was never exposed — its
control store is a separate redb file (decision_multi_tenancy.md, D2) —
but single-store is the default deployment. The collections catalog
already hid underscore-prefixed names; this closes the same gap for
reads and writes everywhere else.

## Decision (owner: agent, user-approved scope)

1. **One choke point, not per-route checks.** `AppState` wraps every
   backend in `GuardedBackend`
   (`cognigraph-server/src/system_collections.rs`), a `GraphBackend`
   facade that answers `403 Forbidden` for any underscore-prefixed
   collection on every trait method. Documents, graph, search, batch,
   CGQL execution, and Lua scripts all inherit the policy because they
   all go through `state.backend`; a future route cannot forget the
   check. The whole underscore namespace is reserved, not just the three
   control collections — same convention as ArangoDB system collections
   and the catalog's existing filter.
2. **System components keep raw handles.** The `AuthProvider` is built on
   an unguarded backend handle taken BEFORE the wrap (`main.rs`
   `single_store_backend`); `/api/users`, `/api/tenants`, and login go
   through the provider, never through `state.backend`.
3. **Edges and traversals check endpoints too.** An edge into
   `_users/admin` would let a later traversal fetch the credential
   document, so `_from`/`_to`, `vertex_id`, and traversal start vertices
   are rejected alongside collection names.
4. **CGQL is screened statically.** `Query::referenced_collections()`
   (cognigraph-query) walks the AST — scans, vector-search sources,
   traversal edge collections, mutation targets, subqueries — with the
   planner's variable-first resolution rule. The query routes call it up
   front for a clean 403; the facade also screens `query()` because the
   native backend executes CGQL against itself, bypassing the facade's
   per-method guards. Raw AQL passthrough (Arango) cannot be parsed
   here, so it is screened textually for the three control-collection
   names as standalone tokens, including bind-var values (`@@coll` →
   `"_users"`); attribute accesses like `doc._key` are unaffected.
5. **Snapshots stay whole-database.** `/api/admin/export` and `import`
   remain unguarded on purpose: they are the hot-backup path (Admin
   scope) and must round-trip credentials, matching the restore
   semantics in docs/operations.md.

## Outcome

- `_users` is unreadable and unwritable through documents, graph,
  search (all five modes), batch, `/api/query`, and Lua — proven by
  `system_collections_answer_forbidden` tests in each route module plus
  the facade matrix in `system_collections::tests`.
- Residual acceptance: a pre-existing edge pointing into `_users` could
  still be walked by a native-internal CGQL traversal (start vertex is a
  runtime expression). The facade blocks creating such edges, so the
  window is only stores written before this decision.
- Known asymmetry: raw AQL screening is name-based (the three control
  collections), while CGQL rejects the entire underscore namespace.
  Arango single-store deployments that add NEW underscore collections
  outside the control set would need the list extended.

## M15 addendum — 2026-07-17

M15 closes the residual paths without rewriting this dated decision:

- Direct document creates/updates/replacements and every batch payload are
  checked for `_from`/`_to`, as are `upsert_edge` metadata and returned edges.
  This prevents an Arango upsert update from smuggling a system endpoint through
  its caller-controlled merge object.
- `get_edges` and traversal results are checked before return, so a legacy
  poisoned edge fails closed. Native CGQL now executes against the guarded
  facade, so runtime-bound traversal starts and endpoints cannot bypass it.
- Opaque backend-native AQL is Admin-only. The name-based AQL screen remains
  defense in depth for an already privileged caller, rather than the primary
  confidentiality boundary. Explicit CGQL remains the read-only query path for
  non-admin users.

## M18 supersession — 2026-07-18

The M15 Admin-only AQL assumption is superseded. A live ArangoDB probe proved
that quoted AQL identifiers decode Unicode escapes, so a name such as
`` `_cognigraph_\u006aobs` `` bypasses a textual collection-name screen. Public
opaque backend-native queries are therefore disabled in `/api/search/query`
and Lua `graph.query()` on AQL backends for every role. Parsed CGQL and typed
operations remain public. `GuardedBackend` retains its textual AQL checks only
as defense in depth for server-authored AQL; they are not the security boundary.
