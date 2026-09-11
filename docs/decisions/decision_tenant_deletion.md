# Decision: tenant deletion contract

**Status:** Decided 2026-07-15; implemented same day (auth
`delete_tenant`, `TenantRegistry::retire_store`, DELETE
`/api/tenants/{name}`).

## Context

`DELETE /api/tenants/{name}` deleted only the tenant RECORD in the
control store. Verified live: the `TenantRegistry` kept the open store
handle (memory held for good in resident mode), `{name}.redb` stayed in
`COGNIGRAPH_DATA_DIR`, and recreating a tenant with the same name
silently reattached to the orphaned data — the previous customer's
graph served to the next customer with that name. The tenant's users
also survived in the control store, so recreation revived their
credentials too. decision_multi_tenancy.md D1 promised "deleting a
tenant is deleting a file"; nothing delivered it.

## Decisions

**T1 — Quarantine the store, don't destroy it.** On deletion the
registry evicts the tenant's store and cache handles, then renames
`{name}.redb`, matching Tantivy directories, vector sidecars, and unfinished
vector builds (including legacy names) to `<entry>.deleted-{millis}` in place.
CG-11 added the missing text-directory and temporary-file handling on
2026-09-08. Rejected: `fs::remove_file`. A
tenant's store may be the only copy of their data, and DELETE is one
HTTP call behind one scope (`TenantAdmin`); an atomic rename buys full
recoverability at the cost of disk until the operator purges. Retention
is therefore an OPERATOR policy, not a server behavior: quarantined
names are never matched by the live-file lookup (tenant names cannot contain
`.`), recovery is renaming the file back after admitted work drains and before
recreating the tenant, and purge is `rm`. A `?purge=true` hard-delete
can be added later if retention rules demand it; the default stays
recoverable.

**T2 — The tenant's users (and their tokens) are deleted, not
refused-only.** The middleware gate already refuses users of unknown
tenants, so refuse-only *looks* equivalent — until the name is
recreated and the old users, passwords, and tokens come back alive.
That is the same surprising-retention bug as the store reattachment,
in credentials. Users are cheap control-store records recreated
deliberately; graph data is not — hence delete users, quarantine data.

**T3 — The default tenant is not deletable.** Its gate is implicitly
open (allowed with no record, for absolute back-compat), so "deleting"
it would quarantine live data while requests keep flowing into a fresh
empty store. `delete_tenant` refuses it; suspend it instead.

**T4 — Ordering and races (CG-12, 2026-09-08).** Request admission and
tenant lifecycle changes share the server's lifecycle lock. Under that lock,
admission validates current credentials and tenant status, then captures the
immutable incarnation, concrete store handle, and cache handle. The lock is
released before the handler runs, including provider calls and body extraction.
Every later routed operation uses those handles; Lua carries them into its
blocking worker and cleanup supervisor. A request whose first database call
occurs after deletion cannot open a new store for the absent or recreated name.

Deletion first suspends the tenant, checkpoints durable jobs and fences
promotions, then deletes credentials and the record and retires the store,
all under the lifecycle lock. The rename also holds the registry's stores
write lock, excluding lazy opens during retirement. Already-admitted ordinary
data requests may finish against their captured handles: redb writes land in
the quarantined inode and its handle closes when the last `Arc` drops. They
cannot populate or invalidate the replacement cache or adopt its incarnation.
User/token administration uses the shared control store, so it separately
holds the lifecycle lock from its live-administrator recheck through the
operation. Retiring still runs for a missing record to heal historical orphans.

## Consequences

- Recreate-after-delete starts empty — proven by tests at the registry
  and route layers (`retire_store_evicts_the_handle_and_quarantines_the_file`,
  `delete_tenant_retires_its_store`).
- The DELETE response reports what was quarantined:
  `{ "deleted": true, "quarantined": ["acme.redb.deleted-1752…"] }`.
- The CLI (`cognigraph tenant delete NAME`) calls this endpoint, so it
  inherits the contract unchanged.
- Suspend/resume still performs no eviction or file move. Since M17,
  suspension also checkpoints and parks durable queue work, while activation
  reconciles authoritative job state, rebuilds capacity, and reschedules work
  before the tenant is reopened; resume is therefore not promised to be
  instant (documented in operations.md and
  decision_m17_queue_scale_governance.md).

## Outcome

The contract shipped: deletion removes the tenant record and its credentials,
evicts the live backend, quarantines redb/vector files with a timestamped
suffix, and refuses deletion of the default tenant. Route and registry tests
cover recreate-after-delete and orphan healing; the CLI uses the same endpoint.
Quarantine is recoverable, while permanent purge remains an explicit operator
retention action.

CG-11 additionally binds persistent derivatives to an immutable database UUID
and the exact committed revision, protecting same-name recreation and
divergent restores even when numeric generations repeat. See the
[Native identity decision](decision_native_derivative_identity.md) for schema
migration and current verification. CG-12's request pinning and control-store
fence are verified by paused-provider and slow-body release HTTP regressions
in resident/paged modes, including recreation and restart. See
[request-isolation evidence](../issues/request-isolation-2026-09-08.md).
