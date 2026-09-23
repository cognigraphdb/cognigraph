# Decision: whole-server lifecycle tests run on an explicit 16 MiB thread

Status: accepted and implemented 2026-09-23 for [CG-96](../issues/CG-96.md).

## Context

The six promotion lifecycle tests each drive every stage of a governed
promotion inside one `#[tokio::test]` future. The macro pins that future on
the default 2 MiB test thread. In debug builds the nested async state
machine and its poll frames now exceed 2 MiB on x86_64 (between 1.5 and
2 MiB on aarch64), which aborted the Enterprise suite on the CI runner
while the same suite passed on Apple Silicon. The release build of the
same flow needs under 512 KiB.

## Decision

1. **Explicit thread, not a global knob.** A shared `lifecycle` helper in
   the promotions test module spawns a named thread with a 16 MiB stack
   and runs the flow under a current-thread Tokio runtime with all drivers
   enabled, the same runtime shape `#[tokio::test]` provides. The six
   lifecycle tests keep their names and bodies; only the entry point
   changes.
2. **Rejected: `RUST_MIN_STACK` in CI or `.cargo/config.toml`.** It
   silently changes every test thread in every crate, must be reproduced
   by each developer's shell, and setting it during compilation crashes
   rustc's own worker threads (observed as SIGBUS while measuring).
3. **Rejected: boxing the test future.** `Box::pin` may still materialize
   the full state machine on the stack before moving it, so the peak is
   compiler-placement dependent; the explicit thread is deterministic.
4. **Rejected: splitting the flows.** Each test asserts one end-to-end
   lifecycle whose stages depend on earlier state; splitting would
   duplicate fixtures without reducing the per-stage nesting.
5. **Guard.** The fix is accepted only with a measured margin: every other
   test in both editions passes with a 1 MiB test-thread stack, so the
   next test to approach the limit is detectable before it aborts CI.

## Consequences

- No runtime, storage or API change; production worker threads were never
  near their limit (release measurement in the ticket).
- Future whole-server flows in tests should use the helper rather than
  `#[tokio::test]`; a new overflow on CI points at a test that did not.
- Revisit if the promotion flows are restructured into smaller async units
  or if a stack-usage lint becomes available for the test profile.
