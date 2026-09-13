# M25 governed Semantic Repair authority

- Date: 2026-07-19
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:898-935` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **M25 governed Semantic Repair authority.** Added immutable tenant-
  incarnation/target-bound Semantic Repair revisions containing the exact
  unchanged M22 typed construction candidate, signed by an active
  PolicyAuthor, plus one independently signed final approve/reject review from
  a distinct PolicyApprover principal. Both statements bind the current base
  promotion-head decision id, with explicit JSON null required for a fresh
  target. Governed resolution keeps the existing
  M18-M24 promotion head as the sole selector and succeeds only when that head
  selects the exact approved candidate digest; M25 adds no shadow head or new
  promotion wire generation. HTTP routes under `/api/semantic-repairs` and
  matching `semantic-repair` CLI commands submit, list, inspect, review, and
  resolve authority. Per tenant incarnation, immutable revision/review counts
  are capped at 10,000 each, embedded candidates retain the 8 MiB M22 limit,
  and canonical candidate bytes have a checked 64 MiB aggregate cap across
  admission, recovery, current resolution, and Native snapshot preflight.
  Keyset scans fetch full records one at a time; public revision pages are
  cursor-bound and capped at eight, avoiding whole-tenant payload reads per
  page. Explicit `/api/construct/governed-ingest` resolves that
  authority and grounds supplied chunks directly from the embedded candidate
  on an atomic-batch backend; selection alone does not materialize a graph.
  Generic mutation of `space_types`, `neurons`, `review_policies`, `eval_specs`,
  `entities`, `chunks`, `mentions`, and `facts` is denied across document,
  embedding, collection, batch, graph, Lua, and CGQL write paths while generic
  reads remain compatible. Raw backend-native query text cannot reference
  managed collections. Existing rows retain their historical readable meaning
  without retroactive signatures. The LLM review lane now auto-accepts eligible
  `relation_hint` proposals only; legacy policies may still name `alias`, but
  aliases always queue. It uses a documented default audit sampling rate
  of 0.1, requests strict OpenAI structured output, and sends malformed
  screener or quality responses to human review. This is not signed judge
  qualification, automatic drift repair, corpus re-ingestion, graph-generation
  switching, deployment, distributed coordination, or HA. The exact Rust gates
  and authenticated release-binary persistent-Native and live-ArangoDB
  authority lifecycles passed, including revocation, rollback, mutation
  barriers, fail-closed materialization, tamper/recovery, restart, and exact
  cleanup; timings and the complete evidence matrix are recorded in the M25
  decision.
