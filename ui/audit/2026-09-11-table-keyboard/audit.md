# CG-57: keyboard access to console table actions

Date: 2026-09-11. Result: **PASS for CG-57's scoped acceptance**.

## Candidate and environment

- Base commit `991cf42`, plus the preceding uncommitted CG-56 changes and this
  CG-57 implementation. The [manifest](manifest.json) binds source, assets,
  evidence and the runtime binary. Earlier CG-56 captures remain unchanged.
- Bun 1.4.2 production UI (`index-63vwgfhg.js`, `index-rhypvyqh.css`) served by
  the verified Enterprise 2.7.0 Rust release binary. No Rust source changed.
- Native backend; authentication enabled; synthetic Admin and Viewer accounts
  in tenant `default`. UI/API origin `http://127.0.0.1:38487`; disposable store
  `/tmp/cg57-qa/store.redb`; embedding provider disabled. No external model calls.
- Codex in-app browser: initial 1280 × 720 CSS pixels. Additional 1280 × 800 and
  1067 × 667 sizes emulate 125%/150% of a 1600 × 1000 desktop and straddle the
  console's 1180px breakpoint. This is resize emulation, not native OS scaling.

## Fixture and reproduction

[seed.py](seed.py) imports two documents with canonical IDs and orthogonal
two-dimensional vectors, one edge, a space type and one proposed neuron. It also
creates a Viewer account. Start an isolated auth-enabled Native server with
`COGNIGRAPH_UI_DIST` pointing to the built `ui/dist`, then run the script with
`CG57_QA_PASSWORD` set to its temporary administrator password. Never use an
existing application store. [Initial readback](seed.json).

Sign in, load each starting route, then use Tab to reach the named table action
and Enter/Space to activate it. The route loads and vector form setup are setup
actions; the table-to-destination journeys themselves use only keyboard input.
[Keyboard trace](keyboard-steps.json) records the real Tab events, reached names,
native tags, URLs, focus outlines and activation keys. The inspector defaults to
the first document; selecting Vega with Space verifies an explicit change.

## Executed acceptance

| Check | Observed result |
|---|---|
| Catalog → collection → document | Tab reaches the collection anchor; Enter opens the collection; Tab/Space selects Vega in its JSON inspector. |
| Overview → collection | Tab/Enter opens the named collection through its native link. |
| Users → account | Tab/Enter opens the Viewer account; the direct route survives reload. |
| Review → inspector | The named native button works with both Space and Enter, exposes pressed state and keeps keyboard focus. Tab/Space reaches and closes the inspector. Selection reloads from its URL. |
| Search → document | Real vector search returns Rigel. Tab/Enter follows its complete document handle; the document inspector, direct URL and reload identify Rigel. |
| Pointer shortcuts | Ordinary cells still open/select rows on all five tables. Nested collection link clicks work. Delete opens only its confirmation; Cancel preserves the catalog and records. Edge rows have no document link. |
| Empty/error search | A nonmatching vector shows zero hits; invalid vector JSON shows its validation error. Neither state retains a document link. |
| Viewer boundaries | Collection and neuron inspection remain keyboard accessible; collection creation/deletion and neuron verdicts stay unavailable. Users is denied in the UI and returns 403 through a fresh authenticated API request. |

[Pointer checks](pointer-checks.json), [Viewer checks](viewer-checks.json),
[empty/error states](states.json), and [fresh HTTP/asset readback](readback.json).
Fresh document, neuron and catalog reads exactly match the seeded values;
navigation/selection is not a data mutation. The cancelled deletion did not
remove anything. The served JavaScript and CSS bytes match the local build.

## Focus and layout

[Measurements](layout.json) cover all five changed table surfaces at both scaled
desktop sizes and the selected Review inspector. Focused actions have nonzero
bounds and visible outlines; each table remains keyboard reachable. Document
rows retain their existing controls rather than becoming additional tab stops.

Resizing the Admin Overview while its last link was already focused temporarily
left that link below the viewport. A subsequent fresh Tab journey scrolls the
page workspace and brings it fully into view, for both Admin and Viewer. The
record preserves both observations; it does not claim resizing automatically
scrolls an existing focus target into view. No horizontal page overflow was
observed at the measured sizes. Internal page/inspector scrolling remains intact.

Selected captures:

- [Before: pointer-only catalog](01-before.jpg) and [new catalog focus](02-catalog-focus.jpg).
- [Document selection](03-document-selected.jpg), [Overview focus](04-overview-focus.jpg),
  [account link](05-users-focus.jpg) and [account detail](06-user-detail.jpg).
- [Review focus at 1067 × 667](13-review-focus-1067.jpg) and
  [selected neuron at that size](14-review-selected-1067.jpg).
- [Search link focus](17-search-focus-1067.jpg), [document destination](10-search-document.jpg)
  and [Delete confirmation isolation](11-delete-isolation.jpg).
- [Last Overview link reached by Tab](24-admin-last-reached-1067.jpg),
  [Viewer catalog](21-viewer-catalog.jpg), [Viewer inspector](22-viewer-review.jpg)
  and [denied account route](23-viewer-users-denied.jpg).

## Validation, diagnostics and limits

- `bun run check`: PASS (118 files, Biome and TypeScript). An initial import-order
  finding was corrected before this build and browser verification.
- `bun test`: PASS, 146 tests across 21 files, 733 assertions. Search route tests
  cover query punctuation and incomplete/ambiguous handles.
- `bun run build`: PASS, 1731 modules. Documentation and issue checks pass.
- [Browser warning/error log](console.json): empty for the run. Server responses
  included expected signed-out session 401s and the intentional Viewer `/users`
  403. An initial fixture read used the nonexistent `/neurons/qa-neuron` route
  and returned 404; the fixture was corrected to `/documents/neurons/qa-neuron`
  before acceptance. No application error was inferred from that setup mistake.
- Search navigation was exercised through Vector mode's shared result table.
  Provider-backed text modes, ArangoDB, a separate Community runtime,
  screen-reader narration, modifier-click/new-tab behavior, native OS scaling
  and exhaustive network capture were not tested. This is not full WCAG or
  full-console certification. Collapsed navigation names remain CG-63.
- No Rust tests were rerun for this UI-only change. No remote CI, commit, push,
  release or deployment was performed in this turn.

## Cleanup and follow-up

Signed out and closed the owned QA tab, restored the browser viewport, stopped
the owned server and removed its disposable store and text-index sidecar. The
user's browser tab, existing application stores and prior evidence were retained.

[CG-57](../../../docs/issues/CG-57.md) is resolved. Next is
[CG-58](../../../docs/issues/CG-58.md), empty/error guidance.
