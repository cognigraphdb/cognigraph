# Sidebar accessible names — CG-63

## Build and environment

- Base commit: `6ef434c` (CG-58 and CG-59 committed at the start of this turn).
  Only `ui/src/components/Sidebar.tsx` changes in the UI; the fix is uncommitted.
- Final production assets: `index-38ghcf4g.js`, `index-0pnq4bz5.css`, served by
  the real Enterprise 2.7.0 Rust binary at `http://127.0.0.1:38495`.
- Disposable Native store, authenticated Admin and HostAdmin, no application
  fixtures or mutations. Providers disabled; no model calls. The reused binary
  and final source/assets are identified in the [manifest](manifest.json).
- Browser: Codex in-app browser on macOS. Actual CSS viewports 1280×800 and
  1067×667 emulate 125% and 150% scaling of a 1600×1000 desktop. This tests both
  sides of the 1180px responsive breakpoint, not native Windows scaling.

## Results

| Check | Result and evidence |
|---|---|
| Reproduction | PASS: [expanded baseline](01-before-expanded.txt) names all nine Admin links; [manual collapse](02-before-collapsed.txt) loses their names. Baseline uses the previously committed production UI. |
| Expanded names and keyboard | PASS: Tab visits Overview, Collections, Query, Graph, Review, Construct, Lua, Users and Operations in order, with a visible 2px outline. Enter opens Operations. [Capture](03-expanded-keyboard.jpg), [accessibility tree](03-expanded-keyboard.txt). |
| Manually collapsed names | PASS: the same nine names and tab order remain while visible labels use `display: none`. Enter opens Collections. [Capture](04-manual-collapsed-keyboard.jpg), [tree](04-manual-collapsed-keyboard.txt). |
| Responsive names | PASS: all nine links retain names and focus at 1067×667, where visible text uses zero font size. Enter opens Operations. [Clean focus capture](07-responsive-focus-clean.jpg), [tree](06-responsive-keyboard.txt). |
| Pointer labels | PASS: moving the pointer onto Query displays its tooltip while focus remains on the document body and the route stays Collections. [Capture](05-pointer-query-tooltip.jpg), [pointer observation](pointer-tooltip.json). The tool lacks a standalone hover method, so a supported pointer drag from noninteractive page content supplied movement; incidental selected text in captures 05/06 comes from that test action. |
| Focus labels | PASS: keyboard focus displays the same destination tooltip; Tab away from Tenants dismisses it. Tooltip wrapping adds no focus stop. [Keyboard trace](keyboard.json) records link names, destinations and outlines; settled screenshots establish tooltip appearance. |
| Host-only Tenants | PASS: a real HostAdmin session exposes only Overview and Tenants. Tenants is named in [responsive](08-host-responsive-tenants.txt), [expanded](09-host-expanded-tenants.txt) and [manual](10-host-collapsed-tenants.txt) modes, opens with Enter and retains its named active link after [direct reload](11-host-direct-reload.txt). |
| Role boundaries | PASS: Admin collection access is 200 and tenant access is 403; HostAdmin has the reciprocal result. The host's inherited Operations URL shows the existing unavailable-page gate before navigating to Tenants. [API checks](api-checks.json). No authorization code changed. |
| Layout | PASS: navigation targets remain within the rail, visible tooltips remain within the viewport and the body does not overflow. All links, including the last destination, remain reachable. [Geometry](layout.json). The existing link sizes, gutters, typography and focus style are unchanged. |

## Validation and diagnostics

`bun run check`, all 161 existing tests (22 files, 783 assertions) and
`bun run build` pass. Browser [warning/error logs](console.json) are empty.
Operations' recent-error list shows the expected pre-login session 401s;
[API checks](api-checks.json) separately record intentional role-denial 403s.
No unexpected network failure was observed on the executed navigation paths.
Documentation, decision-index and issue-registry checks pass.
[Validation record](validation.json).

No Rust source changed or Rust tests were rerun. This does not establish
screen-reader speech output, all mobile layouts, ArangoDB coverage or remote CI.
The existing unit suite was retained; live accessibility and interaction checks
verify this small component change. No new browser framework was installed.

## Cleanup and status

Signed out both QA actors, closed owned tab 36, restored the viewport, stopped
the owned Rust process and removed `/tmp/cg63-qa`. User tab 13 (Docker Hub) and
existing databases were retained. CG-58 and CG-59 sealed evidence remains
unchanged. [CG-63](../../../docs/issues/CG-63.md) is resolved; CG-61 and CG-60
remain open. No push or release was performed.
