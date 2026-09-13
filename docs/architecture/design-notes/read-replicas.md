# Read replicas for the Native backend — design note

Status: PROPOSED 2026-09-10. Not implemented. Scope: read scaling and a warm
standby for a single-writer store with manual, operator-controlled promotion.
Tier: **Community**, confirmed by the owner on 2026-09-10. Sharding, consensus,
automatic failover and multi-writer are Enterprise (see the
[licensing decision](../../decisions/decision_licensing.md)) and are not
designed here.

## Why this and not a cluster

The Native backend is one redb file with one writer. Most single-node
deployments that outgrow it do so on reads first — a lookup service, a
console, an agent hammering `POST /api/search/query` — while the write rate
stays modest. A log-shipping replica answers that with almost no new
machinery: the write path already has exactly one choke point, derivatives
are already rebuildable, and the HTTP surface already separates read-only
from read-write routes. A consensus cluster answers a different question
(write availability) at ten times the cost and would have to be paid for.

## What already exists

- **One commit point.** Every Native write — document CRUD, edges, CGQL
  mutations, `POST /api/batch`, collection create/drop, snapshot import —
  ends in `Store::apply(&[StoreOp])` (`crates/cognigraph-native/src/storage.rs`),
  which commits the batch in one redb write transaction and increments
  `meta.data_generation`. `StoreOp` is a small closed enum —
  `PutCollection`, `DropCollection`, `PutDocument`, `DeleteDocument` — and
  edges are documents in edge collections, so that is the entire write
  vocabulary.
- **Rebuildable derivatives.** Vector sidecars and tantivy text indexes are
  caches over redb truth ([rebuildable derivatives](../../decisions/decision_rebuildable_derivatives.md));
  a replica rebuilds its own and never ships them.
- **Read/write route split.** `POST /api/search/query` is read-only CGQL;
  `POST /api/query` is the only mutating CGQL route and is already gated by
  `COGNIGRAPH_CGQL_MUTATIONS_ENABLED`. Document/edge/batch write routes are
  distinct handlers.
- **Snapshot surface.** `GET /api/admin/export` / `POST /api/admin/import`
  give a replica its initial state.

## Design

### 1. Replication log table

Add a `replication_log` table to the redb file: key `u64` sequence, value
the serialized `Vec<StoreOp>` of one committed `apply` plus a wall-clock
stamp and the resulting `data_generation`. It is written in the **same
transaction** as the ops, so log and data cannot diverge. The sequence is
`data_generation` itself — it already increments once per commit.

Retention: keep the last N entries or T hours (`COGNIGRAPH_REPLICATION_LOG_RETAIN`),
trimmed in the same transaction. A replica that falls behind the retained
window re-bootstraps from a snapshot (§3).

Cost on the primary: one extra table insert per commit; the ops are already
serialized JSON values, so this is an append of bytes already in memory.
Off by default; enabled by `COGNIGRAPH_REPLICATION_ROLE=primary`.

### 2. Log endpoint

`GET /api/replication/log?after=<seq>&limit=<n>` on the primary, returning
entries `(seq, generation, stamp, ops[])` in order, plus the primary's
current `head` and `store_identity` (the redb identity UUID). Authenticated
with a dedicated `replication` scope so a replica credential can do nothing
else. `GET /api/replication/status` returns `head`, `oldest_retained`,
`identity`.

Ops travel as the same JSON they were committed with; the wire format is the
`StoreOp` enum, tagged with a log-format version in `meta` so a newer
primary refuses an older replica rather than corrupting it.

### 3. Replica

`COGNIGRAPH_REPLICATION_ROLE=replica` and `COGNIGRAPH_PRIMARY_URL` turn a
normal server into a follower:

1. **Bootstrap.** If the local store is empty or its identity differs from
   the primary's, fetch `export` from the primary, note `head` at that
   moment, `import` locally, and record `applied_seq = head`. Import is
   already all-or-nothing on Native.
2. **Tail.** Loop: `GET /api/replication/log?after=applied_seq`; for each
   entry call the local `Store::apply(ops)` (the same function the primary
   used), then persist `applied_seq` in the replica's `meta` table in that
   same transaction. Derivatives invalidate exactly as they do for a local
   write. Poll interval `COGNIGRAPH_REPLICATION_POLL_MS` (default 500);
   long-poll on the primary side is an optimization, not a requirement.
3. **Gap.** If `after` is older than `oldest_retained`, go back to step 1.
4. **Serve reads.** All read routes work. All write routes — document/edge
   CRUD, batch, `POST /api/query`, Lua in write mode, import, collection
   create/drop — return `409 {"error":"read_replica","primary":"<url>"}`.
   Auth and users are replicated data like anything else (they live in the
   same file in single-tenant mode); a replica validates tokens locally.
5. **Headers.** Every response carries `X-CogniGraph-Applied-Seq`. A client
   that needs read-your-writes reads the primary's `X-CogniGraph-Seq` from
   its write response and sends `X-CogniGraph-Min-Seq` to a replica; the
   replica answers `425 Too Early` if it has not applied that far, so a
   load balancer can retry on the primary.

Readiness: `GET /health/database` on a replica also reports `lag_seqs` and
`lag_ms`, and returns 503 if lag exceeds `COGNIGRAPH_REPLICATION_MAX_LAG_MS`
(default: unlimited), so Kubernetes stops routing to a stale replica without
killing it.

### 4. Kubernetes shape

The chart grows a second StatefulSet, `cognigraph-replica`, with N pods and
its own Service (`cognigraph-read`). Ordinal 0 of the existing StatefulSet
stays the only writer. Applications point writes at `cognigraph` and reads
at `cognigraph-read`; the TypeScript SDK will do that split by default
(read-only CGQL, GET routes → read service; everything else → primary).

Promotion is **manual** in this design: scale the primary to 0, pick the
replica with the highest `applied_seq`, restart it with
`COGNIGRAPH_REPLICATION_ROLE=primary`, repoint the others. That is a warm
standby, not HA. Automatic failover needs a lease and fencing, which is the
Enterprise line.

### 5. Semantic cache, jobs, tenancy

- The semantic query cache is per node and unaffected.
- Durable jobs, construction, side views and the governance chain write
  through the same `apply`, so their *data* replicates. The job *runner*
  runs only on the primary; a replica never claims queue work.
- Multi-tenant mode (Enterprise) ships one log per tenant store plus the
  control store; the replica mirrors `COGNIGRAPH_DATA_DIR` layout. Not in
  the first cut.

## What it does not solve

Write throughput, write availability during a primary restart, cross-region
latency for writes, and consistency stronger than "eventually, monotonic per
replica, with an opt-in read-your-writes fence". Say so in the docs; a user
who needs those needs the Enterprise cluster or a different database.

## Effort

Log table and trimming: 1–2 days. Endpoint, scope, status: 1 day. Replica
loop, bootstrap, write-route refusal, headers, readiness lag: 3–4 days.
Chart and docs: 1 day. Corpus tests for op replay parity and a fault test
that kills the replica mid-tail: 2 days. Roughly two weeks solo, no new
dependencies.

## Open questions

- Should the log also carry `import_snapshot` as one entry (potentially
  huge) or force re-bootstrap? Proposed: one entry with a size cap; above it,
  the entry is a marker that forces re-bootstrap.
- Long-poll vs. plain poll for the first release. Proposed: plain poll.
- Whether `409` on a replica should optionally proxy the write to the
  primary. Proposed: no; keep the replica honest and let the SDK route.
