# CG-53 console capabilities — 2026-09-11

**PASS for the scoped acceptance criteria in [CG-53](../../../docs/issues/CG-53.md).**
Navigation, direct routes and action controls now follow a verified server
session's scopes and edition. Governance identities can enter an appropriate
workspace. These checks do not establish complete UI or signed-governance acceptance.

## Candidate and environment

- Base commit: `87da230639fb6dc0f356e2b0d24192dae658a257`; uncommitted CG-53
  Rust, UI and documentation changes. Product version remains 2.7.0.
- Real macOS release binaries, Community and Enterprise, served the Bun production
  build through `COGNIGRAPH_UI_DIST`. Final assets: `index-0n30fcfz.js` and
  `index-a0evxjb5.css`. The [manifest](manifest.json) binds source and asset hashes.
- Isolated Native servers: Community `127.0.0.1:38476`, Enterprise `:38477`,
  anonymous Community `:38478`, anonymous Enterprise `:38479`. All storage and
  runtime configuration lived under the owned temporary `/tmp/cg53-runtime`.
  Enterprise authentication used the Native multi-tenant configuration.
- Synthetic users covered all nine roles, with two documents, one relationship,
  a space draft, one deterministic imported chunk and a proposed relation hint.
  No existing application data was changed. QA servers used provider `none`.
- Codex in-app browser on macOS. CSS viewport measurements: 1600×1000, 1280×800
  and 1067×667, approximating 100%, 125% and 150% desktop scaling through resize
  emulation. This is not native Windows scaling or mobile coverage.

Screenshot 01 is the pre-change baseline. Screenshots 02–06 used the functional
candidate `index-bvj31byh.js`, before final read-only copy, anonymous metadata and
denial-button cleanup. Screenshots 07–11 used `index-pdhbya75.js`; only the denial
return button's navigation markup changed afterward. Screenshots 12–13, the full
final browser matrix and keyboard return check use `index-0n30fcfz.js`.
Earlier captures remain evidence of their recorded candidate, not the final build.
Initial and final release-binary hashes are recorded separately; their only
intervening Rust change quotes an OpenAPI response description containing commas.
The final HTTP matrix was rerun against the final binaries.

## Executed acceptance

| Check | Observed result |
| --- | --- |
| Verified identity | PASS: `/api/auth/session` returns current identity, server scopes, edition and explicit auth mode. Route tests cover all nine roles through JWT and API token, invalid/missing credentials, deleted identity, tenant suspension and host-control identity. Successful responses use `Cache-Control: no-store`. |
| Navigation and direct routes | PASS: all nine roles signed in in both editions; exact allowed navigation and a forbidden direct route were checked in each of 18 browser journeys. Denied screens show a capability explanation instead of mounting their request effects. |
| Governance and host control | PASS: all four governance roles enter Overview with an accurate API handoff. Enterprise host-admin lands on Tenants; Community explains the edition boundary. Data and user controls stay unavailable to these identities. |
| Reader data access | PASS: Community viewer opens a deep-linked document, reloads it and executes a query returning both synthetic records. Create/Edit/Delete/Embed controls are unavailable. Real API mutation probes remain denied. |
| Persisted editor actions | PASS: Enterprise editor changes a document title and reloads it, retaining the independent numeric field. A relation hint is accepted with a review note; reload and fresh HTTP reads retain the status and `reviewed_by: qa-editor`. |
| Enterprise reader workflows | PASS: Viewer inspects the reviewed neuron without verdict controls. Construct draft/accept/import/propose/review mutations remain disabled; Measure and Advise execute successfully. The zero-question evaluation fixture tests authorization only, not evaluation quality. |
| Lua and graph boundaries | PASS: script-runner executes `return 53` with accurate read-only guidance. Graph exploration follows the existing GraphWrite scope on POST traversal; readers retain Query and receive a specific direct-route explanation. No server scope was broadened. |
| Anonymous development | PASS in both editions: explicit authentication-disabled state, existing data operations and read-only Lua remain available. Identity-dependent user/tenant administration and snapshot export are unavailable. A Lua write receives 403 and creates no document. No Admin principal is invented. |
| Revocation | PASS: deleting the signed-in governance user makes session introspection return 401. Its open browser returns to login on the 15-second verification cycle, without manual reload. |
| Keyboard and scaled layout | PASS: Enter on the denied-page return button opens the permitted workspace. Measured overview panels and construction controls fit the recorded widths; construction results remain reachable through scrolling at 1067×667. |

The [HTTP harness](verify_runtime.py) was executed against disposable seeded
servers and produced [225 HTTP checks and 18 verified identities](runtime-matrix.json).
It takes temporary passwords from the environment and holds bearer tokens only
in memory. The [final browser matrix](browser-matrix.json) records all 18 role/edition
journeys and direct-route denials. [Anonymous results](anonymous-browser.json)
cover both editions. [Independent persistence and revocation reads](persistence-and-revocation.json)
confirm the browser writes after restarting the release servers. The deleted
synthetic policy-author account was subsequently recreated for the final matrix;
its earlier revoked identity remains recorded separately.

## Browser evidence

- [Baseline](01-community-before.jpg), [viewer document](02-viewer-document.jpg),
  [viewer query](03-viewer-query.jpg), [host workspace](04-host-tenant-workspace.jpg),
  [governance workspace](05-governance-workspace.jpg).
- [Editor save after reload](06-editor-save-reloaded.jpg),
  [read-only review](07-viewer-review.jpg), [read-only construction](08-viewer-construction.jpg),
  [scaled advice result](09-viewer-scaled-advice.jpg), [Lua execution](10-script-runner-lua.jpg).
- [Anonymous identity-route denial](11-anonymous-identity-route-denied.jpg),
  [final scaled governance view](12-final-governance-scaled.jpg),
  [revoked account returned to login](13-revoked-governance-login.jpg).
- [Construction geometry](construction-geometry.json), [overview geometry](overview-geometry.json),
  [keyboard return](keyboard-return-dom.txt), [editor review](editor-review-dom.txt).

Overview main panels have equal client and scroll widths at all three measured
sizes. At 1067×667 the governance notice remains within x=100–1039; lower health
cards may require vertical scrolling. This is scoped layout evidence, not a claim
that all content is simultaneously visible. No password values or bearer tokens
are retained in screenshots, DOM snapshots or response records.

## Gates, diagnostics and cleanup

- `python3 scripts/verify.py --suite ci`: PASS, including formatting, 59 Python
  tests, modularity, edition, docs, decision-index and issue checks, strict Clippy
  and Rust tests in both editions. Rust reports 779 default and 970 Enterprise
  passing tests; these totals include environment-gated tests that returned early.
- Arango integration tests skipped for unavailable credentials (eight per edition).
  They are not executed Arango coverage. Existing OpenAI and Gemini embedding
  smoke tests loaded workspace credentials and executed successfully in both
  suites. No completion benchmark, corpus qualification or holdout was run.
- `bun run check`: PASS, Biome and TypeScript (107 files). `bun test`: PASS,
  120 tests in 18 files, 622 assertions. `bun run build`: PASS, 1723 modules.
- The first full CI run caught an OpenAPI flow-YAML description whose unquoted
  commas created extra response keys. The description was corrected, both release
  binaries rebuilt, and the full CI suite rerun successfully.
- [Validation summary](validation.json) records executed commands and boundaries.
  [Captured browser warnings/errors](console.json) are empty. The browser tool
  did not expose an independent network trace; browser persistence and separate
  real HTTP probes provide request/result evidence. Expected denials are recorded.
- The owned browser tab was signed out and closed and its viewport override reset.
  All four owned QA servers were stopped and their temporary data/binaries removed.
  The earlier baseline container and its anonymous volume were also removed.
  Existing user tabs, services and databases were preserved.

No push, remote CI, version bump, Docker image build/publication or deployment
was performed for this candidate. Local success does not establish remote CI.

## Remaining boundaries

Signed-governance console workflows remain planned feature work; the overview
handoff does not implement them. Real provider-backed construction, snapshot
download, transport-failure recovery and mobile layouts were not requalified in
this run. Unit tests cover malformed initial session metadata and bounded probes;
after an established session, transient background transport failures retain the
last verified context while every backend request remains authorized normally.

The next defect is [CG-54](../../../docs/issues/CG-54.md), truncated review and space
lists. The other eight P2 UI issues remain open; this capability work does not
close their execution, provenance, keyboard, error, search, CI, dependency or
navigation-label acceptance criteria. The registry owns the current backlog.
