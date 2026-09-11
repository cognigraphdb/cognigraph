# Decision: Side-view generation — separate provider config + pre-build model benchmark

**Status:** Implemented end to end (2026-07-20). All five slices are in the tree
and gate-green: foundation (generation contract + provider benchmark), storage
(write-protected `side_views` collection), the durable generation job + batch
endpoint, retrieval fusion, and the delete cascade. Owner: skitsanos. See the
"Implementation" addendum below for the final shape.

## Context

"Side-views" (context expansion) generate 10–20 question/answer pairs per
document or large-enough chunk with an LLM and index them alongside the original
prose. At query time a user's question often matches a generated side-view
question far better than the raw text, lifting retrieval recall — the doc2query /
hypothetical-questions / HyDE / contextual-retrieval family. The user ran this
pattern successfully before and proposed it for CogniGraph (see the
`context-expansion-idea` memory).

Two facts shape the design:

1. **Cost is dominated by the side-view model.** One completion per proposal is
   cheap; 10–20 generations *per document* at ingest is the real spend and the
   real embedding-vector multiplier. So the model used for side-views is the
   dominant cost lever and deserves to be chosen on evidence, not by default —
   consistent with the benchmarks-first convention.
2. **Side-views are a retrieval aid, not governed facts.** They are
   non-authoritative and non-deterministic. Unlike Semantic Neurons fact
   construction (precision-first, punished by LLM noise), retrieval *tolerates*
   noisy extra surfaces — more recall, with precision handled by downstream
   ranking. This is a favorable use of LLM generation, but it must stay
   quarantined from the fact graph.

## Decision

### D1. Side-views are a quarantined retrieval layer, never governed facts

Generated Q&A live in their own provenance-linked layer, separate from the
managed fact collections. They must respect the M25 mutation barrier: nothing
here writes to `neurons`, `facts`, `entities`, `chunks`, etc. This decision doc
covers only the generation *contract*; where the quarantined nodes live and how
retrieval consumes them is a later slice (see Next).

### D2. A separate provider/model axis for side-views, inheriting completions

Completions keep `COGNIGRAPH_COMPLETION_PROVIDER` (`openai` | `gemini`) and
`COGNIGRAPH_COMPLETION_MODEL`. Side-views add a parallel pair so they can run on
a cheaper model independently:

- `COGNIGRAPH_SIDEVIEWS_PROVIDER` (`openai` | `gemini`)
- `COGNIGRAPH_SIDEVIEWS_MODEL`

Resolution (`CompletionConfig`, shared by server startup and the
`completion_from_env`, `sideviews_provider_from_env`, and `provider_named`
harness helpers in `cognigraph-embeddings`; CG-29, 2026-09-08):

- If `COGNIGRAPH_SIDEVIEWS_PROVIDER` is set, use it with
  `COGNIGRAPH_SIDEVIEWS_MODEL` (or the provider default when the model is unset).
- If it is unset, **inherit** the completion provider, and take the model from
  `COGNIGRAPH_SIDEVIEWS_MODEL` if set, else `COGNIGRAPH_COMPLETION_MODEL`.
- Completion selection honors its explicit provider even when both keys exist.
  Otherwise it prefers a nonempty `OPENAI_API_KEY`, then `GEMINI_API_KEY`.
  No keys and no explicit provider disables both optional server lanes;
  harness helpers requiring a provider return an error.
- Invalid explicit providers, including empty strings, or a missing key for
  the selected provider fail server startup before stores are opened. Side-view
  configuration errors are no longer silently treated as a disabled feature.
  Provider names are lowercase, with surrounding whitespace trimmed. Remove
  an override to inherit; do not set it to an empty string.
- Server and harness completion lanes honor `OPENAI_BASE_URL` and
  `GEMINI_BASE_URL`. Selected URLs must be absolute HTTP(S) base URLs without a
  query or fragment; trailing slashes are normalized. Whitespace-only keys are
  absent, and blank models or URLs use provider defaults. An explicit benchmark
  `provider_named` pair uses the provider default when its model is omitted,
  independently of the main completion model.

So the default is "same as completions", and a one-line override moves side-views
onto a cheaper model without touching completion behavior. (Names kept from the
user's proposal; `SIDEVIEWS` reads as the plural feature name.)

CG-29 closure (2026-09-08): the pre-fix release reproduced explicit Gemini
construction being routed to OpenAI. The corrected release passed 13 server
configurations (including completed side-view jobs), 13 invalid-startup checks,
and 3 benchmark runs against synthetic loopback providers. Formatting, Clippy,
and all 888 Rust tests passed. No validation or live check failed; external
provider availability and model quality were not tested. See
[the reproducible routing evidence](../issues/completion-provider-2026-09-08.md).

### D3. Strict JSON schema everywhere — never prose JSON

Every side-view call goes through `CompletionProvider::complete_json` with a
closed, fully-required schema (`{ pairs: [{ question, answer }] }`). That schema
qualifies for OpenAI strict mode and Gemini's native `responseJsonSchema`, so the
shape is enforced server-side. We never ask a model to "return JSON" in prose —
schema-constrained output is measurably more reliable (the 2026-07-07 cross-family
run showed ~14% malformed first attempts from prompt-embedded schemas). A new
`supports_openai_strict_mode` is exposed so authors can assert a schema earns
strict enforcement; `sideviews::sideviews_schema` is unit-tested against it.

### D4. One generation contract, shared by the benchmark and the future job

`cognigraph_construct::sideviews` owns the system prompt, schema, user turn, and
`generate_sideviews(provider, text, count)`. The benchmark and any future ingest
job call the same function, so what we benchmarked is exactly what will ship. The
prompt encodes the two properties that make a side-view a good search surface:
self-contained questions (standalone queries, never "the passage above") and
answers grounded strictly in the source (a search aid must not invent facts).

## Evaluation follow-up (2026-09-09)

The original issue list is now resolved or explicitly closed. The user retired
CUAD and selected a [captured Luna construction baseline](decision_luna_baseline.md)
with a bounded Astra reference. This is separate from the side-view measurements
below. The subsequent [DeepSeek/GLM directed comparison](decision_luna_baseline.md#cross-provider-extension--2026-09-09)
is now captured, with a fresh Luna JSON-mode reference. GLM Flash is the leading
candidate for a representative-data trial; a failed DeepSeek Flash configuration
remains unranked. Luna remains the default, and no new judge or side-view model
is qualified by these directed-extraction experiments.

## Current model defaults (2026-09-08)

At this September 8 checkpoint, the user deferred broader model benchmarking until the remediation
list (CG-1 through CG-32) is resolved or explicitly closed. `gpt-5.6-luna`
remains the economical baseline until then. At that checkpoint, revisit the
latest available DeepSeek, GLM, and other relevant candidates, verifying their
versions and pricing then. Compare against the recorded Luna baseline with
reproducible quality, latency, and cost evidence; construction/judge
qualification remains a separate evaluation. Track this follow-up in the
[issue registry](../issues/README.md#after-the-current-ticket-list).

Following the user's model choice, OpenAI completions now default to
`gpt-5.6-luna`; Gemini completions default to the explicit stable model
`gemini-3.8-flash`. These defaults also apply to separately selected side-view
providers and named benchmark providers when no model override is supplied.
Inherited side-views continue to follow the main completion model.

Google lists [Gemini 3.8 Flash](https://ai.google.dev/gemini-api/docs/models/gemini-3.8-flash)
as stable, with structured outputs, and its
[migration guide](https://ai.google.dev/gemini-api/docs/latest-model)
retains GenerateContent support. Our one-shot text/schema request does not send
the removed sampling, candidate-count, or thinking-budget parameters.

[GPT-5.6 Luna](https://developers.openai.com/api/docs/models/gpt-5.6-luna)
supports Chat Completions and structured outputs. Its default reasoning is
medium, whereas [GPT-5.4 Mini](https://developers.openai.com/api/docs/models/gpt-5.4-mini)
used none. The September 8 refresh initially set `reasoning_effort: "none"`.
The [September 9 selection and runtime adoption](decision_luna_baseline.md)
now set `reasoning_effort: "low"` wherever Luna is resolved. Other OpenAI model
overrides retain their request shape. Schemas, prompts, timeouts, and response parsing remain
the same; no function tools, multimodal input, or conversation replay are used
by this completion adapter.

Current example: `COGNIGRAPH_COMPLETION_MODEL=gpt-5.6-luna` for OpenAI;
`COGNIGRAPH_SIDEVIEWS_PROVIDER=gemini` and
`COGNIGRAPH_SIDEVIEWS_MODEL=gemini-3.8-flash` for independent Gemini side-views.
The July benchmark below records the older models' measured behavior; it does
not establish comparative quality, latency, or cost for these new defaults.

Verification: `cargo fmt --all -- --check`,
`cargo clippy --all-targets -- -D warnings`, and `cargo test --all` passed
(888 tests). The release server and benchmark also passed the 13 server
configurations, 13 rejected startups, and 3 harness runs with synthetic
loopback providers, including assertions on Luna's reasoning setting. No
validation or live regression failed. That initial validation did not measure
external API access, quality, or latency. The subsequent real comparison is
recorded below. See the initial
[model-default HTTP evidence](../issues/evidence/completion-model-defaults-http-2026-09-08.json).

## Real comparison of the new defaults (2026-09-08)

The existing Rust harness subsequently made **32 successful real API calls**:
two preflights and 30 measured requests (five passages, three repetitions,
two providers). The APIs returned the requested `gpt-5.6-luna` and
`gemini-3.8-flash` model identities. The
[full report and reproducible artifacts](../../fixtures/semantic-neurons/sideviews-model-comparison-2026-09-08/README.md)
include per-call usage, outputs, request hashes, and a qualitative source review.

| Measured result | Luna, reasoning none | Gemini 3.8 Flash, default thinking |
|---|---:|---:|
| Schema-valid requests | 15/15 | 15/15 |
| Exactly 12 Q&A pairs | 13/15 | 15/15 |
| Total pairs | 178 | 180 |
| Mean upstream HTTP latency | 3.679 s | 5.141 s |
| Estimated cost per 1,000 similar passages | $0.4844 | $4.9993 |

Gemini's estimated token cost was 10.32 times Luna's; Luna's observed mean
latency was 28.4% lower. Gemini delivered the requested count more consistently. Neither
valid JSON nor lexical scores guarantee source fidelity: the review records a
variant-length detail added by Luna and a launch-customer premise added by
Gemini, each unsupported by its passage. Both also produced semantic repetition
and omitted some source details. The report states the limits of the unblinded
assistant review; it is not a calibrated judge or retrieval-recall evaluation.

Keep Luna as the economical baseline, with side-views inheriting unless
explicitly configured. This small sample gives no cost or demonstrated quality
reason to switch the default side-view lane to Gemini. The measured settings
use different reasoning budgets; pricing is an estimate using current standard
rates, not an observed invoice. The historical July price comparison below is
superseded for these new model/settings pairs. No model default or judge
qualification was changed by this experiment.

All requests completed without retry or API failure. Output/schema validation,
offline-versus-Rust metric parity, repeated request identity, and artifact
credential-pattern checks passed. No Rust code changed, so the prior 888-test
release validation remains the implementation checkpoint. Return to CG-7;
construction and judge replay/injection comparisons remain separate gates before
making product-wide model claims.

## Benchmark (2026-07-20)

`cargo run -p cognigraph-construct --bin sideviews-benchmark` over 5 sample
passages (`fixtures/sideviews/benchmark-docs.json`: geography, software,
biography, engineering, clinical), target 12 pairs each, live calls. Metrics are
cheap automatic proxies, not a judge; the run also dumps the full Q&A for manual
inspection.

| model | ok | err | pairs/doc | hit% | distinctQ | selfC% | ground% | ansWd | lat_ms |
|---|---|---|---|---|---|---|---|---|---|
| `openai:gpt-5.4-mini` | 5 | 0 | 11.8 | 80% | 100% | 100% | 99% | 7.6 | 2734 |
| `gemini:gemini-flash-latest` | 5 | 0 | 12.0 | 100% | 100% | 98% | 95% | 10.9 | 7723 |

- **hit%** = docs returning ≥ target pairs · **distinctQ** = within-doc unique
  questions · **selfC%** = standalone questions (no source-frame reference) ·
  **ground%** = answer content words found in the source (lexical hallucination
  proxy) · **ansWd** = mean answer words · **lat_ms** = mean per-doc latency.

**Historical verdict: use Gemini flash for side-views.** Both models are structurally
excellent (fully distinct, self-contained, target-count met). Manual inspection
(clinical + biography passages) found **no invented facts from either**; Gemini's
lower ground% reflects fuller-sentence phrasing, not hallucination. For the
side-view use case those fuller answers are *better* retrieval surfaces — a
standalone sentence ("Ibuprofen was first marketed in 1969.") embeds far more
strongly than a bare token ("1969."). Gemini flash is also the cheaper model, and
its higher latency (7.7s vs 2.7s) is irrelevant for a background ingest job.

Historical recommended config: `COGNIGRAPH_SIDEVIEWS_PROVIDER=gemini`,
`COGNIGRAPH_SIDEVIEWS_MODEL=gemini-flash-latest`, with completions left on their
own model. The current model defaults above supersede these model IDs for new
configuration; the recorded benchmark results remain unchanged.

## Implementation (2026-07-20)

The feature landed as five gate-green slices (each committed separately), all
honoring D1–D4.

### Slice A — foundation (`f0357f9`)
Provider axis (D2), the shared generation contract (D4), strict-schema
enforcement (D3), the `sideviews-benchmark` binary + fixtures, and the recorded
benchmark verdict.

### Slice B — storage / write-protection (`3064ccb`)
A new `GENERATED_COLLECTIONS` guard category in `system_collections.rs`.
`side_views` is publicly **readable and vector-searchable** but public mutation
is denied on every surface (typed, CGQL, raw AQL) — nobody can forge or vandalize
a retrieval surface. Kept SEPARATE from `MANAGED_COLLECTIONS` so it is never swept
into promotion/attestation. Only the generation job writes it, through the trusted
`managed_backend` handle (the same path construction uses for `chunks`/`facts`).

### Slice C — generation job + batch endpoint (`bff8f6f`)
`JobKind::SideviewsGenerate` in the durable M16/M17 `JobManager`: for each source
document it calls `generate_sideviews` (one strict-schema completion → ~12 pairs),
embeds the questions, and writes rows `{document_id, kind:"side_view", question,
answer, embedding}` to `side_views` — all through `TenantScoped` on the trusted
handle, mirroring the Ingest job's checkpoint/cancel/resume semantics. Idempotent:
skips documents that already have side-views; `regenerate` replaces them per
document in one atomic batch. `AppState.sideviews_completion` is wired from
`sideviews_provider_from_env` at startup (optional). Trigger is the **explicit,
opt-in** `POST /api/sideviews/generate` (no auto-on-write, so LLM cost is never
silently incurred), validating providers + rejecting system/managed/generated
source collections before enumerating.

### Slice D — retrieval fusion (`488ffc9`)
An **opt-in** side-view leg in `hybrid_search` (`include_side_views`): generated
side-view vectors join the RRF fusion as an extra leg. Because each row carries a
`document_id` pointer, its contribution collapses onto the PARENT document via the
existing chunk→source convention — a query matching a generated question boosts the
source document, never the synthetic Q&A. Opt-in because native `vector_search`
errors on a missing collection; a missing `side_views` is caught, never fatal.

### Slice E — delete cascade (`9c780a8`)
Deleting a source document cascades to delete its side-views (matched by
`document_id`) through the trusted handle; the delete response reports
`side_views_deleted`. Orphan prevention is the point — an orphaned side-view would
match queries and resolve to a missing document. Consistent with the opt-in
trigger, an **update does not auto-regenerate** (that would spend LLM calls on
every write); stale side-views are refreshed via the endpoint's `regenerate` flag,
and retrieval already serves the current parent content regardless.

### Shared deletion and publication contract (CG-13, 2026-09-09)

All public document deletes now enter the same lifecycle boundary: direct HTTP,
atomic batches, CGQL mutations, Lua document/batch/query calls, and collection
drops. Native deletes the source and matching generated rows in one transaction;
appended cascade operations do not change the caller's batch result count/order.
Direct DELETE retains `deleted`, `_id`, and `side_views_deleted`, including cleanup
of a legacy row whose parent is already absent.

Source capture and publication share a short lock with deletion, scoped to the
tenant incarnation. Provider calls run outside that lock. Deletion revokes the
captured source token, including a delete/reinsert batch or drop/recreation of
its collection. Publication rechecks that token and source existence while
holding the lock through the atomic side-view write. Invalidated work fails the
job with a source-conflict diagnostic; an explicit job retry reads the current
source or skips a missing source. A delete attempt reaching the write phase
conservatively invalidates earlier generation even if that write then fails. Concurrent default generation skips rows
published meanwhile; regeneration rereads and atomically replaces the current
set, rather than an obsolete pre-provider snapshot.

Collection drops clean up side-views **before** DDL. A cleanup error prevents the
drop; a later drop error or interruption can leave the source with fewer
retrieval aids, but cannot leave its side-views orphaned. Retry the drop or
regenerate. Cleanup read/write errors propagate instead of being converted to
zero. Native batches roll back source/derivative deletes together. Backends
without atomic batches cannot generate side-views; direct delete/drop can clean
up imported side-views sequentially before removing the source. Partial cleanup
is recoverable by retry. Failed cleanup/drop also invalidates cached search
results; successful per-document generation invalidates them even if a later
source in the same job fails.

Source collection names cannot contain `/` (the document-handle separator).
CG-13 originally rejected non-NFC sources because Native rewrote reference
strings and job payloads. [CG-33](decision_exact_reference_identity.md) removes
that temporary restriction: collection names, keys, frozen payloads, and generated
parent handles now retain exact Unicode forms, including after job restart.
Existing malformed references need the documented explicit diagnosis/repair
workflow; an upgrade does not repair them automatically.
There is no automatic source-update regeneration or historical orphan sweep.
Admin whole-store restore retains its separate job-quiescence boundary. Public
opaque backend-native query passthrough remains disabled (CG-7); this change does
not promise coordination with out-of-process direct database writers.

Final verification passed formatting, strict Clippy, all 922 Rust tests, and the
release server's 21 delete cases, 42 provider races/retries, six source-identity
rejections, regeneration/rollback cases, and three restarts across all supported
persistent Native modes. Initial harness mistakes (mutation-key syntax and vector
assertions on sidecar listings) and a Clippy type-complexity finding were corrected
before the final passes. The [CG-13 report](../issues/side-view-lifecycle-2026-09-09.md)
records these attempts, fault-injection limits, and reproducible artifacts. All
provider calls in this regression used synthetic loopback servers.

### Config summary
`COGNIGRAPH_SIDEVIEWS_PROVIDER` / `COGNIGRAPH_SIDEVIEWS_MODEL` (inherit the
completion provider/model when unset). Enable retrieval with
`{"include_side_views": true}` on `/api/search/hybrid`. Generate with
`POST /api/sideviews/generate {"collection": "...", "count"?, "regenerate"?}`.

## Recall evaluation (2026-07-20, dailymed)

Live-validated the whole chain end to end on the dailymed tenant, then measured
recall: a 130-label corpus, 1,300 generated side-views, 30 OpenAI-authored lay
queries (drug names stripped), recall@k with vs without the side-view leg.

**The recall lift is real but GATED on embedding quality — the dominant variable.**
On the SAME corpus/side-views/queries, re-embedded per model (vector-only, RRF):
Ollama `nomic-embed-text` (768d) made side-views *hurt* (MRR 0.497→0.400); OpenAI
`text-embedding-3-small` helped (0.625→0.656); **Gemini `gemini-embedding-2` won
biggest (0.538→0.703, recall@5 63→93%).** Full-system re-run (BM25 + `gemini-
embedding-2` vectors) confirmed it: baseline MRR 0.618 → **+side-views 0.706** at
`side_view_weight` 0.35 — recall@5 77→**93%**, recall@1 50→53%, no metric regressing
(better on 10/30 queries, worse on 6). The equal-weight (0.5) default is already
net-positive with a strong embedder.

**Recommendations:** (1) pair side-views with a strong embedder —
`gemini-embedding-2` (a coherent all-Gemini stack with `gemini-flash-latest`
generation) or OpenAI `text-embedding-3-*`; **do NOT judge the feature on Ollama**,
which inverted the result. (2) `side_view_weight ≈ 0.35` is a touch better than the
0.5 default but 0.5 is fine. (3) A "recall-backfill" fusion (inject side-view-only
docs to the tail; never re-rank docs the base already found) remains a worthwhile
follow-up to protect recall@1 on very strong bases (e.g. `3-large`), but is now
optional polish rather than a fix.

**Bug caught + fixed by the live run** (`d3bba30`): the generation job wrote
`side_views` before creating it; the native backend rejects operations on a
missing collection, so the first-ever run for a tenant failed. Fixed with an
idempotent `ensure_collection` in the run_job arm. The slice-C plumbing tests did
not exercise the run_job write path (follow-up: a mock-provider integration test).

### Candidate retirement and independent labels (2026-09-09)

The user dropped DeepSeek V4 Flash entirely from future evaluations. Its earlier
failed capture remains historical evidence; no configuration follow-up is planned.
The completed
[600-sentence independent-label supplied-pair trial](../../fixtures/semantic-neurons/glm-luna-semeval-2026-09-09/results.md)
scored GLM Flash at 65.75% and Luna at 42.67% pooled raw accuracy. Both
retained substantial Other-case restraint errors.
This benchmarks directed relation classification against published human labels,
with a shared pair-aware evaluation adapter. It is separate from side-view
retrieval and judge qualification. Luna remains the current product baseline.

The [low-effort extension](../../fixtures/semantic-neurons/terra-luna-low-semeval-2026-09-09/results.md)
adds Terra low (72.42%) and Luna low (70.08%) on the identical task. Terra's
2.33-point advantage costs about 10.4 times as much; Luna low and GLM Flash
are the recommended candidates for representative document qualification.
All three retain substantial restraint errors. This directed-construction
result does not change side-view defaults or qualify a retrieval model or judge.
