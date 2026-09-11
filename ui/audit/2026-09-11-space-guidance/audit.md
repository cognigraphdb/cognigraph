# CG-58: supported space setup and truthful catalog states

Date: 2026-09-11. Result: **PASS for CG-58's scoped acceptance**.

## Candidate and environment

- Base commit `4de66d5` contains CG-56/CG-57. This report covers uncommitted
  CG-58 source changes. The [manifest](manifest.json) binds source, built assets,
  runtime binaries and evidence. Earlier ticket evidence remains unchanged.
- Bun 1.4.2 production UI, final JavaScript `index-vt4b3g5q.js` and CSS
  `index-rhypvyqh.css`, served by real Rust Native servers through
  `COGNIGRAPH_UI_DIST`. No Rust source changed or binary rebuild was required.
- Enterprise 2.7.0: UI/API `http://127.0.0.1:38489`, fresh Native default tenant,
  synthetic Admin, Viewer and HostAdmin. Proxy UI `http://127.0.0.1:38490` forwards
  to that instance. All models/providers were disabled for these QA servers.
- Community 2.7.0: `http://127.0.0.1:38491`, separate fresh store and Admin.
  This uses the cached initial CG-53 Community binary, hash beginning `757f2b28`,
  solely to verify current UI session/edition gating. It is not a rebuild of the
  latest Rust tree. Enterprise uses hash beginning `186c1911`.
- Codex in-app browser, initial 1280 × 720 CSS pixels. Final error layouts were
  measured at 1280 × 800 and 1067 × 667, emulating 125%/150% of a 1600 × 1000
  desktop across the current layout breakpoint. No native OS scaling claim.

## Runtime findings and fixture

The baseline fresh tenant did not yet contain `space_types`. Native returned
404 for its document listing, and the previous UI displayed a load failure.
CG-58 now verifies collection absence through a successful `/api/collections`
read before treating this first-page 404 as empty. A 404 for an existing
collection, a failed fallback and a failed continuation remain errors.

[seed.py](seed.py) checks the fresh catalog, attempts the forbidden generic
accepted-space POST, creates an inert `space_type_drafts/qa-guidance` document
through the documents API and creates the Viewer fixture. The draft contains
two synthetic entities, Rigel and Vega, and one relation rule. It is not accepted
by the script. [Seed readback](seed.json).

To reproduce, start the two isolated servers with temporary credentials and
`COGNIGRAPH_EMBEDDING_PROVIDER=none`; run the seed with `CG58_QA_PASSWORD` matching
the Enterprise administrator. The [proxy](proxy.py) uses `CG58_QA_HOST_PASSWORD`
for its synthetic host account and an owned `/tmp/cg58-qa/catalog-mode` file.
Never point these scripts at an existing application store. Tokens stay in
process memory; captures contain no credentials.

## Executed acceptance

| Check | Observed result |
|---|---|
| Fresh Enterprise tenant | The absent collection is independently confirmed. Review and Construct show “No accepted spaces in this tenant,” with the supported draft workflow. An inert draft does not change that state. |
| Reader guidance | Viewer sees guidance to an editor/administrator, with no create/accept instruction presented as an available Viewer action. |
| Draft → review → accept | Review's Open Construct link works. Entering the existing draft ID and following Review stored draft opens its current JSON. Accept draft calls the dedicated endpoint, refreshes the catalog and selects the accepted space. No model call is needed. |
| Persistence | Review opens the accepted space with an empty neuron queue. Reload and independent HTTP reads retain the space, draft acceptance and `accepted_by: admin`. |
| Managed mutation guards | Generic POST to `space_types` returns 403 before acceptance; generic PATCH returns 403 afterward. The accepted document remains unchanged. |
| Existing space, denied role | HostAdmin cannot open Review or Construct and is not told the tenant is empty. A direct space-list request returns 403 while the accepted space exists. |
| Edition absence | Community Admin receives the Enterprise requirement on both direct routes; no empty-space bootstrap guidance appears. |
| Catalog denial | Both screens distinguish a backend 403 as Space access denied, with Retry. The proxy obtains this real 403 by forwarding the catalog request with the synthetic host token while preserving the browser's Admin session. This is explicitly an authorization-mismatch fixture, not a claim about normal Admin permissions. |
| Transport failure and retry | Closing the space-list connection produces Unable to load spaces / Failed to fetch. Neither screen claims zero spaces. Retry recovers the real catalog; Construct retains its requested ID and re-enables dependent actions only after the catalog succeeds. |
| Loading and keyboard retry | An eight-second delay of a real response shows Loading spaces, without empty guidance. Tab/Enter activates Construct's Retry button with visible focus; the request recovers. |

[Final HTTP readback and served-asset hashes](readback.json),
[request trace](requests.json), and [keyboard record](retry-keyboard.json).
The trace omits headers, tokens and bodies. Fault injection affects only the
space-list route: `fail` closes the connection before forwarding, `slow` delays
the real request, and `denied` obtains the host's real backend denial. These
controlled cases do not indicate a production outage or changed Admin scopes.

Selected captures:

- [Final fresh Review](03-final-fresh-review.jpg), [fresh Construct](04-fresh-construct.jpg),
  [Viewer Review](06-viewer-empty-review.jpg), [Viewer Construct](07-viewer-empty-construct.jpg).
- [Stored draft](05-stored-draft.jpg), [accepted result](08-accepted-space.jpg),
  [Review after acceptance](09-review-after-acceptance.jpg), [reload](11-accepted-space-reload.jpg).
- [Host Review denial](10-host-review-denied.jpg), [Host Construct denial](12-host-construct-denied.jpg),
  [Community Review](14-community-review.jpg), [Community Construct](16-community-construct.jpg).
- [Review failure](13-review-transport-failure.jpg), [loading retry](15-review-loading-retry.jpg),
  [recovery](17-review-recovered.jpg), [catalog denial](18-review-catalog-denied.jpg).
- [Construct denial](19-construct-catalog-denied.jpg), [keyboard loading](23-construct-loading-keyboard-retry.jpg)
  and [recovery](24-construct-recovered.jpg).

## Layout and candidate chronology

[Final measurements](layout-final.json) confirm both screens' error notice and
Retry controls fit the actual 1280 × 800 and 1067 × 667 viewports, with no
horizontal page overflow. Construct uses its existing scrolling workspace.
[Construct at 1067 × 667](25-construct-error-1067-final.jpg) and
[Review at 1067 × 667](28-review-error-1067-final.jpg) were visually inspected.

Capture 01 is the previous build. Capture 02 uses the intermediate CG-58 build
`index-4d7vkxsr.js`, before dependent-space loading/failure guards; captures 03
onward use the final bundle. Captures 21/22 were attempted resize checks while
another QA tab owned the viewport override. Their actual dimensions remained
1280 × 720, as [initial measurements](layout-initial.json) show. They are not
scaled-layout evidence; captures 25–28 and the final measurements replace that
claim without rewriting the initial files.

## Validation, diagnostics and limits

- `bun run check`: PASS, 119 files plus TypeScript. Formatting and a mock-fetch
  type annotation were corrected before final verification.
- `bun test`: PASS, 151 tests across 21 files, 745 assertions. New cases cover
  successful emptiness, verified absence, existing-collection 404, preserved
  403/503 status and failed/invalid fallback catalogs. Existing continuation,
  cancellation and role/edition tests remain passing.
- `bun run build`: PASS, 1732 modules. Both servers' served JS/CSS bytes match
  the final local production build. Documentation and registry checks pass.
- Captured console warning/error lists are empty: [direct](console-direct.json),
  [proxy](console-proxy.json), [Community](console-community.json). Expected
  network failures and denials are recorded separately in the request trace and
  HTTP readback; console emptiness is not a claim that no HTTP request failed.
- No provider-backed drafting, signed candidate construction, ArangoDB,
  screen-reader narration, native OS scaling or remote CI was requalified.
  The manually prepared draft and typed legacy-authoring acceptance path are
  the tested workflow. No model benchmark or sealed holdout ran.

## Cleanup and follow-up

All three owned QA tabs were signed out and closed; the temporary viewport
override was reset. Both servers and the proxy were stopped, and their owned
stores, text index and mode file were removed. The user's existing browser tab,
application stores and earlier evidence were retained. A discarded binary
identification attempt briefly started a debug in-memory server on port 3000;
it was stopped before any QA request was sent to it and is not acceptance evidence.

[CG-58](../../../docs/issues/CG-58.md) is resolved. The new changes remain
uncommitted; the preceding CG-56/CG-57 changes were committed as `4de66d5`.
Next is [CG-59](../../../docs/issues/CG-59.md), collection search/filter completeness.
