# Side-view lifecycle (CG-13)

- Date: 2026-09-09
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:264-275` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Side-view lifecycle (CG-13).** Direct, batch, CGQL, Lua, and collection-drop
  deletions share cascade cleanup and a generation-publication fence. Stale
  provider results fail after deletion/recreation; retries use the current
  source. Cleanup failures propagate, Native document batches stay atomic,
  and collection drops clean up before DDL. CG-13 temporarily required NFC
  source identifiers; CG-33 removes that restriction while retaining the `/`
  collection-name rejection. Formatting, Clippy, all 922 tests, 21 release delete cases,
  42 provider races/retries, six identifier rejections, regeneration/rollback,
  and three restarts passed in all persistent Native modes. Early test-harness
  and lint failures were corrected and recorded in the
  [verification report](../issues/side-view-lifecycle-2026-09-09.md).
