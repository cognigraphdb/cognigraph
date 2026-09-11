# CogniGraph issue registry

This is the local issue registry for CogniGraph. Each issue has a permanent,
unpadded `CG-{number}.md` identifier. Allocate the next unused integer; do not
renumber or reuse closed identifiers. Update this index when adding or changing
an issue.

The initial [2026-09-08 product-status and Rust review](review-2026-09-08.md)
examined revision `bc4bff1`, the current documentation, research drafts, and
fixtures. It recorded **30 open issues**: 9 P1, 20 P2, and 1 P3.
No finding is marked fixed by the review.

After the [first remediation batch](fixes-2026-09-08.md), **5 issues are
Resolved** (CG-2, CG-4, CG-9, CG-17, CG-18). A separate hybrid result-cache
collision found during remediation is CG-31.

The [Lua execution-control batch](lua-controls-2026-09-08.md) additionally
resolved CG-1 and CG-6, and the [directed deployment fence](directed-fence-2026-09-08.md)
resolved CG-3. The [paged-cache batch](paged-cache-2026-09-08.md) resolved CG-10
and CG-14. [Tenant derivative isolation](derivative-isolation-2026-09-08.md)
resolved CG-11. [Request isolation](request-isolation-2026-09-08.md) resolved
CG-12. [Hybrid cache identity](hybrid-cache-2026-09-08.md) resolved CG-31 and
recorded sibling-route fingerprint defects as CG-32.
[Semantic and graph cache identity](search-cache-2026-09-08.md) resolved CG-32.
[Shared completion-provider selection](completion-provider-2026-09-08.md)
resolved CG-29. [Mutation backend-read validation](mutation-backend-reads-2026-09-08.md)
resolved CG-7 using pre-execution rejection of unsupported reads.
[Shared dynamic query execution](dynamic-query-2026-09-08.md) resolved CG-8
and CG-15, including analysis parity and cumulative row accounting.
[Document lookup error propagation](document-errors-2026-09-08.md) resolved
CG-16, preserving backend failures through normal/analyzed queries and Lua.
[Canonical construction evidence](unicode-evidence-2026-09-09.md) resolved CG-5,
using NFC text before grounding, hashes, byte spans, and occurrence keys.
[Shared side-view lifecycle](side-view-lifecycle-2026-09-09.md) resolved CG-13,
covering cascades, generation races, and retryable cleanup. Its remediation also
identified [opaque reference normalization](CG-33.md).
[Traversal confidence parity](traversal-confidence-2026-09-09.md) resolved CG-19,
with per-edge pruning, default confidence, depth zero, and live Arango verification.
[Arango vector candidate filtering](vector-model-filter-2026-09-09.md) resolved CG-20,
including parent deduplication beyond a fixed candidate prefix.
[Native sidecar model selection](sidecar-model-filter-2026-09-09.md) resolved
CG-34 across base and delta candidates, writes, rebuilds, and restarts. Its
runtime verification identified unnecessary startup derivative invalidation
as [CG-35](CG-35.md).
[Neuron lifecycle serialization](neuron-lifecycle-2026-09-09.md) resolved CG-21,
including stale automated review publication. Its runtime setup identified
dedicated judge endpoint configuration as [CG-36](CG-36.md).
[Exact reference identity](exact-identity-2026-09-09.md) resolved CG-33, preserving
opaque values through writes, queries, durable job restart, and traversal. Its
bounded offline diagnosis/repair commands require explicit mappings and preserve
protected records. [API/operator contract parity](api-contract-2026-09-09.md)
resolved CG-22, including all four job kinds, directed replacement, provider
configuration, executable examples, and schema/request drift checks.
[Roadmap/workload parity](roadmap-parity-2026-09-09.md) resolved CG-23, separating
historical failures and measurements from delivered D1–D12 and the active backlog.
[Subquery specification consistency](subquery-contract-2026-09-09.md) resolved
CG-28, with executable/rejected examples and exact grouping/mutation limits.
Its probes found nested generated-name collisions ([CG-37](CG-37.md)) and
read-only query error misclassification ([CG-38](CG-38.md)).
[Whole-query generated bindings](subquery-bindings-2026-09-09.md) resolved CG-37,
including nested/sibling correlation, dynamic reads, and analysis/budget parity.
[Public query error classification](query-error-status-2026-09-09.md) resolved
CG-38, returning matching HTTP 400 plan errors while preserving backend statuses.
[Idempotent collection ensures](collection-ensure-2026-09-09.md) resolved CG-35,
preserving current vector/text files through authenticated startup and retaining
invalidation after real writes.
[Shared dedicated judge endpoints](judge-endpoint-2026-09-09.md) resolved CG-36,
preserving explicit model selection and qualification while validating both
judges through the shared OpenAI configuration before storage opens.
[Paper counting units](paper-counting-units-2026-09-09.md) resolved CG-24,
reconciling distinct construction facts and per-question answer mentions across
the draft and its publication exports, with twelve offline construction replays.
[WebNLG status and artifact reconciliation](webnlg-status-2026-09-09.md) resolved
CG-27, distinguishing completed experiment stages, development-set tradeoffs,
and candidate review from the full runtime lifecycle, with offline replays.
[Decision navigation](decision-index-2026-09-09.md) resolved CG-30, indexing all
53 records with scoped statuses, amendments, and current owner documents.
[CUAD evidence recovery](cuad-recovery-2026-09-09.md) partially remediates CG-25:
pinned inputs and score replay are available, mixed-unit recall/F1 claims are
withdrawn, and the original extraction-provenance gap remains documented.
The user subsequently retired CUAD; CG-25 is **Closed without change** for
the missing historical execution evidence. The [Luna baseline](../decisions/decision_luna_baseline.md)
is a new captured evaluation, not a reconstruction of that history.
[Server modularity](server-modularity-2026-09-09.md) resolves CG-26 across its six
named hotspots, with compatible entry points and seven documented size exceptions.
The original remediation batch closed all 38 tickets. The subsequent
[Luna document trial](../../fixtures/semantic-neurons/luna-documents-2026-09-09/results.md)
identified two new directed-construction findings: endpoint substring acceptance
(CG-39) and unconstrained completion identifiers (CG-40). The current registry
had **39 Resolved, 1 Closed without change, and 0 Open issues** after the
[policy v2 fixes and development verification](../decisions/decision_directed_extraction_contracts.md).

The [2026-09-10 Collections QA](../../ui/audit/2026-09-10-collections/audit.md)
resolved CG-41–CG-44. That checkpoint had **43 Resolved, 1 Closed without change,
and 0 Open issues**. Packaging review added CG-45–CG-48; the
[Community/Enterprise build split](CG-45.md) closes the last of these.
The [2026-09-11 full UI review](../../ui/audit/2026-09-11-full-review/audit.md)
adds CG-49–CG-63: **15 Open issues (4 P1, 11 P2)** at that checkpoint.
The review includes real Community/Enterprise browser and API evidence, and a
separate map of backend capabilities not yet exposed by the console. No finding
was fixed by the audit; this count is not complete UI or product acceptance.

The [data-preservation fixes](../../ui/audit/2026-09-11-data-preservation/audit.md)
resolve CG-50 and CG-62. That checkpoint had **49 Resolved, 1 Closed without change,
and 13 Open issues (2 P1, 11 P2)**.
[Production origin and connection-state verification](../../ui/audit/2026-09-11-production-origin/audit.md)
resolves CG-49. That checkpoint had **50 Resolved, 1 Closed without change, and
12 Open issues (1 P1, 11 P2)**.
[Tenant deletion disclosure and lifecycle verification](../../ui/audit/2026-09-11-tenant-deletion/audit.md)
resolves CG-51 and adds Enterprise host-admin probe coverage to CG-49. That
checkpoint had **51 Resolved, 1 Closed without change, and 11 Open issues (all P2)**.
[Tenant onboarding and user provisioning](../../ui/audit/2026-09-11-provisioning/audit.md)
resolves CG-52. That checkpoint had **52 Resolved, 1 Closed without change, and
10 Open issues (all P2)**.
[Verified console capabilities](../../ui/audit/2026-09-11-capabilities/audit.md)
resolves CG-53. That checkpoint had **53 Resolved, 1 Closed without change, and
9 Open issues (all P2)**.
[Review paging and complete space catalogs](../../ui/audit/2026-09-11-review-paging/audit.md)
resolves CG-54. Current registry: **54 Resolved, 1 Closed without change, and
8 Open issues (all P2)**.

## Conventions

- Status: Open, In progress, Blocked, Resolved, or Closed without change. Record the reason and evidence for the last two.
- Priority: P1 = urgent isolation, execution-control, or data-integrity defect; P2 = correctness, contract, reproducibility, or material maintainability work; P3 = lower-impact improvement. No P0 was assigned in this review.
- Each issue records the affected behavior, source evidence, reproduction or failure sequence, and acceptance criteria. Source-confirmed findings are distinct from live reproductions.
- Close a Rust issue only after the required formatting, Clippy, and test gates plus its focused real-runtime regression pass. Documentation-only closure requires source/link/example verification appropriate to the change.
- Keep research counting units and experiment provenance explicit. Historical milestone completion is not evidence that a newly discovered defect is resolved.

## Issues

| Issue | Priority | Status | Finding |
|---|---|---|---|
| [CG-1](CG-1.md) | P1 | Resolved | Lua scripts can re-enable JIT and bypass the instruction limit |
| [CG-2](CG-2.md) | P1 | Resolved | Collection names can escape the Native text-index directory |
| [CG-3](CG-3.md) | P1 | Resolved | Directed construction bypasses the deployed-generation mutation fence |
| [CG-4](CG-4.md) | P1 | Resolved | Malformed directed-completion output is treated as an empty replacement |
| [CG-5](CG-5.md) | P2 | Resolved | Native NFC stamping breaks construction hashes and byte spans |
| [CG-6](CG-6.md) | P1 | Resolved | Lua graph.query does not receive the configured CGQL budgets |
| [CG-7](CG-7.md) | P2 | Resolved | DOCUMENT expressions in mutations silently evaluate to null |
| [CG-8](CG-8.md) | P2 | Resolved | EXPLAIN ANALYZE uses different DOCUMENT semantics than execution |
| [CG-9](CG-9.md) | P2 | Resolved | UPSERT ignores additional match fields when _key is present |
| [CG-10](CG-10.md) | P1 | Resolved | Paged document reads can remain stale after a committed concurrent write |
| [CG-11](CG-11.md) | P1 | Resolved | Tenant recreation reuses the deleted tenant's text index |
| [CG-12](CG-12.md) | P1 | Resolved | In-flight requests can resume into a recreated tenant's store |
| [CG-13](CG-13.md) | P2 | Resolved | Side-view orphan prevention is bypassed by alternate deletes |
| [CG-14](CG-14.md) | P1 | Resolved | Dropping a collection does not clear paged document cache entries |
| [CG-15](CG-15.md) | P2 | Resolved | Dynamic query resolution resets source-row accounting |
| [CG-16](CG-16.md) | P2 | Resolved | DOCUMENT converts backend failures into successful null results |
| [CG-17](CG-17.md) | P2 | Resolved | Duplicate text-search fields panic instead of returning validation errors |
| [CG-18](CG-18.md) | P2 | Resolved | Text-index cache keys conflate distinct field lists |
| [CG-19](CG-19.md) | P2 | Resolved | Arango traversal confidence differs from Native semantics |
| [CG-20](CG-20.md) | P2 | Resolved | Arango indexed vector search filters model identity too late |
| [CG-21](CG-21.md) | P2 | Resolved | Concurrent neuron acceptance can commit a forbidden hint/blocker pair |
| [CG-22](CG-22.md) | P2 | Resolved | Current API and operator references omit implemented construction and job surfaces |
| [CG-23](CG-23.md) | P2 | Resolved | Current roadmap still reports CGQL workload gaps that were closed |
| [CG-24](CG-24.md) | P2 | Resolved | Paper revision 2 reintroduces superseded fact denominators |
| [CG-25](CG-25.md) | P2 | Closed without change | CUAD headline results lack a repository reproduction package |
| [CG-26](CG-26.md) | P2 | Resolved | Governance and server modules substantially exceed the modularity convention |
| [CG-27](CG-27.md) | P2 | Resolved | WebNLG status and artifact links lag the recorded experiment outcomes |
| [CG-28](CG-28.md) | P2 | Resolved | CGQL specification both supports and forbids expression-position subqueries |
| [CG-29](CG-29.md) | P2 | Resolved | Server completion selection ignores the documented provider override |
| [CG-30](CG-30.md) | P3 | Resolved | Decision log needs an index of current and superseded contracts |
| [CG-31](CG-31.md) | P2 | Resolved | Hybrid result-cache fingerprints conflate distinct request parameters |
| [CG-32](CG-32.md) | P2 | Resolved | Semantic and graph-augmented cache keys still conflate distinct parameters |
| [CG-33](CG-33.md) | P2 | Resolved | Native normalization changes opaque references in jobs and edges |
| [CG-34](CG-34.md) | P2 | Resolved | Native sidecar vector search filters model identity after truncation |
| [CG-35](CG-35.md) | P2 | Resolved | Native collection ensures invalidate derivatives on unchanged startup |
| [CG-36](CG-36.md) | P2 | Resolved | Dedicated review judges ignore the configured OpenAI base URL |
| [CG-37](CG-37.md) | P2 | Resolved | Nested lifted subqueries can reuse a synthetic binding name |
| [CG-38](CG-38.md) | P2 | Resolved | Read-only query route reports parse and validation errors as HTTP 500 |
| [CG-39](CG-39.md) | P2 | Resolved | Directed endpoint checks accept names embedded inside longer words |
| [CG-40](CG-40.md) | P2 | Resolved | Directed completion schema leaves request-bound identifiers unconstrained |
| [CG-41](CG-41.md) | P1 | Resolved | Background refresh discards unsaved document edits |
| [CG-42](CG-42.md) | P2 | Resolved | Collections deep links select the wrong document beyond page one |
| [CG-43](CG-43.md) | P2 | Resolved | Collections controls overflow at scaled desktop widths |
| [CG-44](CG-44.md) | P2 | Resolved | Closing a Collections dialog loses keyboard focus |
| [CG-45](CG-45.md) | P2 | Resolved | Community binary cannot exclude Enterprise Components |
| [CG-46](CG-46.md) | P2 | Resolved | Helm backups need isolated selectors and safe snapshot handling |
| [CG-47](CG-47.md) | P2 | Resolved | Issue identity checks must survive commits and concurrent allocation |
| [CG-48](CG-48.md) | P2 | Resolved | Quick-start examples must match network and query contracts |
| [CG-49](CG-49.md) | P1 | Resolved | Production console always targets API port 3001 |
| [CG-50](CG-50.md) | P1 | Resolved | Document inspector rewrites valid stored JSON through a lossy projection |
| [CG-51](CG-51.md) | P1 | Resolved | Tenant deletion confirmation promises recovery that recreation does not provide |
| [CG-52](CG-52.md) | P2 | Resolved | Tenant onboarding and user provisioning diverge from the authorization contract |
| [CG-53](CG-53.md) | P2 | Resolved | Console actions ignore server edition and authenticated role capabilities |
| [CG-54](CG-54.md) | P2 | Resolved | Review queue and space selectors silently truncate their datasets |
| [CG-55](CG-55.md) | P2 | Open | Console keyboard shortcuts bypass the running request guard |
| [CG-56](CG-56.md) | P2 | Open | Console results remain attached to changed inputs and failed graph requests |
| [CG-57](CG-57.md) | P2 | Open | Clickable table rows cannot be opened with the keyboard |
| [CG-58](CG-58.md) | P2 | Open | Review empty-state guidance recommends a forbidden space creation path |
| [CG-59](CG-59.md) | P2 | Open | Collection search and filters present partial matches as complete results |
| [CG-60](CG-60.md) | P2 | Open | CI omits the UI checks and browser regression coverage |
| [CG-61](CG-61.md) | P2 | Open | React Router lockfile retains an advisory-affected release |
| [CG-62](CG-62.md) | P1 | Resolved | Plain-text construction imports reuse chunk IDs and replace earlier evidence |
| [CG-63](CG-63.md) | P2 | Open | Collapsed sidebar removes the accessible names of navigation links |

Next available identifier: **CG-64**.
## After the current ticket list

CG-1 through CG-38 are now resolved or explicitly closed. The user selected
a [captured Luna baseline and bounded Astra reference](../decisions/decision_luna_baseline.md)
after retiring CUAD. The subsequent
[DeepSeek/GLM comparison](../../fixtures/semantic-neurons/cross-provider-baseline-2026-09-09/results.md)
has completed. The user has dropped DeepSeek V4 Flash from all future runs;
its failed capture is retained as history. The
[larger independently labelled supplied-pair trial](../../fixtures/semantic-neurons/glm-luna-semeval-2026-09-09/results.md)
is complete: GLM Flash scored 65.75% versus Luna's 42.67% pooled accuracy,
with substantial Other-case restraint errors in both models.
The subsequent [Terra low / Luna low extension](../../fixtures/semantic-neurons/terra-luna-low-semeval-2026-09-09/results.md)
scored 72.42% and 70.08%. Terra's 2.33-point margin costs about 10.4 times as
much. That experiment recommended Luna low and GLM Flash as economical
candidates, with Terra as a stronger reference; its supplied-pair results do not
qualify document extraction. The user subsequently selected **Luna low**, now
adopted in the shared runtime with [completed Rust and live HTTP verification](luna-low-runtime-2026-09-09.md). CG-26 and
the completed evaluation evidence are checkpointed locally as `8b8cceb`. Next
comes representative document qualification and restraint work on separate
development and holdout splits. The original document trial was checkpointed
as `a0e4b24`; its CG-39/CG-40 findings are now resolved with a
[separately frozen development candidate](../../fixtures/semantic-neurons/luna-directed-v2-2026-09-09/results.md).
Next comes independent reference/evidence-policy review before precision claims
or holdout execution. Further model comparisons are optional references.
See the [implementation plan](../implementation-plan.md#review-checkpoint--2026-09-09)
and [existing measured baseline](../../fixtures/semantic-neurons/sideviews-model-comparison-2026-09-08/README.md).

## Review evidence

- [Product status, crate map, scope, validation and remediation order](review-2026-09-08.md)
- [Sanitized local HTTP observations](evidence/http-2026-09-08.json)

The source citations inside issues refer to the reviewed revision. Recheck line
numbers and behavior when implementing a fix, especially after the modularity work.
