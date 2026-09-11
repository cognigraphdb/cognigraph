# UI Development Tracker

Working checklist for the CogniGraph console. One line per item; tick it in
the same commit that lands the work (git blame on the line gives the commit;
add the hash inline when referencing an already-landed commit). The original
endpoint-coverage backlog came from the 2026-07-15 audit; verify current server
capabilities before implementing an item.

Keep planned UI work here. Record defects in the shared
[CG issue registry](../docs/issues/README.md) and link the relevant tickets from
this tracker. [UI instructions](AGENTS.md) define the current workflow.

## Current review — 2026-09-11

The [full code/contract/browser audit](audit/2026-09-11-full-review/audit.md)
recorded 15 open findings at its checkpoint, with real Native Community/Enterprise evidence and
explicit untested boundaries. The historical Done entries below describe
implementation checkpoints; they do not establish current acceptance. The UI
suite now passes 146 helper tests after CG-49–CG-57/CG-62 remediation,
but CI omits it and no browser regression suite exists yet. Keep defect status
in the linked registry rather than duplicating it.

- [x] [Table keyboard access CG-57](../docs/issues/CG-57.md): native links for
  navigation and buttons for review selection, with visible focus and isolated
  pointer shortcuts. [Browser/API evidence](audit/2026-09-11-table-keyboard/audit.md)
  covers Tab/Enter/Space, direct reloads, scaled layouts and Viewer restrictions.
  Five P2s remain open.
- [x] [Result ownership CG-56](../docs/issues/CG-56.md): clear results/timing on
  input edits and reruns; ignore obsolete responses; share graph loading/error
  feedback across both views. [Browser/API evidence](audit/2026-09-11-result-ownership/audit.md)
  covers delayed/reversed requests, failures, expansion and persisted mutations.
  That checkpoint left six P2s open.
- [x] [Execution guards CG-55](../docs/issues/CG-55.md): synchronous submission
  ownership for Lua/CGQL buttons, shortcuts and form submit, with obsolete
  completion suppression. [Browser/API evidence](audit/2026-09-11-execution-guards/audit.md)
  verifies delayed real executions, persisted write counts, errors and recovery.
  That checkpoint left seven P2s open.
- [x] [Review/space paging CG-54](../docs/issues/CG-54.md): truthful queue ranges,
  complete searchable catalogs and off-page selection/graduation links.
  [Browser/API evidence](audit/2026-09-11-review-paging/audit.md) covers 101 spaces,
  402 seeded neurons, persisted verdicts and catalog retry. That checkpoint left eight P2s open.
- [x] [Edition/role capabilities CG-53](../docs/issues/CG-53.md): verified session
  context, scoped navigation/direct routes/actions, governance-role entry and
  explicit anonymous development policy. [Browser/API evidence](audit/2026-09-11-capabilities/audit.md)
  covers all nine roles in both editions. That checkpoint left nine P2s open.
- [x] [Tenant/user provisioning CG-52](../docs/issues/CG-52.md): resumable
  first-admin setup, current-tenant requests and edition-appropriate roles.
  [Browser/API evidence](audit/2026-09-11-provisioning/audit.md) covers new-admin
  login, persisted accounts and denied authority crossings. That checkpoint left ten P2s open.
- [x] [Tenant deletion disclosure CG-51](../docs/issues/CG-51.md): explicit
  consequences, actual quarantine results, cancel/recreation and failure checks.
  [Browser/API evidence](audit/2026-09-11-tenant-deletion/audit.md), including the
  CG-49 follow-up for host-admin catalog verification. All audited P1s are resolved;
  that checkpoint left 11 P2s open.
- [x] [Production origin CG-49](../docs/issues/CG-49.md): same-origin production,
  explicit dev port, verified authentication state and stale-target recovery.
  [Browser/API evidence](audit/2026-09-11-production-origin/audit.md).
- [x] [Document JSON fidelity CG-50](../docs/issues/CG-50.md) and
  [construction import identity CG-62](../docs/issues/CG-62.md): raw JSON editing,
  changed-field PATCH, creation readback, stable import IDs and explicit replacement
  review. [Browser/API evidence](audit/2026-09-11-data-preservation/audit.md).
- [ ] Reconcile [empty/error guidance CG-58](../docs/issues/CG-58.md) and
  [collection search/filter scope CG-59](../docs/issues/CG-59.md).
- [ ] Repair [collapsed navigation names CG-63](../docs/issues/CG-63.md).
- [ ] Establish [UI CI/browser coverage CG-60](../docs/issues/CG-60.md) and
  refresh the [advisory-affected dependency CG-61](../docs/issues/CG-61.md).

## Backend workflows not yet covered by the console

Use the audit's capability map to prioritize complete operator journeys after
defect remediation; these are planned features, not claims of broken endpoints.

- [ ] Durable jobs, asynchronous drafting, status/artifacts, cancellation and recovery.
- [ ] Directed/governed construction, answer evaluation and reviewable deployment.
- [ ] Signed governance keys/policies/approvals, artifact attestations and custody.
- [ ] Promotions and materialized semantic repair/deployment workflows.
- [ ] Dedicated side-view lifecycle and tenant quota management.
- [ ] Complete provider-backed and role/edition browser qualification, including
  actual downloaded snapshot verification. Use controlled disposable data;
  research holdouts and model benchmarks remain separately authorized work.

## Done

- [x] [Product logo](audit/2026-09-10-logo/audit.md): use the supplied CogniGraph
  mark in the expanded/compact sidebar and sign-in screen, with surface colors
  and measured alignment. [Cleaned master refresh](audit/2026-09-11-logo-refresh/audit.md)
  preserves the supplied geometry and existing console colors.
- [x] [Collections QA and fixes](audit/2026-09-10-collections/audit.md): CG-41–CG-44
  cover draft preservation, API save errors, off-page links, scaled desktop
  controls and keyboard dialog focus. Scoped Native/admin verification only.

- [x] UI instruction refresh: executable Bun checks, scoped browser verification,
  shared components, session/routing rules and compatibility-workaround provenance;
  defect tracking aligned with the CG registry.
- [x] [UI QA skill](../.agents/skills/cognigraph-ui-qa/SKILL.md): adapt Symphony's
  reload, measured layout and evidence practices to CogniGraph console flows,
  with shared Codex/Claude guidance. This establishes the workflow, not a fresh
  application acceptance run.
- [x] Console shell: sidebar nav, topbar, health polling, antd v6 theme + phosphor icons (`0f56566`)
- [x] Collections: document list/create/edit/delete with JSON inspector (`documents` collection only) (`0f56566`)
- [x] Query: read-only CGQL console with history (`/api/search/query`) (`0f56566`)
- [x] Graph: canvas-first traversal explorer (G6), JSON peer view, neighborhood expand (`/api/graph/traverse`) (`0f56566`)
- [x] Users: read-only account list (`/api/users`) (`0f56566`)
- [x] Operations: cache stats/clear, snapshot export (`0f56566`)
- [x] Auth: username/password login, JWT session, expiry drops to login (401 handling) (`83efc9e`, `0f56566`)
- [x] Review workspace: neuron queue, accept/reject/retire with attribution + notes, graduation flags, propose dialog (`/api/neurons*`, space types) (`0f56566`)
- [x] Consistency pass: `--workspace-pad` token, base/derivative classes, zero console errors/warnings (`0f56566`)
- [x] Users management: create + delete with role metadata, self-delete guard, token-revocation warning (`POST/DELETE /api/users`; the server has no user-update endpoint, so no edit UI)

- [x] API token lifecycle: issue (TTL presets), rotate, revoke, plaintext shown exactly once (`/api/users/{key}/tokens*`)
- [x] URL routing: react-router with copyable path URLs for every page, deep links survive the login gate, `/collections?doc={key}` opens a document, tab titles follow the route
- [x] User detail page `/users/{username}`: role/tenant/key facts, token lifecycle, guarded delete — replaces the row-expand UX
- [x] Collection catalog end to end: `GET /api/collections` (new `GraphBackend::list_collections`, system collections hidden) + `/collections` index page; the document browser moved to `/collections/{collection}` with one-shot `?doc` deep links and collection-aware dialogs
- [x] Server serves the console: `COGNIGRAPH_UI_DIST` + SPA fallback (real files win, non-`/api` paths get index.html, unknown `/api` paths stay JSON 404); build emits absolute asset URLs and cleans `dist/`

- [x] Collection deletion: `DELETE /api/collections/{name}` (DocumentsWrite scope, system names refused by the guard, semantic cache invalidated) + a per-row Delete on the index behind a confirm that spells out the entry count
- [x] Explicit collection creation end to end: `POST /api/collections` (validated, idempotent, DocumentsWrite scope, `_` names rejected before the GuardedBackend even sees them) + Create-collection dialog on the index that lands in the new empty browser
- [x] Overview is tenant-specific: session identity (user/role/tenant), tenant data metric, and a clickable collections summary from the catalog; the obsolete API-connection card (pre-login era) is gone
- [x] Graph screen honesty: no mock auto-run (the hardcoded vertex + edge collection caused "Collection not found" in any tenant without them); the edge collection is now a Select fed by the catalog, Run is gated on both fields, and the empty state prompts instead of erroring
- [x] FDA-scale test corpus: 10,000 DailyMed labels loaded into the `dailymed` tenant (user `fda`, editor) via the batch API in ~35s; exposed and fixed two scale bugs — the heading showed the loaded page size instead of the catalog count, and pagination was a disabled placeholder (offset hardcoded 0); the browser now pages server-side with true totals
- [x] Session tenant surfaced truthfully: login returns `tenant` (identity-scoped tenancy — the token, not a switcher, selects the tenant); TopBar shows the session's tenant; isolation verified live in `COGNIGRAPH_DATA_DIR` mode (admin@default and bob@acme see disjoint catalogs)
- [x] Tenants screen (host-admin only): list with status/store state, create, suspend/resume, delete (`/api/tenants*`); found and fixed the server minting sessions for suspended tenants' users

## Next up
- [x] Query screen: search-mode tabs — semantic / hybrid / vector / graph-augmented (`/api/search/*`)

## Later

- [x] Documents: server-side search in the browser — search box now queries `POST /api/search/text` (BM25 over the whole collection) plus an exact-key lookup
- [x] Graph: create relationships from the canvas (`POST /api/graph/relationships`) — New-relationship dialog with vertex-address validation; fixing its refresh exposed the G6 updateData crash on newly introduced nodes (setData + render now handles data changes)
- [x] Documents: "generate embedding" action in the inspector (`POST /api/documents/embed` gained an `upsert` mode — the endpoint was insert-only, so re-embedding an existing key answered 409); embedding state is now derived honestly from the stored vector, not just a status field

- [x] Construct pipeline surface beyond neurons: `/construct` screen with draft+accept, ingest, evaluate, propose, judge review, and gate advisor (`/api/construct/*`) — provider/spec/policy requirements surface as the server's own actionable errors
- [x] Lua scripting console (`POST /api/lua/execute`) — `/lua` page with a CodeMirror Lua editor and snippets; building it exposed and fixed the Lua engine escaping tenant routing (spawn_blocking loses the CURRENT_TENANT task-local — scripts read and wrote the DEFAULT tenant's store; TenantScoped now pins the caller's tenant)
- [ ] Snapshot import with preview + impact summary + explicit confirmation (`POST /api/admin/import`; gated per decision_management_ui.md)
- [x] Operations: surface `/metrics` beyond cache stats — a metrics band (requests, 2xx–3xx / 4xx / 5xx, mean latency, uptime) parsed from the root Prometheus endpoint, auto-loaded with a Refresh action; honest `—` state when metrics are unreachable Also added a **Recent errors** panel from `GET /api/admin/logs` (non-2xx ring, tenant-scoped) and non-2xx stdout logging on the server.
- [ ] Mutation-capable query console (`POST /api/query`) and batch operations (`POST /api/batch`)
- [x] antd-purity sweep: `DocumentTable.tsx` converted from a raw `<table>` to antd `Table` (sticky header, fixed column widths, and selection preserved via `tableLayout="fixed"` and a scoped `.table-region .ant-table-thead > tr > th` override — see `components.css`); a multi-agent audit for other raw-HTML-instead-of-antd markup also fixed `LoginScreen`'s hand-rolled error banner (now `ErrorAlert`) and `TopBar`'s native `title` tooltip (now antd `Tooltip`)

## Conventions

- Build only against endpoints that exist and work; show disabled/conditional
  backend capabilities honestly (see [AGENTS.md](AGENTS.md)).
- New screens reuse the base classes (`workspace-header-base`,
  `empty-state-base`, `actions-row-base`, `chip-base`) and `--workspace-pad`.
- Inline errors go through `ErrorAlert`; toasts through `notify()`.
