# Login centering check — 2026-09-13

Result: **centered at desktop widths; off-center below 960 CSS pixels** on both
the local packaged console and the live Railway deployment. Recorded as
[CG-72](../../../docs/issues/CG-72.md). This check does not change application code.

| Field | Evidence |
|---|---|
| Local build | Community v2.7.7, image `sha256:76b1906434fc4cb6663de5ce42c2beabd846e06609b45e6f7f5f87693ec962a0`, source `c072a43b594ba67ea17a22b144fc80f12482f3f0` |
| Current checkout | `0802064`; UI source, package/lockfile and Dockerfile unchanged from the packaged candidate |
| Local origin | `http://127.0.0.1:63242`, Rust-served production assets, disposable authenticated Native container |
| Remote origin | [Railway console](https://database-production-fe77.up.railway.app); live `/health` reports Community v2.7.7 |
| Browser | Playwright Chromium; fresh unauthenticated contexts, CSS viewport resizing, no stored credentials |
| Scope | Initial login card geometry at seven viewport sizes per environment; read-only HTTP/DOM checks |
| Diagnostics | No page errors or failed requests; initial authentication-probe 401 is expected |
| Evidence | [Measurements](measurements.json), [served asset hashes](assets.json), selected screenshots below |
| Cleanup | Disposable local container removed; no persistent test volume, records or remote changes |

The served CSS and JavaScript hashes match exactly between local and Railway.
The browser version was Chromium 153.0.8010.12. Documentation, decision-index and
issue-registry checks passed; no Rust or UI source changed, so build/unit suites
were not rerun for this read-only layout check.

Offset is the card center minus the viewport center, in CSS pixels. Positive
horizontal values mean displacement to the right. Both environments measured
the following:

| CSS viewport | Horizontal offset | Vertical offset | Card fully visible | Centering |
|---|---:|---:|---|---|
| 1600 × 1000 | 0 | 0 | Yes | PASS |
| 1280 × 800 | 0 | 0 | Yes | PASS |
| 1067 × 667 | 0 | 0 | Yes | PASS |
| 960 × 800 | 0 | 0 | Yes | PASS |
| 800 × 800 | +80 | 0 | Yes | FAIL |
| 640 × 800 | +160 | 0 | No | FAIL |
| 390 × 844 | +56 after autofocus scrolling | 0 | No | FAIL |

The root document remains 960 pixels wide below that boundary. At 640 pixels,
the 380-pixel card spans x=290 to x=670, clipping its right edge. At 390 pixels,
Chromium scrolls while focusing the username input; the card still extends
beyond the visible viewport. Those narrow views do not qualify mobile support.

The live card is about 392 pixels high versus 372 locally because the longer
Railway hostname wraps the footer onto an additional line. Both remain exactly
vertically centered at the tested heights. The 1280 × 800 and 1067 × 667 sizes
emulate effective 125%/150% desktop scaling on 1600 × 1000, not native OS scaling.
Short windows, virtual keyboards, validation errors and other browser engines
were not part of this check.

The cause is [base.css](../../src/styles/base.css): `html`, `body`, `#root` and
the Ant Design wrapper all have `min-width: 960px`; body overflow is hidden.
[auth.css](../../src/styles/auth.css)'s `place-items: center` then centers within
that oversized layout. The workspace minimum width needs to be separated from
the authentication screen, with a real browser regression for narrow windows.

Screenshots visually inspected:

- [Railway: centered at 1280 × 800](railway-1280x800.png)
- [Railway: shifted right at 800 × 800](railway-800x800.png)
- [Local: clipped at 640 × 800](local-640x800.png)

Reproduce without logging in: load each origin in a fresh browser context,
resize to the listed CSS dimensions, wait for `.auth-card` and fonts, and compute
`card.x + card.width / 2 - innerWidth / 2` and the equivalent height expression
from `getBoundingClientRect()`. Capture the viewport and inspect root scroll
dimensions. No persistence or mutation behavior is claimed by this layout check.
