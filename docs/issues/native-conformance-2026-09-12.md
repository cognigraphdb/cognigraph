# Native conformance coverage before adapter removal

- Date: 2026-09-12
- Issue: [CG-65](CG-65.md)
- Baseline: 2.7.1, `4273ff3`, with the CG-65 working-tree changes
- Scope: Retained Native behavior and capability boundaries; adapter removal is [CG-67](CG-67.md)

## Coverage disposition

The adapter's storage-contract checks already have Native counterparts. Preserve
those contracts and extend the mode coverage below. Do not copy adapter transport
tests into Native or require a live external database to retire its driver.

| Former adapter test purpose | Retained Native coverage / disposition |
|---|---|
| Document CRUD, conflict and missing-collection errors, implicit creation and unbounded scans | The ten helpers in the [shared contract suite](../../crates/cognigraph-core/src/contract.rs), including its traversal-confidence module, run in all four Native modes below. |
| Edge direction, traversal depth/confidence/decay, vector order/limit, filtered/projected/keyset scans | Keep the same shared suite and Native-specific search/traversal tests; no AQL result capture is needed. |
| Arango indexed versus fallback model selection | Retain [Native model lifecycle tests](../../crates/cognigraph-native/tests/sidecar_model_filter.rs) against synthetic expected keys/scores in all three persistent modes, including update/replace/delete/batch, sidecar rebuild and warm reopen. |
| Schema initialization and repeat ensures | Retain Native collection/schema tests and persistence lifecycle coverage. Remove Arango database/index provisioning tests with the adapter. |
| Opaque AQL execution, generated scan AQL, Arango query-language/mode configuration | Delete adapter-only assertions in CG-67. Preserve parsed public CGQL, authorization and capability checks. |
| HTTP client URL escaping, authorization headers, response envelopes, Arango error codes and index payloads | Delete the adapter's client/backend unit-test groups in CG-67; these are driver details, not Native storage contracts. Native typed errors and identity tests remain. |

At this checkpoint the only Rust adapter imports outside its crate are in server
startup, which CG-67 removes. Four former adapter-dependent tests now use the
shared [NoAccessBackend](../../crates/cognigraph-core/src/contract/no_access.rs):

| Boundary | Preserved assertion |
|---|---|
| [Construction ingest](../../crates/cognigraph-construct/tests/ingest_integrity.rs) | Missing atomic-batch capability is rejected before preparatory writes. The construct crate no longer has an Arango dev-dependency. |
| [HTTP search query](../../crates/cognigraph-server/src/routes/search/basic.rs) | Every tested role, including Admin, is denied opaque queries; explicitly parsed CGQL remains available. |
| [Lua route](../../crates/cognigraph-server/src/routes/lua.rs) | ScriptRunner, Editor and Admin cannot gain opaque-query authority; ordinary Native mutation authorization assertions remain. |
| [Enterprise materialization](../../crates/cognigraph-server/src/materialized_repairs/tests/backend_boundary.rs) | Missing atomic capability rejects the build before backend or CAS access, with the exact typed error and unchanged CAS contents. |

The double advertises a generic opaque query language and no atomic capability.
Every storage operation fails and increments a counter; each test checks zero
access after completion. This detects even an attempted access whose error was
caught, with no socket, unreachable host or credentials involved. The double
does not implement storage or serve as an alternative production backend.

## Native mode coverage

| Mode | Shared contract | Expected CGQL results | Reopen / lifecycle |
|---|---|---|---|
| Memory | Existing `native_backend.rs` | 126 existing cases | No persistent reopen; existing CRUD/atomic-batch tests |
| Resident, embedded vectors | Existing `persistence.rs` | Added all 126 before and after reopen | Existing document/deletion/snapshot/text-index persistence tests |
| Resident, sidecar vectors | Added full suite to `sidecar_mode_full_lifecycle` | Added all 126 before and after reopen | Existing vector lifecycle, delta and model-selection tests |
| Paged, sidecar vectors | Existing `paged_mode_conformance_and_lifecycle` | Added all 126 before and after reopen, using a 1 KiB document cache | Existing paged CRUD/batch/vector lifecycle and model-selection tests |

The [Native corpus runner](../../crates/cognigraph-native/tests/cgql_corpus.rs)
imports the original dataset once per store and reopens without reseeding. It
performs 882 expected-result comparisons per suite: 126 in memory plus 126 × 2
phases × 3 persistent modes. The original dataset, 126 queries and 126 JSON
expected arrays are unchanged. No output is regenerated from Native and then
used as its own expected answer. These modes share CGQL code, so agreement does
not constitute an independently implemented language oracle.

Result arrays do not replace behavioral assertions. Retain the
[parser/validation/executor tests](../../crates/cognigraph-query/tests/),
[Native mutation restrictions](../../crates/cognigraph-native/tests/mutation_backend_reads.rs),
[identity tests](../../crates/cognigraph-native/tests/exact_identity.rs),
[query budget tests](../../crates/cognigraph-query/tests/backend_executor.rs),
[Lua limits/cancellation tests](../../crates/cognigraph-lua/tests/execution_limits.rs),
and server authorization, tenant, cache and materialization tests. Atomic rollback,
forbidden writes, cancellation and typed errors cannot be qualified by a read
query corpus alone.

## Verification

Verified locally against 2.7.1 at `4273ff3` plus the recorded CG-65 source
changes. The [verification manifest](../evidence/engineering-historical-checks.md#artifact-5f0b76c1483f7861c7c7)
contains changed-source, binary, harness, log and capture hashes plus the exact
commands. No production behavior changed in this ticket.

- `cargo fmt --all -- --check` passes.
- `cargo clippy --all-targets -- -D warnings` and its `--features enterprise`
  variant pass.
- `cargo test --all` reports 780 passed; the Enterprise variant reports 971
  passed. Each reports two ignored tests. The early-returning cases below are
  included in those reported passing totals, not counted as live qualification.
- The focused `cargo test -p cognigraph-native --test cgql_corpus --test persistence_modes`
  run passes seven tests, including every expected-result comparison and the
  added sidecar contract. Both full suites subsequently repeat that coverage.
- Edition dependency, governed server modularity, documentation navigation,
  decision-index and issue-identity checks pass. The 253 corpus data/query/result
  files were compared byte-for-byte with the base commit and are unchanged.

Both freshly built release editions pass the existing harnesses with synthetic
data, authentication enabled, isolated directories and no embedding provider:

| Executed live harness | Community | Enterprise | Coverage |
|---|---|---|---|
| [Subqueries and public query contracts](../verification/harnesses/subquery-contract-http.py) | [288 checks](../evidence/engineering-historical-checks.md#artifact-ba512fd37cba1eed4af8) | [288 checks](../evidence/engineering-historical-checks.md#artifact-c8ce57dad4b81668f1d5) | Resident/paged sidecar stores; HTTP query/search and Lua reads, mutation results, unchanged data after rejection, analysis, client/runtime error mapping and reserved-collection denial |
| [Vector model lifecycle](../evidence/engineering-historical-checks.md#artifact-1f814aafa876b8b70981) | [162 checks](../evidence/engineering-historical-checks.md#artifact-12f304d9861b36328e86) | [162 checks](../evidence/engineering-historical-checks.md#artifact-08e34689c787cb164c5c) | Resident embedded, resident sidecar and paged sidecar; HTTP/Lua scores and keys, CRUD/batch changes, sidecar rebuilds and vector continuity after server restart |

All **900 live checks pass**, with zero vector-result mismatches. The unsupported
backend double is exercised by Rust tests; no production server exposes it.
These live captures qualify Native query/lifecycle behavior, not an unsupported
runtime backend. They are fresh files; older captures and harnesses are unchanged.

To reproduce, build each edition with `cargo build --release -p cognigraph-server`
(add `--features enterprise` for Enterprise), saving each resulting executable
separately before the next build. Run both linked harnesses with `--binary` set
to that executable and `--output` set to a new JSON path. The manifest records
the exact paths used for this run; local build logs are under `target/cg65/`.

The adapter remains compiled until CG-67. Its eight credential-gated integration
entries return early without `ARANGO_PASSWORD`; that is not executed backend
coverage. The two live construction-loop entries also return early with
`COGNIGRAPH_LIVE_LLM=0`, and the two explicit embedding-provider tests remain
ignored. No provider request or research holdout is executed. This ticket does
not establish Native-only packaging or first-deployment readiness;
CG-68 owns those gates after removal.
