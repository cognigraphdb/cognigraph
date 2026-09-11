# Product logo verification — 2026-09-10

## Build and source

Captured on base commit `a156f7c` with the logo changes before their commit. The existing 2.6.1 Rust
release server served the production UI (`index-956ykrwv.js`,
`index-85hrkams.css`, `cognigraph-mark-tqpzhxtm.svg`).

The [product master](../../../../docs/graphics/cognigraph-mark.svg) remains unchanged:
SHA-256 `8c92e1fcae5eba3584bfb209cd6b622203b32188a9bda81781850eebf68d2087`.
The [bundled copy](../../src/assets/cognigraph-mark.svg) differs only in its
viewBox: `190.06 152.98 890 890`. The original path and attributes match exactly.
Its measured cubic-path ink bounds are x=222.102–1048.024 and
y=161.082–1034.888; the square viewBox removes unused canvas with a small inset.
The original file's purple fill is retained; the UI alpha mask uses CSS color.

## Environment and acceptance

Codex in-app browser on macOS, UI/API `http://127.0.0.1:3001`, isolated
Native/redb database, default-tenant admin session. No user data or provider keys
were used. Viewport resizing approximates 125% and 150% desktop scaling; native
OS scaling, mobile and other browser engines were not tested.

| Check | Result |
| --- | --- |
| Expanded sidebar | PASS: mark 32 × 32 at x=22, y=19; wordmark starts x=64. Both vertical centers are y=35. Mark center x=38 aligns within 0.5 pixels of the navigation icon center. |
| Manual collapse/expand | PASS with keyboard Enter: the 32-pixel mark sits at x=20 in a 72-pixel rail, exactly centered at x=36. Hidden text adds no flex gap. |
| Responsive rail | PASS at 1280 × 800, 1067 × 667 and widths 1181/1179 around the breakpoint. Mark dimensions remain 32 × 32 without clipping. |
| Color | PASS: sidebar `#27b9ac`; sign-in `#087f7c` from the existing accent. |
| Sign-in screen | PASS: 38 × 38 mark beside the heading, vertically centered in the brand row. Sign-in and logout worked against the real server. |
| Accessibility | PASS: sidebar branding exposes the CogniGraph name even when collapsed; decorative mark beside the login heading is hidden from accessibility. Native AX retained navigation link names in compact mode. |
| Production asset and refresh | PASS: hard reload retains the mark; HTTP asset response is 200 with `image/svg+xml` and exact bundled bytes. |
| Diagnostics | PASS: final browser warning/error log was empty. Server's existing authentication probes produce expected access-denial responses. No missing-logo asset request was observed. |

[Recorded geometry](geometry.json) retains all measured states.

## Screenshots

- Before: [sign-in](01-login-before.jpg), [sidebar](02-sidebar-before.jpg).
- After: [expanded](03-sidebar-expanded.jpg), [collapsed](04-sidebar-collapsed.jpg),
  [1067-pixel compact rail](05-sidebar-1067.jpg), [sign-in](06-login-final.jpg).

## Checks and cleanup

Biome/TypeScript, 72 Bun tests (161 expectations), production bundling and
documentation checks passed. This visual change adds no new test framework or
Rust code. The temporary server/database and browser tabs were removed after
verification; viewport settings were reset. Existing data and the product master
SVG were preserved. No push was performed.
