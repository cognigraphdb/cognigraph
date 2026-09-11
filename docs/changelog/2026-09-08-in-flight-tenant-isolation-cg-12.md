# In-flight tenant isolation (CG-12)

- Date: 2026-09-08
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:365-374` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **In-flight tenant isolation (CG-12).** Admission captures the tenant's
  immutable incarnation, store, and cache under the lifecycle lock. Lua keeps
  those handles across worker/supervisor tasks; user/token administration
  fences shared-control-store access through its live-identity check and
  operation. Old requests cannot reopen a deleted store or modify a recreated
  tenant. Already-admitted ordinary work may still finish against quarantined
  data. Formatting, Clippy, all 869 tests, and resident/paged release HTTP
  paused-provider, slow-body, suspension/deletion/recreation, and restart checks
  passed. See [verification and tradeoffs](../issues/request-isolation-2026-09-08.md).
