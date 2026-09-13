# Decision: multi-tenancy (D1–D6)

**Status:** Decided 2026-07-07 (user approved all six); implementation
in milestones M1–M4 (M1 landing first: identity + back-compat).

## Context

Today one deployment = one store: any authenticated editor can read or
write any collection, raw CGQL can name any collection, `_users` and
`_tokens` live inside the same store as the data, and space-scoping
exists only inside the construct layer. Tenancy is what turns a
deployment-per-customer into a platform — and it must be built on the
repo's standing principle: **isolation is structural, never
predicate-discipline** (the drafts-collection precedent: a flag every
consumer must remember to check is the set-but-ignored failure mode,
and here it would be a cross-customer data leak).

## Decisions (owner: user, 2026-07-07 design session)

**D1 — One backend store per tenant.** Each tenant gets its own
`NativeBackend` (own redb file under a data directory); the router
resolves the tenant and hands the request that tenant's backend
handle. Everything downstream — collections, CGQL, search, construct —
is untouched and structurally cannot reach another tenant's data: raw
CGQL needs no query rewriter because there is nothing cross-tenant to
name. Rejected: row-level tenant_id predicates (one missed predicate
anywhere = silent leak) and collection-name prefixing
(validation-dependent). Free consequences: export/import become
per-tenant backup/restore, deleting a tenant is deleting a file,
memory cost is per-tenant and observable, and "global entities" become
per-tenant automatically (correct — names collide across customers
with different meanings). The N-open-stores overhead gets a 100-tenant
dry-run measurement in M2.

**D2 — Tenant identity in a control store; resolution before
routing.** Tenants are first-class records in a small CONTROL store —
a separate backend holding `_tenants`, `_users`, `_tokens` (auth moves
OUT of tenant data: restoring or deleting a tenant's store must not
touch credentials). A user belongs to exactly one tenant in v1; tokens
and JWT claims carry the tenant; the auth middleware resolves
request → user → tenant → backend handle before any route logic runs.
**Back-compat is absolute:** no tenant records (or auth disabled) →
the implicit `default` tenant → today's behavior byte-identical,
including single-file `COGNIGRAPH_NATIVE_PATH`.

**D3 — Shared vs per-tenant.** Per-tenant: ALL graph data (documents,
chunks, entities, facts, neurons, space types, review policies, eval
specs, drafts), query caches, and — deliberately — embedding caches:
sharing them would save API cost (embeddings are deterministic) but a
cache hit reveals that another tenant embedded the same text — a side
channel, rejected and recorded. Shared in v1: the server process and
provider API keys/model config (per-tenant provider overrides are a
recorded later-item; review policies are already per-space documents,
so judge/lane config is already per-tenant data).

**D4 — RBAC composes; one new role.** `Admin/Editor/Viewer/
ScriptRunner` keep their exact meanings WITHIN a tenant — no scope
changes. New control-store-level role **host-admin** (scope
`TenantAdmin`) for tenant lifecycle only: create, suspend, delete,
quota edits. A tenant's Admin manages that tenant's users only.
**Cross-tenant data access exists for nobody, host-admin included** —
host-admin manages tenants, it does not read their graphs (the honest
default for NDA-grade corpora); break-glass export stays an audited,
per-tenant act.

**D5 — Quota fields now, enforcement deferred.** The tenant record
carries optional quota fields (max documents, max storage bytes, CGQL
budget overrides) — schema reserved, enforcement deferred with a
recorded trigger: first multi-customer deployment or pilot data
showing contention. CGQL per-query budgets and per-IP rate limiting
already bound worst-case single-request behavior; quota enforcement
designed now would be hypothetical-driven.

**D6 — Ops surfaces and migration.** `/api/admin/tenants` CRUD
(TenantAdmin scope); per-tenant export/import via the existing
snapshot machinery; `/metrics` tenant label; per-tenant store health.
Migration for an existing deployment: a one-time `migrate-to-tenant
NAME` admin verb moving the current store into the tenant directory —
reversible by moving the file back.

## Milestones

- **M1** — identity: control records (`_tenants`), `User.tenant`
  (default `default`), resolution middleware (unknown/suspended →
  403), `Role::HostAdmin` + `Scope::TenantAdmin`, `/api/admin/tenants`,
  back-compat pinned byte-identical.
- **M2** — isolation: per-tenant stores under `COGNIGRAPH_DATA_DIR`,
  per-request backend routing, 100-tenant overhead dry-run.
- **M3** — ops: per-tenant export/import, metrics labels, CLI verbs,
  `migrate-to-tenant`.
- **M4** — docs: operations runbook, architecture, OpenAPI.

## Boundaries kept

- No cross-tenant read path exists structurally, for any role.
- Auth data lives outside tenant data.
- Single-tenant deployments never notice tenancy exists.

## Outcome

All four milestones landed 2026-07-07 (M1 commit 0d4b256, M2 fa7b64b,
M3+M4 following): identity + middleware gate + host-admin; per-tenant
stores behind the `RoutedBackend`/`RoutedCache` facades with the
task-local tenant scope as the single structural choke point (handlers
untouched, CGQL needs no rewriter); observability (`store_open` flags,
open-store counts), full CLI lifecycle, and the operations runbook.
CG-12 (2026-09-08) additionally pins each admitted request's incarnation,
store, and cache under the tenant lifecycle lock, including Lua task transfers.
Admitted ordinary data work may drain into a retired store, while user/token
administration fences its shared-control-store operations against lifecycle
changes. See [the deletion contract](decision_tenant_deletion.md).
**Overhead measured:** 100 real per-tenant stores open in 3.0 s
(~30 ms one-time lazy cost each), write+read verified per store;
isolation test-pinned (tenant A's document invisible to tenant B and
to the default tenant — different stores, not a filter). Deferred with
recorded triggers: quota enforcement (D5 — first multi-customer
deployment), persistent query cache in multi-tenant mode, per-tenant
metrics labels.

The delivered operations surface differs from the D6 sketch in two naming
and scope details: tenant lifecycle is `/api/tenants` (not `/api/admin/tenants`), and
single-store migration is the documented stop/move/start procedure rather
than a `migrate-to-tenant` server verb. Per-tenant metrics labels were also
deferred; current observability reports store-open state and aggregate open
store count without tenant labels. The decision text and milestone list above
remain the original accepted design; this outcome block is authoritative for
what shipped.

**Live smoke (2026-07-13, release build, real HTTP):** server booted
with `COGNIGRAPH_DATA_DIR` + auth + JWT; bootstrap admin landed in the
control store; tenant-Admin refused on `/api/tenants` (403) and host-admin
refused on `/api/documents` (403) — both scope boundaries hold over HTTP;
two tenants created; same-key documents in both tenants fully isolated
(REST 404 across tenants; CGQL sees only the caller's store);
suspension takes effect immediately (403) and reactivation restores
access; per-tenant export verified three ways (acme admin sees only
acme data, default-tenant admin sees neither); `/api/tenants` reports
open stores; disk shows exactly `_control.redb` + one file per
tenant.

**Successor for deletion semantics:**
`decision_tenant_deletion.md` supersedes the original D2 sentence that tenant
deletion must not touch credentials. The delivered security contract deletes
the tenant's users/tokens, evicts its backend, and quarantines its data files so
recreating the same name cannot resurrect credentials or silently reattach old
data.
