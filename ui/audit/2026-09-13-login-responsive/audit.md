# Responsive login acceptance — 2026-09-13

Scope: [CG-72](../../../docs/issues/CG-72.md), making authentication usable on
phone-sized viewports while retaining the desktop workspace. The
[original local/hosted audit](../2026-09-13-login-centering/audit.md) is preserved.

| Field | Evidence |
|---|---|
| Candidate | `0802064` plus the three CSS changes and `e2e/login-layout.spec.ts`; [source fingerprints](source.json) |
| Release runtime | Local Community v2.7.7 Linux/amd64 release image with the newly built production `/ui` mounted read-only; temporary authenticated Native store |
| Browser matrix | Chromium and WebKit; touch emulation, device scale factor 3, CSS viewports 390 × 844, 320 × 568, 844 × 390 and 390 × 280 |
| Browser regressions | Six Community and seven Enterprise cases against the runner's real development-profile Rust servers serving the same production UI |
| Actors/data | Initial unauthenticated login; disposable local Admin login/logout and intentionally invalid credentials in automated tests |
| Result | All local UI gates, thirteen browser cases and sixteen additional mobile-layout states pass |
| Evidence | [Mobile measurements](mobile-results.json), [UI checks](ui-checks.txt), [browser checks](browser-checks.txt) and selected screenshots below |
| Cleanup | Both runner stores/processes and the temporary release container removed; no production data changed |

The root wrappers follow the actual viewport width. Only `.app-shell` retains
the 960-pixel desktop minimum. Authentication uses dynamic viewport height with
a `vh` fallback, its own vertical scrolling and flex auto margins. This centers
the card when it fits and allows it to grow below the viewport without losing
access to its top. Long hostnames wrap; narrow or coarse-pointer input text is
16 pixels with inputs/buttons at least 44 pixels high.

## Executed checks

The deterministic Chromium cases measure card centers at 1600 × 1000,
1280 × 800, 1067 × 667, 960 × 800, 800 × 800, 640 × 800, 390 × 844 and
320 × 568. Both center offsets are within one CSS pixel where the form fits,
with no document horizontal overflow. Actual login at the small viewport
preserves a 960-pixel workspace; returning to 1280 pixels restores the desktop
geometry, and logout centers the authentication screen again.

At 390 × 280, required-field feedback and a real rejected login are reachable
through scrolling, along with both inputs and the submit button. A deliberately
long unbroken server name remains inside the card. This is a presentation-only
text fixture; it does not alter the local API target or mock authentication.

The separate Chromium/WebKit release-container check repeats initial login and
required-field states at four phone/landscape/short sizes. It uses the displayed
Railway hostname as a footer fixture while requests remain on loopback. Both
engines report zero horizontal center offset and no horizontal overflow. The
portrait forms center vertically; shorter forms scroll to every relevant control
and the footer. No browser page errors or failed requests occurred.

The first implementation check caught the intentional `vh`/`dvh` fallback as a
duplicate CSS declaration; it was expressed using `@supports`. Type checking
required explicit viewport tuples, and the first new browser run detected an
Ant Design inherited-font override; the mobile wrapper and input now both use
16-pixel text. All final checks passed after these corrections.

Selected final screenshots, visually inspected:

- [WebKit phone login, 390 × 844](webkit-phone-login.png)
- [WebKit narrow validation, 320 × 568](webkit-narrow-validation.png)
- [Chromium short viewport scrolled to the footer, 390 × 280](chromium-short-validation.png)

## Boundaries and publication

UI lint/types, frozen installation, 161 unit tests and the production build pass.
The complete relevant browser suite passes in both editions. Rust code did not
change; full Rust/CI/Docker release qualification is not claimed for this local
UI follow-up. No external providers, model comparisons or research holdouts ran.

WebKit is browser-engine emulation on macOS, not an actual iPhone or Safari
application check. Touch/viewport settings do not execute a real software
keyboard. The 390 × 280 case verifies constrained-height scrolling, not a
specific phone's keyboard behavior. Authenticated mobile workspace support
remains outside scope. Railway still serves the previous bundle; hosted checks
must repeat after the authorized versioned publication/deployment.
