# Preserve document JSON and construction import evidence

- Date: 2026-09-11
- Status: Unreleased
- Kind: Fix

## Changes

Resolve CG-50 and CG-62. The inspector displays stored JSON and PATCHes only
changed fields, preserving unknown fields, JSON types and hidden sidecar
vectors. Creation reads back the stored record before opening its inspector.
Read-only identity fields and unsupported field deletion have explicit errors.

Construction imports use stable content-based IDs and preview existing sources
before replacing their evidence. Duplicate IDs and stale previews are refused;
confirmation discloses rebuilding facts and mentions. Keyboard focus and preview
scrolling are verified at three desktop sizes.

[Scoped browser and HTTP verification](../evidence/ui-2026-09-11-data-preservation.md#artifact-6afad6337a2d11fc893d)
passed against real Native Community/Enterprise binaries. The UI passes lint,
type checks, 85 helper tests and its production build. Frozen audit captures are
excluded from Biome instead of being reformatted. No Rust code changed, and no
remote CI or publication result is claimed. Thirteen UI issues remain open.
