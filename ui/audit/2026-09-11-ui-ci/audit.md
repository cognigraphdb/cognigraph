# Shared UI CI and browser regression qualification

## Candidate and scope

2026-09-11; [CG-60](../../../docs/issues/CG-60.md). Base `8e74ed4` plus the pending
CG-61 dependency update and this CI/test implementation. Application runtime code
is unchanged; the Rust embedding tests now require explicit provider opt-in.
[Manifest](manifest.json) binds candidate files, production
assets and evidence; runtime records identify the binaries actually executed.
Previous audit captures and manifests remain unchanged.

Playwright 1.63.0, Chromium 153.0.8010.12 (revision 1243), Bun 1.4.2, local macOS
arm64. Each edition is built separately from the workspace with locked Cargo
dependencies, then serves the production UI at a newly allocated loopback port
from a disposable Native store. These are development-profile binaries, not a
new release-build certification. Both APIs require authentication. Provider
configuration is absent; no model calls, benchmarks or holdouts execute.

The [shared CI command](../../../scripts/verify.py) now runs frozen UI install,
Biome/TypeScript, all Bun unit tests and production build, the existing Rust and
repository checks, then the browser runner. The workflow installs Bun and the
matching Chromium/system libraries explicitly. It remains manually triggered.
Pre-push and initial publication invoke the combined suite without duplicate UI
runs. [Setup and scope](../../../docs/operations/ui-testing.md).

## Executed browser cases

| Case | Community | Enterprise |
|---|---|---|
| Fresh production origin, invalid/valid login, encoded off-page key, keyboard links, Back/Forward and reload | PASS | PASS |
| UI document create, raw JSON type-preserving edit, API readback, reload, cancelled and confirmed delete | PASS | PASS |
| Viewer reads; disabled writes; actual API write/user denials; stored data unchanged; route denial and logout | PASS | PASS |
| Collapsed accessible navigation and edition-specific Review access/empty state | PASS | PASS |
| HostAdmin landing, tenant API access, data denial and reload | Not part of Community contract | PASS |

Four tests run in Community and five in Enterprise, with fresh browser contexts,
one worker, zero retries and no skipped tests. Default viewport 1280 × 800;
collapsed navigation uses 1067 × 667. These are CSS viewport dimensions, not
native Windows scaling or a complete responsive audit. All data is synthetic.

The fixtures assert actual persisted responses after document writes and verify
the direct API independently of hidden/disabled UI controls. Browser page
exceptions, unexpected HTTP/transport failures and external-origin requests fail
the run. Missing authentication during the login gate and the deliberate invalid
login are explicit expected 401s. Enterprise's initial absent-space collection
404 is followed by a verified empty catalog. A denied API request is asserted as
403, not presented as successful coverage merely because its UI control is hidden.

During test authoring, two locator/fixture errors failed the runner (the optional
Summary label and a missing diagnostics binding). They were corrected in tests;
the application needed no changes. The final run below supersedes those attempts.

## Failure qualification

- [Broken TypeScript](negative-typecheck.txt): copied the UI into a temporary
  candidate and added a boolean initialized with a string. Called the actual
  shared `verify.run('ci')` against that copy. TypeScript failed, the CI gate
  returned nonzero, and no Rust/browser stage ran. The real source was untouched.
- [Broken production bundle](negative-browser.txt): copied `ui/dist`, replaced
  its JS entry with a deliberate thrown error and ran the actual browser runner
  using `--dist`. The otherwise healthy Community API could not make this a
  passing result: the page exception and missing login control failed the suite
  and propagated a nonzero exit. [Owned runtime cleanup](negative-runtime.json).
  The real production assets were untouched.

Workflow regression tests also guard CI inclusion and early exit on a failed
UI install. Runner tests verify inherited database/provider/Node configuration
cannot enter its allowlisted environment and a missing UI build fails before
server startup. Browser absence is a failure, never an optional success.

The first combined run was interrupted before the external embedding test target
after source review found that those tests loaded `.env` automatically. Both
tests now use Rust's explicit `#[ignore]` qualification boundary before any key
loading; invoking them with `--ignored` requires credentials rather than silently
returning success. The CI runner forces `COGNIGRAPH_LIVE_LLM=0` for the existing
live control-loop tests, even if the caller exported the opt-in. The complete
gate was restarted after this correction. No live provider qualification was run.

## Combined gate and artifacts

The complete local shared gate result is recorded in [ci-gate.txt](ci-gate.txt).
It completed with exit 0: frozen install, lint/types, 161 UI tests, production
build, 64 workflow tests, docs/issue/modularity/edition checks, Rust formatting,
strict Clippy and workspace tests under both feature sets, then all nine browser
cases. Each Rust suite explicitly ignores the two embedding-provider tests.
Actionlint passes; the [Bun audit](dependency-audit.txt) reports no vulnerabilities
across 216 packages. Existing credential-gated Arango/live-loop tests do not
establish new external-service coverage.

Final browser reports and selected screenshots are retained alongside this
report. JSON results include exact case outcomes and zero-retry configuration;
runtime records contain origins and binary/HTML hashes. Credentials, bearer
headers and saved authentication state are not part of the evidence. Playwright
traces and videos are disabled. CI keeps generated diagnostics for seven days.

After reviewing the first screenshots, the test now also asserts that Review
owns the active link class and Operations loses it. Final captures disable CSS
transitions to avoid retaining an intermediate highlight. Lint/types and both
browser suites were rerun after this test-only refinement: [final browser log](browser-final.txt),
[Community results](final-community/results.json), [Enterprise results](final-enterprise/results.json).
All nine cases passed again. The earlier combined-CI reports remain in
`community/` and `enterprise/`; Rust/runtime code did not change after that gate.

Selected final captures: [Community collection after reload](final-community/artifacts/documents-fresh-production-94522-ed-keys-and-browser-history/final-state.png),
[Enterprise collapsed Review](final-enterprise/artifacts/access-edition-specific-re-f5580-ssible-collapsed-navigation/final-state.png)
and [HostAdmin landing](final-enterprise/artifacts/access-HostAdmin-lands-on--4095c-not-access-tenant-documents/final-state.png).
[Verification summary](verification-summary.json) records 777/968 reported Rust
passes for default/Enterprise, with two explicit ignores in each. Counts include
credential-gated entries that return early; they are not executed Arango or model
coverage. `ARANGO_PASSWORD` was not exported for this run.

Servers stop and temporary stores are removed on both success and failure. Only
generated test reports under ignored `ui/test-results/` and reviewed copies here
remain. This run uses its own Chromium process and does not manipulate the
user's browser tabs or existing servers/databases.

No remote CI or Linux Actions execution, Docker image build/publication, ArangoDB,
external-provider flow or full governance/job workflow qualification is claimed.
The ordinary Rust suite retains its existing environment-gated test boundaries;
the new browser cases are executed real Native coverage, with no early-return
service skips. Passing this suite does not mean every backend feature has a UI.
