# Update React Router and verify production navigation

- Date: 2026-09-11
- Status: Unreleased
- Kind: Maintenance

## Changes

Resolve CG-61 by updating React Router from 8.2.0 to 8.3.1 and refreshing its
lockfile entry. The dependency audit now reports no vulnerabilities across 213
packages. The original RSC advisory was not a demonstrated exploit of this
client-side console. No application source or unrelated dependencies changed.

## Verification

Frozen installation, UI lint/types, 161 tests and production build pass.
Real Rust-served Native browser checks cover login/deep links, an encoded document
key, keyboard navigation, Back/Forward, refresh, role landings and denied routes.
HTTP checks verify direct HTML routes, bundled assets and API role boundaries.
[Evidence and limitations](../evidence/ui-2026-09-11-router-update.md#artifact-f2ee775cf86eb46ca1d7).
No Rust changes or remote publication; CI/browser automation remains CG-60.
