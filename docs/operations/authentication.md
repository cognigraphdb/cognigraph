# Authentication

## Auth bootstrap

With auth enabled, `COGNIGRAPH_ADMIN_PASSWORD` bootstraps the default tenant's
`admin` identity and the separate `COGNIGRAPH_HOST_ADMIN_PASSWORD` bootstraps
the control-plane-only `host-admin` identity. Either bootstrap password also
requires `COGNIGRAPH_JWT_SECRET` at startup. Never reuse those passwords.
Issue API tokens (the secret is shown once; only its hash is stored) or use JWT
sessions:

```sh
TOKEN=$(curl -s -X POST :3000/api/auth/login \
  -d '{"username":"admin","password":"..."}' -H 'content-type: application/json' | jq -r .token)
curl -H "Authorization: Bearer $TOKEN" ':3000/api/documents?collection=docs'
```

Roles: `admin` (tenant data/operations, governance trust bootstrap and
recovery, but no policy author/approve/promote/artifact-attest authority), `editor`
(read/write data), `viewer` (read), `script-runner` (read-only Lua),
`policy-author` (signed promotion-policy or Semantic Repair revision),
`policy-approver` (independent signed policy or Semantic Repair review),
`promoter` (evidence, signed selection, M26 generation build, and signed
deployment decisions),
`artifact-attestor` (signed external artifact manifests only), and `host-admin`
(tenant lifecycle only). Route groups declare read/write scope pairs;
GET/HEAD normally select the read scope and other methods the write scope.
Lua typed CRUD, edge, and batch mutations require `documents:write`; opaque
backend-native AQL is disabled on every public surface regardless of role.
`/api/search/query` always parses read-only CGQL; a non-`cgql` language request
is forbidden even for Admin. On a parsed-CGQL backend, Lua `graph.query()` is
available to callers with `lua:execute`: it is read-only unless the caller also
has `documents:write`, which enables permission-gated CGQL mutations. It is
disabled entirely when the active backend language is AQL. Auth-disabled Lua
stays read-only, so disabling RBAC does not silently enable a scripting
mutation surface. Use typed APIs or explicit CGQL.

Lua scripts cannot access `jit`, `loadstring`, or the other disabled loaders.
JIT disabling and hook installation fail closed. Instruction checks run every
1,000 Lua instructions; `pcall`, `xpcall`, and coroutines cannot suppress
resource termination. The configured CGQL time allowance starts once per
script, including parsing and typed graph callbacks, and is not renewed by
another query. Dropping or timing out a request signals the Lua worker and
cancels pending backend futures. Cleanup waits for worker completion and
invalidates the caller's result cache even after partial writes. Synchronous
backend work must yield or return before cancellation can take effect; it is
not forcibly killed, and already committed writes remain committed. Budget
errors retain the existing HTTP 500 error mapping; the outer request timeout
returns 408.

This supersedes the earlier Admin-only opaque-query contract: textual AQL
screening is not a security boundary because quoted identifiers can encode
Unicode escapes. Protected-name/handle checks and the dangerous dynamic,
document, collection-catalog, graph/traversal, geo, and full-text capability
denylist (`CALL`, `APPLY`, `DOCUMENT`, and related helpers) remain
defence-in-depth for internal/server-authored backend-native query paths; they
do not make public raw AQL available.

Governed construction ingestion requires an atomic-batch backend. The native
backend provides the required all-or-nothing replacement; maintenance-mode
Arango rejects `/api/construct/ingest`, `/api/construct/governed-ingest`, and
M26 generation build/deploy and recovery of present M26 authority before CAS
reads or construction writes. General Arango startup and operator status remain
available; when no M26 authority is present, startup recovery is a no-op and
status reports M26 disabled.

When moving
an older store to the occurrence model, legacy chunk rows without raw
`chunk_id` must be rebuilt with their derived `mentions` and `facts` rather
than overwritten ambiguously.

### Token hygiene

Tokens are non-expiring by default; set `COGNIGRAPH_TOKEN_TTL_SECS` (e.g.
`7776000` = 90 days) to give every new token an expiry, or pass
`expires_in_secs` per token (`0` = explicitly non-expiring, either way).
Expired tokens fail exactly like revoked ones but stay visible in
`GET /api/users/{key}/tokens` with `"expired": true` for audit; revoke them to
remove the records. Rotate instead of revoke+recreate when a credential
may be exposed or on a schedule: `POST
/api/users/{key}/tokens/{token_key}/rotate` keeps the record and key, issues a
fresh secret and expiry window, and kills the old secret immediately.
Recommended production policy: `COGNIGRAPH_TOKEN_TTL_SECS=7776000` and
rotation on personnel change or suspected exposure.

## Multi-tenancy (decision_multi_tenancy.md)

One backend store per tenant — isolation is structural, not a filter:
a request's tenant is resolved in the auth middleware and every backend
call routes to that tenant's store; there is no cross-tenant read path
for any role. Setup:

```sh
COGNIGRAPH_DATA_DIR=/var/lib/cognigraph \
COGNIGRAPH_AUTH_ENABLED=true \
COGNIGRAPH_ADMIN_PASSWORD=... \
COGNIGRAPH_HOST_ADMIN_PASSWORD=... \
COGNIGRAPH_JWT_SECRET=... \
cognigraph-server
```

- **Tenant lifecycle** (host-admin role, `TenantAdmin` scope — manages
  tenants, never reads their data): `cognigraph tenant list | create
  NAME | suspend NAME | activate NAME | delete NAME`.
  - **Suspend/resume** refuses the tenant's users at middleware (including
    login), checkpoints current durable work, and parks its queue while keeping
    the store and files untouched. Nonterminal jobs still consume global queue
    capacity. Activation reconciles catalog/archive state, rebuilds active
    accounting, and only then removes the admission fence and reschedules work.
    Suspension does not free resident-mode memory. Already-admitted ordinary
    data requests may finish against their captured store and cache; suspension
    closes admission and is not cancellation of every synchronous request.
  - **Delete** (decision_tenant_deletion.md) removes the tenant record
    AND its users and tokens, evicts the open store handle, and
    quarantines `{name}.redb`, matching `.tantivy` directories, `.vectors`
    sidecars, and unfinished `.vectors.tmp` builds, including legacy names.
    They are renamed to `<entry>.deleted-{millis}` in `COGNIGRAPH_DATA_DIR`. Recreating the
    name starts empty — it never reattaches to quarantined data. The
    default tenant cannot be deleted (suspend it instead). CG-12 pins admitted
    requests to the original incarnation/store/cache, so a delayed provider or
    Lua result cannot write into a replacement tenant or reopen an absent name.
    Such requests may still write the quarantined database until they drain;
    wait for outstanding requests/workers to finish, or stop the server cleanly,
    before recovering or purging those files. User/token administration holds
    the lifecycle lock through its control-store operation and rechecks the
    administrator after any lifecycle wait.
  - **Recover a deleted tenant**: before recreating the name, rename
    the quarantined files back (`mv acme.redb.deleted-1752… acme.redb`)
    and `cognigraph tenant create acme` — users must be recreated.
    **Purge** (actual data removal) is deliberately manual: inspect the
    quarantined entries and remove the selected files and directories under
    your retention policy. Tantivy quarantines are directories.
- **Users**: `cognigraph user create alice editor --tenant acme` —
  roles keep their meanings within the tenant. Legacy users (no tenant
  field) are the implicit `default` tenant.
- **Backup/restore per tenant**: `cognigraph export/import` under a
  tenant user operates on that tenant's store by construction (the
  facade routes it). File-level: each tenant is one redb file.
- **Native schema-2 upgrade (CG-11)**: opening a schema-1 database atomically
  adds a database UUID and commit-revision UUID without rewriting documents.
  Older binaries reject the upgraded file. Retain a pre-upgrade backup for
  binary rollback, or use JSON export/import into a separate older database.
  Derivatives rebuild once under database-scoped names; normal restarts retain
  identity. Restoring a physical backup preserves its UUIDs, and new commits
  get new revisions so divergent restores cannot reuse obsolete indexes.
  See [the migration decision](../decisions/decision_native_derivative_identity.md).
- **Caches** are per-tenant in-memory (a shared cache would be a
  cross-tenant side channel); the persistent query cache is not yet
  supported in multi-tenant mode.
- **Migration of an existing single-store deployment**: stop the
  server, `mv store.redb $DATA_DIR/mytenant.redb`, start with
  `COGNIGRAPH_DATA_DIR`, `cognigraph tenant create mytenant`, and
  point its users at the tenant (`--tenant mytenant` on creation, or
  patch the user documents). Reversible by moving the file back.
- **Overhead** (measured): ~30 ms one-time lazy open per tenant store;
  100 open stores verified in-test.

Single-tenant deployments: do nothing — without `COGNIGRAPH_DATA_DIR`
nothing changes, byte for byte.
