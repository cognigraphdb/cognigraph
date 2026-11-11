# v2.7.47 — Document depth limit

- Date: 2026-11-11
- Status: v2.7.47
- Kind: Data-integrity fix

## Changes

[CG-107](../issues/CG-107.md) is resolved under the
[document depth decision](../decisions/decision_document_depth_limit.md).

Native wrote documents of any nesting depth but reads them back with a JSON
parser that stops at 128 nested containers. Repeated CGQL updates that
wrapped a field in another object produced a document the store could no
longer read, and a resident store then failed to open.

A stored document may now nest arrays and objects at most 100 levels deep.
Every Native write path refuses a deeper document before anything is
committed, and the HTTP routes return 400. A backend validation error raised
during a CGQL mutation is now 400 instead of 500. A stored row that cannot be
parsed is named in the error.

**Breaking:** documents nested deeper than 100 levels are refused.

[CG-196](../issues/CG-196.md) is opened for an offline repair of stores that
already hold an unreadable row.

## Validation

`cargo fmt --all -- --check`, both Clippy gates with `-D warnings`,
`cargo test --locked --all` (832 passed, 2 ignored) and
`cargo test --locked --all --features enterprise` (1,059 passed, 2 ignored)
pass. On a 2.7.47 Community release server the ticket's update succeeds 99
times and the 100th returns HTTP 400; after a restart the server is healthy
and the documents are readable. The registry, docs and decision-index checks
pass.

Not run: the UI, Native, Helm, Docker, advisory and dependency suites; they
run on the finished P1 batch. Nothing was pushed, released or deployed.
