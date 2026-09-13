# CogniGraph implementation plan

Current engineering status, reviewed 2026-09-13. Detailed historical delivery
and test counts live in [change records](changelog/README.md), the
[issue registry](issues/README.md) and [archived implementation history](plans/archive/implementation-history-through-2026-09-09.md).

The shared Railway database is **stopped between explicitly requested hosted QA
sessions**, with its volume retained. Builds, CI, pushes and releases do not
initiate Railway tests. It is an on-demand Community test environment, not a shared
production service or public demo; applications deploy their own instances.
The [hosted QA decision](decisions/decision_hosted_qa_lifecycle.md) and
[CG-76](issues/CG-76.md) own this lifecycle. Automatic database deployments are
disabled; the separate website remains live. The latest hosted qualification
was Community v2.7.11 including the console. [CG-71](issues/CG-71.md) records the first deployment's passing
local/GitHub CI, hosted API/browser checks, restart and both application and
platform restore drills. [Railway operations](operations/railway.md) owns setup
and recovery. [Docker Hub account setup](operations/docker-publishing.md#verified-account-setup--2026-09-13)
is complete: both public edition repositories, immutable tags, overviews and the
existing active publisher PAT are configured. Community uses the included Scout
analysis slot. The [v2.7.11 release](operations/docker-hub/release-2.7.11.md)
passed local and remote CI, published both immutable Linux/amd64 edition images,
and updated Railway after its CI gate. Independent pulls of both digests pass
runtime checks. Hosted data equality, authenticated operations and phone-login
layout checks pass. [CG-74](issues/CG-74.md) resolves the selector race that
blocked the earlier v2.7.10 promotion. The separate website remains live.

The [v2.7.12 source release](changelog/2026-09-13-v2-7-12.md) groups the public
evidence split, CG-75–CG-78, explicit hosted QA and Docker cleanup rules, and
[CG-79](issues/CG-79.md)'s temporary SPA fixture cleanup. Its source promotion
passed the full local and remote CI gates and reached both integration and release
branches. The [v2.7.13 source snapshot](changelog/2026-09-13-v2-7-13.md) retains
that implementation with refreshed release metadata and a reviewed public root.
Docker Hub remains at the previously published v2.7.11 images; new image
publication is a separate operation.

## Completed dependency and branch maintenance

[CG-73](issues/CG-73.md) integrates six incoming dependency PRs, establishes
`develop` as the default integration branch, replaces Dependabot with Renovate,
and removes obsolete topic branches after preserving their work. The
[branch decision](decisions/decision_develop_integration.md) keeps main as the
separately promoted production release branch. Candidate v2.7.8 includes CG-72;
v2.7.9 records the GitHub activation and cleanup; v2.7.10 corrects a login-test
synchronization race found by remote CI.
The owner authorized v2.7.10 promotion and both edition images, but its promotion
PR reproduced a second selector race. CG-74 waits for the actual login form
instead of the transient connection panel. The corrected v2.7.11 candidate
reached main and passed hosted qualification; both edition images are published.
The shared Railway database is now stopped under the on-demand QA lifecycle.

## Delivered scope

- Native is the only runtime backend: redb persistence, resident/paged storage,
  optional mmap vectors, text search, traversal and atomic batches. CG-67 removes
  the Arango adapter and its configuration from the current unreleased tree.
- CGQL v2 and workload follow-ups D1–D12 are implemented. Public query surfaces
  accept parsed CGQL; no opaque query passthrough exists. Mutation queries still reject
  backend `DOCUMENT()` reads and correlated traversal before execution.
- Governed milestones M15–M26 provide durable jobs, signed authority, verified
  artifact consumption and derivation, custody recovery, and separately signed
  Native generation deployment. Operation remains a single-writer deployment;
  there is no HA, replication, automatic healing or autonomous activation.
- The React console remains a prototype. The
  [full UI review](evidence/ui-2026-09-11-full-review.md#artifact-2a08bd45b55e9bd0cb9c) records confirmed
  defects, tested journeys and backend capabilities it does not yet expose;
  scoped follow-up work is tracked in [UI TODO](../ui/TODO.md).

## Completed engineering batch — Native-only storage, 2026-09-12

The owner approved [Native-only storage before first deployment](decisions/decision_native_only.md)
and confirmed that CogniGraph had not yet been provided to anyone. Breaking cleanup
was allowed freely before first delivery. The [batch plan](plans/native-only-2026-09-12.md)
has completed [CG-65](issues/CG-65.md), preserving useful Native coverage, and
[CG-67](issues/CG-67.md), removing the adapter and configuration.
[CG-68](issues/CG-68.md) reconciles active guides/workflows and qualifies local
Native operation, both Linux/amd64 images and Helm backups. No customer migration, compatibility
window or last-Arango release was needed. Local readiness passed; subsequent
[Railway acceptance](issues/railway-community-2026-09-12.md) qualifies the chosen environment.

The [CG-65 report](issues/native-conformance-2026-09-12.md) records four
capability-boundary test replacements, all 126 expected CGQL results across
Native modes and persistent reopen, both Rust validation suites and 900 local
release-binary checks. Adapter-only test groups were removed by CG-67;
existing query goldens and historical captures remain unchanged.

The [CG-67 report](issues/native-runtime-2026-09-12.md) records strict Clippy
and full Rust suites in both editions, 514 fresh release-binary checks across
four Native modes, cross-edition CLI/snapshot/tenant probes and all nine browser
regressions. The [CG-68 acceptance report](issues/native-readiness-2026-09-12.md)
adds fresh shared CI, image/runtime, Helm backup and startup rejection checks,
plus active-guide reconciliation. The runtime cleanup and CI changes reached
protected main before the owner authorized CG-71's deployment. [Sibling product
and website alignment](plans/native-only-alignment-2026-09-12.md) has its own dated
checkpoint. The website remains live; the qualified Community QA database is
stopped between sessions, and both Docker editions are published.
The [first-deployment guide](operations/first-deployment.md) remains
the reference for qualifying additional environments.

[CG-64](issues/CG-64.md) and [CG-66](issues/CG-66.md) are deferred optional
external dump-import work, to reconsider after Native-only readiness. They do
not block this batch or first deployment and are not automatically started next.

The registry now has **76 Resolved, 1 Closed without change, and 2 Open issues**.
The [public evidence batch](plans/public-evidence-boundary-2026-09-13.md) resolves
CG-75–CG-77 locally: private originals are verified, public CI and both Docker
editions pass, and hosted QA is stopped with its volume retained. The
[acceptance summary](verification/public-boundary-2026-09-13.md) records the
unreleased candidate; publishing code and addressing historical public copies
remain separate operations.
[CG-78](issues/CG-78.md) repairs remaining archive navigation and extends the docs
checker to archived reading copies, with hash-bound original preservation. Its
[amendment record](plans/archive-navigation-2026-09-13.md) records passing script
and documentation gates; publication remains pending.
[CG-69](issues/CG-69.md) delivers automatic CI and repository protections,
with both CI PRs merged. [CG-70](issues/CG-70.md) removes the Native lru unsoundness advisory
through a bounded upstream patch, makes unsoundness fatal in CI, and records
a dated optional ONNX maintenance exception. Both local CI and Docker suites
pass; the corrected dependency tree has one visible paste maintenance warning.
CG-64/CG-66
remain deferred optional import work. [CG-71](issues/CG-71.md) resolves the first
Community deployment and console packaging, including a clean backed-up store.
The earlier console defect list remains
closed. The [2026-09-13 login centering audit](evidence/ui-2026-09-13-login-centering.md#artifact-eb63247e87967d1d3597)
adds [CG-72](issues/CG-72.md), a reproduced narrow-window layout defect in both
local and hosted Community consoles. Its [responsive-login fix](evidence/ui-2026-09-13-login-responsive.md#artifact-79f36a0e0176e7eab1b0)
passes local Chromium/WebKit and both-edition browser checks; it reached Railway
in v2.7.11 with [hosted phone acceptance](evidence/ui-2026-09-13-railway-v2.7.11.md#artifact-28071583ad3d71443c64).
Additional console features and the research qualification below are
separate backlogs, not prerequisites for this batch. No model or holdout runs
are scheduled by it.

Retain `GraphBackend`, the existing CGQL corpus and historical Arango evidence.
No major-version jump is required solely for this pre-deployment removal; the
normal per-push increment and checks remain. The last hosted QA version is
2.7.11. Migration command/format support remains an
[unqualified deferred design](plans/arangodump-import-design.md).

## Review checkpoint — 2026-09-09

CG-1 through CG-40 are closed: **39 Resolved and CG-25 Closed without change**.
CUAD's missing execution provenance was explicitly retired, not reconstructed.
The [registry](issues/README.md) and individual verification reports retain the
complete audit trail. Later findings should receive new issue numbers.

The latest Rust checkpoint is `dc65a91` (CG-39/CG-40). Its unchanged source passed
formatting, strict Clippy and 967 reported Rust tests before the checkpoint;
eight Arango entries returned early without the exported password. The release
binary passed 34 synthetic OpenAI/Gemini HTTP contract cases. Those are local
results, not a new remote CI run or release certification.

The economical runtime baseline is **`gpt-5.6-luna` with low reasoning effort**.
GLM Flash and Terra are optional measured references; DeepSeek V4 Flash is
retired. Judge qualification remains independent of default model selection.
See [model policy and measurements](decisions/decision_luna_baseline.md).

## Next qualification work

1. Independently review the development-document references and unlabelled
   near-miss candidates under an explicit-evidence policy. Published-reference
   agreement is not a truth-precision or abstention measure.
2. Freeze qualification criteria and exhaustive labels before using the
   80-document holdout. It has not been run. The 40-document
   [policy-v2 development result](research/experiments/luna-directed-v2-2026-09-09/results.md)
   and all older packages remain sealed.
3. Evaluate further model comparisons only against that qualified task. No
   further provider calls or holdout execution are part of documentation cleanup.

The [research index](research/README.md) locates the source guides and evidence.
Product outcome priorities live in the [separate product roadmap](../../docs/product/roadmap.md).

## Documentation and workflow cleanup — 2026-09-10

The [approved layout](plans/documentation-layout-2026-09-10.md) moves product,
sales and publication material to the sibling docs directory, groups engineering
guides, reduces the root README and splits the changelog. The
[migration report](plans/documentation-migration-2026-09-10.md) records completion
and verification. The sibling directory is now initialized as a Git repository;
its initial commit is `acae190`, separate from code cleanup commit `970e8dc`.

The subsequent [root instruction update](changelog/2026-09-10-agent-instructions.md)
aligns AGENTS.md with current query/backend boundaries, document ownership,
evidence preservation and validation. Overlapping CGQL, Native and backend-contract
skill guidance is aligned with those instructions.

The [UI instruction update](changelog/2026-09-10-ui-agent-instructions.md) adds
the existing Bun check commands, scoped browser verification, current component
and session conventions, and provenance for compatibility workarounds. UI plans
remain in `ui/TODO.md`; defects use the shared CG registry.

The [skill revision](changelog/2026-09-10-agent-skills.md) adapts Symphony's scoped
browser QA to CogniGraph, with one canonical UI skill and Codex/Claude entry points.
It also corrects incoming-review scope, release assumptions and validation reporting
across the existing skills. No new test framework or fresh UI acceptance is claimed.

The subsequent [push policy](changelog/2026-09-10-push-policy.md) requires incoming-PR
processing, a product version increment and every applicable CI check passing
locally before each push, including documentation-only changes.

The [executable push workflow](operations/push.md) now shares its runner with CI,
checks incoming PRs and version changes, and protects the exact outgoing candidate.
Hook regression tests cover protected paths, symlinks, stale versions, dirty trees,
unreviewed PRs and changes during verification. The installed hook also rejected
a real dry-run push while the candidate was dirty.

The [2.6.1 candidate](changelog/2026-09-10-v2-6-1.md) integrates dependency PRs
#43–#52 locally and passes the Rust, optional ONNX build/test and Docker checks.
The [Collections audit](evidence/ui-2026-09-10-collections.md#artifact-088100ac46441b68bcc4) resolved
CG-41–CG-44 with browser/API and measured layout evidence. After the subsequent
packaging fixes, the registry reached 47 Resolved, 1 Closed without change and
0 Open issues, before the full UI review below. Remote publication and CI remain
separate from this local checkpoint.

## Packaging and licensing checkpoint — 2026-09-10

The [licensing decision](decisions/decision_licensing.md) introduces the
Community/Enterprise capability split. [CG-45](issues/CG-45.md) is resolved:
Community is the default server/CLI build and excludes Enterprise crate
dependencies and governed/tenant execution. `--features enterprise` includes
them. Docker and Helm expose the same choice; CI validates both builds. Native
file/snapshot compatibility, Community admission and isolated Enterprise tenants
passed release-binary probes, and both images passed Helm backup checks.
See [build editions](operations/running.md#build-editions).
Read replicas remain a
[design proposal](architecture/design-notes/read-replicas.md), with no runtime
replication or failover delivered. The owner assigned read scaling and manual
standby with operator-controlled promotion to Community on 2026-09-10.
Automatic failover, sharding and multiple writers remain Enterprise scope.

The [single-node Helm chart](../deploy/helm/cognigraph/README.md) passed rendering
and an isolated Docker/HTTP backup check. Pre-publication review resolved
[CG-46](issues/CG-46.md) (backup safety and selectors) and
[CG-47](issues/CG-47.md) (issue identity checks and concurrent allocation), and
[CG-48](issues/CG-48.md) (quick-start network and query guidance).
The code and product docs origins now belong to the `cognigraphdb` organization;
publication is separate from local verification and manual remote CI.

## Repository data removal — 2026-09-11

Customer-specific historical material is removed from the publication candidate.
Affected aggregate research claims and CUAD source regeneration are withdrawn;
retained generic tests use synthetic organizations. This does not change the
Luna-low baseline or execute the qualification holdout. See the
[data-removal record](evidence/operations.md#artifact-3e127d2cb8ebed669e80).

## Fresh repository and edition publication — 2026-09-11

The owner moved the previous private GitHub repository to an archive and created
an empty replacement at `cognigraphdb/cognigraph`. Version 2.7.0 prepares the
sanitized current tree, including CG-45 and the Community read-replica decision,
as one root commit. No earlier branches, tags or commit ancestry are part of
the new publication. Dated commit hashes and PR numbers elsewhere in these docs
refer to the former repository. The archive and private recovery records remain
separate; this is a fresh Git history, not deletion of those copies.
The destination changed to public before publication. The owner approved
excluding the restricted corporate/market research kits and derived judge
captures while keeping licensed public benchmark packages and synthetic tests.
The [evidence policy](operations/evidence-policy.md) records that
boundary; the push gate requires the reviewed visibility to remain unchanged.
See the [publication record](changelog/2026-09-11-v2-7-0.md) and
[first-publication procedure](operations/push.md#initial-publication-to-a-fresh-repository).

## Docker distribution preparation — 2026-09-11

The [manual publishing workflow](operations/docker-publishing.md) adds opt-in
versioned Docker Hub distribution for Community and Enterprise on Linux amd64.
Shared CI and packaged-container runtime checks precede publication; source
identity and existing-tag guards protect the candidate. The dedicated Docker
account credential is configured. Public target repositories and the first
authorized remote publishing run remain pending; this is not evidence of
available prebuilt images. ARM manifests, image signing and automated image
vulnerability scanning are not part of this checkpoint.

## UI review and next remediation — 2026-09-11

The [full review](evidence/ui-2026-09-11-full-review.md#artifact-2a08bd45b55e9bd0cb9c) adds CG-49–CG-63,
with four P1 and eleven P2 findings. The local UI suite passes its 72 helper
tests, type/lint checks and build, but current CI does not invoke that suite and
there is no automated browser coverage. A dependency audit also reports an
affected React Router release; the advisory's RSC mode is not used by this UI.
The review does not establish full product or UI acceptance.

The [data-preservation batch](evidence/ui-2026-09-11-data-preservation.md#artifact-6afad6337a2d11fc893d)
resolves document JSON loss ([CG-50](issues/CG-50.md)) and construction import
identity ([CG-62](issues/CG-62.md)), with real browser/HTTP persistence evidence,
85 passing UI helper tests, type/lint checks and a production build. The registry
now has 13 Open issues (2 P1, 11 P2).

The [production-origin batch](evidence/ui-2026-09-11-production-origin.md#artifact-a495041e72ddd34399af)
resolves [CG-49](issues/CG-49.md), with verified production/development origins,
authentication gating and saved-target recovery. The UI suite now passes 92
tests; 12 issues remain Open (1 P1, 11 P2).

The [tenant deletion batch](evidence/ui-2026-09-11-tenant-deletion.md#artifact-0ec008ada9942ef8213a)
resolves [CG-51](issues/CG-51.md), with explicit destructive consequences and
actual quarantine results. Real Enterprise cancellation, credential invalidation,
recreation and failure checks passed. It also corrects CG-49's host-admin probe
gap. The UI suite passes 96 tests; 11 Open issues remain, all P2.

The [provisioning batch](evidence/ui-2026-09-11-provisioning.md#artifact-e96a930169a1f87db8a2) resolves
[CG-52](issues/CG-52.md): resumable first-admin setup, current-tenant user creation
and edition-appropriate role choices. Real Community/Enterprise browser and HTTP
checks verify new-admin login, persisted accounts and denied authority crossings.
The UI suite passes 100 tests; 10 Open issues remain, all P2.

The [capability batch](evidence/ui-2026-09-11-capabilities.md#artifact-29cda8d45940a3ab08a8) resolves
[CG-53](issues/CG-53.md): verified session introspection, edition/role-aware
navigation and direct routes, scoped action controls and explicit anonymous
development behavior. Both Rust CI-equivalent suites and 120 UI tests pass;
release-binary HTTP and browser checks cover all nine roles in both editions.
Nine Open issues remain, all P2.

The [review paging batch](evidence/ui-2026-09-11-review-paging.md#artifact-cbe96c45605c68a64fba) resolves
[CG-54](issues/CG-54.md): truthful queue ranges and continuation, complete searchable
space catalogs, and selection independent of the visible page. A 101-space,
402-neuron Native fixture verified paging, persisted verdicts and off-page
graduation links; 132 UI tests pass. Eight Open issues remain, all P2.

The [execution guard batch](evidence/ui-2026-09-11-execution-guards.md#artifact-ba0883e40b9f39017230) resolves
[CG-55](issues/CG-55.md): one synchronous submission slot for Lua/CGQL buttons,
shortcuts and form submission, with completion ownership. Delayed real requests
verified one persisted Lua write per intentional run, query/error recovery and
pending state across navigation/tab changes; 137 UI tests pass. Seven Open issues
remain, all P2.

The [result ownership batch](evidence/ui-2026-09-11-result-ownership.md#artifact-8aa8b46fb43543771a2c)
resolves [CG-56](issues/CG-56.md): input edits and reruns clear results/timing;
obsolete responses cannot publish; graph loading/failures appear in both views
without old inspectors or paths. Real delayed requests, reversed vector/graph
completion, failed reruns and persisted Lua/relationship readback pass, alongside
144 UI tests. Six Open issues remain, all P2.

The [table keyboard-access batch](evidence/ui-2026-09-11-table-keyboard.md#artifact-a5b3ed52a87a809a63dc)
resolves [CG-57](issues/CG-57.md): native navigation links and review selection
buttons expose the same actions as pointer rows. Tab/Enter/Space journeys,
direct reloads, Delete isolation, scaled focus and Viewer restrictions pass on
the Rust-served Native console; 146 UI tests pass. Five Open issues remain, all P2.

The [space guidance batch](evidence/ui-2026-09-11-space-guidance.md#artifact-ec49fa342ffd2bbdc01d) resolves
[CG-58](issues/CG-58.md): supported draft/review/accept guidance, verified empty
Native catalogs and explicit loading/denial/failure/retry states. Real draft
acceptance and persisted readback, managed mutation guards, host-role denial,
Community gating and transport-failure recovery pass; 151 UI tests pass.
Four Open issues remain, all P2.

The [collection search/filter batch](evidence/ui-2026-09-11-collection-search-scope.md#artifact-1c2e51790ed0aa822e41)
resolves [CG-59](issues/CG-59.md): explicit text-search caps, retrieved-result
counts, page-local filter scope and usable paging through filtered-empty pages.
A 126-document Native fixture, exact-key lookup, filter/reset behavior,
health-poll stability, failure/retry and scaled layouts pass; 161 UI tests pass.
Three Open issues remain, all P2.

The [sidebar naming fix](evidence/ui-2026-09-11-sidebar-names.md#artifact-2bc6b4831aa16897066a) resolves
[CG-63](issues/CG-63.md): stable accessible names and pointer/focus tooltips
across expanded, manual and responsive collapse. Real Admin/HostAdmin journeys,
keyboard navigation, role boundaries and scaled layouts pass; 161 UI tests pass.
Two Open issues remain, both P2.

The [React Router update](evidence/ui-2026-09-11-router-update.md#artifact-f2ee775cf86eb46ca1d7) resolves
[CG-61](issues/CG-61.md): the scoped 8.3.1 upgrade clears the dependency audit.
Frozen installation, UI lint/types, 161 tests and production build pass, along
with live login/deep links, encoded keys, browser history, refresh, role landings
and Rust-served asset checks. One Open issue remains, P2.

The [shared CI and browser gate](evidence/ui-2026-09-11-ui-ci.md#artifact-d71f3d04fcdc5baa0fdb) resolves
[CG-60](issues/CG-60.md): CI and pre-push include the frozen UI install, checks,
unit tests, build and nine Chromium cases against disposable Community/Enterprise
Native APIs. Broken source/bundle candidates fail; provider qualification is
explicitly excluded. The complete local CI gate passes. The registry now has
62 Resolved, 1 Closed without change and 0 Open issues.

The full UI review's defect list is closed. The
[2.7.1 maintenance version](changelog/2026-09-11-v2-7-1.md) collects CG-49–CG-63
remediation and the shared browser/CI gate. Code was pushed to `origin/main`
at `efe9839` after local CI, browser, Docker and Helm checks on 2026-09-11.
Image publication and remote CI were not triggered. The Native-only batch above
now takes precedence over adding further operator journeys from the UI tracker.

The console does not yet provide complete durable-job, signed-governance,
promotion/repair/deployment, side-view or tenant-quota management workflows. The
[UI tracker](../ui/TODO.md) owns that planned scope. Provider qualification and
the frozen holdout remain separate; the audit made no model calls or application
fixes and preserved all previous work.
