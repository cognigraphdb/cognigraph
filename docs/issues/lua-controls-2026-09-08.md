# Lua execution-control remediation — 2026-09-08

This batch addresses [CG-1](CG-1.md) and [CG-6](CG-6.md), building on the first
five fixes. Changes remain local and uncommitted; unrelated shared work was
preserved.

## Behavior

- Engine creation verifies `jit.off()` succeeded and propagates sandbox and
  global-hook installation failures. `jit` and `loadstring` are removed along
  with the previously blocked modules/loaders; host-loaded scripts are text-only.
- A shared 64-bit instruction counter covers nested functions and coroutines.
  Checks occur every 1,000 instructions. A typed host unwind escapes Lua
  protected calls, preventing `pcall`/`xpcall` from swallowing resource
  termination; ordinary script errors retain their existing behavior.
- Read-only and writable CGQL bindings both receive the server row budget.
  Each query has its own row allowance, while all callbacks consume one script
  deadline. CGQL parsing/planning counts toward its remaining time allowance.
- Every graph callback checks deadline/cancellation before and after execution.
  Pending async backend futures are dropped on cancellation or timeout. An HTTP
  drop guard signals cancellation to the worker; a tenant-scoped supervisor
  joins it and invalidates cached results even if the HTTP future is gone.

## Verification

- `cargo fmt --all -- --check`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo test --all`: passed, 845 tests reported passed, 0 failed, 0 ignored
  across 65 result summaries. Environment-gated external integration tests may
  return early; no live ArangoDB/provider coverage is claimed.
- `cargo build --release -p cognigraph-server`: passed.
- Focused Lua and route tests: passed, including setup failure, JIT/loader
  restoration, nested functions, both coroutine APIs, protected calls,
  per-execution reset, both query modes, a shared deadline across callbacks,
  pending-future cancellation, worker termination, and correct tenant cache
  invalidation after dropping the request.

Live verification used the real release server with a disposable resident
Native store, synthetic users, and loopback HTTP only. No external provider or
production data was used. The [sanitized observations](evidence/lua-controls-http-2026-09-08.json)
record:

| Check | Observed result |
|---|---|
| JIT and string-loader restoration | HTTP 500; globals unavailable |
| Ordinary `pcall`/`xpcall` error handling | HTTP 200; expected caught error values preserved |
| Nested/coroutine/protected-call loop beyond 20,000 instructions | HTTP 500 with instruction-limit error |
| Five-row query under a three-row cap | Rejected through HTTP and Lua, for both script-runner and editor |
| Two-row query under that cap | Successful through both Lua roles |
| Repeated protected two-row queries under an 80 ms script allowance | Time-budget error; HTTP round trip 97 ms |
| One-second HTTP timeout during a bounded long loop | HTTP 408 at 1.004 seconds; prior write remained, later write absent |
| Health and fresh short script after timeout | Successful; combined round trip 1 ms |

Early checks caught two implementation compatibility errors with mlua 0.12
(the chunk-mode module path and a non-Send error used as an unwind payload)
and one JSON numeric representation mistake in the new test. They were fixed
before final validation. All live checks passed on their first run.

## Limits and compatibility

Existing scripts using `loadstring` must move that code into the submitted
script. Resource termination cannot be caught to continue execution; ordinary
`pcall`/`xpcall` error handling remains available. The engine's default remains
one million instructions; server configuration 0 keeps that default.

Cancellation is cooperative, not process isolation: synchronous Rust/backend
work must yield or return, and already committed writes cannot be undone.
Lua mutation attempts continue to invalidate cached results after worker
completion. Existing budget error status codes remain 500, with 408 for the
outer HTTP timeout. This batch does not change CG-15's dynamic-query row
accounting, mutation resolver semantics, or tenant-incarnation fencing.

The next planned batch is [CG-3](CG-3.md), directed deployment fencing.
