# CG-1: Lua scripts can re-enable JIT and bypass the instruction limit

- Status: Resolved
- Priority: P1
- Area: Lua sandbox
- Found: 2026-09-08
- Reviewed revision: `bc4bff1` (workspace 2.6.0)
- Verification: Live HTTP reproduction

## Problem

`LuaEngine` disables JIT once but leaves the `jit` table callable by submitted scripts. A script can turn compilation back on; LuaJIT compiled loops bypass the instruction hook. This defeats the server's advertised execution bound for every role with `lua-execute`.

## Evidence

- `crates/cognigraph-lua/src/runtime.rs:109-145` — sandbox globals, one-time `jit.off()`, and instruction hook.
- `crates/cognigraph-server/src/routes/lua.rs` — executes the engine in `spawn_blocking`; the request timeout cannot stop an already running blocking closure.

## Reproduction / failure sequence

Against the local server, `return {jit=type(jit)}` returned `{"jit":"table"}`. A bounded loop summing 1 through 2,000,000 failed with `Script exceeded instruction limit`. Prefixing that same loop with `jit.on();` returned HTTP 200 and `2000001000000`. No infinite loop was run.

## Acceptance criteria

- [x] Prevent scripts and loaded chunks from enabling JIT; fail closed if the runtime cannot establish its sandbox.
- [x] Add a bounded regression reproducing the observed bypass, including nested functions and loaded code.
- [x] Verify the real HTTP endpoint rejects work beyond its configured execution allowance and remains responsive.

## Resolution — 2026-09-08

Engine creation verifies JIT disabling and propagates hook/sandbox setup
failures. `jit` and `loadstring` are unavailable and host chunks load as text.
The shared instruction hook also covers coroutines; a typed host unwind keeps
protected calls from swallowing resource termination. Ordinary protected-call
error handling remains intact. HTTP cancellation signals the blocking worker,
and a tenant-scoped supervisor joins it before invalidating cached results.

Bounded regressions and release-server HTTP checks covered JIT/loader
restoration, nested functions, coroutines, protected calls, instruction limits,
worker cancellation, and post-timeout responsiveness. Formatting, Clippy, and
the full Rust suite passed. See [validation and cancellation limits](lua-controls-2026-09-08.md).

Run the repository Rust gates and a focused runtime regression before closing this issue. See [review evidence](review-2026-09-08.md).
