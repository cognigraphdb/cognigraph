# CG-55: console execution guards

Date: 2026-09-11. Result: **PASS for CG-55's scoped acceptance**.

## Candidate and environment

- Base commit: `7758cdb` (CG-54); this capture adds the uncommitted CG-55 guard,
  hook, Lua/CGQL integration and regression tests. [Manifest](manifest.json)
  binds source, production assets, runtime binary and evidence hashes.
- Bun 1.4.2 production UI served by the real Enterprise 2.7.0 release binary,
  Native backend, authentication enabled, synthetic Admin in tenant `default`.
  Binary reused from the verified preceding batch; no Rust source changed.
- API: `http://127.0.0.1:38483`; UI/forwarding proxy:
  `http://127.0.0.1:38484`. Isolated store: `/tmp/cg55-qa/store.redb`.
  No provider calls, real corpus data or existing database were used.
- Codex in-app browser at 1280 × 720 CSS pixels, device pixel ratio 2.
  [Recorded geometry](ownership-completed.json) shows no document-width overflow.
  This interaction-only change did not alter styles; alternate viewports and
  native operating-system scaling were not requalified.

## Request control and reproduction

[delay_proxy.py](delay_proxy.py) forwards actual Rust responses unchanged and
delays delivery after backend completion. Initial explicit Lua/CGQL executions
waited ten seconds. Later ownership checks used ten seconds for Lua and thirty
seconds for CGQL (`CG55_QUERY_DELAY_SECONDS=30`); per-request delay fields identify
that second run. Earlier entries inherit the trace's ten-second default.
Automatic non-executing EXPLAIN validation is labelled `validation` and is never
counted as explicit execution. Requests record body hashes, status and timing,
without credentials or bodies.

To reproduce, start an isolated auth-enabled Native server with the production UI,
bootstrap a temporary administrator, and create an empty `qa_execution` document
collection. Run the proxy, sign in at its origin, and perform the cases below.
Use an empty audit-output directory for a new capture: the proxy appends to an
existing request log. Preserve this sealed capture.

The write script intentionally omits `_key`, so duplicate executions would create
separate UUID documents rather than overwrite one key and hide a duplicate:

```lua
return graph.create_document("qa_execution", { trial = "cg55-write" })
```

## Executed cases

| Case | Action and observed outcome |
| --- | --- |
| Lua click and shortcuts | Click Run; immediately press Ctrl+Enter, Command+Enter, Ctrl+Enter. One POST, HTTP 200, button disabled while pending. Fresh direct HTTP read finds exactly one document. |
| Lua intentional rerun | After completion, Command+Enter starts a new request; repeated Ctrl/Command shortcuts do not add requests. Exactly two documents now exist, with different UUIDs. |
| Lua failure and recovery | Repeated shortcuts around `error("CG55 expected failure")` send one request, returning real HTTP 500. Run becomes available; `return "CG55 recovered"` succeeds. |
| CGQL form and shortcuts | Submit the Run button's form, then repeat both editor shortcuts. One explicit POST returns the two persisted document keys. Automatic EXPLAIN calls remain separate. |
| CGQL error and input rejection | `RETURN (` sends one explicit request despite repeated shortcuts and returns real HTTP 400. Invalid `[]` bind variables show local validation and send no execution. |
| CGQL successful empty retry | Restore `{}` and run `FOR d IN qa_execution FILTER d.trial == "absent" RETURN d` with Command+Enter plus repeated shortcuts. One execution returns count 0 and an empty array. |
| Departed completion | Start a Lua error, navigate to Query, then run a new query. The controlled long-query repetition captures the old Lua response as delivered while the new query remains disabled/pending with no result or notice at capture. Its later completion shows the current query's result and releases the button. |
| Query tab lifecycle | During a confirmed pending request, switch Semantic → CGQL and press both shortcuts. The remounted editor retains the existing console guard; no extra execution occurs. |
| Persisted reload | Hard navigation to `/collections/qa_execution` shows exactly two documents and their original stored payloads. |

Evidence:

- Lua: [pending](01-lua-pending.jpg), [completed](02-lua-completed.jpg),
  [server error](03-lua-error.jpg), [recovered](lua-recovered.txt).
- Query: [pending](04-query-pending.jpg), [two keys](05-query-completed.jpg),
  [syntax error](06-query-error.jpg), [empty recovery](07-query-recovered-empty.jpg).
- Ownership: [pending after old completion](08-new-request-keeps-ownership.jpg),
  [timestamped state](ownership-pending.json), [tab check](query-tab-pending-check.json),
  [current completion](ownership-completed.json).
- Persistence: fresh HTTP reads after [one](lua-write-count-first.json) and
  [two](lua-write-count-second.json) intentional writes; [browser reload](09-persisted-writes-reloaded.jpg).
- Complete [request trace](requests.json), verified by
  `python3 ui/audit/2026-09-11-execution-guards/verify_evidence.py` from the code root.

The trace contains six intentional Lua executions and six intentional CGQL
executions. The first short ownership check completed before its final state
capture. A later thirty-second query provided a wider observation window. After
that query completed, an additional intentional shortcut run was started and
used for the pending tab-switch check. The identical query bodies therefore
appear twice, separated by completion; they are not overlapping duplicates.

## Validation and diagnostics

- `bun run check`: PASS, 114 files and TypeScript.
- `bun test`: PASS, 137 tests / 20 files / 704 assertions. Five new guard tests
  cover same-tick/reentrant submissions, synchronous and asynchronous failures,
  invalidation without early unlocking, and obsolete completion ownership.
- `bun run build`: PASS, 1,728 modules, `index-3hmw8q6y.js`.
- Documentation, decision-index, issue-registry and whitespace checks: PASS.
- Browser [warning/error log](console.json): empty. The proxy records expected
  HTTP 500 Lua errors and HTTP 400 CGQL syntax/EXPLAIN errors. No response was
  replaced with a synthetic success/error body.
- One attempted long-running browser DOM observation exceeded the automation
  tool's CDP evaluation timeout. It produced no retained timeline; the report
  uses the successful timestamped state captures and real request trace instead.

## Cleanup and boundaries

Signed out and closed the owned QA tab. Stopped the owned API/proxy and removed
the isolated store and its directory. The user's browser tab and databases were
preserved. Tokens stayed in memory/session storage and are absent from evidence.

This is one execution slot per mounted console and API identity, not an API
idempotency key or cross-tab/reload lock. Leaving a screen does not cancel a
server write. API-identity replacement is covered by helper ownership tests;
the live departure check exercised route unmount. Edited-input/result snapshots,
other retrieval/graph request state and reversed result ownership within those
flows remain [CG-56](../../../docs/issues/CG-56.md). Community/ArangoDB, alternate
roles, transport outages and remote CI were not rerun for this UI-only change.
No commit of CG-55 or remote push was made during this verification.
