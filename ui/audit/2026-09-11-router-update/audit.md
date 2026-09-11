# React Router dependency and production routing verification

## Candidate and environment

- Date: 2026-09-11; resolves [CG-61](../../../docs/issues/CG-61.md).
- Base: `8e74ed4` (CG-63). Candidate changes only `ui/package.json` and
  `ui/bun.lock`, plus this evidence and issue/status documentation.
- Bun 1.4.2; Rust-served production UI, Enterprise 2.7.0, Native backend,
  authentication enabled, no embedding provider or model calls.
- UI/API origin: `http://127.0.0.1:38496`. Isolated store under `/tmp/cg61-qa`;
  synthetic Admin and HostAdmin actors, default tenant. No existing data used.
- In-app browser; initial 1280 × 720, then measured 1280 × 800 and 1067 × 667
  CSS viewports. The latter emulate 125%/150% of 1600 × 1000 by resize, not
  native Windows or OS scaling. This is a routing and navigation check, not a
  new full-screen visual audit.
- Production assets: `index-vqzkrjph.js`, `index-0pnq4bz5.css`; 1735 bundled
  modules. [Manifest](manifest.json) binds source, assets, reused release binary
  and evidence hashes. No Rust source changes or rebuild were required.

## Dependency decision

The [maintainer advisory](https://github.com/remix-run/react-router/security/advisories/GHSA-qwww-vcr4-c8h2)
limits this CSRF issue to unstable React Server Components APIs. Its affected
8.x range ends at 8.3.0. CogniGraph uses `createRoot` and `BrowserRouter` in
[main.tsx](../../src/main.tsx), with static Bun assets and the Rust API; it has
no RSC action integration. This removes an advisory-affected dependency and a
failing audit, not a demonstrated exploitable console CSRF vulnerability.

Selected 8.3.1, the current stable release verified on this date using the
[official changelog](https://reactrouter.com/changelog#v831) and
[registry metadata](registry-metadata.json). Reviewed the intervening 8.3.0
parameter-encoding and NavLink changes and 8.3.1 navigation/redirect fixes.
Framework/RSC, fetcher and server-side APIs are outside this application's
BrowserRouter integration. Existing React/React DOM 19.2.7 satisfy the peers;
the `cookie-es` dependency remains unchanged. The only resolved package update
is React Router. No component or route implementation changed.

| Check | Result | Evidence |
|---|---|---|
| Pre-update `bun audit` | Exit 1, one high advisory for 8.2.0 | [Before](dependency-audit-before.txt) |
| Updated `bun audit` | Exit 0, no findings across 213 packages; no exceptions needed | [After](dependency-audit-after.txt) |
| Frozen install | PASS, lockfile unchanged by installation/validation | [Install](frozen-install.txt) |
| Biome + TypeScript | PASS, 122 files, no fixes | [Check](check.txt) |
| Existing UI tests | PASS, 161 tests / 22 files / 783 assertions | [Tests](tests.txt) |
| Production build | PASS, 1735 modules | [Build](build.txt) |

## Executed browser and HTTP journeys

All rows below passed on the updated production bundle. Each browser capture
has a same-named accessibility-text file beside its JPEG.

| Journey | Observed outcome | Evidence |
|---|---|---|
| Direct document URL while signed out | Login preserves `/collections/qa_routes?doc=item-030` | [Login](01-deep-link-login.jpg) |
| Invalid login then valid Admin login | Inline credential error; successful login opens exact off-page `item-030` of 31 documents | [Error](02-login-error.jpg), [Document](03-deep-link-resolved.jpg) |
| Keyboard Query link, pointer Lua link | Correct route, title and active navigation link | [Query](04-query-keyboard.jpg), [Lua](05-lua-navigation.jpg) |
| Browser Back then Forward | Returns to Query then Lua with corresponding page content and active link | [Back](06-history-back-query.jpg), [Forward](07-history-forward-lua.jpg) |
| Encoded query parameter | `?doc=route%2Bsample` selects literal key `route+sample`; existing behavior consumes the `doc` parameter | [Encoded key](08-encoded-key.jpg) |
| Direct `/users/admin` and reload | Session survives reload; Admin details and empty token list load | [Refresh](09-user-route-reload.jpg) |
| Admin `/` | Redirects to `/collections` | [Admin landing](10-admin-root-landing.jpg) |
| Expanded and responsive sidebar | All nine Admin links remain named and within bounds, no document overflow; Enter on collapsed Operations opens it | [1280](11-layout-1280.jpg), [1067](12-layout-1067.jpg), [Operations](13-collapsed-operations-keyboard.jpg), [Geometry](layout.json) |
| Logout and HostAdmin login at `/operations` | Logout retains route; HostAdmin sees Page unavailable, with only Overview/Tenants navigation | [Logout](14-logout-keeps-route.jpg), [Denied](15-host-denied-operations.jpg) |
| HostAdmin `/` then reload | Redirects to `/tenants`; refreshed page shows the correct empty tenant catalog | [Host landing](16-host-root-reload.jpg) |
| Direct HTML/assets over HTTP | Eight routes return the exact production HTML; JS and CSS bytes match `dist` | [HTTP checks](http-evidence.json) |
| Actual API authorization and fixture reads | Admin collections 200/tenants 403; HostAdmin inverse; anonymous collections 401; both exact documents read back correctly | [API checks](http-evidence.json) |

The HTTP fixture contains 31 synthetic documents created through the ordinary
document endpoint. Exact-document API readback agrees with the browser inspector.
The UI routing checks themselves do not mutate these records or issue tokens.
The `doc` parameter is intentionally consumed into transient search/selection;
this run does not claim that selection survives reloading the resulting bare URL.

## Diagnostics, cleanup and limits

The available [browser warning/error buffer](browser-console.json) was empty
after the final route reload; this is not a persistent network trace across all
full navigations. The server's [non-success response log](server-errors.json)
contains only expected anonymous session/collection 401s, the deliberate invalid
login 401, and the two role-boundary 403s. No unexpected server failures occurred
in the captured journey window.

Signed out, reset the browser viewport and closed the owned QA tab; the user's
Docker Hub tab remains. Stopped the owned server, verified its port closed and
removed the disposable database directory. [Cleanup](cleanup.json).

No Rust suite, ArangoDB, separate Community binary, native OS scaling, RSC exploit
reproduction, remote CI or publication was run. These scoped dependency/routing
checks do not qualify every console/backend workflow. Durable CI and browser
automation remain [CG-60](../../../docs/issues/CG-60.md). Historical audit captures
and manifests remain unchanged.
