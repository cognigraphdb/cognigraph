# Idempotent Native collection ensures (CG-35)

- Date: 2026-09-09
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:149-159` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Idempotent Native collection ensures (CG-35).** Existing collections now
  return before persistence under the creation write lock, preserving their
  type, data, and revision. Authenticated startup reuses current vector/text
  files; real creation and writes retain derivative invalidation. All 61 Native
  tests, formatting, strict Clippy, and 958 reported workspace tests passed;
  eight unconfigured Arango entries early-returned. The release matrix passed
  112 checks across nine authenticated restarts, preserving 18 derivatives
  unnecessarily rewritten by the saved release. Vector warm loading still
  reconstructs key/model metadata from redb. See the
  [verification report](../issues/collection-ensure-2026-09-09.md).
