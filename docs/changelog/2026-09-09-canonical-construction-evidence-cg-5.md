# Canonical construction evidence (CG-5)

- Date: 2026-09-09
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:276-284` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Canonical construction evidence (CG-5).** Chunk text is NFC before
  ordinary/directed grounding, content hashing, byte spans, and fact occurrence
  keys. Pure derivation/materialization use the same representation; stale
  custom-grounder spans are rejected before writing. Existing inconsistent
  records require explicit re-ingestion. Formatting, Clippy, all 912 tests,
  30 release ingestion/reconciliation calls, six old-binary repairs, three
  snapshot roundtrips, and six restarts passed across all supported persistent
  Native modes. See [representation and verification](../issues/unicode-evidence-2026-09-09.md).
