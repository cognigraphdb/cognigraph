# CG-56: results belong to their executed inputs

Date: 2026-09-11. Result: **PASS for CG-56's scoped acceptance**.

## Candidate and environment

- Base commit `991cf42` (CG-55), plus the uncommitted CG-56 result store/hook,
  shared request feedback and Query/Lua/Search/Graph integration. The
  [manifest](manifest.json) binds source, production assets, evidence and binary.
- Bun 1.4.2 production UI, `index-nfg3bnmz.js`, served by the preceding verified
  Enterprise 2.7.0 release binary. No Rust source changed or Rust rebuild was needed.
- Native backend, authentication enabled, synthetic Admin in tenant `default`.
  [Seed/environment record](seed.json). API `http://127.0.0.1:38485`, proxy/UI
  `http://127.0.0.1:38486`, isolated `/tmp/cg56-qa/store.redb`.
- Codex in-app browser, initial 1280 × 720 CSS pixels. Error layout checks used
  1280 × 800 and 1067 × 667, emulating 125%/150% of a 1600 × 1000 desktop.
  This is viewport emulation, not native Windows scaling coverage.
- No model/provider calls or existing application data were used.

## Fixture and request control

[seed.py](seed.py) imports four synthetic documents (Alpha/Beta/Gamma/Delta),
orthogonal two-dimensional vectors, Alpha → Beta → Gamma → Delta edges and an
empty second edge collection. Before graph verification,
[complete_fixture_ids.py](complete_fixture_ids.py) adds canonical `_id` properties
to the imported records, preserving every record including the Lua write.
[Amended fixture](fixture-with-ids.json). Initial vector captures used `_key`-only
imports: the title/raw response identified Beta, while the document-link column
was empty. These captures do not qualify imported-document link behavior.

[proxy.py](proxy.py) forwards real responses unchanged. Its local control file
selects twenty-second response delays for old-labelled requests and can close a
graph connection before forwarding to produce a real browser transport failure.
The latter is controlled fault injection, not a server outage. The
[trace](requests.json) records synthetic inputs and timing without headers/tokens;
automatic EXPLAIN requests are labelled separately. Direct writes/relationship
results are independently checked through fresh API reads.

Reproduction: start the isolated server with a temporary administrator, run the
fixture scripts with `CG56_QA_PASSWORD`, start the proxy and sign in at its origin.
Use a new evidence directory for another capture; preserve these records.

## Executed acceptance

| Flow | Observed outcome |
| --- | --- |
| Query text and bindings | Run `RETURN @value`, edit text, return to the previous text, rerun, then edit bindings. Every edit immediately clears data and timing; reverting text does not resurrect the result. |
| Delayed query errors and success | Edit text during a delayed error and bindings during a delayed HTTP 200 response. Neither late response publishes data, errors, timing or completion feedback. The current bindings then run successfully. |
| Lua edits and side effects | Script edits clear results. A delayed `graph.create_document` request finishes after the script changes; repeated shortcuts add no execution. Its response stays hidden, one document persists, and a later script succeeds. A real Lua error replaces the prior result with a persistent error. |
| Vector ordering | Start delayed `[1,0]` (Alpha), then change to `[0,1]` (Beta) and run. Beta completes first and remains visible, with its original timing, after Alpha arrives. Threshold edits clear hits/raw/timing; malformed vectors show local errors without old hits. |
| Text-search error | With no embedding provider, Semantic shows the real API's unavailable-provider error. Editing its query clears that error. Successful provider-backed text search was not run. |
| Graph loading and ordering | Start delayed Alpha, inspect pending Visual and JSON, change root to Gamma and run. Gamma completes first; Alpha cannot replace it. Changing root, depth, confidence, direction or edge collection clears graph, inspector and selected path. |
| Graph failure/retry | After success, drop a same-input graph request. Both JSON and Visual show only persistent “Failed to fetch”, with no old graph/inspector/path. The error remains after toast-duration expiry; restoring connectivity and retrying succeeds. |
| Expansion/readback | Expansion preserves its captured base graph. Create Alpha → Delta in the current collection and Gamma → Beta in another collection. Both readbacks succeed; the second updates root/collection before submitting, and JSON identifies the latest traversal. |
| Persistence/reload | Fresh HTTP reads confirm one Lua write and both new edges. Browser reloads show their stored payloads. |

Selected evidence:

- [Query edit clears result](01-query-edited-cleared.jpg),
  [Lua edit during a pending write](02-lua-edited-pending.jpg).
- [Newer vector result](03-vector-new-result.jpg),
  [state after the older response](vector-old-completion-ignored.json).
- [Graph JSON loading](04-graph-json-pending.jpg),
  [new graph owner](05-graph-new-owner.jpg), [timed state](graph-old-completion-ignored.json).
- [JSON error](06-graph-json-error.jpg), [Visual error](07-graph-visual-error.jpg),
  [persistent state](graph-error-still-visible.json).
- [125% error layout](08-error-125-percent.jpg), [150% error layout](09-error-150-percent.jpg),
  [measured geometry](error-geometry.json): no document-width overflow; error bounds
  stay fully within both viewports, including the compact sidebar breakpoint.
- [Different-collection readback](10-new-collection-readback.jpg),
  [fresh persisted records](persisted-final.json), [Lua write after reload](12-write-reloaded.jpg).

Run `python3 ui/audit/2026-09-11-result-ownership/verify_evidence.py` from the code
root to check the captured input resets, real response ordering, pending/error
states, persisted counts, reloads and layout bounds.

## Validation, diagnostics and capture corrections

- `bun run check`: PASS, 118 files and TypeScript.
- `bun test`: PASS, 144 tests / 21 files / 723 assertions. Seven new tests cover
  reset, invalidation, reversed success/failure, current pending state, local
  validation and revisiting discarded input scopes.
- `bun run build`: PASS, 1,731 modules.
- Documentation, decision-index, issue-registry and whitespace checks: PASS.
- Browser warning/error logs were empty [before reload](console-before-reload.json)
  and in the [final reload session](console-reload.json). Expected network/API
  failures are recorded in the proxy trace, including Lua errors, unavailable
  embedding provider and the injected graph disconnection.
- The first delayed query still had an unused bind variable, so it returned a
  real error; it verifies obsolete-error suppression. A corrected `RETURN @value`
  trial separately verifies delayed-success suppression. No failed setup was
  counted as a successful query.
- An [early reload capture](11-write-reload-before-search.jpg) preceded the
  document lookup's debounce. The [settled capture](12-write-reloaded.jpg) verifies
  the document after sign-in and search completion. The early state is retained
  as capture context, not counted as persistence success or a confirmed defect.

## Cleanup and limits

Signed out, closed both owned QA tabs, reset the viewport, stopped the owned API
and proxy, and removed the isolated store, text-index sidecar and control file.
The user's browser tab, processes and application data were preserved.

This change clears results deliberately instead of retaining stale data with a
submitted-input panel. It preserves CG-55's per-console Lua/CGQL submission slot;
changing inputs does not cancel server-side writes or add cross-tab idempotency.
Graph JSON remains the most recent traversal response, explicitly labelled;
Visual combines expanded neighborhoods. The live expansion exercised the selected
root and relationship-driven merges, not every possible node-selection sequence.

Community/ArangoDB, alternate authorization roles, successful provider-backed
semantic/hybrid/graph-augmented search, native OS scaling, remote CI and deployment
were not rerun. The shared retrieval hook is live-tested through Vector and the
Semantic error path. CG-56 remains uncommitted at this checkpoint; no push occurred.
