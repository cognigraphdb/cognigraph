# Decision: M16 durable governed operations

**Status:** Implemented and verified (2026-07-17).

## Context

M15 made construction revisions atomic and truthful once a request reaches the
native backend, but a long-running HTTP request is still the unit of operation.
If the process stops, the caller loses its execution state and cannot tell
whether work committed before the connection disappeared. Repeating the request
also lacks a durable idempotency contract and a durable account of who started,
canceled, or retried the work.

The problem matters most for governed construction. Ingest may process a large
document corpus, while evaluation must remain tied to the exact accepted rules
and specification submitted by the caller. Both operations need to survive an
ordinary process restart without weakening the tenant and authorization
boundaries established by M15.

## Decision

### D1. Make governed operations durable tenant-local jobs

M16 introduces jobs for two operation kinds:

- `construct.ingest` stores the submitted chunks plus a frozen construction
  configuration: the space type, accepted neurons, and ingest batch size.
- `construct.evaluate` stores the submitted evaluation specification and
  persists its result.

Each job is one document in the tenant store's protected
`_cognigraph_jobs` collection. The current state and append-only transition
history are embedded together, so every state change and its audit event are
one document replacement even on backends without multi-document batches. The
collection is inaccessible through public document, CGQL, Lua, traversal,
batch, and backend-native query surfaces. It is included in the tenant's native
snapshot and file backup because it is operational source data, not a
rebuildable cache.

The qualified name is intentional. Live ArangoDB verification found that
Arango owns a built-in `_jobs` system collection; CogniGraph must never reuse
or hide that database-internal queue. `_cognigraph_jobs` was absent before the
probe, is created with `isSystem: true`, and was removed again after the test.

The job identity includes the tenant incarnation as well as the tenant name.
Deleting and recreating a tenant with the same name therefore cannot expose or
resume the deleted tenant's work. The worker captures tenant identity when it
accepts a job and runs the complete background future inside that tenant scope;
it never depends on request-local context after submission.
The non-deletable `default` tenant keeps the stable incarnation `default` even
after an explicit control record is created, so ordinary administration does
not hide its existing jobs.

The job repository itself uses ordinary conflict-detecting document CRUD and
therefore works with native and maintenance-mode ArangoDB. Operation
capabilities remain honest: `construct.evaluate` runs and persists on Arango,
while `construct.ingest` becomes a durable failed job before graph writes
because Arango does not advertise the atomic-batch contract construction
requires.

### D2. Require idempotent submission

`POST /api/jobs` requires an `Idempotency-Key`. The durable job id is derived
from the tenant incarnation and a hash of that key. Canonical `{kind,input}` is
hashed separately, making one key exclusive across operation kinds:

- repeating the key with byte-equivalent canonical input returns the existing
  job and does not enqueue a second execution;
- repeating the key with different input returns HTTP 409;
- the same key in another tenant or tenant incarnation is unrelated.

The record with its first embedded audit event is committed before the API
acknowledges submission. A successful response therefore always names a job
that can be inspected after restart.

### D3. Recover at least once from durable checkpoints

One in-process worker executes jobs through a fair, single-permit queue. A
`construct.ingest` job checkpoints only at chunk-batch boundaries. Each batch
uses the M15 atomic occurrence-replacement path; after a crash between the graph
commit and the checkpoint commit, recovery may run that batch again. Re-running
the same frozen batch is safe, so the observable contract is **at least once
with idempotent effects**, not exactly once.

At startup, jobs left `running` by an unclean stop are moved back to `queued`, a
recovery event is appended, and execution resumes from the last durable
checkpoint. Already queued work is also rescheduled. A graceful shutdown stops
accepting new work, lets a batch boundary become durable, and returns unfinished
work to a recoverable state before the process exits.

`construct.evaluate` is read-only with respect to the graph and persists its
result as one terminal transition. A recovered evaluation may run again because
the process can stop after evaluation but before persisting the terminal
record.

### D4. Make cancellation and retry explicit, cooperative transitions

The initial states are `queued` and `running`; terminal states are `succeeded`,
`failed`, and `canceled`. Cancellation is cooperative:

- a queued job can become `canceled` immediately;
- a running ingest job becomes `cancel_requested` and stops at its next durable
  batch boundary;
- a running evaluation observes cancellation before committing its terminal
  result;
- cancellation never rolls back construction batches already committed.

Only a terminal `failed` or `canceled` job can be retried. Retry requires its
own idempotency key and records the requesting actor, reason, mode, and attempt.
Resume mode keeps the last checkpoint; restart mode resets progress and safely
replays the frozen input. Retrying does not mutate or replace the original
audit events. The retry key is bound to the canonical mode/reason request;
reusing it with changed instructions is a conflict rather than a silent replay.

Tenant suspension prevents new submissions and causes active work to stop at a
safe boundary. Tenant deletion fences and drains that tenant's jobs before its
store is quarantined. Snapshot import is rejected while the target tenant has
active jobs, avoiding replacement of data underneath a worker.

### D5. Preserve an attributable audit trail and narrow API authority

Each job record stores the submitting actor, immutable submitted input, an
internal frozen execution payload, input digest,
attempt and recovery counts, timestamps, progress, result or structured error,
and current state. Each transition appends an event with actor, reason, attempt,
and previous/new state. Worker-created transitions use an explicit system
actor; they do not impersonate the original request.

The M16 API is tenant-scoped:

- `POST /api/jobs` submits a job;
- `GET /api/jobs` lists bounded summaries for the caller's tenant (maximum
  limit 200 and offset 10,000); immutable payloads and events remain on the
  detail endpoint;
- `GET /api/jobs/{id}` returns status, immutable input, result/error, and audit
  events;
- `POST /api/jobs/{id}/cancel` requests cancellation;
- `POST /api/jobs/{id}/retry` retries eligible terminal work.

Submission, cancellation, and retry require graph-write authority. Listing and
status require graph-read authority. Cancel/retry additionally require the
original owner or an Admin. A job id from another tenant returns the same 404 as
an unknown id.

The CLI mirrors the API with `cognigraph job submit`, `list`, `status`,
`cancel`, and `retry`. Synchronous construction endpoints remain available for
small interactive work; M16 does not silently turn existing requests into jobs.

### D6. Expose bounded-cardinality operational metrics

`GET /metrics` exposes aggregate job counters and the current active-job gauge:
submissions, idempotent replays, starts, checkpoints, successes, failures,
cancellations, cancellation requests, retries, and restart recoveries. Metrics
use a fixed label set only. Tenant ids, user ids, job ids, idempotency keys,
space ids, and error text never become labels; detailed diagnosis belongs in
the tenant-scoped status and event records.

## Consequences and limits

- Job durability follows the tenant store. In-memory native mode provides the
  API and execution semantics for development but cannot recover jobs across a
  process restart; production recovery requires `COGNIGRAPH_NATIVE_PATH` or
  `COGNIGRAPH_DATA_DIR`.
- The single worker respects the product's singleton-writer boundary and
  keeps checkpoint ordering simple. It is not a distributed scheduler, and
  multiple server processes over one redb file remain unsupported.
- Durable jobs improve restart behavior; they do not provide high availability.
  A stopped singleton makes no progress until that process (or its cold
  replacement with exclusive access to the store) starts.
- Maintenance-mode ArangoDB durably stores job records and evaluation results;
  it still cannot execute construction ingest because it lacks atomic graph
  batches. This is a capability failure recorded on the job, not a partial
  write.
- At-least-once recovery is intentionally visible. Downstream side effects are
  out of M16 scope; the two foundation job kinds operate only on idempotent
  construction state or a persisted evaluation result.
- Queue fairness is process-local. There are no priorities, cron schedules,
  distributed leases, replicas, or cross-tenant worker pools in this milestone.

## Verification evidence

The exact workspace gates passed after the final implementation:

```sh
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
```

Release-binary HTTP verification used a fresh persistent native redb store and
`COGNIGRAPH_JOB_INGEST_BATCH_SIZE=1`:

- New evaluation submission returned `202`; the identical key and canonical
  input replayed the same id with `200`; changed input returned `409`.
- A 5,000-chunk ingest was forcibly stopped with `SIGKILL` while `running` at
  checkpoint 184. Restart logged one recovered job; its record showed
  `attempt=2`, `recoveries=1`, and the event sequence
  `submitted, started, recovered, started, succeeded`.
- The recovered result reported 5,000 completed chunks and 5,000 grounded
  facts. The live `facts` collection contained exactly 5,000 restart-probe
  occurrences and 5,000 unique provenance identities: replaying the uncertain
  batch did not duplicate effects.
- A queued job canceled immediately with a `canceled` audit event, then an
  idempotent resume retry produced `retry_requested` and succeeded. Reusing
  that retry key with a changed mode/reason returned `409`. A separate failed
  evaluation retried after its missing graph input was constructed and advanced
  from attempt 1 to attempt 2.
- `GET /api/jobs` returned projected summaries without input or events, while
  invalid `limit=0` and `offset=10001` requests returned `400`.
- `SIGTERM` during active ingest recorded `interrupted` at a safe boundary;
  startup rescheduled the queued job and it completed on attempt 2 without
  being counted as an unclean recovery.
- `/health/jobs` returned ready, `_cognigraph_jobs` returned `403` through the
  document API, and `/metrics` exposed only fixed outcome/status labels. After
  restart, `cognigraph_job_recoveries_total` was 1 and no tenant, actor, job id,
  space, or idempotency value appeared in labels.
- Multi-tenant tests execute the worker against an explicit `TenantScoped`
  backend and prove that tenant `acme` writes neither tenant `other` nor the
  implicit `default` store. Tenant incarnation, suspension/deletion drain, hot
  import fencing, cross-tenant 404, and owner-or-Admin mutation checks are
  covered in the server/auth suites.

The supplied database-scoped Arango credentials returned HTTP 200 on the live
database endpoints (the root endpoint correctly returned 401 for that scoped
user). A release server created `_cognigraph_jobs` as an Arango system
collection, persisted and replayed an evaluation across a server restart, and
executed the projected job-list query successfully. It recorded an ingest
failure stating that atomic batches are unsupported; the probe chunk remained
absent (`404`). The probe space document and CogniGraph job collection were
then removed; Arango's built-in `_jobs` remained intact.
