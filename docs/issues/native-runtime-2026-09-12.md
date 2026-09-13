# Native runtime removal and verification

- Date: 2026-09-12
- Issue: [CG-67](CG-67.md)
- Baseline: 2.7.1, `bcec5b0`, plus CG-67 changes
- Scope: Runtime/dependencies/configuration; final readiness is [CG-68](CG-68.md)

## Implemented contract

Both server editions open Native directly. No backend selector is read, no
unknown-backend fallback exists, and no Arango connection/vector configuration
is consumed. `COGNIGRAPH_NATIVE_PATH`, `COGNIGRAPH_DATA_DIR` and the supported
Native storage/vector modes keep their contracts. No local environment file or
existing user database was changed. The owner explicitly permits removal of
pre-deployment settings without a compatibility diagnostic.

The adapter and its Cargo dependency edges are removed. Hybrid search uses the
typed Native text-search operation; its obsolete `search_view` input is absent
from the request type and OpenAPI. Promotion history uses typed filtered scans.
The [dependency check](../../scripts/check-editions.py) inspects workspace
members and normal dependencies of both binaries in both editions.

All public query text is parsed CGQL. The Lua embedding API cannot opt into
opaque passthrough, even with write permission. The generic guarded backend
rejects unsupported query languages before storage access. Its removed AQL text
filter is not retained as an authorization mechanism. Parsed collection guards,
dynamic `DOCUMENT()` checks, read/mutation permissions, budgets, cancellation,
cache invalidation and tenant-scoped handles remain in force.

The [CG-65 contract matrix](native-conformance-2026-09-12.md) remains the storage
coverage baseline. Its corpus dataset and expected results are unchanged. New
Lua embedding and guarded-backend regressions require zero calls to the counted
no-access double when an unsupported query language is rejected. Existing HTTP
and Lua role tests retain the same pre-access rejection boundary.

## Historical material

Adapter-only client, wire-protocol, AQL-generation and live-service tests are
deleted under the CG-65 disposition. No final external database run is required.
The removed crate's synthetic `tests/fixtures/vector-model-filter.json` is
preserved at [its evidence location](../evidence/engineering-arango.md#artifact-85619321477dcd112762),
with unchanged SHA-256:
`fce58fa5125ac15e908e216a6568c10cfc7e39769e806b3458f2ccb1212ca5ec`.

Historical issue links now point to those preserved bytes; retired source paths
are identified with their original revision. Old measurements, capture JSON,
research packages and expected-result fixtures are not rewritten. The external
[AQL migration guide](../reference/aql-to-cgql.md) remains query/API education;
direct dump import is still deferred optional work.

## Verification

All required CG-67 checks passed locally. The
[Native runtime harness](../verification/harnesses/native-runtime-http.py) uses owned temporary
stores, fresh authentication and a deterministic loopback embedding endpoint.
It does not configure a backend selector or connect to an external database.
It records result assertions and expected HTTP statuses, excluding credentials
and exported authentication records from captures.

The [manifest](../evidence/engineering-native-runtime-2026-09-12.md#artifact-1b2d524317ee357c0ed6) binds the base
commit, changed and retained executable sources, removed paths, corpus files,
four release binaries and captured outputs. The work was verified on macOS
with CG-67 changes uncommitted; the base commit alone does not contain this change.
Raw logs and [Community](../evidence/engineering-native-runtime-2026-09-12.md#artifact-f1a8992232b6dc5d8156)/
[Enterprise](../evidence/engineering-native-runtime-2026-09-12.md#artifact-e3273bb44d3a73585b3b) captures are
preserved beside the manifest. The 353 corpus-directory files and all tracked
research fixtures are unchanged from the base revision.

| Check | Result |
|---|---|
| `cargo fmt --all -- --check` | Passed |
| `cargo clippy --all-targets -- -D warnings` | Passed |
| `cargo test --all` | 722 reported passed, 0 failed, 2 ignored |
| `cargo clippy --all-targets --features enterprise -- -D warnings` | Passed |
| `cargo test --all --features enterprise` | 913 reported passed, 0 failed, 2 ignored |
| Community/Enterprise server and CLI release builds | Passed |
| Native HTTP/Lua harness | 257 checks per edition; 514 total |
| `scripts/check-editions-live.py` | CLI, cross-edition files/snapshots, Community refusal and Enterprise tenant isolation passed |
| UI install/check/tests/production build | Passed; 161 tests |
| Chromium against the real Rust APIs | 4 Community + 5 Enterprise journeys passed |
| Script tests | 64 passed |
| Edition dependency guard | Four normal binary dependency trees passed |
| Server modularity | Six families, 240 files, seven documented exceptions; passed |
| Helm rendering | Ten variants and eleven rejected configurations passed |
| Documentation, decision and issue validation | Passed |

Each runtime edition covers memory, resident/embedded, resident/sidecar and
paged/sidecar stores. The harness verifies persisted CRUD and query/Lua writes,
read-role/system-collection denial, query language rejection, explicit row-budget
errors, atomic batch rollback, traversal scores, text/vector/hybrid retrieval,
cache reuse, snapshots and reopen behavior. Query-mode and row-budget errors
retain the existing HTTP 500 mapping; the capture checks their error messages
and verifies that the rejected mutation created no document. Authorization and
unsupported language requests return 403. No selector remains to exercise.

The [browser record](../evidence/ui-2026-09-12-native-runtime.md#artifact-c663fb393e4f7b0b7363) preserves
results, binary/UI hashes and screenshots for production assets served by fresh
Rust processes. Tests cover document persistence, direct routes and history,
Viewer denial, edition availability and HostAdmin isolation. These are the
existing scoped regressions, not a new complete UI audit.

The reported Rust totals include two construction live-loop cases per suite
that return early with `COGNIGRAPH_LIVE_LLM=0`. Two embedding-provider cases per
suite remain ignored. No hosted provider coverage is claimed. The 58 removed
adapter tests (50 unit and eight service-gated entries) account for the decrease
from CG-65; one new Lua regression replaces the count of one removed
ArangoSearch-only test. The counted zero-access doubles remain tested.

CG-68 still owns final Docker/Helm packaging qualification and the wider
reconciliation of active guides with historical decisions. No remote CI result,
image publication or live deployment is claimed by this local ticket.
