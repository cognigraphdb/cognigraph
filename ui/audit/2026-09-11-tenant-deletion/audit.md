# Tenant deletion disclosure and actual outcomes

Date: 2026-09-11. Scope: [CG-51](../../../docs/issues/CG-51.md) and the Enterprise
host-admin verification gap discovered in [CG-49](../../../docs/issues/CG-49.md).
Result: PASS for the scoped acceptance below; 11 audited P2 issues remain open.

## Candidate and environment

- Base commit `50ed036` committed CG-49 before this work. These UI, test and
  documentation changes were uncommitted during verification. Rust was unchanged.
- Bun 1.4.2, Chromium in the Codex in-app browser on macOS.
- Actual Rust Enterprise 2.7.0 binary in local image `cognigraph:ci-enterprise`,
  ID `sha256:58c53921b84ab7dc25e7ccab1cbfdbb273326a67a7f1d7ee2fd2310c54b020a6`.
- Native isolated stores at `/data/tenants`, authentication enabled, synthetic
  host-admin/default-admin and tenant administrators; no model/provider calls.
- Production console served by Rust at `http://127.0.0.1:38473/tenants`, with
  `COGNIGRAPH_UI_DIST=/opt/ui` mounted from `ui/dist`. The container was restarted
  after rebuilds so the replaced distribution directory was remounted.
- Final assets: `index-9dechgys.js`, `index-0bvaa15g.css`. Source and capture hashes
  are in [manifest.json](manifest.json). Screenshots 01–10 include intermediate
  candidates; 11–13 and the final DOM files use the final bundle. The final small
  amendment replaced a deprecated Ant Design mask option; lifecycle behavior
  had already passed and final cancel/delete/focus/authorization checks passed again.

## Executed results

| Scenario | Result and evidence |
| --- | --- |
| Baseline | Confirmed the old wording promised that recreating the name would restore access. [Old confirmation](02-old-confirmation.png). |
| Host-admin verification | Initial CG-49 bundle blocked a valid host-admin because the data catalog returned 403. The fix verifies the permitted tenant catalog after that denial. Fresh login and reload then succeeded; host-admin data reads still returned 403. [Regression](01-host-admin-probe-regression.png), [final login DOM](final-host-admin-dom.txt), [HTTP](lifecycle.json). |
| Consequences before confirmation | PASS. The dialog names the target and states account/API-token removal, session rejection, queued-work/promotion retirement, quarantine, separate recovery and empty recreation. It recommends Suspend for a temporary access pause. [Final dialog](11-final-modal.png), [longer name](03-corrected-confirmation.png), [DOM](confirmation-dom.txt). |
| Cancel | PASS. Keyboard Cancel closed the dialog; an independent HTTP read returned the unchanged synthetic document and the existing login/API token still worked. Focus returned to the Delete button. [HTTP](lifecycle.json), [final keyboard state](final-keyboard.json). |
| Confirm deletion | PASS. The tenant disappeared from the catalog, including after reload. The page reported its exact `.redb.deleted-…` entry, which independently matched the filesystem. Login, the old JWT and API token all returned 401. [Result](04-deletion-result.png), [reload](05-deletion-reloaded.png), [DOM](deletion-result-dom.txt), [HTTP/filesystem](lifecycle.json). |
| Recreate the same name | PASS. Created the same name through the browser. Its incarnation changed; old login/JWT/API token stayed invalid. A new first administrator was bootstrapped through the API and saw an empty catalog. The new active `.redb` and the old quarantined file coexisted. [Independent evidence](lifecycle.json). |
| No files and stale record | PASS. Deleting unopened `qa-empty` displayed no reported quarantine entries. A separate HTTP client deleted `qa-stale` while its browser confirmation was open; confirming the stale row displayed `No tenant record was deleted`, reflecting `deleted: false`. [Empty result](07-no-quarantine-entries.png), [stale result](08-already-deleted.png). |
| Disconnected request | PASS. Stopped the disposable API with a dialog open, then confirmed. The dialog reported `Failed to fetch`, did not claim success and disabled another attempt until closed. After restart, an independent catalog read confirmed the tenant remained. Refresh and a subsequent real deletion succeeded. [Failure](09-request-failure.png), [DOM](request-failure-dom.txt). |
| Final bundle | PASS. Keyboard Cancel returned to Delete; a confirmed deletion returned to Refresh after the list finished loading. No-quarantine success was shown and the deleted row disappeared. [Final result](12-final-success.png), [DOM](final-bundle-result-dom.txt), [focus](final-keyboard.json). |
| Authorization boundary | PASS. A fresh ordinary data-admin login succeeded, but direct `/tenants` access displayed the server's 403 and disabled creation. Host-admin login was separately verified. [Denied page](13-data-admin-denied.png), [DOM](data-admin-denied-dom.txt). |

The [interactive verifier](verify_lifecycle.py) held issued secrets only in process
memory while the browser performed cancel/delete/recreate. It asserted response
statuses, unchanged document content, changed incarnation, empty replacement
catalog and quarantine files, then wrote the sanitized [lifecycle record](lifecycle.json).
It is intended only for the disposable configuration above; supply its named
password environment variables and follow its prompts on a fresh test instance.

## Checks, layout and diagnostics

- `bun run check`: PASS, Biome and TypeScript, 103 files.
- `bun test`: PASS, 96 tests across 17 files, 267 assertions. New coverage checks
  actual deletion flags/quarantine lists, malformed results, host-admin catalog
  fallback and failure/expiry propagation. Fixtures are unit coverage; the
  lifecycle table above records separate real-server execution.
- `bun run build`: PASS, 1,720 modules. No dependency/lockfile change.
- [Geometry](dialog-geometry.json) measured the complete dialog and all actions
  within 1600×1000 and 1280×800 CSS viewports, without internal clipping or
  horizontal page overflow. The third immediate measurement still reported
  1280×800 while the resize was applying; it is not a 1067×667 measurement.
  The subsequent [final screenshot](11-final-modal.png) is 1067×667 and was
  visually inspected with all copy/actions visible. These are resize emulations,
  not native OS scaling tests. Keyboard focus is visible and the dialog is named.
- [Console history](console.json) records intermediate `maskClosable` deprecation
  warnings. The final bundle uses `mask.closable`; after opening, cancelling and
  confirming the final dialog, its only recorded message was informational React
  DevTools advice, with no application warning/error. The 403 catalog probe and
  deliberately disconnected request were expected failures. No browser network
  trace is claimed; HTTP assertions and visible outcomes corroborate the flow.

## Cleanup and boundaries

Signed out the synthetic browser session, closed the owned test tab and restored
the viewport. Removed `cg-tenant-deletion-qa` and its anonymous data volume,
including all synthetic accounts, tokens, active stores and quarantines. Existing
user tabs/services and local environment files were preserved.

Queued-job retirement, promotions under load, admitted-request draining and
operator file recovery/purge were not exercised in this UI run; the confirmation
reflects the existing [server contract](../../../docs/decisions/decision_tenant_deletion.md).
No full Rust suite, ArangoDB, remote CI, deployment or publication was run.
Mobile, native Windows scaling and screen-reader coverage are untested. The
result panel is in-page feedback and clears on navigation/reload; it is not a
durable audit log. [CG-52](../../../docs/issues/CG-52.md) provisioning and
[CG-53](../../../docs/issues/CG-53.md) role/edition presentation remain next.
