# Complete review queues and space choices

- Date: 2026-09-11
- Status: Unreleased
- Kind: Fix

## Changes

Resolve CG-54 with 50-row review pages, visible ranges and explicit continuation.
Review/Construct space choices load every catalog page and support search and
retry. The neuron inspector resolves identities outside the visible page,
preserves selection during paging, and refreshes persisted verdicts. Review
filters/selection and Construct's space remain addressable through their URLs.
The status filter retains its full width so later choices are not clipped when
the inspector is open at scaled desktop widths.

## Verification

UI lint/types, 132 tests and production build pass. Real Enterprise/Native browser
checks covered 101 spaces and 402 seeded neurons, full proposed-queue traversal,
final-page acceptance, off-page retirement, reload, searchable space choices and
a new proposal in the 101st space. Fresh HTTP reads confirm persisted results.
A labelled proxy failure verifies incomplete catalog handling and retry.
[Evidence and limitations](../evidence/ui-2026-09-11-review-paging.md#artifact-cbe96c45605c68a64fba).
[Final status-filter layout and keyboard verification](../evidence/ui-2026-09-11-review-paging.md#artifact-7e6b4dce7be6ec199447).
No Rust source changed; no push or publication was performed.
