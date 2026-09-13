# Decision: M17 queue scale and governance

> Current storage scope (2026-09-12): [Native-only storage](decision_native_only.md)
> supersedes this record's runtime-backend choices and adapter-specific paths.
> Both editions now use Native; public HTTP/Lua queries are parsed CGQL.
> Storage-independent contracts below remain applicable. Earlier backend
> behavior, configuration and verification are retained as dated history,
> not current setup instructions. Use the [operator guides](../operations/README.md).

**Status:** Verified (2026-07-17).

## Context

M16 made governed construction and evaluation durable, idempotent, attributable,
and restart-recoverable. Its intentionally small queue was still unsuitable for
a long-lived operational history: listing computed an exact total by scanning
full job records, terminal records stayed in the hot collection indefinitely,
admission had no explicit capacity contract, and one tenant's long ingest could
retain the singleton worker until the whole job completed.

M17 scales this boundary without changing CogniGraph's deployment model. It
keeps one authoritative writer and one in-process dispatcher, adds bounded
tenant-aware scheduling and storage maintenance, and makes every derived queue
structure repairable from durable job records. It does not introduce replicas,
distributed leases, or high availability.

## Decision

### D1. Keep hot and archived job records authoritative

The tenant store has three protected collections:

- `_cognigraph_jobs` is the authoritative hot collection. It contains every
  nonterminal job and terminal jobs that have not been archived.
- `_cognigraph_job_archive` is the authoritative archive. It contains complete
  terminal `JobRecord` values, including immutable input, result or error, and
  attributed event history.
- `_cognigraph_job_catalog` is a derived listing catalog. It is not a source of
  truth for execution, recovery, admission, detail lookup, or audit.

Detail lookup and idempotency replay consult the archive first and then hot
storage. This order fails closed if copy-first archival temporarily leaves a
duplicate. An archived job remains inspectable and replayable by its original
submission key, but it is immutable through the job API: it cannot be canceled
or retried. The hot and archive collections remain tenant-incarnation scoped,
protected from document, CGQL, Lua, traversal, batch, and backend-native query
surfaces, and included in native backup/restore as operational source data.

The catalog contains only the summary needed for listing plus an `archived`
marker. Its key is deterministic:

```text
sha256(tenant + NUL + incarnation)-inverted_created_ms-sha256(job_id)
```

The fixed-width inverted timestamp makes ascending key order newest-first. The
scope hash keeps one tenant incarnation contiguous and makes stale-incarnation
or cross-tenant cursors rejectable. Catalog synchronization happens after an
authoritative hot-record write. A catalog write failure must not reinterpret a
committed job transition; it records degraded catalog health and is repaired by
reconciliation.

### D2. Make cursor pagination the bounded default

`GET /api/jobs` uses cursor pagination unless the caller explicitly supplies
`offset`. The cursor path has these contracts:

- `limit` is 1 through 200;
- cursors are versioned, purpose-bound, tenant-incarnation scoped, and bound to
  the requested kind, status, and archive filter. M17 emits v2 cursors with a
  base64url-encoded storage position and continues to read safe legacy v1
  cursors;
- a changed filter, stale cursor, or cross-tenant cursor is HTTP 400;
- `archived=exclude|include|only` controls the catalog tier selection;
- each request inspects at most 2,048 lightweight catalog rows, in internal
  pages of at most 64 rows;
- selective filters may therefore return fewer than `limit` rows together
  with a non-null `next_cursor`;
- `total` is deliberately `null`; `scanned` reports the bounded catalog work
  performed by that request.

The portable storage primitive is
`GraphBackend::list_documents_after_key`: an exclusive `_key` cursor with
stable ascending order and projection. Native resident mode uses its ordered
map, native paged mode uses a redb composite-key range, and ArangoDB uses an
explicit `_key` filter, sort, limit, and projection.

Offset pagination remains only as a deprecated compatibility path. Supplying
`offset` requests the old exact-total scan, is capped at 10,000, cannot be
combined with `cursor`, and cannot include archived records. New clients and
the CLI use cursors.

### D3. Apply explicit tenant and process backpressure

Nonterminal jobs consume admission capacity from durable creation or retry
until they become terminal. Capacity has two host controls:

- `COGNIGRAPH_JOB_MAX_ACTIVE_TOTAL`, default 1,000;
- `COGNIGRAPH_JOB_MAX_ACTIVE_PER_TENANT`, default 100.

An authenticated tenant may have `quotas.max_active_jobs`; its effective limit
is the configured value capped by the host per-tenant ceiling. Tenant quota
updates reject negative, non-integer, or above-ceiling values. A value of zero
fences new admissions without rewriting existing records.

Submission checks capacity before expensive payload preparation and reserves
capacity again while holding the transition lock before the durable insert.
The final reservation re-reads the effective tenant quota under that barrier,
and quota merge/write uses the same barrier, so a successful quota reduction
cannot be bypassed by an in-flight candidate carrying a stale limit. Retry uses
the same admission boundary. A valid replay of an already durable
submission or retry is returned without consuming a second slot. Capacity
exhaustion returns HTTP 429 with `Retry-After: 1`, distinguishing transient
backpressure from an idempotency conflict or authorization failure.

The in-memory active set is an admission index, not durable truth. Startup,
tenant activation, and post-import recovery rebuild it from authoritative hot
records before accepting work for that tenant incarnation. Suspended tenants
remain fenced and unscheduled, but their nonterminal jobs are rebuilt into the
global active count; suspension does not create hidden admission capacity.

### D4. Schedule fairly at durable checkpoint boundaries

The singleton dispatcher maintains a FIFO queue per tenant and a round-robin
rotation of tenants. Only one job executes at a time. A construction ingest
dispatch processes one configured chunk batch, commits its graph effects and
job checkpoint, then yields. If work remains, the job returns to the front of
its tenant queue while that tenant moves to the back of the rotation. This lets
another ready tenant run between durable ingest checkpoints without weakening
M16's at-least-once recovery contract.

Evaluation currently runs as one dispatch because it has no intermediate
durable checkpoint. Fairness is therefore checkpoint-granular, not a CPU-time
preemption guarantee. FIFO order is preserved within a tenant; there are no
priorities, deadlines, or multiple worker pools. Suspension, deletion,
shutdown, cancellation, and restart continue to park or recover work only at a
durable boundary.

Execution occupancy is owned outside the joined worker task. If a worker
panics after claiming a job, the dispatcher clears the running gauge and tenant
marker, attempts a durable failed transition, and finishes scheduler cleanup
before dispatching more work. A successful terminal write releases the active
slot. If storage prevents terminalization, health degrades and the slot remains
conservatively occupied; it is not silently reused. A panic therefore cannot
leave suspension or recovery waiting on a stale running tenant.

### D5. Archive copy-first and never purge implicitly

`POST /api/admin/jobs/archive` is an explicit, Admin-scoped retention
operation. `COGNIGRAPH_JOB_RETENTION_SECS` supplies the default eligibility age
(30 days), and `COGNIGRAPH_JOB_ARCHIVE_BATCH_SIZE` supplies the default batch
size (100). Operators may provide an explicit cutoff, bounded limit, cursor,
reason, and `dry_run`. A request processes no more than 1,000 eligible records
and no more than the bounded scan budget.

For each eligible terminal job, archival is copy-first:

1. stamp `archived_at` and append an attributed `archived` event;
2. insert the complete record in `_cognigraph_job_archive`; an insert conflict
   preserves the existing immutable copy after validating its lineage;
3. update the derived catalog to the archived summary;
4. delete the hot copy.

This order is deliberate for backends without cross-collection transactions.
A stop after the archive copy can leave a duplicate, but cannot lose the
authoritative record. Detail and lifecycle lookup prefer the archive while the
duplicate exists. Reconciliation removes the hot copy only when it is still
terminal and its immutable identity, input, execution payload, outcome, and
pre-archive history match the valid marked archive. A retried, divergent,
nonterminal, unmarked, or unrelated duplicate is an error, not something
silently normalized or deleted.

M17 has no purge endpoint and no background deleter. The retention setting is
an eligibility default for an operator action, not authorization to destroy
history. Physical deletion of archived job evidence requires a future,
separately governed policy.

### D6. Reconcile every derived structure from authoritative records

`POST /api/admin/jobs/reconcile` performs a bounded, cursor-resumable pass with
`dry_run` support. Its phases are `live`, `archive`, and `catalog`:

- live and archive phases create or update missing or stale catalog summaries;
- a lineage-equivalent terminal archive/hot duplicate is resolved in favor of
  the archive; divergent duplicates fail closed;
- the catalog phase deletes orphan summaries and repairs mismatches;
- catalog rows are checked against their current schema, embedded archive
  marker, tenant incarnation, and canonical derived key. Malformed scoped keys
  cannot truncate a scan: listing skips them and degrades health, while applied
  reconciliation repairs a canonical row from authoritative storage and
  deletes the alias;
- cursors are bound to the tenant incarnation, dry-run mode, and phase;
- a batch limit must be 1 through 1,000.

The operation holds the job transition lock for each bounded slice so a repair
does not race an in-process state transition. A complete non-dry-run pass
clears catalog degradation; a partial or dry-run pass leaves the health signal
intact. Startup recovery, tenant resume, and snapshot import run full
reconciliation before rebuilding and releasing queue state. Startup aborts
instead of binding HTTP if tenant enumeration or authoritative job recovery
fails.

The catalog is never used to decide whether a job should execute, whether a
tenant is at quota, or whether an audit event occurred. Those decisions always
come from `_cognigraph_jobs` and `_cognigraph_job_archive`.

### D7. Give operators bounded status, metrics, and readiness signals

The Admin API and CLI expose:

- `GET /api/admin/jobs/status` / `cognigraph job queue-status`: tenant and
  global active counts, ready/running state, pause/shutdown state, effective
  limits, retention/archive configuration, catalog readiness, and the last
  reconciliation result;
- `POST /api/admin/jobs/reconcile` / `cognigraph job reconcile`: bounded
  inspection or repair;
- `POST /api/admin/jobs/archive` / `cognigraph job archive`: bounded dry-run or
  copy-first archival.

`GET /health/jobs` returns 503 when collection, authoritative data, or catalog
maintenance has a recorded error. Catalog degradation is visible even though
it does not corrupt the authoritative job state.

`GET /metrics` retains bounded-cardinality labels and adds tenant/global
backpressure counters, dispatch count, archive count, catalog upsert/delete
repair counts, reconciliation count, active-state gauges, scheduler entries,
and ready-tenant count. Tenant names, job IDs, user IDs, idempotency keys, and
error text are never metric labels.

## Acceptance matrix

| Area | Acceptance contract | Code status | Release evidence |
|---|---|---|---|
| Authoritative storage | Hot and archive records preserve complete job input, outcome, and audit history; catalog loss cannot lose a job | Implemented | Archive/snapshot regressions plus native and Arango restart probes passed |
| Cursor listing | Newest-first, tenant/filter-bound cursor pages are bounded to 2,048 catalog rows; `total` is null | Implemented | Shared after-key contracts and native/Arango v2 cursor probes passed |
| Offset compatibility | Explicit offset remains capped at 10,000, exact-total, non-archive, and deprecated | Implemented | Route regressions and native exact-total compatibility probe passed |
| Quotas | Global and effective per-tenant active limits reject new admission with HTTP 429 and `Retry-After` while idempotent replay remains available | Implemented | Race/suspension/retirement regressions and native saturated replay probe passed |
| Fair scheduling | FIFO within a tenant and round-robin between tenants at durable ingest checkpoints | Implemented | Two-tenant long-ingest/short-evaluation regression passed |
| Archival | Dry-run and applied bounded passes use archive-copy, catalog-update, then hot-delete; archived detail and replay survive | Implemented | Lineage/conflict regressions and both release-binary probes passed |
| No purge | No API, timer, or retention configuration deletes archive records | Implemented | Source/route audit passed; native purge request returned HTTP 404 |
| Reconciliation | Bounded live/archive/catalog phases repair summaries, delete orphans, and resolve safe duplicates | Implemented | Alias/duplicate regressions and three-phase live passes succeeded |
| Recovery | Reconciliation and authoritative active-set rebuild precede resumed admissions and scheduling | Implemented | Startup/recovery regressions and native/Arango process restarts passed |
| Operations | Admin status, CLI commands, fixed-label metrics, and `/health/jobs` expose queue and catalog state | Implemented | API, release CLI, metrics, and health captures passed |
| Backend portability | Native resident, native paged/redb, and ArangoDB implement stable projected after-key scans | Implemented | Shared contracts plus persistent-native and live-Arango probes passed |
| Deployment boundary | One process-local dispatcher and one writer; no replicas, leases, or HA claim | Explicitly preserved | Code and documentation review passed |

## Consequences and limits

- The catalog adds write amplification at every visible job transition. This is
  accepted because it keeps list payloads light and is explicitly repairable.
- Filtered cursor pages are bounded, not guaranteed full. A caller must follow
  `next_cursor` until it is null.
- Archive storage still grows. M17 separates hot operational state from durable
  history but does not define legal or product retention for destructive purge.
- Fairness reduces tenant monopolization at ingest checkpoints. A single long
  evaluation or one large configured ingest batch can still delay others.
- Queue quotas bound accepted nonterminal work; they do not bound terminal hot
  history. Operators must run the explicit archive workflow.
- Recovery and reconciliation may scan all authoritative history over multiple
  bounded slices before a tenant resumes. This favors correctness over instant
  activation.
- ArangoDB remains a maintenance-mode backend. It can persist, list, archive,
  and reconcile evaluation jobs, but construction ingest still records a
  capability failure because ArangoDB does not advertise atomic graph batches.
- M17 remains single-node by design. Multiple server processes over one redb
  store are unsupported, and the process-local dispatcher is not a distributed
  scheduler. Durable recovery is not high availability: a stopped singleton
  makes no progress until its exclusive replacement starts.

## Verification evidence (release gate passed 2026-07-17)

The final build and workspace gates passed:

```sh
cargo build --release -p cognigraph-server -p cognigraph-cli
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
```

Focused server verification reported 122 passing tests. The suite includes
global/per-tenant capacity release, suspended-capacity reconstruction,
quota-lowering linearization, two-tenant checkpoint fairness, worker-panic
cleanup, archive lineage conflicts, copy-first duplicate recovery, malformed
and aliased catalog keys, punctuation-safe v2 reconciliation cursors, snapshot
validation, and tenant quota merge contracts. Shared backend tests exercise the
projected exclusive after-key primitive in native resident, native paged/redb,
and ArangoDB implementations.

The release server was then run against a fresh persistent native redb store
with ingest batch size 1 and both active limits set to 1. A 1,000-chunk ingest
was accepted, and an exact in-flight replay returned HTTP 200 with the original
job while another submission returned HTTP 429, `Retry-After: 1`, and the
global backpressure metric incremented once. After the ingest and three
evaluations completed, a limit-2 cursor walk emitted v2 cursors and returned
all four summaries with `total: null`; the explicit offset path reported exact
total 4. Direct catalog access returned HTTP 403. Metrics used only fixed
labels, and operator status reported zero active work and a ready catalog.

Native archival dry-run reported four eligible and zero changed; apply reported
four archived. Archived-only listing and attributed detail remained available,
the original submission replayed, retry returned HTTP 409, and the nonexistent
purge route returned HTTP 404. Reconciliation completed its live, archive, and
catalog phases in three requests. After a real process stop and restart, all
four archived summaries and full detail remained readable and `/health/jobs`
returned HTTP 200. The temporary redb stores, logs, and test harness were moved
to Trash after the assertions.

The `.env` ArangoDB credentials returned HTTP 200 for the configured database
(ArangoDB 3.12.9-1 Enterprise). The account returned HTTP 401 for `_system`, so
the test first proved that all three durable-job collections were absent rather
than mutating an unknown queue, then used unique keys in that clean configured
database. A release-server evaluation succeeded, emitted a v2 cursor, archived
with dry-run 1/apply 1, replayed after archival, rejected retry with HTTP 409,
completed three-phase reconciliation, and survived a process restart. The
release CLI reported zero active jobs and ready catalog health. A subsequent
ingest was durably accepted and then failed with progress 0 because ArangoDB
does not advertise atomic batches; the `chunks` collection state was unchanged.

Cleanup deleted only the captured hot job ID, archive job ID, two matching
catalog summaries, and unique test space. Counts for all durable-job
collections were verified as zero, the test space returned HTTP 404, and the
test-created empty `semantic_neurons` collection was removed. The account could
not drop underscore-prefixed collections (HTTP 403), so the three empty
protected collections remain initialized; no test records remain. Test logs
and the harness were moved to Trash. No credential value was printed or stored
in the repository.

## Successor milestone at M17 close: Evaluation Promotion Gates

This successor was delivered and verified as M18 on 2026-07-18. At M17 close,
the requirement was that **Evaluation Promotion Gates** must not turn an
evaluation score into an unaudited deployment toggle. A candidate becomes
promotable only when one durable evidence bundle contains all of the following:

1. **Candidate and configuration identity.** Persist a canonical digest of the
   candidate ontology/rule set/model artifact and a digest of every effective
   construction and evaluation configuration value.
2. **Revision identity.** Pin the corpus revision, graph or snapshot revision,
   and oracle revision independently. A result with an unpinned or incompatible
   revision is not promotable.
3. **Baseline linkage.** Link the candidate evaluation to an immutable baseline
   evaluation record, including its candidate/configuration digests and the
   exact comparable revision tuple.
4. **Versioned policy.** Record the promotion-policy version and resolved
   thresholds. Changing policy creates a new decision context; it does not
   reinterpret an old approval.
5. **Separate recall and restraint gates.** Expected-fact recall and restraint
   against forbidden, reversed, unsupported, or hallucinated facts must pass
   independently. A high recall score cannot average away a restraint failure,
   and vice versa.
6. **Denominators and reproducibility.** Persist exact numerators,
   denominators, exclusions, reviewed-case union, seeds where applicable,
   provider/model versions, commands, and artifact links sufficient to rerun
   the result. Headline percentages without denominators cannot promote.
7. **No oracle leakage.** Construction consumes only the declared document
   inputs. Oracle material is evaluation-only. The promotion record must carry
   a provenance check showing that oracle paths, labels, and expected outputs
   did not enter candidate construction or tuning.
8. **Attributed decisions.** Approve, reject, and rollback are explicit durable
   transitions with actor, role, timestamp, reason, and evidence bundle. An
   automated gate may recommend or block; it must not erase human or policy
   attribution.
9. **Durable promotion record.** Persist the candidate, prior promoted
   revision, evidence links, policy result, decision events, and rollback
   target as an inspectable tenant-scoped record. Promotion and rollback must
   be idempotent and restart-safe rather than transient process state.

Missing evidence, a failed independent gate, revision mismatch, suspected
oracle leakage, or an unattributed decision blocks promotion. This successor
may use the M17 durable queue and governance primitives, but no promotion job,
policy schema, or promoted-state mutation is claimed by M17.
