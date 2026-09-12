# Native-only runtime browser regressions

- Date: 2026-09-12
- Issue: [CG-67](../../../docs/issues/CG-67.md)
- Revision: `bcec5b0` plus the CG-67 changes; workspace 2.7.1, unpublished
- Scope: Existing Chromium journeys against both Native-only Rust editions

`python3 scripts/verify.py --suite ui` passed frozen installation, Biome,
TypeScript, all 161 Bun tests and the production build. The unchanged UI source
was then served by freshly built Rust debug binaries through `COGNIGRAPH_UI_DIST`.
`python3 scripts/verify.py --suite ui-browser` passed four Community and five
Enterprise tests, without retries. Both temporary servers stopped and their
owned stores were removed. Hosted providers were disabled.

| Journey | Community | Enterprise |
|---|---|---|
| Viewer read access; UI/API deny mutations and administration | Passed | Passed |
| Edition-specific availability and accessible collapsed navigation | Passed | Passed |
| Direct production routes, encoded document keys and browser history | Passed | Passed |
| Create, raw JSON edit and confirmed delete persist after reload | Passed | Passed |
| HostAdmin lands on Tenants and cannot read tenant documents | Not applicable | Passed |

[Community results](community-results.json) and [Enterprise results](enterprise-results.json)
preserve test names, timings and outcomes. Their
[Community runtime](community-runtime.json) and [Enterprise runtime](enterprise-runtime.json)
records bind the temporary origins, binary and production HTML hashes. Screenshot
files in this directory preserve each journey's final state. The Community
unavailable-page and Enterprise HostAdmin/Tenants screenshots were also inspected
visually; both show the expected access state without a layout anomaly.

The [runtime evidence manifest](../../../docs/issues/evidence/native-runtime-2026-09-12/manifest.json)
binds the UI/runtime captures and screenshots to the source used. Raw Playwright
attachment paths refer to their original ignored `ui/test-results` location;
the copied screenshots here are the retained record.

This verifies the existing regression journeys, not every console capability or
viewport. The separate [runtime report](../../../docs/issues/native-runtime-2026-09-12.md)
records release-binary HTTP/Lua/search/storage tests. Final packaging readiness
is CG-68; no remote CI or deployment was performed.
