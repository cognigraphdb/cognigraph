# Collections QA — 2026-09-10

## Environment and scope

- Build: local 2.6.1 candidate on `2a26666` plus the CG-41–CG-44 fixes committed
  with this report. Rust release binary serving the production Bun bundle through
  `COGNIGRAPH_UI_DIST`; final assets `index-t85rp07k.js` and `index-3sebfbqg.css`.
- Browser: Codex in-app browser on macOS, UI/API `http://127.0.0.1:3001`.
- Backend: isolated Native/redb database; authentication enabled, default-tenant
  admin. Synthetic collection `qa_collections_20260910`, up to 36 documents.
- Viewports: 1600 × 1000, 1280 × 800 and 1067 × 667 CSS pixels, plus 1181/1179
  pixels around the sidebar breakpoint. These are resize emulations of desktop
  scaling, not native Windows/OS scaling or mobile-device acceptance.

## Findings and regression results

| Criterion | Result and evidence |
| --- | --- |
| Create collection/document, empty collection | PASS: created both in the browser; document appeared in the list and inspector. Empty-title submit displayed required validation. |
| Unsaved JSON through background refresh | Failed initially: the 15-second health cycle discarded input. PASS after [CG-41](../../../docs/issues/CG-41.md): invalid JSON/error and a valid marker remained through multiple refreshes, including a searched off-page document. |
| Invalid JSON and failed API save | PASS: invalid JSON stayed editable. A synthetic 2.2 MB draft produced a real HTTP 413 `Payload Too Large`; the complete draft remained available. Cancel restored the saved value. |
| Successful edit and timestamp | PASS: `QA saved document` survived browser reload and a fresh authenticated GET with a current September timestamp. A second save on the final bundle persisted `QA final persisted record`, independently verified over HTTP. Identity and omitted-field behavior also have two Bun tests. |
| Change selection | PASS: editing record 00 then selecting record 01 removed the old draft and opened record 01 in read mode. |
| Metadata | PASS: collection is `qa_collections_20260910`, with its actual identifier and timestamps. |
| Delete and cancel | PASS: Cancel retained the record; confirmation deleted only the synthetic record. A fresh GET returned 404 and the list count fell from 36 to 35. |
| Off-page direct link | Failed initially: `?doc=qa-page-34` opened the first record. PASS after [CG-42](../../../docs/issues/CG-42.md): exact-key lookup selects the requested record through whole-collection search. The parameter is consumed once; a later plain-route reload returns to the normal listing. |
| Search and paging | PASS: search selected an off-page record; page two remained selected through health refresh. Reset returned to page one. A unique nonexistent query displayed the no-match state without an inspector. Exact-key results are first; text search can also return other token matches. |
| Close inspector | PASS: the inspector stayed closed through subsequent polling. |
| Stale asynchronous reads | Source-verified: effect cleanup prevents obsolete list/search/catalog reads from publishing after dependency changes. Deliberately reordered network responses were not simulated. |
| Scaled desktop layout | Failed initially: Reset crossed under the inspector and edit actions exceeded the viewport. PASS after [CG-43](../../../docs/issues/CG-43.md): all measured panels and actions fit at the five tested widths. |
| Internal scrolling | PASS: table scroll height 1246/client height 311 at 1067 × 667; keyboard selection reached the last row with a positive scroll offset. Pagination and inspector actions remained within the viewport. The JSON textarea has its own scroll. |
| Keyboard dialog behavior | Failed initially: Enter opened Create, Escape dismissed it but focus landed on the body. PASS after [CG-44](../../../docs/issues/CG-44.md): Escape and keyboard Cancel restore visible focus to Create document. The conditionally mounted modal workaround is retained. |
| Login/logout | PASS: authenticated admin session entered Collections; logout returned to the sign-in form. |

## Visual evidence

Before: [created document](01-desktop-created.png),
[unsaved draft](02-unsaved-before-refresh.png),
[draft lost](03-after-background-refresh.png),
[wrong off-page selection](05-off-page-link-before.png),
[long-key edit overflow](08-long-key-1280-before.png),
[1067-pixel overflow](09-layout-1067-before.png).

After: [invalid draft retained](04-invalid-draft-preserved.png),
[off-page selection](06-off-page-link-fixed.png),
[1067-pixel layout](10-layout-1067-fixed.png),
[1280-pixel layout](11-layout-1280-fixed.png),
[final desktop](12-desktop-final.png).
The [intermediate 1280-pixel capture](07-layout-1280-before.png) precedes the
settled viewport capture; geometry and final captures are the layout authority.

Measured evidence: [1067-pixel geometry](geometry-1067.json) and
[other desktop widths](geometry-desktop.json). No measured panel had horizontal
overflow after the fixes. At 1067 pixels the inspector ends at x=1067 and Reset
at x=632.875, within its collection panel ending at x=660.875. Long keys wrap;
edit actions move to another line when needed. Compact filters use two rows.

## Diagnostics and limits

- Final browser warning/error log query returned no entries. Server logs show
  expected 401 login probes, 403 reserved `__auth_probe__` collection probes,
  404 exact-key misses and the intentionally induced 413. The existing auth probe
  can make two read requests during mounting; no duplicate mutations were observed.
- API persistence/deletion checks used real HTTP. Error presentation used the
  real server body limit, not an intercepted success/failure fixture.
- No provider was configured in the isolated server. Embedding generation,
  non-admin/other-tenant flows, ArangoDB UI operation, mobile layouts, screen-reader
  narration and native OS scaling were outside this bounded Collections run.
- This does not establish complete console or product acceptance. Planned work
  remains in [the UI tracker](../../TODO.md).

## Cleanup and checks

The browser signed out and closed; its viewport override was reset. The dedicated
server stopped and its temporary database directory was removed. Existing local
databases, `.env` and sealed experiment fixtures were untouched.

Biome/TypeScript, 72 Bun tests (161 expectations) and production bundling passed.
The commit checkpoint additionally runs the shared Rust/document/workflow gates,
Docker build and UI suite through the installed pre-push hook.
