# CogniGraph implementation plan

Current engineering status, reviewed 2026-09-11. Detailed historical delivery
and test counts live in [change records](changelog/README.md), the
[issue registry](issues/README.md) and [archived implementation history](plans/archive/implementation-history-through-2026-09-09.md).

## Delivered scope

- Native is the strategic backend: redb persistence, resident/paged storage,
  optional mmap vectors, text search, traversal and atomic batches. ArangoDB is
  maintained for reference/conformance, with explicit unsupported boundaries.
- CGQL v2 and workload follow-ups D1–D12 are implemented. Public query surfaces
  accept parsed CGQL; opaque AQL remains internal. Mutation queries still reject
  backend `DOCUMENT()` reads and correlated traversal before execution.
- Governed milestones M15–M26 provide durable jobs, signed authority, verified
  artifact consumption and derivation, custody recovery, and separately signed
  Native generation deployment. Operation remains a single-writer deployment;
  there is no HA, replication, automatic healing or autonomous activation.
- The React console remains a prototype. The
  [full UI review](../ui/audit/2026-09-11-full-review/audit.md) records confirmed
  defects, tested journeys and backend capabilities it does not yet expose;
  scoped follow-up work is tracked in [UI TODO](../ui/TODO.md).

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
   [policy-v2 development result](../fixtures/semantic-neurons/luna-directed-v2-2026-09-09/results.md)
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
The [Collections audit](../ui/audit/2026-09-10-collections/audit.md) resolved
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
[data-removal record](operations/repository-data-removal.md).

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
The [distribution guide](../fixtures/semantic-neurons/README.md) records that
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

The [full review](../ui/audit/2026-09-11-full-review/audit.md) adds CG-49–CG-63,
with four P1 and eleven P2 findings. The local UI suite passes its 72 helper
tests, type/lint checks and build, but current CI does not invoke that suite and
there is no automated browser coverage. A dependency audit also reports an
affected React Router release; the advisory's RSC mode is not used by this UI.
The review does not establish full product or UI acceptance.

The [data-preservation batch](../ui/audit/2026-09-11-data-preservation/audit.md)
resolves document JSON loss ([CG-50](issues/CG-50.md)) and construction import
identity ([CG-62](issues/CG-62.md)), with real browser/HTTP persistence evidence,
85 passing UI helper tests, type/lint checks and a production build. The registry
now has 13 Open issues (2 P1, 11 P2).

The [production-origin batch](../ui/audit/2026-09-11-production-origin/audit.md)
resolves [CG-49](issues/CG-49.md), with verified production/development origins,
authentication gating and saved-target recovery. The UI suite now passes 92
tests; 12 issues remain Open (1 P1, 11 P2).

The [tenant deletion batch](../ui/audit/2026-09-11-tenant-deletion/audit.md)
resolves [CG-51](issues/CG-51.md), with explicit destructive consequences and
actual quarantine results. Real Enterprise cancellation, credential invalidation,
recreation and failure checks passed. It also corrects CG-49's host-admin probe
gap. The UI suite passes 96 tests; 11 Open issues remain, all P2.

The [provisioning batch](../ui/audit/2026-09-11-provisioning/audit.md) resolves
[CG-52](issues/CG-52.md): resumable first-admin setup, current-tenant user creation
and edition-appropriate role choices. Real Community/Enterprise browser and HTTP
checks verify new-admin login, persisted accounts and denied authority crossings.
The UI suite passes 100 tests; 10 Open issues remain, all P2.

The [capability batch](../ui/audit/2026-09-11-capabilities/audit.md) resolves
[CG-53](issues/CG-53.md): verified session introspection, edition/role-aware
navigation and direct routes, scoped action controls and explicit anonymous
development behavior. Both Rust CI-equivalent suites and 120 UI tests pass;
release-binary HTTP and browser checks cover all nine roles in both editions.
Nine Open issues remain, all P2.

The [review paging batch](../ui/audit/2026-09-11-review-paging/audit.md) resolves
[CG-54](issues/CG-54.md): truthful queue ranges and continuation, complete searchable
space catalogs, and selection independent of the visible page. A 101-space,
402-neuron Native fixture verified paging, persisted verdicts and off-page
graduation links; 132 UI tests pass. Eight Open issues remain, all P2.

Next, address execution guards ([CG-55](issues/CG-55.md)), then result state and
accessibility, with real-browser regressions and
CI coverage ([CG-60](issues/CG-60.md)). Keep Docker publication preparation
separate from this pending UI remediation.

The console does not yet provide complete durable-job, signed-governance,
promotion/repair/deployment, side-view or tenant-quota management workflows. The
[UI tracker](../ui/TODO.md) owns that planned scope. Provider qualification and
the frozen holdout remain separate; the audit made no model calls or application
fixes and preserved all previous work.
