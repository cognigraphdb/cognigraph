# Neuron lifecycle serialization (CG-21)

- Date: 2026-09-09
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:223-234` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Neuron lifecycle serialization (CG-21).** Human transitions and automated
  publication share a tenant/incarnation lock and validate the current accepted
  set. Late judge results cannot overwrite changed proposals, human decisions,
  or another review result. The response reports skipped results and retains
  still-pending changed proposals in its count. Accepted-set read failures and
  malformed rows fail closed; review notes truncate safely at UTF-8 boundaries.
  Formatting, strict Clippy, all 937 reported Rust tests (including nine new
  deterministic regressions), 99 release HTTP scenarios, and 162 document
  comparisons after restart passed. Dedicated judge endpoint configuration was
  subsequently resolved by CG-36. The [verification report](../issues/neuron-lifecycle-2026-09-09.md)
  records final Rust gates and corrected fixture/accounting attempts.
