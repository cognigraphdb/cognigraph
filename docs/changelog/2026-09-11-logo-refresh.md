# Adopt the cleaned CogniGraph mark

- Date: 2026-09-11
- Status: Unreleased
- Kind: Maintenance

## Changes

Replace the bundled UI logo with the supplied cleaned product master, preserving
its square canvas and geometry. The shared `BrandMark` mask retains the console's
existing teal colors and centered sidebar alignment. Builds remain independent
of the sibling product-docs checkout.

## Verification

UI lint/types, 120 tests and the production build pass. The real Rust server
served the updated assets; browser verification covered sign-in, expanded and
collapsed branding, scaled desktop layouts, direct routes, reload and sign-out.
[Scoped evidence](../evidence/ui-2026-09-11-logo-refresh.md#artifact-a97141c47ad0254e491c).
