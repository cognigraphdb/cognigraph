# Responsive authentication layout

- Date: 2026-09-13
- Status: Unreleased
- Kind: UI fix

## Changes

[CG-72](../issues/CG-72.md) confines the 960-pixel minimum width to the
authenticated workspace. Login centers in narrow viewports, scrolls on short
screens, wraps long server names and provides larger fields/buttons for narrow
or touch viewports. The rest of the console retains its desktop layout.

[Local acceptance](../evidence/ui-2026-09-13-login-responsive.md#artifact-79f36a0e0176e7eab1b0) includes
Chromium/WebKit phone-size emulation, validation and scrolling, plus all 161 UI
unit tests and thirteen Community/Enterprise browser cases. New automated cases
protect login geometry and the workspace boundary. No Rust source changed.
The Railway deployment is unchanged; hosted verification follows publication.
