# Preserve sidebar navigation names when collapsed

- Date: 2026-09-11
- Status: Unreleased
- Kind: Fix

## Changes

Resolve CG-63 by naming each sidebar link explicitly and exposing its destination
in a tooltip on hover or keyboard focus. Names survive manual and responsive
collapse; native navigation, active-page state and role filtering are retained.

## Verification

UI lint/types, 161 tests and the production build pass. Real Rust-served Native
browser checks cover all Admin links and host-only Tenants, Tab/Enter navigation,
pointer and focus tooltips, direct reload and both desktop scaling viewports.
[Evidence and limitations](../../ui/audit/2026-09-11-sidebar-names/audit.md).
No Rust changes or remote publication.
