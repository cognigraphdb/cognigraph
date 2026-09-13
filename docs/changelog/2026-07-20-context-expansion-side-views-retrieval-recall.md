# Context-expansion side-views (retrieval recall)

- Date: 2026-07-20
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:823-841` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Context-expansion side-views (retrieval recall).** LLM-generated
  question/answer "side-views" are indexed alongside source documents as an
  extra retrieval surface: a query that matches a generated question boosts the
  SOURCE document (via the existing chunk→source RRF collapse), not the
  synthetic Q&A. A provider axis resolved SEPARATELY from completions
  (`COGNIGRAPH_SIDEVIEWS_PROVIDER` / `COGNIGRAPH_SIDEVIEWS_MODEL`, inheriting the
  completion provider/model when unset) lets side-views run on a cheaper model —
  a live benchmark picked `gemini-flash-latest`. Storage is a per-tenant,
  write-protected `side_views` collection (a new `GENERATED_COLLECTIONS` guard
  category: publicly readable/searchable, writable only by the generation job,
  kept SEPARATE from the governed fact-authority collections so it never enters
  promotion/attestation). Generation is a durable per-tenant `sideviews.generate`
  job (M16/M17 `JobManager`) enqueued by the **opt-in** `POST
  /api/sideviews/generate`; retrieval folds in via an **opt-in**
  `include_side_views` leg on `/api/search/hybrid`; deleting a source document
  cascades to delete its side-views. Every model call uses strict JSON-schema
  output. See `docs/decisions/decision_sideviews_provider_and_benchmark.md`.
  Full gates green (fmt, clippy `-D warnings`, `cargo test --all`).
