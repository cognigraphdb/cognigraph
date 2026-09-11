# CG-30 decision index — 2026-09-09

[CG-30](CG-30.md) is resolved. The [decision index](../decisions/README.md)
now covers all 53 records, distinguishes their operative scope, and identifies
current owner documents for 14 subsystem/workflow areas. CG-27 was committed
locally as `44e8890`; CG-30 was committed locally as `e7ea755`. Nothing was pushed.

## Reconciliation

Every decision has one inventory row with a status, operative scope, and
successor/amendment explanation. The six groups contain 6 standing conventions,
18 platform contracts, 8 construction/review contracts, 12 governance records,
8 research records, and 1 superseded identity record. Status counts are
23 Active, 21 Amended, 8 Research, and 1 Historical.

These labels describe scope, not issue closure or a fresh runtime certification.
All 53 decision bodies are unchanged. Important reading paths now make these
distinctions explicit:

| Earlier wording / scope | Operative amendment or owner |
|---|---|
| Generic NFC writes/literals | [CG-33 exact identity](../decisions/decision_exact_reference_identity.md); CG-5 keeps its explicit construction-evidence boundary. |
| Memory-primary storage and generation-only derivative reuse | [Current storage model](../architecture/native-storage.md) distinguishes paged/resident modes and schema-2 identity; CG-35 preserves no-op revisions. |
| Admin-only opaque AQL in M15 | [M18 supersession](../decisions/decision_m15_foundation.md) denies public opaque text for every role. |
| CGQL LET-only expressions and open workload gaps | [V2 amendments](../decisions/decision_cgql_v2.md) and [delivered workload register](../decisions/decision_cgql_v2_workload_gaps.md); current limits remain explicit. |
| Tenant deletion preserves credentials; planned route/migration names | [Tenancy Outcome](../decisions/decision_multi_tenancy.md) and [deletion contract](../decisions/decision_tenant_deletion.md) own delivered behavior and request-incarnation isolation. |
| M16's two initial job kinds | [Jobs runbook](../operations/jobs.md#durable-governed-jobs-m16-m17) documents all four current kinds. |
| Alias auto-accept / legacy judge qualification as authority | [Review-policy current addenda](../decisions/decision_review_policy.md) narrow acceptance, serialize lifecycle transitions, and distinguish mutable configuration from signed authority. |
| Later governance milestones replace all earlier authority | [M19 D11](../decisions/decision_m19_signed_governance_separation_of_duties.md#d11-keep-m18-v1-history-readable-but-remove-it-from-new-promotion-authority) specifically removes new M18 v1 authority; M20–M26 preserve prior meanings and add scoped capabilities. |
| Promotion selection is the end of product deployment scope | [M26](../decisions/decision_m26_verified_semantic_repair_materialization.md) adds a separately signed deployment; promotion itself still only selects. |
| July model recommendations, WebNLG future work, and clinical normalization priorities | [Current model policy](../decisions/decision_sideviews_provider_and_benchmark.md#current-model-defaults-2026-09-08), [CG-27 reading note](../decisions/decision_webnlg_scoring.md#current-reading-2026-09-09-cg-27), and [negative answer comparison](../decisions/decision_answer_head_to_head_real_labels.md) qualify those dated claims. |

Focused source reads also confirmed the key navigation boundaries: exact
metadata stamping in [Native helpers](../../crates/cognigraph-native/src/memory/helpers.rs),
the [public query language gate](../../crates/cognigraph-server/src/routes/search/basic.rs),
the four job variants (then `crates/cognigraph-server/src/jobs.rs`, now the
[jobs module](../../crates/cognigraph-server/src/jobs/mod.rs)),
the legacy unsigned-evidence rejection in promotion handling (then
`crates/cognigraph-server/src/promotions.rs`, now the
[promotions module](../../crates/cognigraph-server/src/promotions/mod.rs)),
and the Luna default in [completion selection](../../crates/cognigraph-embeddings/src/completion.rs).
Existing issue reports retain the associated release/runtime evidence; these
source checks do not represent a new code audit or model evaluation.

## Validation and maintenance

Run from the repository root:

```bash
python3 scripts/check-decision-index.py
```

The dependency-free [checker](../../scripts/check-decision-index.py) validates
complete/unique inventory coverage, four nonempty row cells, known statuses,
inventory sections, and every local path/Markdown anchor linked by the index.
The final run passed for 53 unique decisions, 148 local links, and 19 anchors.
No index link requires an external service.

Negative probes in disposable copies verified rejection of an omitted record,
a duplicate row, an unknown status, an empty amendment cell, an unresolved
local link, and an unresolved anchor. The initial index check correctly
reported the report link as missing before this report was created; the final
check passed. Broader checks verified the changed Markdown links, registry
rows/counts, unchanged decision bodies, untouched Rust/manifests/fixtures,
and preservation of the six pre-existing user-owned paths.
The [machine-readable evidence](evidence/decision-index-validation-2026-09-09.json)
records counts, hashes, and the reviewed revision.

The index documents its maintenance rule: add one row for each decision,
update affected prior rows and owner guides with amendments, and preserve
historical outcomes. The checker validates navigation; semantic status still
requires reviewing the actual decision and its evidence. It does not check
every outgoing link inside all historical records.

## Limits and remaining work

This change affects documentation and its navigation checker. Rust code,
dependencies, APIs, research artifacts, and the React UI are unchanged; no
product feature needs a new runtime test. The Rust gates and live checks from
the prior implementation work were not rerun or relabeled as CG-30 evidence.
The latest full Rust gate remains CG-36's passing formatting, strict Clippy,
and 961 reported tests, with its recorded eight Arango early returns and two
live embedding tests.

The registry now has 36 Resolved and 2 Open issues, both P2: [CG-25](CG-25.md)
for CUAD reproducibility and [CG-26](CG-26.md) for modularity. Next is CG-25:
recover or construct the retained reproduction package and qualify any
headline that cannot be reproduced. Broader DeepSeek/GLM/other-model evaluation
remains deferred until the original ticket list closes; Luna remains the
economical baseline.
