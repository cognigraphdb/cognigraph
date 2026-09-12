# Decision: Native-only storage before first deployment

Date: 2026-09-12 · Status: ACCEPTED; runtime removal and local readiness complete

## Context

Native already owns CogniGraph's storage and query direction. The owner approved
retiring ArangoDB as a runtime backend and organizing the work as an engineering
batch on 2026-09-12. The owner then clarified that CogniGraph has never been
provided to anyone and live deployment will start only after Native-only
completion. Public source versions are development checkpoints, not installed
customer contracts. Breaking cleanup is explicitly permitted in this phase.
This replaces the indefinite maintenance commitment in
[Native-first](decision_native_first.md), without changing its choice of CGQL
or its prohibition on adding other third-party database backends.

At the inspected 2.7.1 revision `efe9839`, Arango support is still compiled into
both server editions through an unconditional dependency. The adapter has about
2,200 Rust lines plus integration points in startup, configuration and tests.
It supplies shared storage-contract coverage for documents, edges, traversal
and vectors; it does not implement Native atomic batches or application
snapshots. See the [server manifest](../../crates/cognigraph-server/Cargo.toml),
adapter source (`crates/cognigraph-arango/src/backend/graph_backend.rs`
at `bcec5b0`) and
[shared contract](../../crates/cognigraph-core/src/contract.rs).

The [CGQL corpus](../../crates/cognigraph-query/tests/corpus/README.md) already
contains 126 execution fixtures with expected results. It runs through local
executor/backend paths, not an ArangoDB differential runner. Native's resident
and paged storage modes are useful conformance targets, but share query code;
they are not independent implementations of the CGQL specification.

The adapter is our HTTP client code, not a bundled ArangoDB engine. The
[Dockerfile](../../Dockerfile) packages CogniGraph binaries. External ArangoDB
operation/distribution has its own licensing conditions; this decision does
not assert that communicating over HTTP changes CogniGraph's licence. Arango's
[edition documentation](https://docs.arango.ai/arangodb/stable/features/#arangodb-editions)
describes its binary and source licensing separately. The engineering rationale
is reducing operational and maintenance scope, independent of that legal question.

## Decision (owner: skitsanos)

1. **Native will be the only runtime database backend in both editions.** Remove
   `cognigraph-arango`, its runtime dependency, startup branch and obsolete AQL
   routing/configuration after the focused Native coverage work passes.
   Do not keep a hidden feature flag or move the adapter to Enterprise.
2. **External migration tooling is optional follow-up work.** Retain the
   [AQL-to-CGQL guide](../reference/aql-to-cgql.md) for developers considering
   CogniGraph. Defer CG-64 and CG-66 until after Native-only readiness and a new
   prioritization decision. Their [import design](../plans/arangodump-import-design.md)
   is an unqualified backlog proposal, not a commitment or a prerequisite for
   removal/deployment. If built, the tool belongs in Community and reads dump
   files without an Arango runtime dependency.
3. **Keep the storage abstraction and behavioral tests.** `GraphBackend`,
   explicit capabilities, tenant/cache wrappers and failure-injection doubles
   remain useful. Preserve and extend conformance across in-memory, persistent
   resident and supported paged/sidecar modes, including reopen behavior. Keep
   independent expected query results and mutation/error/budget tests.
4. **Retain coverage for Native behavior; drop adapter-only tests.** Classify
   tests only far enough to preserve useful storage/query/auth guarantees and
   focused failure doubles. No final live Arango run, new AQL golden corpus or
   exhaustive historical capture is required for deletion. Existing CGQL
   goldens and sealed evidence stay intact. Fixed results do not replace
   atomicity, persistence, authorization or cancellation tests.
5. **Simplify configuration without a migration layer.** Remove Arango connection
   and vector settings and the advertised runtime backend choice. The selector
   may be deleted entirely. If any selector remains, accept only Native and
   reject invalid values before storage opens; remove the unknown-backend
   fallback. Generic configuration validation needs no Arango-specific migration
   diagnostic, deprecation window or compatibility shim.
6. **Allow breaking cleanup before first deployment.** There is no installed
   user contract to preserve, no last-supported-Arango release to maintain and
   no mandatory 3.0.0 bump solely for this removal. The owner-authorized
   pre-deployment exception applies to this batch. Ordinary version increments,
   incoming-work checks and full validation before authorized pushes still apply.
   Native-only readiness must pass before the first live deployment; actual
   publication and deployment remain separate actions.

## Scope and documentation boundary

Active product, architecture, operator and agent documentation should describe
Native directly once the code changes, with no legacy-backend narrative or
instructions to choose a storage engine. Arango may appear in an external AQL
migration comparison or explicitly deferred importer design. Past experiments,
resolved tickets and source evidence belong in historical engineering records;
do not fabricate different test outcomes to make the product story simpler.

No inventory of customer Arango deployments, application-state conversion,
customer cutover or backward rollback plan is needed: the owner confirms that
there are no such users. Local developer data is not a compatibility contract;
use disposable stores for verification and keep data cleanup separate from
documentation/code changes.

This batch does not add replication, failover, sharding, multiple writers, new
database backends, a new query implementation or a general AQL transpiler. The
[licensing/edition decision](decision_licensing.md) remains in force, including
Community ownership of planned read replicas and manual standby. Research
holdouts, model comparisons and broader console feature work are separate.

## Outcome

Direction accepted and runtime cleanup implemented by CG-67. The
[active batch](../plans/native-only-2026-09-12.md) has completed CG-65 and CG-67;
CG-68 has passed local Native-only readiness; publication and deployment remain separate.
CG-64/CG-66 remain deferred optional backlog, outside the readiness gate. This
owner clarification supersedes the initial migration-first ordering and forced
major-version/cutover requirements in the uncommitted September 12 draft.
Current code runs Native directly in both editions without an Arango dependency,
backend selector or opaque query passthrough. No compatibility shim is retained.

[CG-65 coverage verification](../issues/native-conformance-2026-09-12.md) is
complete: retained Native contracts and replacement capability doubles pass
both Rust suites and local release-binary probes. The
[CG-67 runtime report](../issues/native-runtime-2026-09-12.md) records both Rust
suites, 514 Native runtime assertions, cross-edition CLI/snapshot/tenant probes
and nine browser regressions. [CG-68 readiness](../issues/native-readiness-2026-09-12.md)
also passes the full shared CI suite, both Linux/amd64 images and Helm live
backup checks. No new release, image publication or deployment is claimed.
