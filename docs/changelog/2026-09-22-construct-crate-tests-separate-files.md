# v2.7.24 — Every cognigraph-construct module keeps its tests in a sibling file

- Date: 2026-09-22
- Status: v2.7.24
- Kind: Refactor

## Changes

The inline test modules of the eight remaining `cognigraph-construct`
modules move to sibling `_tests.rs` files included with
`#[cfg(test)] #[path = "..."] mod tests;`: `judge`, `validate`, `eval`,
`sideviews`, `advisor`, `rank`, `propose` and `preparation`. With the
[v2.7.23 move](2026-09-22-construct-tests-separate-files.md) of `draft` and
`grounding`, no module in the crate holds tests inline. Test bodies are
unchanged; the crate's library test list is identical before and after
(128 tests). Related: [CG-94](../issues/CG-94.md) and
[CG-95](../issues/CG-95.md), which own the further splits of the largest
modules.

No behavior changes. The workspace version moves to 2.7.24 because every
push carries a version and a change record.

## Validation

`cargo test -p cognigraph-construct` passes with the same test names as
before the move; `cargo clippy --locked --all-targets -- -D warnings` passes
for the crate. Full local gates run before the local merge. Not a release.
