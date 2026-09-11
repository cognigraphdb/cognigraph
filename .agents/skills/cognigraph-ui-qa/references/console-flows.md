# Console flow checks

Read the sections relevant to the changed behavior. Verify routes and endpoint
contracts against [App.tsx](../../../../ui/src/App.tsx), the
[API client](../../../../ui/src/api/client.ts) and
[OpenAPI](../../../../crates/cognigraph-server/openapi.yaml) before running checks.
Use disposable local data; preserve existing confirmation and capability gates.

## Collections and document inspection

For collection/document CRUD, exercise create, selection, edit, cancel and delete
as applicable. Confirm the list, total count, inspector and a fresh read agree
after a mutation. Test empty collections, multiple pages, long keys and invalid
JSON. Verify search reaches records beyond the loaded page and changing search
does not leave unrelated selection presented as a current result.

Check `/collections/:collection` direct navigation and the supported `?doc=` link,
refresh, and back/forward behavior. Confirm collection deletion names the target
and impact, cancellation leaves it intact, and a failed write preserves useful
input with an actionable error.

## Query, graph and Lua

- Associate query results with the query, bindings, collection and search mode
  that actually ran. Changing input must not imply old results came from the new
  values. Clear or visibly distinguish stale results, including after a failed
  rerun. Check that delayed responses cannot overwrite newer results unnoticed.
- Test invalid syntax and empty results separately from transport/provider
  failures. Treat the read query screen and Lua execution according to their
  actual server capabilities. Do not enable the planned mutation query/batch UI
  merely to perform QA.
- For graph changes, verify selected node details, traversal direction/depth,
  expansion and collection choice against a small known graph. Exercise canvas
  resize and navigation away/back. If relationship creation changes, confirm the
  edge through a fresh API read and canvas refresh; a drawn line alone is not proof.
- For Lua changes, use a bounded script with known results. Check output and
  errors, and verify tenant-scoped side effects through a fresh read. Do not
  assume the read-only query screen makes arbitrary Lua scripts read-only.

## Sessions, users and tenants

Check deep links through login, reload, logout, and a token-bearing 401 when
authentication changes. Confirm session identity and tenant labels match the
server; tenancy is identity-scoped, not selected by a cosmetic UI switcher.

For access changes, use the relevant permitted actor and an actor the API must
deny. Verify direct API and route access independently of hidden navigation.
Check user creation/deletion, token issuance/rotation/revocation and tenant
lifecycle only when in scope, preserving confirmations and self-protection rules.
One-time tokens must not reappear after reload or enter tracked evidence. A host
administrator's tenant lifecycle access does not imply access to tenant documents.

## Review, construction and operations

Check review transitions, attribution and notes against persisted records after
reload. Distinguish proposed/draft work from accepted state. For construction,
show required provider/spec/policy errors and verify only the stages in scope;
UI QA does not itself authorize a new model benchmark or holdout run.

Verify unavailable metrics/cache data is distinguishable from a true zero. For
snapshot export, confirm an actual downloaded file, expected format and parseable
content using synthetic data; a success toast is insufficient. Snapshot import
remains gated by preview, impact and confirmation in the
[management UI decision](../../../../docs/decisions/decision_management_ui.md).
An export check does not require restoring a snapshot into an existing database.
