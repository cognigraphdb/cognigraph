# Construction, drafting, and side-view requests

These synthetic request files match the current OpenAPI and are exercised by
the CG-22 regression. Run commands from the repository root against a disposable
Native server. Configure the main completion provider for directed/draft calls;
side-view generation also requires its completion lane and an embedding provider.
With authentication enabled, use an editor/admin bearer token:

```bash
export COGNIGRAPH_URL=http://127.0.0.1:3000
# Set COGNIGRAPH_TOKEN through your normal login/token workflow.
```

## Directed construction

```bash
curl -sS "$COGNIGRAPH_URL/api/construct/directed" \
  -H "Authorization: Bearer $COGNIGRAPH_TOKEN" \
  -H 'Content-Type: application/json' \
  --data-binary @docs/examples/construction/directed.json
```

[directed.json](directed.json) supplies the required space, relation taxonomy,
restraint phrases, and chunks. The route accepts 1–32 chunks and makes one
completion call. It uses the normal 2 MiB HTTP body ceiling and configured request
timeout (30 seconds by default); there is no directed job kind. Successful
reconciliation replaces all earlier occurrences for every supplied chunk,
preserving other chunks. Valid empty output or all-rejected proposals replaces
with zero. A malformed model envelope or provider failure preserves occurrence
data, although a newly auto-created rule-less space can remain. Active governed
deployments reject this legacy route before model calls.

Response fields are `space_type`, `chunks`, `proposed`, `facts_grounded`, `skips`,
and `extracted_by`. Text is NFC before evidence hashes and spans. These gates
constrain grounding; this request is not a benchmark or model qualification.
Changing the requested taxonomy does not isolate replacement to those relations.

## Four durable job kinds

The files below are complete `/api/jobs` envelopes:

| File | Kind | Prerequisite |
|---|---|---|
| [ingest-job.json](ingest-job.json) | `construct.ingest` | Accepted `cg22-demo` space; applies its accepted rules/neurons |
| [evaluate-job.json](evaluate-job.json) | `construct.evaluate` | Existing graph to evaluate; inline spec is frozen, graph is read on execution |
| [draft-job.json](draft-job.json) | `construct.draft` | `cg22-draft-job` must be a new space; main completion provider |
| [sideviews-job.json](sideviews-job.json) | `sideviews.generate` | Ordinary `cg22-notes` source plus side-view completion and embedding providers |

```bash
curl -sS -i "$COGNIGRAPH_URL/api/jobs" \
  -H "Authorization: Bearer $COGNIGRAPH_TOKEN" \
  -H 'Content-Type: application/json' \
  -H 'Idempotency-Key: cg22-draft-job-1' \
  --data-binary @docs/examples/construction/draft-job.json

cognigraph job status JOB_ID
cognigraph job list --kind construct.draft --status succeeded
```

To exercise another row, change both the file and idempotency key. A space
auto-created by directed construction has no rules, so ordinary rule ingestion
into it grounds zero and withdraws the supplied chunk's prior directed facts.
Use an accepted authored rule space when testing rule-based grounding.

New submissions return 202 with `Location: /api/jobs/{id}`; the same canonical
`{kind,input}` and key returns 200 with `replayed: true`. Changed reuse returns
409. Omitted defaults and explicit defaults are different submitted inputs.
Jobs list/detail require graph-read; submission requires graph-write, including
evaluation jobs. Cancellation/retry also require ownership or admin role.

CLI `job submit` takes the **input only**, not the full envelope:

```bash
cognigraph job submit sideviews.generate \
  '{"collection":"cg22-notes","count":12}' --idempotency-key cg22-cli-sideviews-1
```

Input is capped at 16 MiB, plus a 64 KiB HTTP envelope allowance. Ingest jobs
allow at most 100,000 chunks. Draft jobs allow at most 2,000 trimmed-title groups
and always draft per document; they accept `space_type`, `chunks`, `sample_cap`
only. Source keys for side-views are capped at 10,000. Persistence is required
for job restart recovery. Read the [operator reference](../../operations/jobs.md#durable-governed-jobs-m16-m17)
for checkpointing, cancellation, retry, and archive semantics.

## Async draft convenience route

```bash
curl -sS -i "$COGNIGRAPH_URL/api/construct/draft" \
  -H "Authorization: Bearer $COGNIGRAPH_TOKEN" \
  -H 'Content-Type: application/json' \
  -H 'Idempotency-Key: cg22-draft-route-1' \
  --data-binary @docs/examples/construction/draft-async.json
```

[draft-async.json](draft-async.json) requests durable work. The wrapper removes
`async` and `per_document` before submitting `construct.draft`; those fields
are rejected in generic job input. Async mode always groups by trimmed title,
including one shared group for untitled chunks. It returns the job envelope,
not a draft. The accumulator remains `drafting` and unacceptably incomplete
until status `draft` is published. Review the artifact before explicitly using
`cognigraph draft accept SPACE_ID`. Accepting a draft is not part of this example.

Omit `async` or set it false for a synchronous result; `per_document` then selects
grouping. Each document costs two completion calls, so prefer the durable path
for corpora. `sample_cap` omitted/null defaults to 40; zero still samples one
chunk. Self-checks always inspect the full supplied corpus.

## Side-view convenience route

Seed one synthetic ordinary source, then submit:

```bash
cognigraph doc put cg22-notes \
  '{"_key":"note-1","text":"Alpha supplies Beta."}'

curl -sS -i "$COGNIGRAPH_URL/api/sideviews/generate" \
  -H "Authorization: Bearer $COGNIGRAPH_TOKEN" \
  -H 'Content-Type: application/json' \
  -H 'Idempotency-Key: cg22-sideviews-1' \
  --data-binary @docs/examples/construction/sideviews.json
```

[sideviews.json](sideviews.json) is the same input as the generic job kind.
Keys retain exact Unicode identity and are frozen at submission, while text and
providers are read during execution. `text_field` is a top-level field; blank,
null, or omitted selects `text`. Missing/blank/non-string text is skipped.
`count` is a target: omitted/null means 12, nonnegative integers are clamped to
1–50, and model output may contain fewer pairs. Negative/fractional values fail.

Existing rows cause a skip unless `regenerate: true`. Regeneration replaces
per-parent rows atomically after successful generation and embedding; failed
provider work keeps earlier rows. Deletion/recreation during provider work
invalidates publication. Ordinary source updates require explicit regeneration.
No system, governed, generated, or slash-containing source collection is eligible.

Use `COGNIGRAPH_SIDEVIEWS_PROVIDER` / `COGNIGRAPH_SIDEVIEWS_MODEL` to choose a
separate lane; otherwise both inherit the main completion settings. An explicit
side-view provider uses its own default model unless overridden. Current defaults
are OpenAI `gpt-5.6-luna` and Gemini `gemini-3.8-flash`; the source of truth is the
[operator configuration table](../../operations/configuration.md#configuration-reference).
Dedicated OpenAI review judges share the validated `OPENAI_BASE_URL` and
`OPENAI_API_KEY`, even when the main completion provider is Gemini
([CG-36](../../issues/CG-36.md)). A nonempty dedicated model without a nonempty
OpenAI key fails startup; unset/blank models retain primary fallback or an
absent partner. Endpoint configuration does not change model qualification.

## Verification

The checked-in JSON examples, all four job input kinds, schema enums, directed
bounds, and wrapper/default semantics are covered by
`cargo test -p cognigraph-server openapi_drift`. The release harness validates
the served OpenAPI with a standard validator, validates requests/responses
against it, and runs synthetic HTTP and CLI examples with loopback providers:

```bash
cargo build --release -p cognigraph-server -p cognigraph-cli
uv run docs/issues/evidence/api-contract-http.py --output /tmp/cg22-api.json
```

The script declares its Python validation dependencies. It creates its own
disposable store and local providers, and does not use repository credentials.
See the [CG-22 verification report](../../issues/api-contract-2026-09-09.md).
