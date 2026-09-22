# v2.7.23 — Move the draft and grounding tests out of their modules

- Date: 2026-09-22
- Status: v2.7.23
- Kind: Refactor

## Changes

The inline test modules of `crates/cognigraph-construct/src/draft.rs` and
`grounding.rs` move to sibling `draft_tests.rs` and `grounding_tests.rs`,
included with `#[cfg(test)] #[path = "..."] mod tests;` as the crate's other
modules already do. Test bodies are unchanged; the test list is identical
before and after. This is the first step of [CG-95](../issues/CG-95.md),
which keeps the further split of both modules as a cohesion judgment rather
than a line count.

No behavior changes. The workspace version moves to 2.7.23 because every
push carries a version and a change record.

## Validation

`cargo test -p cognigraph-construct` passes with the same test names as
before the move; `cargo clippy --locked --all-targets -- -D warnings` passes
for the crate. Full local gates run before the local merge. Not a release.
