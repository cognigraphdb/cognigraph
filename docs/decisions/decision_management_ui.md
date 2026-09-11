# Decision: Management UI Starts From Existing Operator APIs

**Status:** Accepted (2026-07-13); amended by the dated addenda below.
The original text describes the 2026-07-13 baseline — the addendum records
the `/api` namespace, JWT login, URL routing, server-hosted console, and the
Review/Users surfaces that supersede parts of it.

## Context

The first CogniGraph UI direction was accepted as a familiar database-management
console: a compact navigation rail, collection table, and contextual JSON
inspector. The next step is to make that console useful without inventing server
contracts or coupling the prototype to unfinished functionality.

## Decision

The standalone React UI under `ui/` calls existing `cognigraph-server` HTTP
endpoints directly. CORS is already enabled by the server. The first integrated
operator slice includes:

- `/health` and `/health/database` for service readiness;
- `/api/documents` and `/api/documents/{collection}/{key}` for document CRUD;
- `/api/search/query` for read-only CGQL;
- `/api/graph/traverse` for graph inspection;
- `/api/users` for authenticated administrative inspection;
- `/api/cache/stats` and `/api/cache/clear` for cache operations;
- `/api/admin/export` for reviewed snapshot download; and
- `/openapi.yaml` as the server-owned API contract.

Server URL and bearer token are session-scoped browser settings. The UI reports
offline, missing-auth, and disabled-feature states explicitly. It does not turn
conditional endpoints into optimistic chrome.

Snapshot import remains disabled until the UI has a file preview, impact
summary, and explicit confirmation flow. Collection discovery also remains out
of scope because the server has document operations but no collection-catalog
endpoint. The initial browser therefore targets the `documents` collection.

## Live Verification

A release `cognigraph-server` binary was run on port 3001 with the native
in-memory backend, query cache, and CGQL mutations enabled. Thirteen realistic
documents and one relationship were seeded through real HTTP calls.

The browser-rendered UI on port 3000 successfully exercised:

- service and database health;
- live document listing, create, JSON edit/save, and delete;
- a read-only CGQL query returning 13 rows;
- an outbound traversal returning the seeded relationship path;
- cache statistics;
- the explicit auth-disabled state for user management; and
- all primary navigation surfaces with no browser console warnings or errors.

An audit follow-up on 2026-07-13 checked every primary page at 1440 x 1024.
It repaired clipped JSON line-number gutters shared by Query, Graph, and
Operations; made sidebar collapse and document page sizing functional; removed
menu affordances without menus; and clarified tenant and auth-disabled states.
Biome, Bun tests, the production build, and a second live-browser pass were
clean after the changes. Evidence is recorded in `ui/audit/audit.md`.

A Graph Explorer follow-up on the same date replaced the traversal-only JSON
panel with a canvas-first visual explorer while preserving JSON as a peer view.
It renders the real `/api/graph/traverse` response as a centered root surrounded by
concentric depth rings, compact draggable document markers, and typed,
confidence-labelled directed edges. It provides zoom, fit, interaction locking,
minimap, node and relationship inspection; opens selected documents in
Collections; expands a selected neighborhood through the same endpoint; and
keeps the currently selected path visible below the canvas. A live
native-backend traversal with 10 paths across two depths verified the dense
state, selection, expansion, Visual/JSON switching, canvas controls, and
document hand-off without any `cognigraph-server` changes.

The canvas was subsequently migrated from React Flow to exact-pinned
`@antv/g6` 5.1.1. G6 now owns the radial layout, canvas drawing, pan/zoom,
node dragging, directed edges, and minimap. CogniGraph retains the approved
inspector, selection/path emphasis, and traversal controls as its application
layer. Bun's direct HTML development server exposed a
circular-initialization failure in G6's ESM barrel, so the UI imports G6's
official prebundled distribution at runtime while continuing to consume the
package's public TypeScript types. The package version, registry integrity, and
absence of install lifecycle scripts were verified before installation.

The CGQL console now uses direct CodeMirror 6 packages rather than a React
wrapper. Its lightweight stream language covers the current CGQL keywords,
built-in functions, operators, literals, bind variables, properties, strings,
and line comments. Rust remains the syntax and semantic authority: after an
idle delay, the editor submits a coordinate-preserving `EXPLAIN` form through
the existing read-only query route and maps server parse locations back to the
source. `EXPLAIN ANALYZE` is reduced to `EXPLAIN` only for background
validation, preventing editor checks from executing work. A local lexical pass
adds missing-bind diagnostics because `EXPLAIN` reports required bind names but
does not require their values.

Live browser verification covered visible line-number gutters, token coloring,
server-mapped syntax errors, missing-bind recovery, keyboard execution, a
three-row live result, and the compact 1280 x 720 layout. The browser console
reported no warnings or errors. Evidence is stored in
`ui/cgql-editor-valid.png` and `ui/cgql-editor-diagnostic.png`.

A shortcut follow-up verified the two combinations independently on macOS.
`Command+Enter` and `Control+Enter` now both execute the query and neither adds
an editor newline; the previous platform-dependent `Mod+Enter` binding only
guaranteed the first behavior on macOS.

An Ant Design follow-up adopted exact-pinned `antd` 6.5.1 as the management
control layer. CogniGraph keeps ownership of the shell, typography, color and
spacing tokens, CodeMirror CGQL editor, JSON renderers, and AntV G6 canvas.
Ant Design now provides forms and validation, inputs and selects, dialogs and
confirmations, buttons, status messages, tabs, loading/empty states, and the
Users table. The document list intentionally retains its compact native table
and cursor-style pager because `/api/documents` reports the loaded page but not a
server-side total; showing Ant Design's numbered pagination would imply a
contract the API does not provide.

Bun's direct HTML bundler exposed CommonJS default-interoperability warnings in
Ant Design's bundled icon definitions. All visible and transient component
icons are therefore supplied from the UI's existing Phosphor icon family,
including loading and confirmation states. This keeps the runtime warning-free
without a build plugin or a second icon system. The migration increased the
minified JavaScript artifact by roughly 0.9 MB, an accepted prototype tradeoff
for consistent validation, dialog, select, table, and state behavior.

Live verification repeated Overview URL validation; Collections selection,
tabs, create validation, and a live temporary create/delete cycle; CGQL execution through
`Control+Enter`; traversal, rerun, expansion, and Visual/JSON switching; the
auth-disabled Users state; and cache statistics plus cache-clear confirmation.
The G6 minimap update race discovered in the baseline audit was fixed by
separating graph construction from subsequent data updates. Browser logs were
clean after a fresh-session pass. Evidence is stored under
`ui/audit/antd-baseline/` and `ui/audit/antd-final/`.

A subsequent rapid-navigation regression showed a second minimap lifecycle
edge case: leaving Graph immediately after entry allowed G6's debounced
`AFTER_RENDER` callback to outlive graph destruction and call `getData()` on a
cleared model. CogniGraph now sets the minimap delay explicitly and gives its
queued render one short browser frame to finish before destroying the graph.
Ten consecutive rapid Graph-to-Users transitions completed without a runtime
overlay, warning, or error, and a settled Graph pass confirmed that the minimap
still mounts. Evidence is in `ui/audit/graph-users-regression/`.

## Consequences

The UI can evolve independently while using the actual server contract. New
management screens should only appear when their endpoint is functional, their
authorization behavior is clear, and destructive actions have an explicit
review step.

Graph visualization remains a read and exploration surface. Relationship
editing and ad hoc graph mutation are deferred until their interaction and
review model is designed explicitly.

## Addendum (2026-07-15): API namespace, sessions, routing, self-hosting

The console matured from a prototype pointed at a dev server into an
operator surface the server hosts itself. Decisions, in dependency order:

**The application API moved under `/api`.** The UI owns `/`; operational
routes (`/health`, `/metrics`, `/openapi.yaml`) stay at the root because
scrapers and load balancers expect fixed paths. The OpenAPI drift tests
enforce the mapping in both directions.

**Sessions replaced pasted bearer tokens.** A username/password login
screen exchanges credentials for a JWT (`POST /api/auth/login`); the token
lives in sessionStorage and an expired or revoked session drops the console
back to sign-in (a 401 on any token-bearing request clears the dead token).
The login screen does not accept an API server URL — the target derives
from the load origin and is displayed instead, so credentials cannot be
redirected by a mistyped or planted address. Operators can still repoint
the API from Overview after signing in.

**Every page has a copyable URL.** react-router with path-based routes
(`/collections`, `/review`, `/users/{username}`); deep links survive the
login gate and land on the requested page; cross-page context rides the
URL (`/collections?doc={key}`), never component state.

**The server hosts the console.** `COGNIGRAPH_UI_DIST` points the server
at `ui/dist`; real files win, any other non-API path serves `index.html`
(the SPA fallback), and unknown `/api` paths answer JSON 404 so an API
error can never come back as 200 text/html. Validating this live caught
two build defects: relative asset URLs broke deep-linked loads, and
`dist/` accumulated stale hashed bundles — the build now emits absolute
URLs and cleans its output directory.

**New screens since the baseline.** Review (neuron queue with
accept/reject/retire and reviewer attribution, graduation flags, a
propose dialog validated by the server's ontology) and Users (account
list + create/delete with a self-delete guard, and a `/users/{username}`
page carrying role facts plus the API-token lifecycle — issue with TTL
presets, rotate, revoke, plaintext shown exactly once).

**Environment conventions hardened by defects found live.** antd Modals
mount/unmount rather than toggling `open` (rc-motion transitions hang
under the Bun dev bundle in both directions; theme motion is disabled and
a CSS guard keeps a stuck frame from hiding a mounted modal). Buttons
never combine a custom icon with antd's `loading` prop. All tables share
the canonical document-table look via theme tokens plus one grouped CSS
block. `ui/TODO.md` tracks progress; `ui/AGENTS.md` records the
conventions.

## Addendum (2026-07-16): retrieval surfaced, search made honest at scale

**The Query console hosts every retrieval mode.** Five tabs — CGQL plus
semantic, hybrid, vector, and graph-augmented — each a toolbar-style form
whose collection selects come from the tenant catalog, with hit tables
that deep-link into the document browser and a collapsible raw response.
Capability gaps stay honest: no embedding provider disables the text
modes with actionable copy, and graph-augmented disables Search until the
tenant has an edge collection. Verifying the tabs against real data
(Ollama embeddings over a DailyMed fixture) exposed two server defects
fixed in the same arc: vector-search hits carried no top-level id (the
UI now falls back to `document._id`), and the query cache ignored
request parameters entirely (decision_cache_backends.md, addendum).

**The document browser searches the whole collection.** The search box
had filtered only the loaded page — decorative at 10,000 documents. A new
`POST /api/search/text` (plain BM25 via `GraphBackend::text_search`, no
embedder required) now backs it: a non-empty query switches the table to
server hits merged with an exact-key lookup (a pasted key pins that
document first), and clearing restores the paged listing. Hits are
addressed by the document's own `_id`, never an application-level
`document_id` field that could shadow the collection/key address.


## Addendum (2026-09-11): preserve document and source identity

The JSON inspector edits raw API fields, separate from display defaults and
inferred embedding metadata. Save PATCHes only changed top-level fields from
the edit-start snapshot. A full replacement reconstructed from API reads is
unsafe because Native sidecar reads omit stored vectors. Creation receipts are
also not stored documents: read the created record before opening its inspector.

Identity, revision and server timestamps are read-only in this editor. Field
removal is explicitly refused, including nested object removals, because Native
and Arango differ in nested PATCH merging. Null is an explicit stored value;
this UI does not equate it with deletion. This amendment governs console editing,
not a new backend concurrency or replacement contract.

Construction imports generate stable versioned content IDs for plain text and
JSONL without IDs. They preserve supplied IDs and reject duplicates/collisions.
Every import previews new/existing sources; rebuilding an existing source's
facts and mentions requires explicit confirmation with current/incoming text.
A pre-submit read detects changes while the preview was open. It does not add
atomic compare-and-swap guarantees against concurrent external writers.

[CG-50](../issues/CG-50.md), [CG-62](../issues/CG-62.md), and the
[dated browser/HTTP report](../../ui/audit/2026-09-11-data-preservation/audit.md)
record the rationale, implementation and verification boundaries. Earlier UI
checkpoints above remain historical evidence, not current full-console acceptance.

## Addendum (2026-09-11): production origin and verified connection state

The production console uses the page origin for API calls. A separate, explicit
Bun development configuration selects the split-port API; production builds
remove that development override. A saved target remains bound to its saved
credentials and can be cleared from the connection error screen.

The application waits for a recognizable successful protected catalog response
before rendering console routes. A 401 opens login. Transport, permission,
server and response-format failures leave authentication unverified and expose
Retry instead of an anonymous admin-labelled shell. Returning to the default
server clears both the old target and its authentication state first.

[CG-49](../issues/CG-49.md) and its
[browser/HTTP evidence](../../ui/audit/2026-09-11-production-origin/audit.md)
record production, development, recovery and anonymous-mode verification.
The [running guide](../operations/running.md#management-console) owns the commands.

### Enterprise probe and deletion follow-up — 2026-09-11

The [CG-51 verification](../../ui/audit/2026-09-11-tenant-deletion/audit.md) exposed
and corrected missing host-admin coverage in CG-49: collection-catalog denial
now triggers a protected tenant-catalog probe. A successful, recognizable
response is still required; the UI does not treat 403 itself as authorization.

Tenant deletion now follows the existing [deletion contract](decision_tenant_deletion.md)
in both its confirmation and result. The dialog names the tenant and consequences;
the page retains the actual quarantine entries. It distinguishes suspension and
same-name recreation, reports an unconfirmed outcome on errors, and restores
keyboard focus after cancellation or completion.

## Addendum (2026-09-11): tenant onboarding and user provisioning

The console creates a tenant and then offers its first tenant-local Admin using
`POST /api/tenants/{name}/admin`. Closing the second step preserves the tenant;
Set up admin on an active tenant row resumes it. The server decides whether an
Admin already exists and refuses repeat bootstrap. A host-admin does not gain
user or data access through onboarding, and the console does not switch sessions
automatically. Success instructs the operator to sign in as the new tenant Admin.

User creation derives its tenant from the authenticated Admin on the server;
the console displays that tenant and sends no tenant override. Host-admin is
never offered. Community offers the four data roles; verified Enterprise edition
metadata enables the four governance roles. Unknown edition metadata keeps the
conservative Community choices. The API remains the authorization boundary.
Role descriptions separate governance authority from tenant data access.

Creation uses the server's default tenant quotas and correctly documents that
`max_active_jobs` is enforced. Quota editing is still a separate planned workflow.
The [auth runbook](../operations/authentication.md#console-tenant-onboarding)
owns operator instructions. [CG-52](../issues/CG-52.md) and the
[browser/HTTP report](../../ui/audit/2026-09-11-provisioning/audit.md) record
verification; role-aware navigation and governance console entry remain CG-53.

## Addendum (2026-09-11): verified edition and role capabilities

The console now uses `GET /api/auth/session` as its connection/identity gate.
The endpoint reuses bearer validation and tenant admission without a required
data scope, returning the actual user, granted scopes, edition and auth mode.
It replaces the catalog probes described above; a governance role need not read
a data catalog to sign in. No existing backend route authorization is broadened.

A shared UI policy governs sidebar entries, direct-route mounting and actions.
An unavailable page explains edition or role restrictions before starting screen
effects. Data readers retain browse/query access and Enterprise review/evaluation
reads; mutations require their existing server scopes. Graph exploration follows
the current POST traversal route's GraphWrite restriction. Governance identities
receive an accurate overview/API handoff, not an empty data dashboard. Host-admin
lands on Enterprise tenant administration. Periodic identity verification detects
revocation even on the public-health-only overview.

Auth-disabled development is explicit, with no invented Admin identity: current
data operations and read-only Lua remain available, while user/tenant administration,
snapshots and signed governance require identity. The
[auth runbook](../operations/authentication.md#console-capability-policy) owns the
policy and [CG-53](../issues/CG-53.md) records scoped runtime acceptance.

## Addendum (2026-09-11): review continuation and independent selection

Review shows 50 rows and fetches one lookahead row to establish continuation.
The visible range and end/more indicator describe the live page; the endpoint's
`count` is not a full-queue total. Space choices load every catalog page before
being published, with explicit failure/retry and searchable options.

Review's space, status, page and selected neuron are URL state, as is Construct's
selected space. The inspector resolves its identity directly, independently of
queue membership. Paging and refresh retain selection; verdicts reload its
persisted status even when it leaves the filter. Graduation links can therefore
open accepted neurons outside the first page. Offset pages do not promise a
stable snapshot across concurrent writes. [CG-54](../issues/CG-54.md) owns the
acceptance record and links the 101-space/402-neuron runtime fixture.

## Addendum (2026-09-11): console execution ownership

Each Lua/CGQL console claims one synchronous submission slot shared by its Run
button, editor shortcuts and query form. React's disabled-button rendering is
feedback, while the slot prevents duplicate execution before the next render.
The slot lasts until the request settles, including failures. Leaving a screen
or connection invalidates its completion callbacks; it does not cancel work
already sent to the server or promise idempotency across tabs/reloads.

[CG-55](../issues/CG-55.md) records the delayed real-request and persisted-write
regressions. Automatic non-executing EXPLAIN validation is separate from explicit
CGQL execution. Submitted-input snapshots and stale-result presentation remain
[CG-56](../issues/CG-56.md).

## Addendum (2026-09-11): clear results when their inputs change

Query, Lua, search and graph results now belong to one API identity, input scope
and execution owner. Editing inputs clears results, errors and timings immediately;
reruns clear the previous result before making the request. Returning to previous
input values does not restore an earlier result. Obsolete completions cannot
publish results or success callbacks. Lua/CGQL continue holding their execution
slot while an already-submitted request finishes, even after input edits.

Graph loading and errors are persistent in both Visual and JSON views, and old
inspectors/paths are removed with the graph. Expansion captures its base graph for
the new result; changing root or edge collection after relationship creation starts
readback in the updated input scope. JSON identifies the most recent traversal
response; Visual combines expanded neighborhoods. [CG-56](../issues/CG-56.md)
owns the verification record and coverage limits.

## Addendum (2026-09-11): native controls inside clickable tables

Navigation rows expose descriptive native links in their identity columns;
Review facts expose native selection buttons with current selection state.
Whole rows do not add tab stops. Row clicks remain a pointer convenience, while
nested links/buttons stop propagation and retain their own action. Anchors use
the shared visible-focus treatment. Edge collections have no document-browser
link, and incomplete search handles have no navigation target.

[CG-57](../issues/CG-57.md) records keyboard, pointer, direct-load, scaled-layout
and read-only-role checks against the Rust-served production console. This does
not certify screen-reader narration or remove the separately tracked collapsed
navigation naming issue.
