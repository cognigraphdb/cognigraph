# Dependency maintenance console acceptance

Date: 2026-09-13. Candidate: v2.7.8, [CG-73](../../../docs/issues/CG-73.md).
React/React DOM 19.3.0, CodeMirror view 6.43.11, Bun 1.4.2. Production UI assets
served by disposable authenticated Native binaries in both editions.

The full shared suite passes 161 UI unit tests and 15 Chromium cases: seven
Community and eight Enterprise. Coverage includes login and narrow/short-screen
geometry, document persistence and dialogs, roles/tenancy, deep links and history,
and [real CGQL/Lua editor execution](../../e2e/editors.spec.ts). The new editor
case verifies edited Unicode text reaches the actual endpoint and its returned
value appears. Unexpected browser exceptions, HTTP failures and external
requests fail the harness. No such unexpected failures occurred.

The [Community runtime](community-runtime.json) and [Enterprise runtime](enterprise-runtime.json)
record binary and HTML hashes, loopback origins and cleanup. The selected
[Community screenshot](community-lua.png) shows Lua's graph.query result at
1280 × 800. Existing [CG-72 evidence](../2026-09-13-login-responsive/audit.md)
remains unchanged. This run does not qualify physical phones, every console
workflow, live providers or Railway's older deployment.
