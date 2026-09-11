# Full UI code and contract review — 2026-09-11

## Outcome

The console is **not fully verified or a complete surface for the implemented
backend**. The review records **15 open findings: 4 P1 and 11 P2**, allocated as
CG-49–CG-63. No application fixes, commits or publication were made in this audit.
Passing helper tests and Rust/Docker checks had not established UI acceptance.

Prioritize the two data-replacement defects, production-origin failure and
misleading tenant-delete confirmation before the next UI release. Then repair
capability/provisioning, result completeness, execution and accessibility, and
make the regression checks part of CI. Product features missing from the console
are listed separately below; their absence is not automatically a runtime bug.

## Reviewed build and environment

- Code HEAD: `8830e8000c3af075e34df607e2a2d7072feb1dfb`, version 2.7.0. UI source
  was unchanged at the start and throughout this audit. Earlier uncommitted
  README and Docker-publication work was preserved. The pending Docker and
  workflow changes were included in the coverage review, not attributed to this audit.
- Reviewed the 13 screen modules, 25 shared components, API/session/routing code,
  domain helpers, theme/styles, build configuration and test/CI structure. The
  [manifest](manifest.json) hashes all 91 files under `ui/src`, including tests,
  declarations and assets, and records the runtime health responses.
- Bun 1.4.2 production build, served by real Rust release binaries in the already
  built local Community and Enterprise images. Assets: `index-956ykrwv.js`,
  `index-85hrkams.css`; the UI was mounted read-only and enabled explicitly with
  `COGNIGRAPH_UI_DIST`. The Docker images do not bundle the UI by default.
- Browser: Codex in-app browser on macOS. Community UI/server at
  `http://127.0.0.1:38471`; Enterprise at `http://127.0.0.1:38472`.
  Both use Native/redb, authentication and disposable data; Enterprise uses
  isolated tenant stores. Synthetic identities and documents only.
- The fresh production-origin check ran with nothing on port 3001 and failed.
  To continue the audit without editing the application, a local proxy forwarded
  port 3001 to the currently tested release server. It preserved real responses
  and authorization. Only the Lua concurrency experiment added a three-second
  response delay. Switching the proxy was an audit workaround, not a product fix.
- Browser sessions: Community admin/viewer and Enterprise admin/host-admin.
  Direct API probes additionally covered editor and script-runner in both
  editions, and an isolated Enterprise tenant administrator.
- No external embedding/completion providers, real customer data, paid model
  calls, frozen experiment changes or holdout execution were used.

## Findings

| Issue | Priority | Confirmed behavior | Evidence level |
| --- | --- | --- | --- |
| [CG-49](../../../docs/issues/CG-49.md) | P1 | Production UI always calls port 3001, preventing first-use login at the actual origin | Fresh browser + direct health |
| [CG-50](../../../docs/issues/CG-50.md) | P1 | Editing only a title overwrites valid string content through a lossy JSON projection | Browser write + independent GET |
| [CG-51](../../../docs/issues/CG-51.md) | P1 | Tenant-delete confirmation falsely implies recreation restores users | Browser delete + HTTP + quarantined file |
| [CG-52](../../../docs/issues/CG-52.md) | P2 | Tenant bootstrap is missing; user provisioning offers forbidden choices and omits current governance roles | Browser + API + source |
| [CG-53](../../../docs/issues/CG-53.md) | P2 | UI capabilities ignore edition and most role restrictions | Browser + 60 API probes |
| [CG-54](../../../docs/issues/CG-54.md) | P2 | Review stops at 200 neurons; space selectors stop at 100 | 201-neuron browser/API; selector source |
| [CG-55](../../../docs/issues/CG-55.md) | P2 | Keyboard shortcuts start another execution while Run is disabled | Two real delayed Lua requests |
| [CG-56](../../../docs/issues/CG-56.md) | P2 | Edited inputs keep unlabelled old results; failed graph JSON reruns hide persistent error | Browser input change + real proxy outage |
| [CG-57](../../../docs/issues/CG-57.md) | P2 | Table navigation/selection is pointer-only | Rendered DOM/accessibility + source |
| [CG-58](../../../docs/issues/CG-58.md) | P2 | Empty review state recommends forbidden writes and masks denied/failed loads | Real Enterprise 403 + browser/source |
| [CG-59](../../../docs/issues/CG-59.md) | P2 | Search caps at 100 and filters cover only loaded records without saying so | 101-document browser/API |
| [CG-60](../../../docs/issues/CG-60.md) | P2 | CI omits UI gates; no browser/API-client regression coverage exists | Workflow and tests review |
| [CG-61](../../../docs/issues/CG-61.md) | P2 | Router dependency audit fails; affected RSC mode is not used here | Bun audit + upstream applicability |
| [CG-62](../../../docs/issues/CG-62.md) | P1 | New plain-text imports reuse positional chunk IDs, replacing older evidence/facts | Browser first ingest + exact-payload HTTP second ingest |
| [CG-63](../../../docs/issues/CG-63.md) | P2 | Manual sidebar collapse removes navigation link names | Browser accessibility tree |

Individual tickets contain source locations, reproduction sequences and closure
criteria. No existing resolved issue was reopened or renumbered.

## Executed flow checks

“Pass” below applies only to the named observation, using the proxy workaround
where stated. It does not establish complete acceptance of the screen.

| Surface | Executed result | Boundaries / failures |
| --- | --- | --- |
| Production load | FAIL: assets load, but a fresh tab calls the wrong API origin | Direct server health succeeds; CG-49 |
| Login/session | PASS via proxy: wrong password shows an inline error; admin/viewer/host-admin login, logout and deep-link continuation work | Revoking the synthetic viewer account through the API made its next query return to login; transport failure is not an auth-success signal |
| Overview | PASS: current identity, tenant catalog and status render at wide desktop size | Viewer cache stats are unavailable; role/edition presentation is incomplete |
| Collections index | PASS: actual catalog and counts, pointer navigation to selected collection | Creation/deletion dialog regressions from the previous Collections audit were not repeated; keyboard open is absent |
| Document inspector | FAIL: string content lost on title-only save, confirmed by GET | Viewer save returns 403 and retains the editable draft; valid object-template behavior alone cannot qualify arbitrary JSON |
| Collection search/paging | PASS: normal server pagination and search return real documents; FAIL: hidden cap and page-local filter options | 101 records, 100 displayed matches versus 101 API matches |
| CGQL | PASS: real query returned the four seeded keys; compact-viewport `RETURN 1067` returned 1067 | Old result remains beside changed input; this run did not repeat the whole CGQL grammar suite |
| Vector retrieval | PASS: `[1,0]` returned Alpha at 1.0000 and Beta at 0.9701 from synthetic two-dimensional vectors | Semantic/hybrid/graph-augmented modes were source-reviewed, not provider-qualified |
| Graph | PASS: Alpha → Beta SUPPORTS traversal rendered, including selected root, depth-one path and JSON; nonexistent root returned an empty result | Failed transport rerun retained old Alpha JSON for entered Gamma; relationship creation, all directions and expansion paths were not requalified |
| Lua | PASS: bounded backend-info script returned successfully; FAIL: Run plus Ctrl+Enter generated two real executions | No duplicate-write experiment was needed to establish the submission defect; tenant-scoped Lua writes were not exhaustively tested |
| Users/detail/tokens | PASS: account list/detail; issue, close one-time grant, rotate, close grant and revoke returned to zero tokens | No plaintext entered tracked evidence. Token authentication before/after rotation was not separately exercised. Forbidden host-admin creation stayed in the form with 403 |
| Tenants | PASS: host-admin create/delete reaches real API; FAIL: confirmation and onboarding instructions contradict current behavior | API bootstrap/new tenant write worked; after delete/recreate old login/session failed and old store was quarantined. Suspend/resume was source-reviewed, not rerun |
| Review | PASS: explicit accept confirmation moved one synthetic neuron from proposed to accepted with admin attribution | 201-row truncation; Community/denied role states misleading. Reject/retire, all proposal kinds and graduation selection were source-reviewed, not exhaustively executed |
| Construct | PASS: one browser plain-text ingest grounded one fact under the seeded accepted space | Second exact-parser-payload HTTP ingest replaced that evidence and removed the fact. Draft/propose/judge provider workflows and governed deployment were not executed |
| Operations | PASS: real metrics/logs, cache stats and export response; export UI reported five collections | Downloaded-file contents were not verified: browser API exposed no saved path and Downloads enumeration was denied. Export is therefore only partially verified. Cache clear and restore not executed; restore remains intentionally disabled |

The [60-case API matrix](role-api-matrix.json) checks catalog read, document write,
read-query, bounded Lua, user administration and tenant administration for five
roles in both editions. Expected denials were enforced; **no authorization bypass
was observed in these cases**. This is not exhaustive security or tenant-isolation
certification. Four additional governance roles were source-reviewed but not
included in the live role matrix.

## Backend coverage map

This map compares [App routes](../../src/App.tsx) with the server's
[router composition](../../../crates/cognigraph-server/src/main.rs) and endpoint
implementations. An arbitrary Lua/query/JSON console is not a complete management
workflow for every backend capability.

| Backend capability | Current UI surface | Status / next work |
| --- | --- | --- |
| Authentication, users and API tokens | Login, Users, account detail | Implemented subset; provisioning/roles need CG-52/CG-53 |
| Collections and documents | Catalog, browser, inspector | Implemented subset; arbitrary JSON fidelity, filters and capabilities need repair |
| Read CGQL and search | Query tabs | Read CGQL and four search forms exist; provider-backed acceptance remains unverified |
| Graph traversal/relationships | Graph | Traversal and create relationship exist; not every graph API operation has a dedicated action |
| Lua | Lua console | Implemented; submission/results need CG-55/CG-56 |
| Tenant lifecycle and quotas | Tenants | Create/status/delete subset; first-admin bootstrap and quota editing absent |
| Neuron lifecycle/graduation | Review | Queue/transitions/proposals exist, with paging and access-state gaps |
| Ontology construction | Construct | Synchronous draft/accept, ingest, evaluate, propose, review and advise subset |
| Durable jobs, async drafting, cancellation/recovery/artifacts | None | Backend implemented; planned UI work. Current draft form always submits synchronously |
| Directed/governed construction and answer evaluation | No dedicated workflow | Backend implemented; not covered by the current simple Construct stages |
| Signed keys/policies/approvals, artifact attestations and custody | None | Backend implemented; no dedicated UI workflow or qualified role journey |
| Promotion decisions and materialized semantic repair/deployment | None | Backend implemented; not equivalent to Review's neuron acceptance |
| Side-view lifecycle | No dedicated workflow | Generic document browsing is not creation/activation/curation coverage |
| Mutation query and batch API | None | Already planned in UI TODO; keep separately authorized from read query |
| Metrics/cache/logs/export | Operations | Basic subset; export file acceptance still pending |
| Snapshot import | Disabled | Deliberate gate pending preview, impact summary and confirmation; not a defect |
| Read replicas/manual standby/failover | None | Runtime is still a design proposal, not a missing UI for delivered replication |

Keep the missing workflow work in [UI TODO](../../TODO.md), after the defects.
Do not infer that every internal backend operation needs a UI button; prioritize
complete operator journeys and explicitly document supported scope.

## Layout, accessibility and diagnostics

Observed CSS viewports: 1600 × 1000 (Overview), 1280 × 800 (graph, users and
operations), and 1067 × 667 (collections/query). These are desktop resize
emulations, not native OS scaling or mobile acceptance. `base.css` currently
requires a 960-pixel minimum viewport.

At 1067 pixels the collection panel ended at x=660.875 and the inspector at
x=1067. Filters, pagination, inspector buttons and footer fit their containers;
page scroll dimensions were 1067 × 667. The stacked query console's Run button
was initially below the fold, then successfully scrolled into view and ran the
query; main scrollTop was 341.5. At 1280 graph and 1600 Overview, document scroll
width equalled viewport width. See [collection](11-collection-search-1067.png),
[compact query](12-query-1067.png), [graph](05-graph-1280.png) and
[wide overview](13-overview-1600.png).

Keyboard/DOM checks found pointer-only table actions and unnamed manually
collapsed navigation. Token dialogs and denied-save input remained usable, but
this run did not repeat every modal focus-return test, both sides of each CSS
breakpoint, screen-reader narration, contrast measurements, native Windows
scaling, cross-browser rendering or mobile behavior. No broad accessibility
pass is claimed.

The browser warning/error-log queries returned no entries. The
[sanitized proxy request log](browser-requests.jsonl) retains real API outcomes
without headers or bodies. Expected 401/403 denials and 404 exact-key probes are
not evidence of a backend authorization regression. The deliberately stopped
proxy caused the final transport failures. Setup mistakes (invalid synthetic IDs
and an initial incorrect document endpoint) were corrected before the relevant
reproductions and are not recorded as product defects.

## Checks and evidence limits

`python3 scripts/verify.py --suite ui` passed frozen install, Biome/TypeScript,
**72 tests in 14 helper-test files, 161 assertions**, and production build. The
bundle includes 1,715 modules and a 3.47 MB JavaScript output. No bundle-size
performance conclusion follows without transfer/compression/runtime measurement.
The [executed gate log](ui-gates.txt) preserves the local results.

`bun audit` failed with one high-severity React Router advisory. Upstream limits
[GHSA-qwww-vcr4-c8h2](https://github.com/advisories/GHSA-qwww-vcr4-c8h2) to unstable
RSC APIs; this static BrowserRouter application has no identified RSC action
integration. CG-61 tracks updating the affected lockfile, not a demonstrated
CSRF exploit here. See [dependency audit](dependency-audit.txt).

The preceding Rust/Docker verification remains a separate checkpoint. This
documentation-only audit did not rerun or claim new full Rust, ArangoDB, remote
CI, deployment, load, provider or holdout coverage. There is no defensible numeric
“percentage covered” without a defined feature/role/edition acceptance matrix.

## Cleanup

The audit's proxy stopped, the two disposable containers and their anonymous
data volumes were removed, the viewport override reset and audit browser tabs
closed. Existing databases, the unrelated local PostgreSQL container, `.env`,
user browser tabs and prior worktree changes were preserved. Temporary synthetic
credentials were removed after evidence capture. Documentation checks passed
with 366 code documents and 1,529 links, or 386 documents and 1,629 links with
the sibling product checkout included. The decision index passed for 56 records;
the issue check passed for 63 issues/registry rows. `git diff --check` passed.
