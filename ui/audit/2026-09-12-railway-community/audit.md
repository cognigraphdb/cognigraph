# Railway Community console acceptance — 2026-09-12

Scope: [CG-71](../../../docs/issues/CG-71.md), the first hosted console packaged
and served by the real Community Rust server. No UI interaction/layout changes.

| Field | Evidence |
|---|---|
| Build | v2.7.7, main `92187713786fb9256fa732da5a197586ac56315b`; production assets in the Railway Docker image |
| Origin | [Hosted console and API](https://database-production-fe77.up.railway.app), valid HTTPS, Native persistence |
| Browser | Playwright Chromium; 1280 × 800 and 1067 × 667 CSS viewports, resize emulation of 125%/150% on 1600 × 1000 |
| Actor | Authenticated Community Admin; synthetic probe collection only |
| Result | Live login/logout, document creation/reload, direct links, keyboard Query navigation and CGQL readback PASS |
| Diagnostics | No unexpected browser/request failures or external-origin requests; expected unauthenticated 401 verified separately |
| Geometry | clientWidth/scrollWidth both 1280 at 1280; both 1067 at 1067; screenshots inspected for header, hostname, table and controls |
| Cleanup | Probe collection and records removed after snapshot/restart/restore qualification; final live store has no user collections |

Created `CG-71 hosted console probe` through the real UI, reloaded its collection
and confirmed its persisted key through authenticated HTTP and CGQL. The API
also created, updated and deleted its own documents, including nested JSON.
Both retained documents survived a hosted restart and the executed platform
restore before cleanup. [Browser result](../../../docs/issues/evidence/cg-71-2026-09-12/browser.json)
and [deployment acceptance](../../../docs/issues/railway-community-2026-09-12.md)
retain the measured scope and recovery evidence.

Selected sanitized screenshots:

- [Login](login.png)
- [Collections at 1280 × 800](collections-1280.png)
- [Collections at 1067 × 667](collections-1067.png)

This is desktop hosted packaging acceptance. It does not establish mobile or
native OS scaling behavior, all console workflows, hosted Enterprise coverage,
or hosted Viewer/tenant acceptance. Those boundaries remain distinct from the
nine Community/Enterprise local and CI browser regressions. Screenshots predate
cleanup and intentionally show only the synthetic records used for this drill.
