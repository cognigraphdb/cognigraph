# Cleaned logo adoption — 2026-09-11

**PASS for the requested logo refresh.** The bundled SVG is a byte-for-byte copy
of the user's cleaned product master. The existing shared CSS mask handles its
color and dimensions; no component or stylesheet changes were necessary.

## Candidate and environment

- Base commit `87da230639fb6dc0f356e2b0d24192dae658a257`, with the existing
  uncommitted CG-53 changes and this logo refresh. No Rust source changed during
  the refresh. Earlier CG-53 captures and hashes are preserved as historical evidence.
- Bun 1.4.2 production build: `index-a26eybm7.js`, `index-a0evxjb5.css` and
  `cognigraph-mark-b1k1wyxb.svg`. [Manifest](manifest.json) records source, build
  and evidence hashes, including the product master and release binary.
- Real Enterprise release binary, Native disposable storage, authenticated
  synthetic `admin` in tenant `default`, `http://127.0.0.1:38480`.
  `COGNIGRAPH_UI_DIST` serves the built console; no provider calls were made.
- Codex in-app browser on macOS; normal viewport 1294×1065 and measured CSS
  sizes 1600×1000, 1280×800, 1067×667. These emulate desktop scaling through
  resize, not native Windows scaling or mobile rendering.

## Executed checks

| Check | Result |
| --- | --- |
| Master adoption | PASS: local source and served production SVG match the cleaned master exactly. Its `0 0 752 752` viewBox needs no additional cropping. |
| Login | PASS: the 38×38 mark remains vertically centered with the login wordmark. Its CSS color remains `#087f7c`. Login from a direct Collections URL succeeds. |
| Expanded sidebar | PASS: the 32×32 mark stays at x=22, y=19 inside the 194×70 brand region at both expanded desktop widths. Color remains `#27b9ac`. |
| Compact sidebar | PASS: the mark sits at x=20, centered in the 72px sidebar, both at the responsive breakpoint and after manual keyboard collapse. The brand retains its CogniGraph accessible name. |
| Production loading | PASS: direct Collections and Overview requests return the app, all built assets return 200 and match their local bytes, Overview survives browser reload, sign-out returns to login. |
| Diagnostics | PASS: no captured browser warnings/errors. [HTTP reads](http.json) separately verify health and asset responses. The browser tool does not expose an independent network trace. |

[Measured geometry](geometry.json) records dimensions, widths, CSS colors and
mask URLs for login at all three sizes and expanded/responsive/manual sidebar
states. The brand groups have equal client and scroll widths. Keyboard Enter on
Collapse sidebar changes the control to Expand sidebar and centers the mark.

## Captures and validation

- [Previous logo](01-before-login.jpg), [cleaned logo](02-refreshed-login.jpg),
  [scaled login](03-scaled-login.jpg).
- [Expanded sidebar](04-expanded-sidebar.jpg), [responsive sidebar](05-responsive-sidebar.jpg),
  [manually collapsed sidebar](06-collapsed-sidebar.jpg).
- [Console diagnostics](console.json).

`bun run check` passed Biome and TypeScript (107 files); `bun test` passed 120
tests in 18 files with 622 assertions; `bun run build` passed (1723 modules).
Documentation and decision-index checks passed. No new tests were added for this
asset-only behavior, and the prior Rust suites were not rerun for the refresh.

The browser was signed out, its viewport override reset and its owned tab closed.
The owned server and temporary Native data were removed. User tabs, services,
existing changes and the sibling master were preserved. No commit, push, version
bump or publication was performed. CG-54 remains the next ticket; this scoped
visual check does not requalify the full role/edition matrix or other open issues.
