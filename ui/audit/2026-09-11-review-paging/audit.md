# CG-54 review paging and complete space catalogs — 2026-09-11

**PASS for [CG-54](../../../docs/issues/CG-54.md).** Review queues now expose
continuation with truthful ranges. Both workspace selectors and the proposal
dialog can use space types beyond the old first-100 cutoff. Selection is
independent of the current queue page and survives persisted status changes.

## Candidate and environment

- Base commit: `3d3f9c8a033535eb5bce521d1229b205910e0136`; uncommitted CG-54
  UI/documentation changes. Product version 2.7.0; Rust source unchanged.
- Existing Enterprise release binary, disposable Native storage,
  `http://127.0.0.1:38481`, authenticated synthetic Admin in tenant `default`.
  `COGNIGRAPH_UI_DIST` served the real Bun production build; provider disabled.
- [Seed harness](seed.py): 101 synthetic spaces, 201 proposed and 201 accepted
  neurons. The accepted set contains 200 rank hints and one relation hint flagged
  as covered by a base rule, so its graduation target lies beyond the old cutoff.
  These labelled fixtures were imported into the isolated database; this is not
  evidence of 402 separately validated authoring operations.
- [Independent baseline HTTP reads](seed.json) confirm the 101st space, the 201st
  proposal and the graduation target. No existing user data or research corpus
  was used. No provider, benchmark or holdout calls were made.
- Codex in-app browser on macOS. Captures 01–09 use the normal 1280×720 viewport;
  measured desktop sizes are 1600×1000, 1280×800 and 1067×667. These approximate
  desktop scaling through resize emulation, not native Windows or mobile coverage.

The [manifest](manifest.json) binds source, production assets and evidence.
Capture 01 uses the prior `index-a26eybm7.js` build. Captures 02–09 and the
initial queue/verdict journeys use `index-qk4ara9z.js`. The final change makes
Construct's selected space URL state; captures 10–13, geometry, Construct reload,
catalog failure/retry and the final Review direct-link/scroll checks use
`index-nv021cbh.js` with `index-mzz9nwc1.css`. Review behavior is unchanged between
those two CG-54 bundles; earlier captures retain their original candidate scope.

## Executed acceptance

| Check | Observed result |
| --- | --- |
| Reproduction | PASS: the prior build displayed exactly 200 rows and “200 neurons” for the 201-proposal fixture. |
| Complete queue traversal | PASS: five browser pages yielded 201 distinct proposed IDs exactly once. The first page says “1–50 shown · More available”; the last says “201–201 shown · End of queue” with Next disabled. No invented total is shown. |
| Final-page verdict | PASS: accepting `qa-proposed-200` removes the last visible proposal, returns the queue to page one and keeps the inspector open with its new Accepted status. Reload retains the selected identity, status, note and reviewer. |
| Off-page graduation | PASS: clicking `qa-accepted-200` opens its inspector while the queue shows the first 50 accepted rank hints. The UI explains that the selected neuron is outside the page/filter. |
| Selected-item continuity | PASS: keyboard Next and Refresh retain the off-page identity and unsaved review note. Retirement removes its graduation flag; the inspector reloads Retired status and attribution, also retained after browser reload. Explicit space/status changes clear the old selected identity. |
| More than 100 spaces | PASS: Review and Construct report 101 spaces; searching selects `qa-space-100`. The proposal dialog offers that space, and submitting a valid new relation hint selects its actual identity and space. Fresh reads and reload retain the proposal. |
| Copyable state | PASS: Review URL carries space/status/page/neuron. Final Construct URL carries `space=qa-space-100`, retained on reload. Stale page fallback, invalid page values and wrong-space selected identities also have unit coverage. |
| Incomplete catalog failure | PASS with labelled injection: a 503 on page two displays “Space catalog unavailable” and “Unable to load spaces”, without reporting the first 100 as complete. Both Review and Construct expose the error. Disabling the fixture and pressing Review Refresh restores all 101 choices. |
| Layout and keyboard | PASS: pager, selectors and inspector remain within measured widths. Keyboard Enter pages the queue. The final table row can be scrolled into view and selected at 1067×667. |

[Browser page records](browser-pages.json) contain the first five pages followed
by named verdict/selection cases. [Fresh persisted HTTP reads](persisted.json)
confirm acceptance and retirement by `admin`, their notes, and the proposal in
space 101. The final primary-space counts are 200 proposed, 201 accepted and
one retired neuron; the newly authored proposal belongs to the other space.
Draft notes persist across in-place paging/refresh, not hard reload; stored
review notes and URL-selected identities do survive reload.

## Captures and measured evidence

- [Original truncation](01-before.jpg), [first page](02-first-page.jpg),
  [last proposal](03-last-neuron.jpg), [accepted after reload](04-accepted-reloaded.jpg).
- [Off-page graduation](05-off-page-graduation.jpg),
  [retired after reload](06-retired-reloaded.jpg), [101st Review space](07-review-space-101.jpg),
  [proposal in that space](08-proposed-space-101-reloaded.jpg).
- [Construct selector](09-construct-space-101.jpg),
  [final scaled Construct](10-final-construct-scaled.jpg),
  [final scaled Review](11-final-review-scaled.jpg).
- [Synthetic failed continuation](12-space-continuation-failed.jpg),
  [retry recovery](13-space-retry-recovered.jpg), [failure/recovery state](failure-recovery.json),
  [Construct failure state](construct-catalog-error.json), [Construct reload](construct-reload.json).

[Geometry](geometry.json) records bounds, client/scroll widths and heights.
At 1067×667, Review's pager is inside the 589px queue panel and the 405px inspector
ends at the viewport edge. All measured containers have equal client and scroll
widths. The queue and Construct main area own vertical scrolling.
[Last-row scroll evidence](last-row-scroll.json) places the selected 50th row at
y=591–639 within the queue's y=66–667 bounds, after scrolling 2269px.

The [failure proxy](failure_proxy.py) runs only on local port 38482, forwards to
the real QA binary, and substitutes one specific catalog continuation response
while an owned temporary flag exists. This is presentation/retry coverage, not
an observed backend outage. Request bodies and bearer tokens are never logged.

## Gates, diagnostics and cleanup

- `bun run check`: PASS, Biome and TypeScript (111 files).
- `bun test`: PASS, 132 tests in 19 files, 684 assertions. Twelve new tests cover
  continuation boundaries, page-local counts, all 201 records, failed/duplicate
  catalogs, cancellation, stale pages and independent selection.
- `bun run build`: PASS, 1726 modules. Documentation, decision-index, issue
  registry and whitespace checks passed. [Validation summary](validation.json).
- [Direct browser console](console-direct.json) and [proxy console](console-proxy.json)
  contain no captured warnings/errors. The available browser stream does not
  expose an independent network trace. Expected synthetic 503 responses are
  separately recorded; persisted effects were checked through fresh real HTTP.
- The owned browser session was signed out and closed; its viewport override
  was reset. The QA server and proxy were stopped and their owned temporary
  storage/flag removed. Existing user tabs, services and data were preserved.

No Rust source changed, so the previous Rust/Clippy suites were not rerun for
this UI-only batch. Arango, other editions/roles, provider-backed construction,
remote CI, Docker publication and mobile layouts were not requalified here.
Offset pages reflect live state and are not a snapshot across concurrent writes.
CG-54 is resolved within these boundaries; [CG-55](../../../docs/issues/CG-55.md)
is next. Eight P2 issues remain open. No commit or push of this new batch was made.
