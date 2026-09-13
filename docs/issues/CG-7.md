# CG-7: DOCUMENT expressions in mutations silently evaluate to null

- Status: Resolved
- Priority: P2
- Area: CGQL mutation correctness
- Found: 2026-09-08
- Reviewed revision: `bc4bff1` (workspace 2.6.0)
- Verification: Pre-fix release reproduction; final Rust gates and release HTTP/Lua regressions passed

## Problem

Mutation validation accepts `DOCUMENT()`, but the mutation executor constructs evaluation contexts without the document resolver used by read queries. A valid reference consequently evaluates to null, and that null can be committed to a field instead of the referenced value. The same missing resolver affects expressions in mutation input and return projections.

## Evidence

- `crates/cognigraph-query/src/executor/mutation.rs:26-37,54-85,110-173` — bare execution/evaluation contexts.
- `crates/cognigraph-query/src/executor/mod.rs:294-299,407-475` — resolver path used for reads.
- `docs/cgql-v1.md:367-399,546-581` — DOCUMENT and mutation language contracts.

## Reproduction / failure sequence

With `notes/b` containing `v:2`, execute `UPDATE "a" WITH {copied:DOCUMENT("notes/b").v} IN notes RETURN NEW.copied`. The server returned HTTP 200 with `[null]` and stored the null, while ordinary DOCUMENT reads resolved the same reference.

## Acceptance criteria

- [x] Either support document/correlated resolution consistently in mutations or reject unsupported expressions before the first write.
- [x] Do not replay already committed mutation operations while resolving dependencies.
- [x] Test mutation selectors, INSERT/UPDATE/REPLACE/UPSERT expressions, and OLD/NEW return projections against existing and absent documents.

## Resolution — 2026-09-08

Selected the accepted validation remedy. Mutation queries containing
`DOCUMENT()` or correlated traversal starts now fail before materialization or
any write, including nested read subqueries, untaken branches, and return
projections. Dynamic reads inside mutations remain unsupported; no writes are
replayed. The HTTP endpoint maps planning/validation errors to HTTP 400.

Formatting, Clippy, all **893 tests**, and **256 release HTTP/Lua rejection
cases** passed. Resident and paged stores remained exactly unchanged after
each rejection; supported mutation lifecycles and restart checks also passed.
The first live attempt exposed a 500/400 mapping defect, corrected and retested
before closure. See [implementation, compatibility, and evidence](mutation-backend-reads-2026-09-08.md).
