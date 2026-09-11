# Production API origin and connection verification

Date: 2026-09-11. Scope: [CG-49](../../../docs/issues/CG-49.md).
Result: PASS for the acceptance criteria below. This is scoped UI verification,
not complete product acceptance.

## Candidate and environment

- Base commit: `17095e7ceaf253b0501d8868418d0233083c0867`; the CG-49 UI, build,
  tests and documentation changes were uncommitted during verification.
- Bun 1.4.2; Chromium through the Codex in-app browser on macOS.
- Real Rust Community 2.7.0 server from local `cognigraph:ci`, image
  `sha256:7bf8cd56310eeb90d805702b81953eee7c23fc040bde5af6712cc639ff3e8255`.
  Native backend, isolated disposable stores, synthetic admin/default-tenant
  actor, providers disabled. No existing product database was reset.
- Production `ui/dist` mounted through `COGNIGRAPH_UI_DIST=/opt/ui` on
  `http://127.0.0.1:38471`, then `http://localhost:3000` for saved-target migration.
  A separate authentication-disabled instance used `http://127.0.0.1:38472`.
- Bun default development UI: `http://localhost:3000`, API
  `http://localhost:3001`. Custom development UI: `http://localhost:3020`, API
  `http://localhost:38471`.
- Final production assets: `index-sg26dbbh.js` and `index-grrbwas2.css`.
  The final build ran with `COGNIGRAPH_UI_DEV_API_PORT=49999`; the production
  definition still removed the development override. Final browser reloads used
  the page origin. Earlier screenshots capture intermediate candidates in the
  same verification run; screenshots 10–12 record the final production build.
- [Manifest](manifest.json) records final source, asset and capture hashes.

## Executed acceptance

| Criterion | Result and evidence |
| --- | --- |
| Fresh production session uses its load origin | PASS. With port 3001 unused, direct loading a collection/document link on port 38471 displayed login for that server. Login mounted the intended collection and selected the synthetic document. [Login](01-production-login.png), [final build login](10-final-production-origin.png). |
| Authenticated operation persists | PASS. Edited `qa_origin/probe` title to `Saved on port 38471`; Save succeeded, browser reload retained it, and a fresh authenticated HTTP read returned that title with the original content. [Reload](02-production-reloaded.png), [initial response](seed.json), [fresh read](persisted.json). |
| Direct links, refresh and logout work | PASS. Collection/document and query routes loaded directly; refresh retained the session and configured origin. Signing out returned to the matching login target. The final bundle executed `RETURN { origin: "production-3000", ok: true }` with one expected result. [Final query](12-final-production-query.png), [DOM](final-production-query-dom.txt). |
| Default Bun development remains functional | PASS. `bun run dev` targeted localhost:3001. With its API stopped, the app displayed an unverified-server gate. Starting the real API and selecting Retry returned to login; login retained `/query`, and a constant CGQL query returned the expected row. [Unavailable](03-dev-api-unavailable.png), [DOM](dev-unavailable-dom.txt), [recovered route](04-dev-recovered-query.png). |
| Explicit development override works | PASS. `COGNIGRAPH_UI_DEV_API_PORT=38471 bun --port=3020 ./index.html` displayed the intended API origin and accepted login against that real server. [Custom target](08-custom-dev-api-port.png). |
| Saved target can be recovered safely | PASS. Logged in through Bun on localhost:3000 targeting localhost:3001, stopped Bun and that API, then served production Rust on the same browser origin. Reload displayed the unreachable saved target. Keyboard activation of Use default server cleared the old target/session and required login at localhost:3000. Login, query and reload succeeded. [Saved target](06-saved-target-recovery.png), [new login](07-default-server-login.png). |
| Reset cannot establish access while offline | PASS. With the custom development API stopped, reset cleared its saved session but retained the unverified gate. After the API restarted, Retry required login. [Cleared session while unavailable](09-session-cleared-unavailable.png). |
| Authentication-disabled mode is labelled honestly | PASS. A fresh session against the separate real auth-disabled server opened its empty collection catalog with `Authentication disabled` in the top bar. [Final build](11-anonymous-mode.png). |

## Checks and diagnostics

- `bun run check`: PASS (Biome and TypeScript; 102 files checked).
- `bun test`: PASS (92 tests, 17 files, 250 assertions). New tests cover page
  origin preservation, explicit development ports, HTTPS/IPv6 URL formatting,
  successful protected catalog reads, 401/session expiry, 403/404/429/5xx,
  malformed/HTML responses, network failure and timeout propagation.
- `COGNIGRAPH_UI_DEV_API_PORT=49999 bun run build`: PASS (1,719 modules).
- The recovery card and buttons fit CSS viewports 1600×1000, 1280×800 and
  1067×667, with no measured horizontal page overflow. These are desktop resize
  emulations of 100%, 125% and 150% scaling, not native OS scaling coverage.
  [Geometry](gate-geometry.json); the recovery screenshot was visually inspected.
  Retry and reset were also activated using the keyboard.
- [Console diagnostics](console.json) contain no application errors. Two Bun HMR
  disconnect warnings came from deliberately stopping the development server
  during migration. Network failures while APIs were deliberately stopped were
  expected and displayed by the gate. The initial IPv4 Bun URL failed because
  Bun listened on `::1`; its advertised `localhost` URL worked.
- No browser network trace is claimed: the available DOM tool did not expose
  Performance entries. Observed UI server targets, port isolation, successful
  real API operations and independent HTTP persistence reads establish the
  tested routing behavior.

## Cleanup and boundaries

Removed the owned `cg-origin-production`, `cg-origin-dev-api`,
`cg-origin-migration` and `cg-origin-anonymous` containers and their disposable
anonymous volumes. Stopped the owned Bun servers. Signed out the remaining
authenticated test session, restored the browser viewport and closed the four
application test tabs. One browser-generated connection-error tab could not be
selected for cleanup because the browser tool blocks its `data:` URL. Existing
user tabs and unrelated services were preserved.

Live TLS/IPv6 deployments, Enterprise/ArangoDB, other browsers, mobile layouts,
screen readers and the full Rust suite were not exercised for this UI-only
change. Unit HTTP-error fixtures verify failure handling separately from the
real authentication/persistence flows above. No remote CI, push or publication
was performed. Remaining UI defects are tracked in the
[issue registry](../../../docs/issues/README.md), with
[CG-51](../../../docs/issues/CG-51.md) next.
