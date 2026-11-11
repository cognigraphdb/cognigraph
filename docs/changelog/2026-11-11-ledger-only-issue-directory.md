# v2.7.42 — Ledger-only issue directory

- Date: 2026-11-11
- Status: v2.7.42
- Kind: Documentation and issue registry

## Changes

[CG-193](../issues/CG-193.md) is resolved under the
[private issue records decision](../decisions/decision_issue_records_private.md).

- `docs/issues/` now tracks only the `CG-N.md` tickets and the registry. The
  38 dated review, remediation and verification records moved unchanged to
  the ignored `docs/private/issues/` folder.
- `python3 scripts/issue.py check` fails when anything other than a ticket or
  the registry is added to `docs/issues/`.
- `scripts/check-docs.py` and `scripts/check-decision-index.py` treat a link
  to `docs/issues/<name>-YYYY-MM-DD.md` as a private record. They verify it
  when the private folder is present and skip and count it otherwise, so
  existing links in tickets, changelog records, decisions and archived plans
  stay as written. Any other missing target still fails.
- The evidence policy, the repository instructions and the registry
  conventions say where resolution evidence and longer dated records go.

No runtime behavior changes. Records published before this change remain in
the public Git history.

## Validation

`python3 scripts/issue.py check`, `python3 scripts/check-docs.py`,
`python3 scripts/check-decision-index.py` and
`python3 scripts/check-public-distribution.py` pass twice: with the private
folder present (every link to a dated record verified, including anchors) and
with it moved out of the checkout (248 and 6 links skipped).
`python3 -m unittest discover -s scripts/tests` passes with three new tests.
`cargo fmt --all -- --check` and `cargo metadata --locked` pass; no Rust
source changed, so Clippy and the Rust suites were not rerun. Nothing was
pushed, released or deployed.
