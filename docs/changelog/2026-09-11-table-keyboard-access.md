# Open console table entries with the keyboard

- Date: 2026-09-11
- Status: Unreleased
- Kind: Fix

## Changes

Resolve CG-57 with native collection/account/document links and neuron selection
buttons. Descriptive names and visible focus expose the existing row actions to
keyboard users. Pointer shortcuts and Delete confirmation remain independent;
edge collections and incomplete search handles do not gain document links.

## Verification

UI lint/types, 146 tests and the production build pass. Real Rust/Native browser
journeys verify Tab/Enter/Space, direct reloads, mouse shortcuts, Delete/Cancel,
scaled focus and Viewer restrictions. Fresh API reads confirm unchanged fixtures
and denied Viewer user administration. [Evidence and limits](../evidence/ui-2026-09-11-table-keyboard.md#artifact-a5b3ed52a87a809a63dc).
No Rust changes or remote publication.
