# Wait for login rendering before CI geometry checks — v2.7.10

- Date: 2026-09-13
- Status: v2.7.10
- Kind: Browser regression reliability

## Changes

GitHub CI exposed a race after logout in the login-layout regression: geometry
was read before React rendered the authentication card. The test now waits for
the card to be visible before measuring it. Pixel tolerances, viewport coverage
and overflow assertions are preserved. Runtime source and styles are unchanged.

[CG-73](../issues/CG-73.md) retains the failed v2.7.8 run and the correction's
verification boundary. This candidate also carries the
[GitHub activation evidence](2026-09-13-branch-activation-v2.7.9.md) through the
protected develop PR. Production remains on main's Community v2.7.7.

## Verification

Full local CI and both Docker editions pass, including all 15 browser cases.
The [correction report](../../ui/audit/2026-09-13-login-ci-wait/audit.md) retains
the remote failure and fresh local evidence. Remote PR CI remains required
before protected develop integration.
