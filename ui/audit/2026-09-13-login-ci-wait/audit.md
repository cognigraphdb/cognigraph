# Login geometry waits for the rendered card

Date: 2026-09-13. Follow-up to [CG-72](../../../docs/issues/CG-72.md) within
[CG-73](../../../docs/issues/CG-73.md).

GitHub [run 34746292322](https://github.com/cognigraphdb/cognigraph/actions/runs/34746292322)
at v2.7.8 passed both Rust and Native suites and all seven Community browser
cases. Enterprise failed its seventh case after logout: `boundingBox()` returned
null before the login card was rendered. The remaining Enterprise case and
Docker job were skipped. The [failure screenshot](remote-after-logout.png),
captured immediately afterward at 390 × 844, shows the visible centered card.
The error context points to the final post-logout `expectCentered()` call.

`App.logout()` changes authentication state; React renders the resulting login
view asynchronously. A successful click does not establish that the replacement
card is visible. The v2.7.10 test helper now awaits Playwright's visible-card
assertion before reading geometry. All existing one-pixel tolerances, viewport
matrix and overflow checks are preserved; no runtime source or CSS changes.

The correction is qualified through the shared browser/local CI gates and the
final protected develop PR. Earlier captures remain unchanged and do not imply
a passing remote result for v2.7.8. Hosted production remains on v2.7.7.

## Local correction acceptance

The v2.7.10 candidate passes the full local CI and Docker suites, including all
15 Chromium cases (seven Community/eight Enterprise). The
[Enterprise post-logout capture](local-after-logout.png) shows the awaited card.
[Community](community-runtime.json) and [Enterprise](enterprise-runtime.json)
receipts identify the actual binaries and production assets and confirm disposal
of their temporary stores. Remote qualification belongs to the protected final
PR; no passing result is retroactively assigned to the earlier failed run.
