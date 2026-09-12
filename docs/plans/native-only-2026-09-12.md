# Native-only storage engineering batch

- Date: 2026-09-12
- Status: CG-65 complete; CG-67 next, followed by CG-68
- Decision: [Native-only storage](../decisions/decision_native_only.md)
- Baseline: CogniGraph 2.7.1, revision `efe9839`

## Goal and owner clarification

Complete Native-only cleanup before the first live deployment. The owner confirms
that CogniGraph has never been provided to anyone; breaking changes are allowed
freely during this phase. Public source checkpoints do not create an installed
customer migration requirement. The adapter still exists today; this plan does
not claim removal or readiness has already passed.

The initial migration-first draft is replaced by the order below. There is no
customer cutover, legacy-support period, compatibility bridge, application-state
migration or mandatory major-version jump for removing this unused backend.

## Active work order

| Order | Issue | Deliverable | Prerequisites |
|---|---|---|---|
| 1 | [CG-65](../issues/CG-65.md) | Preserve useful Native contracts and replace necessary Arango-dependent test doubles | Accepted decision |
| 2 | [CG-67](../issues/CG-67.md) | Delete Arango runtime/dependencies/configuration and simplify Native startup | CG-65 |
| 3 | [CG-68](../issues/CG-68.md) | Reconcile active docs/workflows and verify Native-only readiness before live deployment | CG-65 and CG-67 |

CG-65 is complete: the [coverage report](../issues/native-conformance-2026-09-12.md)
records the retained contracts, replacement doubles and local Rust/release-binary
verification. Proceed with CG-67; it does not wait for an importer. The
[registry](../issues/README.md) owns statuses.

## Deferred optional backlog

| Issue | Possible future work | Scheduling |
|---|---|---|
| [CG-64](../issues/CG-64.md) | Qualify external Arango dump formats and synthetic fixtures | Reconsider after Native-only readiness |
| [CG-66](../issues/CG-66.md) | Build a Community offline dump importer | Only if prioritized later, after CG-64 |

These two tickets remain Open with explicit deferred scheduling; they are neither
implemented nor prerequisites for completion of this batch or first deployment.
The [import design](arangodump-import-design.md) is an unqualified proposal.
Keep the [AQL-to-CGQL guide](../reference/aql-to-cgql.md) as external migration
education; it must not suggest that CogniGraph needs or runs over Arango.

## Coverage and implementation scope

Retain shared CRUD, edge, traversal, vector, scan and query behavior across Native
memory, persistent resident and supported paged/sidecar modes. Preserve expected
CGQL results, authorization/error/budget tests and necessary failure doubles.
Delete adapter-only tests. No fresh Arango capture, service, compatibility corpus
or one-to-one preservation of every adapter test is required.

Remove the crate and all dependency paths, startup branches, Arango settings and
obsolete AQL/query-language dispatch. Retain `GraphBackend`, useful capabilities
and routed/cache boundaries; simplify obsolete abstractions without moving
storage implementation into HTTP/Lua handlers. Native storage modes remain.

There is no required legacy configuration shim. Remove the backend selector or
validate any retained selector generically as Native-only. Eliminate fallback
from an invalid backend value to a newly created store. Do not keep Arango-only
help text just to explain its former existence.

## Active documentation cleanup

Once implemented, describe Native directly. Remove legacy-backend choices and
maintenance/deprecation narratives from normal setup, architecture and user flows.
The implementation plan and decision log may record the engineering transition;
prior verified results remain in their dated records rather than being rewritten.

| Surface | Required reconciliation |
|---|---|
| Workspace and builds | Cargo manifests/lockfile, both edition dependency trees, Docker and CI scripts |
| Runtime | Server config/startup, capabilities, health/status, Lua/query/search branches |
| Operators | Running/configuration/recovery/CLI guides, `.env.example`, Helm examples; no Arango setup or migration prerequisite |
| Architecture and workflow | Components, decision index, implementation plan, AGENTS.md, relevant skills and executable guards |
| Console | Backend labels and capability assumptions; regress affected journeys without adding unrelated features |
| Engineering history | Preserve old issue identities and sealed evidence; remove historical assertions from active runtime guidance |
| Separate product material | Record any remaining sibling docs/website alignment with its path; A9 may remain external migration education. These repositories are not published by this code batch |

## Readiness gates

For Rust changes, run formatting, strict Clippy and all workspace tests required
by [AGENTS.md](../../AGENTS.md). Resolve tickets only after real-binary checks.
The final candidate must demonstrate:

- Both editions build with no Arango crate or replacement runtime driver.
- Fresh Native startup, auth, CRUD, CGQL, Lua, traversal, text/vector retrieval,
  snapshots and restart persistence work with synthetic local data. Cover the
  supported storage modes and relevant Enterprise tenant/authority boundaries.
- Removed configuration has no operational path; any retained selector rejects
  unsupported values before writes. Current startup/help/docs contain no backend
  choice that cannot execute.
- The shared local CI suite, current browser regressions, both Linux/amd64 Docker
  images and packaged runtime checks pass. Run both Helm live backup checks when
  chart/application packaging changes; re-read current CI for the final gate.
- New issue evidence records the exact commit, version, edition, commands,
  fixture hashes, results and exclusions. No historical test result or skipped
  external-service case is presented as new Native acceptance.

Importer execution, external Arango checks, model calls and research holdouts are
outside this gate. The completed console defects stay closed; new UI features
remain a separate backlog.

## Publication and first deployment

The owner permits breaking cleanup before first delivery; no 3.0.0 requirement
is imposed solely by adapter removal. Select the outgoing version under the
[push workflow](../operations/push.md), which still requires a version increment,
incoming-work review and full applicable validation. Planning does not bump Cargo.

CG-68 records local Native-only readiness. Only then may live-environment
preparation/deployment proceed when requested by the owner. Source publication,
remote CI, image publication and actual deployment remain separately reported
operations. No existing-customer rollback or last-Arango release is required.
