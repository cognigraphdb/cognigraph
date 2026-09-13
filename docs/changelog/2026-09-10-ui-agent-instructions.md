# UI agent instructions and tracking

- Date: 2026-09-10
- Status: Unreleased
- Kind: Documentation

## Change

Update [UI instructions](../../ui/AGENTS.md) as an extension of the root rules.
Document the existing Bun commands, browser verification scope, shared component
and theme conventions, API/session boundaries and safe navigation state. Link
approved visual evidence and the recorded Modal/icon workarounds; require
regression verification before retiring those workarounds.

Align [UI tracking](../../ui/TODO.md) with the shared CG defect registry and use
Git blame for the commit that lands an item. Retain the import preview, impact
summary and confirmation requirement. Broader QA additions await the reference
project the user will provide.

## Validation

Documentation links, changelog index, decision index and whitespace checks
passed. Documented Bun commands match the existing package scripts and lockfile.
Only instructions, tracking and documentation changed; application suites,
browser flows and Rust checks were not rerun for this wording update.
