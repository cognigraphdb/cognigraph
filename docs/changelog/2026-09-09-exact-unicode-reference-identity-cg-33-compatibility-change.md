# Exact Unicode reference identity (CG-33; compatibility change)

- Date: 2026-09-09
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:207-222` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Exact Unicode reference identity (CG-33; compatibility change).** Generic
  Native writes and CGQL literals now preserve caller-owned strings exactly.
  Canonically equivalent keys stay distinct; endpoint, parent, model, and frozen
  job references no longer silently select another identity. Text callers can
  use `NORMALIZE_NFC(...)` explicitly; construction evidence retains its NFC
  boundary. Non-NFC side-view sources work through interrupted-job recovery and
  cascade deletion. New offline `references audit` / `references repair` commands
  diagnose legacy ambiguity and apply explicit snapshot-hash-bound mappings to
  a separate private snapshot; protected records and keys are not patched.
  Formatting, strict Clippy, all 945 reported Rust tests, 138 release identity
  checks, six restarts, three old-binary repairs, and the CG-13/CG-5 release
  regressions passed. The first build ran out of disk space and initial test
  fixture errors were corrected; see the
  [verification report](../issues/exact-identity-2026-09-09.md) and
  [compatibility/repair procedure](../operations/reference-repair.md).
