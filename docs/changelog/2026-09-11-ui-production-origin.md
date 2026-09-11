# Use the production origin and verify console access

- Date: 2026-09-11
- Status: Unreleased
- Kind: Fix

## Changes

Resolve CG-49. The Rust-served production console calls its own origin, while
Bun development selects an explicit API port. The auth probe validates a
protected catalog response: unavailable or unverified servers show connection
feedback and Retry, and stale target recovery clears its associated credentials.
A fresh authentication-disabled session no longer claims the admin identity.

## Verification

UI lint/types, 92 tests and the production build pass. Real Native Community
browser checks cover fresh production login on port 38471, a document save and
reload, default/custom Bun ports, offline recovery, a saved split-port session
moved to production, and authentication-disabled operation.
[Report and captures](../../ui/audit/2026-09-11-production-origin/audit.md).
No Rust source changed. Remote CI, push and publication were not performed.
