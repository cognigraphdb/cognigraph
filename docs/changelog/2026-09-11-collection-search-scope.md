# Disclose collection search limits and page-local filter scope

- Date: 2026-09-11
- Status: Unreleased
- Kind: Fix

## Changes

Resolve CG-59 with explicit 100-text-hit retrieval limits, exact-key merging and
retrieved-result pagination. Category and embedding filters identify their
loaded-page or retrieved-set scope. Empty filtered pages retain collection
navigation; search filters reset paging and Reset restores collection counts.
Search failures disclose partial results with Retry. Query changes clear prior
results, while periodic health probes preserve them.

## Verification

UI lint/types, 161 tests and the production build pass. A real Native fixture
with 125 text matches plus an exact key verifies caps, terminal paging, later
categories/embedding states, reset and off-page lookup. Delayed requests,
partial failure, keyboard Retry and scaled layouts pass.
[Evidence and limits](../evidence/ui-2026-09-11-collection-search-scope.md#artifact-1c2e51790ed0aa822e41).
No Rust changes, model calls or remote publication.
