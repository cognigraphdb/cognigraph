# Native-only readiness browser regression

- Date: 2026-09-12
- Issue: [CG-68](../../../docs/issues/CG-68.md)
- Revision: `4fdaa47` plus CG-68 documentation and verification changes
- Environment: Chromium, production UI assets served by disposable Rust processes

The full `python3 scripts/verify.py --suite ci` run passed frozen UI installation,
Biome, TypeScript, 161 Bun tests, the production build and all nine browser
journeys. Community passed four tests; Enterprise passed five. No retries,
unexpected results or flaky outcomes were recorded.

The journeys verify persisted document creation/edit/deletion, encoded keys,
direct routes and browser history, Viewer UI/API restrictions, edition-specific
availability, accessible collapsed navigation and Enterprise HostAdmin isolation.
The [Community results](community-results.json) and [Enterprise results](enterprise-results.json)
retain the exact test names and outcomes. All final screenshots are copied into
this directory. Raw attachment paths refer to the original ignored test-results
directory, not the retained copies.

[Community runtime](community-runtime.json) and [Enterprise runtime](enterprise-runtime.json)
records identify the temporary origins, binary/HTML hashes and completed cleanup.
The servers stopped and owned stores were removed. Providers were disabled.
The [readiness manifest](../../../docs/issues/evidence/native-readiness-2026-09-12/manifest.json)
binds these outputs to the source and separate release/image checks.

This is the existing deterministic browser suite on the Native-only revision,
not a new full UI/UX or cross-browser audit. See the
[acceptance report](../../../docs/issues/native-readiness-2026-09-12.md) for
scope, provider skips and the local-versus-deployed boundary.
