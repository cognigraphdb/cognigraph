# Lua execution controls (CG-1, CG-6)

- Date: 2026-09-08
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:403-412` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Lua execution controls (CG-1, CG-6).** LuaJIT and hook setup now fail
  closed; scripts cannot restore JIT or dynamically load string chunks.
  Protected calls and coroutines cannot swallow instruction termination.
  Both Lua query permission modes apply the server's CGQL row budget, while
  one deadline covers the script and its callbacks. HTTP cancellation signals
  the worker; a supervisor joins it and invalidates the caller's cache after
  partial writes. Release-server HTTP checks verified limits, role parity,
  shared deadlines, and timeout behavior. See
  [verification and compatibility details](../issues/lua-controls-2026-09-08.md).
