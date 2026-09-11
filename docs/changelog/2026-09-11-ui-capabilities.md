# Verify console identity, edition and role capabilities

- Date: 2026-09-11
- Status: Unreleased
- Kind: Fix

## Changes

Resolve CG-53: add `GET /api/auth/session` using existing bearer and tenant
admission checks. The console derives page access and controls from verified
scopes and edition, supports governance-role sign-in, blocks forbidden direct
routes before their effects run and clears revoked sessions. Read-only data and
Enterprise evaluation remain available. Anonymous development is explicitly
labelled and excludes identity-dependent workflows. Existing backend scopes
are preserved, including the Graph explorer's write-scoped traversal route.

## Verification

Full local CI-equivalent checks pass in both editions: formatting, strict Clippy,
Rust tests and repository checks. UI lint/types, 120 tests and production build
pass. Real release binaries pass 225 HTTP role checks and browser journeys for
all nine roles in both editions, both anonymous modes, persisted writes/reviews,
read-only execution, governance revocation and scaled layouts.
[Evidence and limitations](../../ui/audit/2026-09-11-capabilities/audit.md).
Arango integration was skipped; existing OpenAI/Gemini embedding smoke tests ran.
No push, remote CI or image publication was performed.
