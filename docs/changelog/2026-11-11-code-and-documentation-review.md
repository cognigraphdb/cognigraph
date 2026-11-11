# v2.7.40 — Code and documentation review tickets

- Date: 2026-11-11
- Status: v2.7.40
- Kind: Issue registry

## Changes

A full review of the Rust workspace, build and deployment tooling, the
TypeScript client and the current documentation is recorded in the
[review record](../issues/review-2026-11-11.md). It examined revision
`42d1739` (v2.7.39) and registers 88 open tickets,
[CG-104](../issues/CG-104.md) to [CG-191](../issues/CG-191.md): 11 P1, 43 P2
and 34 P3. Fourteen findings were reproduced against a Community release
build with a disposable store; the others are source-confirmed, and each
ticket's verification line says which. The React console was not reviewed.

The P1 tickets are:

- [CG-104](../issues/CG-104.md): Lua scripts reach `os`, `io` and `jit`
  through the leftover `module()` global.
- [CG-105](../issues/CG-105.md): deeply nested CGQL text overflows the stack
  and aborts the server.
- [CG-106](../issues/CG-106.md): CGQL mutations ignore the collection part of
  a full document id.
- [CG-107](../issues/CG-107.md): a document nested deeper than 128 levels
  cannot be read back and blocks startup.
- [CG-108](../issues/CG-108.md): requests rejected by the auth middleware are
  shown to every tenant's admin.
- [CG-109](../issues/CG-109.md): text search passes an unvalidated limit and
  field names into Tantivy.
- [CG-110](../issues/CG-110.md): typed traversal accepts unbounded depth
  outside every budget.
- [CG-111](../issues/CG-111.md): CGQL execution does not bound intermediate
  rows or value size.
- [CG-112](../issues/CG-112.md): reusing an Idempotency-Key across governance
  record kinds breaks a tenant's promotion authority.
- [CG-113](../issues/CG-113.md): snapshot import lets a tenant Admin create
  unsigned legacy promotion authority.
- [CG-114](../issues/CG-114.md): CLI export can replace a good backup with
  garbage.

[CG-103](../issues/CG-103.md), opened on a ticket branch on 2026-09-24 and
paused, is reserved on the integration branch with its original title and
content, because the allocator reads only the current branch's history
([CG-152](../issues/CG-152.md)). The registry narrative gains a checkpoint
for this review.

No runtime behavior changes and no finding is fixed. The workspace version
moves to 2.7.40 because every change set carries a version and a change
record.

## Validation

`cargo fmt --all -- --check`, `python3 scripts/issue.py check`,
`python3 scripts/check-docs.py`, `python3 scripts/check-decision-index.py`
and `python3 scripts/check-public-distribution.py` report no errors. The
lockfile was refreshed for workspace members only and passes
`cargo metadata --locked`.

On the reviewed revision with rustc 1.99.1, Community Clippy and both test
suites pass (809 Community and 1,035 Enterprise tests, 2 ignored in each).
`cargo clippy --locked --all-targets --features enterprise -- -D warnings`
**fails** on a deprecated `fetch_update` call
([CG-151](../issues/CG-151.md)); this change set does not fix it, so the
push gate is not satisfied. The UI, Native, Helm, Docker, advisory and
dependency suites were not run. Nothing was pushed, released or deployed.
