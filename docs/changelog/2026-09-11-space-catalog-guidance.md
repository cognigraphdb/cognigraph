# Guide space setup through drafts and distinguish catalog failures

- Date: 2026-09-11
- Status: Unreleased
- Kind: Fix

## Changes

Resolve CG-58 by directing operators through editable space drafts, stored-JSON
review and dedicated acceptance. Review and Construct distinguish loading,
permission denial, failed requests and confirmed empty tenants, with Retry.
A fresh Native tenant's missing collection counts as empty only after the
collection catalog confirms its absence. Dependent actions wait for a usable
catalog. Existing managed mutation, role and edition boundaries are preserved.

## Verification

UI lint/types, 151 tests and the production build pass. Real Native browser/API
checks cover provider-free draft acceptance and persisted attribution, denied
generic mutations, host-role denial, Community absence, delayed requests,
transport failure/retry, keyboard retry and scaled error layouts.
[Evidence and runtime limits](../../ui/audit/2026-09-11-space-guidance/audit.md).
No Rust source changed; no remote publication.
