# v2.7.44 — Lua sandbox allow-list

- Date: 2026-11-11
- Status: v2.7.44
- Kind: Security fix

## Changes

[CG-104](../issues/CG-104.md) is resolved under the
[Lua sandbox decision](../decisions/decision_lua_sandbox_libraries.md).

Lua scripts could reach the hidden `os`, `io`, `package` and `jit` libraries
through the package library's `module()` function. That allowed shell
commands and file access from `POST /api/lua/execute`, and re-enabling the
JIT lifted the instruction limit.

The Lua state now loads only `table`, `string`, `math`, `bit` and the base
library. `io`, `os`, `package`, `debug` and `ffi` are never loaded, `jit` is
removed after compilation is switched off, and engine construction fails if
anything forbidden is still reachable. A test pins the exact list of script
globals.

**Breaking for scripts:** `os.time`, `os.date` and `os.clock` are gone, as
are `require` and `module`.

## Validation

`cargo fmt --all -- --check`, both Clippy gates with `-D warnings`,
`cargo test --locked --all` (815 passed, 2 ignored) and
`cargo test --locked --all --features enterprise` (1,042 passed, 2 ignored)
pass. On a 2.7.44 Community release server the three scripts from the ticket
are refused over HTTP, a bare loop stops at the instruction limit, and a
script using the allowed libraries returns its result. The registry, docs and
decision-index checks pass.

Not run: the UI, Native, Helm, Docker, advisory and dependency suites; they
run on the finished P1 batch. Nothing was pushed, released or deployed.
