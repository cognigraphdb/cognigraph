# Decision: Lua scripts get an allow-list of libraries, not a deny-list of globals

Status: accepted and implemented 2026-11-11 for [CG-104](../issues/CG-104.md).

## Context

The Lua engine loaded every standard library that mlua calls safe and then
set the dangerous globals to nil. That left the package library's `module()`
function in place. `module(name)` looks a library up in the registry, where
the hidden `os`, `io`, `package` and `jit` tables still lived, and makes it
the environment of the calling chunk. A script with the Lua execute scope
could run shell commands, read files and switch the JIT back on, which also
lifted the instruction limit.

## Decision

1. **Allow-list.** The Lua state loads `table`, `string`, `math` and `bit`,
   plus LuaJIT's base library, which mlua always opens and which provides
   `coroutine`. `io`, `os`, `package`, `debug` and `ffi` are never loaded.
2. **JIT.** LuaJIT can only be switched off through its `jit` library. The
   library is loaded, compilation is switched off and verified, and the
   library is then removed from the globals and from the registry.
3. **Code loading.** `load`, `loadstring`, `loadfile` and `dofile` stay
   removed. Scripts are compiled once, by the host, from text.
4. **Fail closed.** After set-up the engine checks that none of the forbidden
   globals exists and that the registry's loaded and preload tables hold only
   the allowed libraries. Construction fails otherwise.
5. **Pinned surface.** A test fixes the exact list of script globals. A
   dependency update or a code change that adds one fails that test.
6. **Rejected: removing `module` from the deny-list approach.** It closes the
   one known route and keeps the design that produced it: every library is
   present and safety depends on enumerating every way to reach it.

## Consequences

- Scripts that used `os.time`, `os.date` or `os.clock` no longer have them.
  None of the documented examples did. A time source, if scripts need one,
  should be a host binding with a stated contract.
- `print`, `collectgarbage`, `getfenv`, `setfenv` and `newproxy` remain. They
  do not reach the host, but `print` writes to the server's standard output;
  [CG-129](../issues/CG-129.md) covers that and the missing memory limit.
- The glibc functions that LuaJIT's `io`, `os` and `package` libraries import
  stay linked into the binary but have no caller a script can reach. The
  container-security note is amended accordingly.
