# CG-4: Malformed directed-completion output is treated as an empty replacement

- Status: Resolved
- Priority: P1
- Area: Directed construction / data integrity
- Found: 2026-09-08
- Reviewed revision: `bc4bff1` (workspace 2.6.0)
- Verification: Live HTTP reproduction with local provider stub

## Problem

Directed parsing defaults a missing/non-array `facts` field to an empty list and silently drops malformed items. The result then reaches the occurrence-replacement writer. A provider returning syntactically valid but schema-invalid JSON can erase previously grounded facts while the API reports success and no rejection reasons.

## Evidence

- `crates/cognigraph-construct/src/directed.rs:371-380` — default-empty and `filter_map` parsing.
- `crates/cognigraph-construct/src/directed.rs:407-420` — sends the parsed result to the replacement writer.
- `crates/cognigraph-embeddings/src/completion.rs:99-138` — provider adapter parses JSON but does not locally enforce the response schema.

## Reproduction / failure sequence

A local OpenAI-compatible stub first returned one valid `Alpha SUPPLIES Beta` fact for chunk `c1`; directed ingestion wrote one fact. On the next identical request the stub returned `{}`. The endpoint returned HTTP 200 with `proposed:0`, `facts_grounded:0`, and `skips:[]`; querying `facts` showed the previous occurrence had been removed.

## Acceptance criteria

- [x] Validate the complete response shape and every fact before any graph mutation.
- [x] Distinguish an explicitly valid empty fact list from malformed or incomplete provider output.
- [x] On schema failure preserve the previous projection and return an actionable error; test missing fields, wrong types, and mixed valid/invalid items.

## Resolution — 2026-09-08

Directed ingestion strictly deserializes the entire provider response before
backend access. All required fields are enforced and unknown fields rejected.
Regression tests compare the complete store snapshot after each schema failure;
live release-server checks returned HTTP 500 for malformed upstream responses
with the prior facts intact, while valid `facts:[]` still reconciled to empty.
All Rust gates passed; see [batch verification](fixes-2026-09-08.md).

Run the repository Rust gates and a focused runtime regression before closing this issue. See [review evidence](review-2026-09-08.md).
