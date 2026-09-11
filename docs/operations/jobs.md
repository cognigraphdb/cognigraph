# Jobs

## Durable governed jobs (M16-M17)

Durable jobs cover four kinds. Every generic submission is `{kind,input}`;
unknown envelope/input fields are rejected. The accepted input shapes differ:

| Kind | Input and submission bounds | Frozen execution data and behavior |
|---|---|---|
| `construct.ingest` | `space_type`, 1–100,000 `chunks` | Resolved space and accepted neurons plus supplied chunks; atomic per-checkpoint reconciliation. Needs an existing space and Native atomic batches. |
| `construct.evaluate` | `space_type`, optional `eval`, optional `promotion_context` | Resolved EvalSpec/context; `eval.space_id` must match. Omitted/null eval uses the stored spec. Ordinary evaluation reads graph facts during execution; M21–M23 verify and freeze their artifact-derived inputs under the separate governance contract. |
| `construct.draft` | New `space_type`, nonempty `chunks`, optional `sample_cap` (40) | Chunks grouped by trimmed title, at most 2,000 groups; untitled chunks share one group. Always per-document, two main-provider completion calls per group. The accumulator is inert and cannot be accepted while `drafting`. |
| `sideviews.generate` | Ordinary `collection`, optional `text_field` (`text`), `count` (12), `regenerate` (false) | At most 10,000 exact source keys; text and configured providers are read when work executes. Requires side-view completion, embedding, and Native atomic batches. |

All jobs cap canonical submitted input at 16 MiB; `/api/jobs`,
`/api/sideviews/generate`, and `/api/construct/draft` allow a 16 MiB + 64 KiB
HTTP body for the envelope. Other construction routes use the normal 2 MiB
body limit. HTTP size failures return 413; invalid typed JSON is 422 and
semantic job-input validation is 400. Capacity failures return 429.

Submission requires graph-write authority and a 1–128 character printable-ASCII
`Idempotency-Key`. The same key plus the same canonical submitted `{kind,input}`
returns the existing live or archived job (200, `replayed: true`); changed reuse
returns 409. New work returns 202 and `Location: /api/jobs/{id}`. Idempotency is
scoped to the tenant incarnation across all four kinds and convenience routes.
It does not treat omitted defaults as identical to explicit defaults. Listing
and status require graph-read authority. Cancel and retry require graph-write
authority and either original ownership or Admin. Persistent storage is required
for restart recovery; completed work is not rolled back by cancellation.

The tenant-scoped lifecycle and Admin operations are:

```text
POST /api/jobs                       submit {kind, input}
GET  /api/jobs                       bounded cursor summaries (default)
GET  /api/jobs/{id}                  detail, result/error, and audit events
POST /api/jobs/{id}/cancel           cooperatively cancel live work
POST /api/jobs/{id}/retry            resume or restart eligible live work
GET  /api/admin/jobs/status          tenant queue, limits, and catalog state
POST /api/admin/jobs/reconcile       inspect or repair catalog/archive drift
POST /api/admin/jobs/archive         copy terminal records out of the hot queue
POST /api/tenants/{name}/admin       HostAdmin provisions the first tenant Admin
POST /api/tenants/{name}/quotas      HostAdmin quota update
```

### Directed construction and asynchronous drafting

`POST /api/construct/directed` is synchronous, with a required `space_type`,
nonempty `taxonomy`, and 1–32 chunks per request. Each taxonomy entry supplies
`relation`, `description`, and nonempty `require_in_sentence` vocabulary.
Relations must be nonblank and unique after trimming; vocabulary must include
at least one nonblank phrase. Unknown top-level fields fail. It needs graph-write
scope and the main completion provider, and sends one completion request over
all chunks. It uses the normal 2 MiB body ceiling and
`COGNIGRAPH_REQUEST_TIMEOUT_SECS` (30 seconds by default); split larger corpora
into consecutive requests. There is no `construct.directed` job kind.

A missing space is created as an accepted, rule-less space before completion;
that space can remain after a later failure. Facts pass taxonomy, quote,
endpoint, and affirmative-restraint gates. Text is NFC before hashes and spans;
quote matching also tolerates ASCII case and whitespace runs. A valid result
atomically replaces **all occurrences for each supplied chunk**, even if no
proposal survives or the model returns `{"facts":[]}`. It is not append-only or
restricted to replacing the requested taxonomy's relations. Other chunks keep
their evidence. Malformed/failed completions do not replace occurrences.
Model output may vary between calls, so request replay is not a frozen result.
Active governed deployments fence this legacy writer before provider calls;
use the governed workflow for deployed generations. See the
[construction examples](../examples/construction/README.md).

`POST /api/construct/draft` defaults to synchronous drafting; `per_document: true`
groups by trimmed title. `async: true` requires `Idempotency-Key`, always groups
per document, and returns a `construct.draft` job envelope (202 new, 200 replay).
The generic job input accepts neither `async` nor `per_document`: the wrapper
removes those controls. `sample_cap` omitted/null defaults to 40; zero samples
one chunk, while checks still use the full corpus. Async mode bounds groups at
2,000 and checkpoints into `space_type_drafts/{space_type}`. Its status remains
`drafting` until the final `draft` result can be reviewed and explicitly accepted.
Provider/model configuration is not pinned by a job: restarting with changed
settings can change later attempts. Settings are resolved at startup, with no
environment-based hot reload. Use durable drafting for multi-document
corpora that can exceed the HTTP deadline.

### Side-view generation and configuration

`POST /api/sideviews/generate` is a convenience submission for
`sideviews.generate`, with the identical input and idempotency namespace as
`POST /api/jobs`. The source collection must be ordinary; system, governed,
generated, and slash-containing collection names are ineligible. Source
collection names, keys, and parent handles preserve exact Unicode identity.
Missing/deleted parents and absent, non-string, or blank text are skipped.
`text_field` selects a top-level field; omitted/null/blank selects `text`.
`count` omitted/null defaults to 12; nonnegative integers are accepted and
clamped to 1–50. It is a requested count, not a guarantee of that many model
pairs. Negative/fractional counts fail input validation.

Existing rows cause a parent to be skipped unless `regenerate: true`. Regeneration
atomically replaces a parent's rows after successful completion/embedding;
provider failure preserves its earlier rows. Only source keys are frozen, not
text, completion model, or embedding model. Newly added documents are outside
that job. Deletes cascade through the shared lifecycle; deleting/recreating a
source during a provider call rejects stale publication, and retry uses the
current source. Ordinary updates require explicit regeneration. Recovery from
old malformed Unicode references follows the [CG-33 repair procedure](reference-repair.md).

The configuration table above covers independent provider/model selection.
For example, set the main lane to OpenAI/Luna and the side-view lane to Gemini
with both provider keys configured. Merely setting `COGNIGRAPH_SIDEVIEWS_MODEL`
keeps the inherited provider. Dedicated review judges use the shared validated
OpenAI endpoint and their explicit models after [CG-36](../issues/CG-36.md).
Their qualification rules remain separate. Side-views remain non-authoritative retrieval aids; hybrid
retrieval must explicitly set `include_side_views: true`.

### Listing and cursor compatibility

`GET /api/jobs` uses cursor pagination unless the request explicitly supplies
`offset`. The default response has `pagination: "cursor"`, `total: null`, a
bounded `scanned` count, and `next_cursor`; feed that cursor into the next
request until it is null. Cursors are opaque, tenant-incarnation scoped, and
bound to the `kind`, `status`, and `archived` filters. Reusing one with changed
filters, another tenant, or a replaced tenant incarnation returns HTTP 400.
Status filtering is a live queue view, not a frozen snapshot, so a concurrent
transition can change which later page contains a job.
Responses emit opaque v2 cursors whose storage position is base64url encoded;
the server still accepts safe v1 cursors created before M17 verification.

The `archived` filter is `exclude` (default), `include` (hot and archived), or
`only`. Supplying deprecated `offset` selects the old exact-total path: it scans
the hot collection, cannot include archived records, and is capped at offset
10,000. `cursor` and `offset` are mutually exclusive.

The CLI mirrors these surfaces. Prefix a JSON file with `@`; without that
prefix the argument itself must be valid inline JSON:

```sh
cognigraph job submit construct.ingest @chunks.json --idempotency-key import-2026-07
cognigraph job submit construct.evaluate @eval.json --idempotency-key eval-2026-07
cognigraph job submit construct.draft @draft-input.json --idempotency-key draft-1
cognigraph job submit sideviews.generate @sideviews-input.json --idempotency-key sideviews-1
cognigraph job list --status running --archived exclude
cognigraph job list --archived include --cursor NEXT_CURSOR
cognigraph job list --offset 500                    # deprecated compatibility path
cognigraph job status JOB_ID
cognigraph job cancel JOB_ID --reason 'superseded source snapshot'
cognigraph job retry JOB_ID --mode resume --idempotency-key retry-1
cognigraph job queue-status
cognigraph job reconcile --dry-run --limit 100
cognigraph job reconcile --limit 100 --cursor NEXT_CURSOR
cognigraph job archive --before-ms 1780000000000 --dry-run --limit 100
```

The equivalent Admin requests expose the same bounded controls and return a
`next_cursor` when another slice remains:

```sh
curl -H "Authorization: Bearer $ADMIN_TOKEN" \
  :3000/api/admin/jobs/status
curl -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H 'content-type: application/json' -X POST \
  :3000/api/admin/jobs/reconcile -d '{"dry_run":true,"limit":100}'
curl -H "Authorization: Bearer $ADMIN_TOKEN" \
  -H 'content-type: application/json' -X POST \
  :3000/api/admin/jobs/archive \
  -d '{"before_ms":1780000000000,"dry_run":true,"limit":100,"reason":"retention review"}'
```

`job queue-status`, `job reconcile`, and `job archive` call tenant-local Admin
endpoints. A HostAdmin deliberately cannot inspect tenant jobs; it can manage
admission policy through the separate TenantAdmin endpoint. For example:

```sh
curl -H "Authorization: Bearer $HOST_ADMIN_TOKEN" \
  -H 'content-type: application/json' -X POST \
  :3000/api/tenants/acme/admin \
  -d '{"username":"acme-admin","password":"use-a-separate-secret"}'
curl -H "Authorization: Bearer $HOST_ADMIN_TOKEN" \
  -H 'content-type: application/json' -X POST \
  :3000/api/tenants/acme/quotas -d '{"max_active_jobs":25}'
```

`POST /api/tenants/{name}/admin` is a narrow bootstrap/recovery operation: it
creates only an `admin` in the named active tenant and returns `409` once that
tenant already has one. After bootstrap, that tenant Admin provisions its
editor/viewer, M19 policy-author/policy-approver/promoter identities, and M20
artifact-attestor identity through `/api/users`; HostAdmin never gains tenant
data or governance scopes.
The CLI equivalent is `cognigraph tenant bootstrap-admin acme acme-admin`
(password from `COGNIGRAPH_PASSWORD` or stdin).

`max_active_jobs` counts `queued`, `running`, and `cancel_requested` jobs in the
tenant incarnation. It may be any non-negative integer up to
`COGNIGRAPH_JOB_MAX_ACTIVE_PER_TENANT`; zero blocks new admission. Sending null
removes the override. Lowering a quota below current usage never cancels or
drops accepted work—it blocks new submissions and retries until the queue
drains. The process-wide `COGNIGRAPH_JOB_MAX_ACTIVE_TOTAL` limit applies in
addition. A capacity rejection is HTTP 429 with `Retry-After: 1`; replaying an
accepted submission or an already applied retry on a live job remains allowed
while full.

Quota merge/write and final job-slot reservation share one transition barrier.
If a lowering request succeeds first, an in-flight candidate must observe the
new limit; if the job reservation succeeds first, that already accepted job is
allowed to drain under the lower quota.

### Fair scheduling and recovery

One process-local dispatcher runs exactly one operation step at a time. Jobs
are FIFO within each tenant. Ready tenants rotate round-robin after each durable
ingest checkpoint, so one large ingest yields the singleton writer after every
configured chunk batch. Evaluation has no intermediate checkpoint and yields
when that evaluation finishes. This is process-local fairness, not a
distributed scheduler.

Ingest recovers **at least once**: a forced stop can replay the last uncertain
batch, whose occurrence-replacement effects are idempotent. Cancellation takes
effect at an ingest batch boundary and does not undo committed construction;
evaluation cancellation is observed before its terminal result checkpoint.
Restart recovery rebuilds active counts and schedules existing durable work
without discarding it merely because a quota was later lowered. Suspended
tenants are also rebuilt into the global active count but remain fenced and
unscheduled. Startup aborts before binding HTTP if tenant enumeration or
authoritative job recovery cannot complete. A worker panic clears stale
running/scheduler state and attempts a durable failed transition; its slot is
released only when that terminal write succeeds, otherwise health degrades and
capacity remains conservatively occupied.

### Catalog reconciliation and archival

The authoritative live record and append-only transition history remain one
protected `_cognigraph_jobs` document, keeping each state/event transition
atomic. The protected `_cognigraph_job_catalog` is a lightweight, repairable
listing index. `POST /api/admin/jobs/reconcile` scans live records, archived
records, and catalog rows in bounded phases; use `dry_run: true` to inspect, and
follow `next_cursor` until null for a complete pass. Safe missing, stale, orphan,
and copy-first duplicate rows are repaired. Catalog schema, archive marker,
tenant scope, and canonical key are validated; malformed scoped aliases are
skipped by listing, degrade `/health/jobs`, and are removed by an applied full
reconciliation. Malformed authoritative records fail closed.

`POST /api/admin/jobs/archive` accepts only terminal records whose
`finished_at` is at or before `before_ms`. If HTTP clients omit `before_ms`, the
server derives the cutoff from `COGNIGRAPH_JOB_RETENTION_SECS`; the CLI requires
an explicit `--before-ms`. Omitted `limit` uses
`COGNIGRAPH_JOB_ARCHIVE_BATCH_SIZE`, and every request is capped at 1000. Use
`dry_run: true` first, then repeat bounded requests or follow the HTTP
`next_cursor` until complete.

Archival is copy-first: it writes the full terminal record and an attributed
`archived` event to protected `_cognigraph_job_archive`, updates the catalog,
then removes the hot copy. Detail lookup, audit history, and idempotency replay
continue to work. Archived records are immutable, so cancel or retry returns
HTTP 409. Retention is an eligibility cutoff, not an automatic deletion timer,
and M17 performs **no purge**; any destructive archive-retention policy is a
separate operator decision.

Do not shorten `_cognigraph_jobs` to `_jobs`: that is an ArangoDB-owned system
collection. Persistent native stores include the live records, catalog, and
archive in snapshots and cold backups. In-memory mode executes the API but
cannot provide restart recovery. Maintenance-mode ArangoDB persists jobs,
catalog, archive, and evaluations; `construct.ingest` becomes a failed job
before graph writes because Arango does not advertise atomic batches.

`GET /health/jobs` reports collection, data, and catalog errors latched by
startup recovery or queue/list/archive/reconciliation operations; it is not a
full catalog scan on every health request. `GET /metrics`
exposes fixed-label lifecycle, backpressure, scheduler, archive, and
reconciliation counters and gauges. Tenant, user, job, idempotency-key, space,
cursor, and error values stay out of labels; use `job queue-status` and the
tenant-scoped event history for detailed diagnosis.

Hot snapshot import atomically fences an idle target tenant; it rejects a
target with nonterminal jobs without interrupting or stranding them, and also
rejects malformed/nonterminal live or archive records. Complete or cancel
active work first. Tenant suspension checkpoints active work and parks queued
work; activation reconciles and reschedules it. Tenant deletion suspends and
drains the worker before quarantining the entire tenant store.

A stopped singleton makes no progress until it restarts. Multiple processes
over one redb file, shared-volume replicas, distributed leases, and HA remain
unsupported.
