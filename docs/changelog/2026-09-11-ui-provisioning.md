# Align tenant onboarding and user provisioning with authorization

- Date: 2026-09-11
- Status: Unreleased
- Kind: Fix

## Changes

Resolve CG-52: tenant creation leads to first-admin setup, which can be resumed
from the tenant row. Conflicts remain inline and success explains how to sign in
as the tenant administrator. User creation uses the authenticated tenant, excludes
host-admin, and includes governance roles on verified Enterprise servers. Quota
guidance names the enforced active-job limit. Dialogs preserve keyboard return
focus and prevent closing while a request is pending.

## Verification

UI lint/types, 100 tests and production build pass. Real Native Community and
Enterprise browser/HTTP checks cover first-admin setup and conflicts, new-admin
login, persisted role assignments, denied cross-tenant/host-admin provisioning,
data-access boundaries and desktop/scaled layouts.
[Report](../evidence/ui-2026-09-11-provisioning.md#artifact-e96a930169a1f87db8a2). Governance console entry
and role-aware navigation remain CG-53. No Rust source changed; no remote CI,
push or publication was performed.
