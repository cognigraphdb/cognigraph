# Explain tenant deletion and display quarantine outcomes

- Date: 2026-09-11
- Status: Unreleased
- Kind: Fix

## Changes

Resolve CG-51: tenant deletion confirmation explains credential removal, work
retirement and data quarantine, and distinguishes deletion from suspension.
Recreating a name does not restore credentials or data. The console validates
and retains actual deletion/quarantine results, handles uncertain failures, and
restores keyboard focus after cancellation or completion.

The Enterprise verification also corrects CG-49's host-admin probe gap: a denied
collection catalog falls back to a verified protected tenant catalog.

## Verification

UI lint/types, 96 tests and production build pass. Real Native Enterprise
browser/HTTP checks cover cancellation with data and credentials, deletion,
same-name recreation, old JWT/API-token/login rejection, empty and already-absent
results, connection failure and recovery, keyboard controls, host-admin login
and ordinary-admin denial. [Report](../../ui/audit/2026-09-11-tenant-deletion/audit.md).
No Rust source changed. No remote CI, push or publication was performed.
