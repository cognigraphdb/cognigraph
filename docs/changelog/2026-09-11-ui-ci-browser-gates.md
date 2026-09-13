# Include the console in CI and add real browser regressions

- Date: 2026-09-11
- Status: Unreleased
- Kind: Maintenance

## Changes

The shared CI gate now includes Bun frozen installation, UI lint/types, unit
tests, production build and Chromium regression cases against separately built
Community and Enterprise servers. CI installs Bun and Chromium explicitly and
retains browser diagnostics. Pre-push and initial-publication checks use the
combined gate without repeating the UI suite. Manual CI triggering is unchanged.

The two external embedding-provider tests require explicit `--ignored`
qualification before loading keys. CI also disables the live LLM loop opt-in;
ambient developer credentials cannot turn the routine gate into a model run.

The browser runner owns temporary Native stores, loopback origins and synthetic
credentials, with cleanup on success/failure. It excludes providers and holdouts.
Tests cover production login/deep links, history, document JSON round trips,
cancelled/confirmed deletion, Viewer restrictions, edition gates and HostAdmin
boundaries. See [the testing guide](../operations/ui-testing.md).

## Verification

Four Community and five Enterprise Chromium cases pass. Deliberately broken
TypeScript and production-bundle candidates fail the expected gates.
[CG-60 evidence and limits](../evidence/ui-2026-09-11-ui-ci.md#artifact-d71f3d04fcdc5baa0fdb) records the
combined local CI result; no remote CI run or publication is implied.
