# CG-52 tenant onboarding and user provisioning — 2026-09-11

**PASS for the scoped acceptance criteria in [CG-52](../../../docs/issues/CG-52.md).**
The console now supports host-admin first-admin bootstrap and current-tenant
user creation with edition-appropriate role choices. These checks do not
establish complete UI or governance-workflow acceptance.

## Candidate and environment

- Base commit: `afea7dc6480762b5a511a8f6e1b6846afc0f9869`; uncommitted CG-52 UI changes.
  Rust source and dependencies unchanged; server version 2.7.0.
- Bun 1.4.2 production build served by real Rust binaries using
  `COGNIGRAPH_UI_DIST=/opt/console/dist`. The UI parent directory was mounted
  read-only so rebuilt assets remained visible without replacing the data store.
- Enterprise: `http://127.0.0.1:38474`, disposable Native multi-tenant storage,
  authentication enabled, `cognigraph:ci-enterprise` image
  `sha256:58c53921b84ab7dc25e7ccab1cbfdbb273326a67a7f1d7ee2fd2310c54b020a6`.
- Community: `http://127.0.0.1:38475`, disposable Native default-tenant storage,
  authentication enabled, `cognigraph:ci` image
  `sha256:7bf8cd56310eeb90d805702b81953eee7c23fc040bde5af6712cc639ff3e8255`.
- Codex in-app browser on macOS. CSS viewport measurements: 1600×1000,
  1280×800 and 1067×667, approximating 100%, 125% and 150% desktop scaling
  through resize emulation. This is not native Windows scaling or mobile coverage.
- Synthetic tenant `qa-onboarding`; host-admin, its first tenant Admin, four
  governance identities and a viewer. Community used the default Admin and a
  synthetic viewer. No provider calls, research data or existing user records.

The [manifest](manifest.json) binds the final source, assets and evidence.
Screenshot 01 is the pre-change baseline. Screenshots 02–08 cover the functional
candidate before the final user-dialog return-focus correction. Screenshots
09–14, keyboard evidence and the final bootstrap check use the final production
bundle, `index-8ftgtswh.js` / `index-a0evxjb5.css`. The earlier candidate used
`index-zf6dn0yx.js`; no provisioning request or role logic changed in that final
focus correction. Earlier captures are retained as observations of that candidate.

## Executed acceptance

| Check | Observed result |
| --- | --- |
| New tenant → first Admin | PASS: create `qa-onboarding` opens first-admin setup immediately. Cancel leaves the tenant; reload retains it and Set up admin resumes the step. |
| Conflict recovery | PASS: existing global username `admin` is refused with inline 409. Changing the username creates `qa-onboarding-admin`. Attempting another bootstrap after that returns the existing-Admin conflict. No duplicate account is created. |
| Session handoff | PASS: success identifies the new Admin/tenant and explains sign-out/sign-in. The host-admin session is unchanged. The new Admin signs in, opens Users and creates accounts. Role-aware landing is still CG-53. |
| Current-tenant provisioning | PASS: the form displays `qa-onboarding` without a tenant input; all six persisted accounts have that tenant after browser reload and independent fresh HTTP reads. |
| Enterprise roles | PASS: eight tenant roles are offered, without host-admin. Browser-created Policy author, Policy approver, Promoter, Artifact attestor and Viewer display the correct roles after reload. All five credentials succeed through real HTTP login. |
| Community roles | PASS: only Admin, Editor, Viewer and Script runner are offered. A browser-created viewer survives reload, signs in and can read collections; Users returns 403 and its create control is disabled. |
| Authority boundaries | PASS: direct cross-tenant and host-admin role creation, host-admin user creation/data/user reads, and ordinary Admin bootstrap all return 403. Governance identities cannot read collections or create users. The user list is unchanged after denied attempts. |
| Unknown edition | PASS in unit tests: missing/unrecognized edition metadata does not enable governance choices. Not a separate live deployment. |
| Quota guidance | PASS by source/contract inspection: creation uses defaults and identifies `max_active_jobs` as enforced. This run did not execute a quota-exhaustion workload. |
| Keyboard and layout | PASS: keyboard cancellation restores the trigger; completed creation restores Refresh after list reload. Measured forms fit at the recorded desktop sizes. Tenant actions wrap within the 1067-pixel viewport. |

The Enterprise verification script [verify_authorization.py](verify_authorization.py)
was executed against the browser-created fixtures. It takes temporary passwords
from the environment, keeps successful login tokens in memory, checks expected
status codes and writes [sanitized authorization results](authorization.json).
It must run only against an isolated server with the documented synthetic
accounts. [Community HTTP results](community.json) independently confirm edition,
persisted roles, viewer data access and user-administration denial.

## Browser evidence

- [Old tenant guidance](01-old-onboarding.png), [automatic bootstrap step](02-bootstrap-step.png),
  [username conflict](03-bootstrap-conflict.png), [first Admin created](04-admin-created.png),
  [repeat bootstrap refused](05-existing-admin-denied.png).
- [Enterprise role choices](06-enterprise-role-options.png),
  [created accounts after reload](07-provisioned-accounts-reloaded.png),
  [scaled user form](08-user-form-scaled.png).
- [Community role choices](09-community-role-options.png),
  [Community account after reload](10-community-created-reloaded.png),
  [viewer denial](11-viewer-user-admin-denied.png),
  [host-admin user-management denial](12-host-user-admin-denied.png).
- [Scaled tenant actions](13-tenant-actions-scaled.png),
  [final scaled bootstrap form](14-final-bootstrap-form.png).
- [User form geometry](user-form-geometry.json),
  [tenant action geometry](tenant-actions-geometry.json),
  [bootstrap geometry](bootstrap-geometry.json), [return focus](keyboard.json).

The user form is 520 CSS pixels wide, with no internal overflow at all three
measured sizes. At 1067×667, the bootstrap dialog lies within x=273.5–793.5 and
y=100–461; the tenant action group remains inside the viewport. Password inputs
were cleared before conflict captures; no passwords or login tokens are retained
in screenshots, DOM snapshots or response records.

## Gates, diagnostics and cleanup

- `bun run check`: PASS, Biome and TypeScript (104 files).
- `bun test`: PASS, 100 tests in 17 files, 303 assertions.
- `bun run build`: PASS, 1721 modules. Final asset hashes are in the manifest.
- Browser diagnostics: [no captured warnings or errors](console.json) in either
  final session. The browser's available log stream did not expose an independent
  network trace; inline error states, persisted browser results and separate real
  HTTP probes provide the request/result evidence. Expected 403/409 responses
  are accounted for above.
- Both temporary browser sessions were signed out and closed; the viewport
  override was reset. Containers `cg-provisioning-qa` and
  `cg-provisioning-community` and their anonymous data volumes were removed.
  Existing user services and tabs were preserved.

No Rust source changed, so the full Rust suites were not rerun. No remote CI,
push, version bump, image publication or deployment was performed.

## Remaining boundaries

[CG-53](../../../docs/issues/CG-53.md) still owns edition/role-aware console
navigation, including governance-role entry: HTTP credentials work, but the
current catalog-based console gate does not admit governance-only identities.
Signed-governance workflows and quota editing remain planned UI features.
[CG-58](../../../docs/issues/CG-58.md) still owns misleading enable-auth guidance
on an authenticated user's 403. Existing table-row keyboard access is CG-57.
No error fixture interception was used in this run; unknown edition handling is
unit coverage and runtime transport-failure recovery was not requalified here.
