# CG-26 server modularity — 2026-09-09

**Later user decision (2026-09-09):** CUAD is retired from active evaluation.
CG-25 is closed without change to its missing execution evidence; the status
counts below describe this report's earlier checkpoint. See the
[Luna baseline decision](../decisions/decision_luna_baseline.md) and current
[registry](README.md).

[CG-26](CG-26.md) is resolved for its six named server hotspots. The refactor
separates contracts, validation, transitions, storage, recovery, and tests while
preserving existing caller paths and behavior. It does not claim that the
entire Rust workspace now meets the line-count convention.

CG-25's partial evidence recovery was committed locally as `3fa7db7`; its
remaining scope was later closed by the user as described above. CG-26 and the
completed model-evaluation evidence were checkpointed locally as `8b8cceb`.
Nothing was pushed or published, and the six unrelated user-owned paths remain
unchanged.

## Source map

All paths below are under `crates/cognigraph-server/src`. Counts include tests,
comments, and blank lines. Each `mod.rs` retains the previous module's caller
paths through re-exports; child modules remain private implementation details.

| Family | Previous file lines | Files now | Entry point lines | Main seams |
|---|---:|---:|---:|---|
| [promotions](../../crates/cognigraph-server/src/promotions/mod.rs) | 15,020 | 66 | 223 | Identity/context/policy contracts, gate assessment, evidence registration, decisions, rollback, head reconciliation, snapshot validation, themed lifecycle fixtures/tests. |
| [jobs](../../crates/cognigraph-server/src/jobs/mod.rs) | 6,821 | 48 | 154 | Payload/record contracts, admission, queue dispatch, worker transitions, capacity, catalog/archive, recovery, tenant lifecycle, tests. |
| [materialized repairs](../../crates/cognigraph-server/src/materialized_repairs/mod.rs) | 4,602 | 35 | 155 | Projection/impact contracts, generation build and validation, deployment, target replacement, signed intent, snapshot authority and recovery, capacity tests. |
| [artifact consumption](../../crates/cognigraph-server/src/artifact_consumption/mod.rs) | 4,117 | 34 | 184 | Plans, receipts, preparation/derivation authority, corpus/config/oracle contracts, verified consumption, manifest and grounding checks, tests. |
| [governance](../../crates/cognigraph-server/src/governance/mod.rs) | 3,996 | 29 | 113 | Key/policy/intent contracts, registration/revocation, signed authorization, record validation, historical authority, snapshot checks, tests. |
| [construction routes](../../crates/cognigraph-server/src/routes/construct/mod.rs) | 2,868 | 28 | 106 | Routing, directed/ordinary/governed ingest, evaluation, drafting/acceptance, proposals, review policy/lanes, tests. |

Of **240 files, 233 are within the 450-line soft cap**. The seven
[documented exceptions](evidence/server-modularity-exceptions.json) are two
existing production execution methods, four complete lifecycle scenarios, and
one fixture factory. The largest production file is the 680-line worker;
the largest test remains the 2,019-line M26 scenario. These shared-state units
stay intact because splitting them would require a separate behavior-reviewed
redesign. Their explicit size budgets prevent silent growth.

The [size checker](../../scripts/check-server-modularity.py) runs locally and
in the existing manual CI workflow. It rejects new oversized files, growth past
an exception's budget, missing module/test entry points, and stale exceptions.
All four negative probes passed. Sixteen other server source/test files still
exceed the cap outside these six families; their measured inventory is retained
in the validation evidence. This is a scoped remediation, not a global size claim.

## Behavior preservation

A temporary `syn` parser compared **1,087 constants, types, functions, and
inherent/trait methods** against `3fa7db7`. The full multiset matched after
normalizing visibility, syntactic trailing commas, parsed string literals, and
relocated `include_str!` paths. This preserves literal values as well as control
flow. The [movement ledger](evidence/server-modularity-motion-2026-09-09.json)
provides each old/new source location and normalized token digest.

The remaining edits are module/import scaffolding, `pub(super)` access within
the same original module boundary, and updated compile-time fixture paths.
Public and crate-visible caller paths remain available. Existing test function
names and assertions remain intact; their module-qualified locations change.
The OpenAPI drift test now reads the extracted routing source. No wire fields,
policy/signature domains, quotas, transaction/lock lifetimes, or state transitions
were intentionally changed. No Cargo dependencies or feature flags changed.

## Verification

The exact Rust gates passed on the baseline and final tree:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo build --release -p cognigraph-server
python3 scripts/check-server-modularity.py
actionlint .github/workflows/ci.yml
```

Both full suites reported **961 passing tests**, with the same test-name
multiset. Eight environment-gated Arango integration entries returned early in
the workspace suite; the two existing embedding-provider tests passed. These
counts are Cargo's reported results, not 961 external integrations.

Live verification used disposable stores, authenticated release processes, and
synthetic fixtures:

- **Signed M26 authority:** the saved and final releases each passed import,
  exact current deployment, two retained generations, idempotent build/deploy,
  wrong-role rejection, zero-repair recovery, and graceful restart in
  resident/embedded, resident/sidecar, and paged/sidecar Native storage. The six
  observations per binary have identical authority/graph digests; two active
  target facts and one other-space fact were preserved. This also exercises
  stored jobs, signed keys/policy/repair authority, evaluation evidence, and
  promotion-history recovery. The complete build/deploy/rollback and tamper
  branches remain covered by the existing M21–M26 Rust lifecycle tests.
- **API contract replay:** the existing CG-22 release HTTP/CLI harness passed
  57 checks, including all four durable job kinds and construction routes,
  using 16 loopback provider calls.
- **Neuron lifecycle replay:** the existing CG-21 harness passed 99 scenarios
  across all three Native modes, including 162 restart-preserved documents,
  using 90 loopback judge calls.
- **Arango boundary:** a disposable Arango 3.12.11 container passed authenticated
  startup and healthy operator status. M26 remained disabled; a Promoter's build
  returned HTTP 503 before creating M26 authority collections. Application and
  CogniGraph control collection counts were unchanged. The test database and
  container were removed. Full signed Arango lifecycle/conformance suites were
  not rerun.

No live probe called an external completion/judge provider, and no model
benchmark or qualification was performed. The reused API harness's original
issue/revision labels are historical metadata; the saved release digest and
added CG-26 replay note identify the actual tested binary.

The first extraction/verification attempts found module-wrapper, conditional
re-export, relocated fixture/OpenAPI-path, and import errors; these were fixed
before the successful Rust gates. Final seam review moved ten methods to their
correct responsibility modules and repeated all gates. A first HTTP probe used
an incorrect deployment-head field name; correcting the probe to the existing
`applied_deployment_decision_id` contract made both releases pass. No product
behavior fix was introduced to make the probes pass.

See [validation evidence](evidence/server-modularity-validation-2026-09-09.json),
[baseline HTTP](evidence/server-modularity-baseline-http-2026-09-09.json),
[final HTTP](evidence/server-modularity-http-2026-09-09.json),
[API replay](evidence/server-modularity-api-http-2026-09-09.json),
[neuron replay](evidence/server-modularity-neuron-http-2026-09-09.json), and
[Arango boundary](evidence/server-modularity-arango-2026-09-09.json).

To regenerate the disposable signed fixture and replay Native HTTP:

```bash
COGNIGRAPH_M26_LIVE_FIXTURE_DIR=/tmp/cg26-fixture cargo test -p cognigraph-server verified_semantic_repair_materializes_switches_and_recovers_exact_authority
python3 docs/issues/evidence/server-modularity-http.py --binary target/release/cognigraph-server --fixture /tmp/cg26-fixture --output /tmp/cg26-http.json
```

The registry is now **37 Resolved / 1 Open**. CG-25's missing original
model/settings and complete extraction trace remain the next closure boundary.
Broader DeepSeek/GLM/other-model benchmarking remains deferred; Luna remains
the economical baseline.
