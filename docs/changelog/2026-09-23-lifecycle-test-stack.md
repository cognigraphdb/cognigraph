# v2.7.29 — Lifecycle tests on an explicit stack

- Date: 2026-09-23
- Status: v2.7.29
- Kind: Tests and CI

## Changes

The first remote run of the v2.7.21–v2.7.28 integration candidate aborted the
Enterprise test suite on `ubuntu-24.04` with a stack overflow in a promotion
lifecycle test ([CG-96](../issues/CG-96.md),
[decision record](../decisions/decision_lifecycle_test_stack.md)). The six
promotion lifecycle tests now run their whole-server flow through a shared
`lifecycle` helper on a named thread with an explicit 16 MiB stack under a
current-thread Tokio runtime, instead of `#[tokio::test]` pinning the future
on the 2 MiB default test thread. Test names and bodies are unchanged; no
runtime code changes.

The workspace version moves to 2.7.29.

## Validation

Before the fix the governed flow needed more than 1.5 MiB in a debug build
on aarch64 and overflowed 2 MiB on x86_64; after it, the same test passes
with a 256 KiB test-thread stack. Every other test in both editions passes
with a 1 MiB test-thread stack. The release build of the flow needs under
512 KiB. Full local gate before the re-push of the integration PR.
