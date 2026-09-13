# Decision: CGQL v2 — joins, subqueries, positional semantics

**Date:** 2026-07-04
**Owner:** skitsanos (approved all six), Claude (design + implementation)

## Context

CGQL v1 was deliberately single-FOR. The v2 arc (roadmap-2026-h2.md, Arc A)
adds the compositional layer. AQL is the reference semantics given the
project's heritage; deviations are explicit.

## Decisions (all six approved 2026-07-04)

These are the original July 4 decisions. The amendments below supersede the
LET-only expression restriction and later record current implementation limits.
Correlated traversal was subsequently delivered as D2 in the
[workload follow-ups](decision_cgql_v2_workload_gaps.md).

1. **Join model: no JOIN keyword.** Joins are nested `FOR` + `FILTER`;
   `FOR x IN <expr>` iterates anything that evaluates to an array —
   collection name, in-scope variable, document field (`d.tags`), array
   literal, function result. One mechanism = joins + un-nesting + computed
   iteration. Bare identifiers resolve as in-scope variables first, then
   as collection names; shadowing is a validation error, so resolution is
   deterministic.
2. **Positional clause semantics (breaking).** The body (`FOR`/`LET`/
   `FILTER`) executes in written order; each clause sees exactly the
   variables bound above it. Retires M5's "all LETs before all FILTERs" —
   a FILTER referencing a later LET is now a use-before-definition
   validation error. Bonus that fell out: a body with zero FORs runs on
   one synthetic row, so `LET x = (…) RETURN x` and `RETURN 1` now work
   (AQL parity).
3. **Subqueries: full read-query in parentheses, LET-position only in v2.**
   `LET xs = (FOR … COLLECT/SORT/LIMIT … RETURN …)` — the body is the
   complete read grammar, so COLLECT/SORT/LIMIT/DISTINCT inside come free.
   Correlated subqueries allowed (execute per outer row); uncorrelated
   ones execute once. No mutations inside (grammar), nesting depth capped
   at 4, `VECTOR_SEARCH`/traversal sources only in the outermost first FOR.
4. **Execution: ordered operator pipeline, materialize-once scans.** The
   plan body is a `Vec<PlanOp>`; both engines share one synchronous
   pipeline; engine-specific code reduces to a materialization phase that
   fetches every collection-scan site ONCE (per site, with that site's own
   projection/filter pushdown — pushed predicates are literal/bind-only,
   so materialize-once stays valid even inside correlated subqueries).
   Correlated join predicates stay in-engine (hash joins are a later,
   EXPLAIN-visible optimization). LIMIT pushdown survives per-plan for the
   single-FOR shape, including inside subqueries with their own LIMIT.
5. **Budgets:** `max_source_rows` counts all materialized source rows —
   every scan site, every expression expansion, every (re-)executed
   correlated subquery's rows — against one shared budget, enforced
   incrementally. Time budget unchanged.
6. **v2 non-goals** (spec'd as decisions, not gaps): JOIN/OUTER keywords,
   subqueries outside LET, multi-FOR-driven mutations (mutations keep the
   single-optional-FOR shape), cost-based join reordering, named views,
   UDFs.

## Outcome

- 2026-07-04, all milestones landed in one arc: grammar/AST body list,
  positional validation (use-before-definition, special-source-first,
  depth cap), planner operator pipeline with name resolution + correlation
  detection, shared synchronous runner over engine-specific
  materialization, per-site pushdown, EXPLAIN v2 (flat sources array).
- Corpus: 8 new exec cases (join, un-nesting, positional filter, array
  literal/var sources, un/correlated subqueries, COLLECT-in-subquery,
  FOR-less body) + 4 validate_err cases, all dual-engine byte-identical.
  v1 migration cost: one parse_err case became validate_err (FOR-less
  bodies now parse), one parser test moved to validation (malformed
  VECTOR_SEARCH now rejected by reserved-name check).
- Benchmarks-first caught a 20-35% regression in the first pipeline
  version (materialized docs were CLONED into rows; v1 moved them).
  Fixed by single-pass site consumption: bodies that execute once take
  ownership of their sites; only re-executed correlated subquery bodies
  clone. Post-fix numbers within noise of v1 (benchmarks.md).
- 312 tests green, clippy -D warnings, fmt clean.

## Amendment (2026-07-21) — subqueries in expression position, by desugaring

**Approved by skitsanos.** Decision 3 above restricted subqueries to LET position
and the "Explicitly Unsupported" list names "subqueries outside LET values". That
exclusion is narrowed, not lifted.

A real evaluation workload showed `LENGTH((FOR …))` is a reflex every AQL author
has, and the parse error it produced read as "CGQL cannot count a subquery"
rather than "write it as a LET"
(`decision_cgql_v2_workload_gaps.md`, F5/D5).

**What changed is the parser only.** A subquery appearing in an expression is
lifted into a synthetic `LET` immediately before the clause that uses it, so the
validator, planner and executor still only ever see the LET-position shape this
decision approved. There is no new execution form, correlation analysis is
untouched, and a test asserts the sugared query returns exactly what the
hand-written LET returns.

Synthetic bindings are named `$sqN`. `$` is not a legal identifier character, so
a lifted binding can never collide with a user variable.

**One position is refused rather than lifted:** a subquery in `SORT` or `RETURN`
*after* a `COLLECT`. `COLLECT` drops row variables, so hoisting past it would
change what the subquery sees. That case returns a parse error naming `COLLECT`
and telling the author to bind a `LET` before it — the alternative was silently
answering from the wrong scope.

The original amendment also described "subqueries driving mutations" as
unsupported. That phrase is too broad for the implemented contract: ordinary
read subqueries can feed the optional mutation `FOR` or body `LET`. Inline
mutation operands and nested writes have the narrower restrictions recorded
below. JOIN keywords, cost-based reordering, UDFs, named views, and wildcard
projections remain unsupported.

## Documentation reconciliation (CG-28, 2026-09-09)

This clarifies current behavior; it does not enable new grammar or execution.
Expression lifting covers body `FOR`/`LET`/`FILTER`, `COLLECT` group keys and
aggregate arguments, and `SORT`/`RETURN` when no `COLLECT` intervenes in that
query. A post-`COLLECT` inline subquery in `SORT`/`RETURN` is a **parse error**,
including uncorrelated ones. A precomputed value must also be carried through
grouping to remain available afterward.

The parser does not lift inline subqueries from `COLLECT … INTO` projections
or mutation selectors/payloads; validation rejects those remaining raw subquery
expressions. A read subquery in the mutation body is allowed, subject to CG-7's
dynamic-read rejection below. Subquery bodies use only the read grammar, so
nested writes are not supported. The optional outer mutation `FOR` can iterate
a read subquery; the one-outer-FOR restriction is not a ban on that form.

The [specification's position table](../reference/cgql.md#subqueries-in-expression-position)
links representative executable and rejected corpus cases. Nested generated
binding collisions found during this verification are tracked separately as
[CG-37](../issues/CG-37.md), resolved below. See the
[CG-28 verification report](../issues/subquery-contract-2026-09-09.md) for live
HTTP/Lua results, Rust gates, and scope limits.

## Generated subquery names (CG-37, 2026-09-09)

The parser now allocates synthetic `$sqN` bindings from one counter for the
complete parsed query, passing it into nested desugaring of both explicit LET
subqueries and lifted expressions. Nested scopes can see earlier outer
bindings, so independent counters were unsafe even though users cannot write
`$` identifiers. The allocator resets for each new parse; no global state or
new grammar is introduced. Internal names can be renumbered in EXPLAIN output.

User shadowing remains rejected, correlation retains the enclosing row, and
the four-level nesting cap and CG-28/CG-7 placement/mutation restrictions remain.
The original reproducer and four additional nested/sibling/correlated/bind and
DOCUMENT examples have expected execution results. Tests also check repeatable
parsing, bind collection, user shadowing, depth boundaries, compiled dynamic
reads, analysis parity, and exact source-row budget boundaries. See the
[CG-37 verification report](../issues/subquery-bindings-2026-09-09.md) for the
old-release reproduction, corrected release results, and Rust gates.

## Public CGQL plan errors (CG-38, 2026-09-09)

Both public CGQL HTTP routes classify `ExecutionError::Plan` as a validation
error and return HTTP 400, preserving the parser/validator diagnostic. This
includes normal queries, EXPLAIN, and EXPLAIN ANALYZE. The read-only route
previously wrapped these failures as query-execution errors and returned 500.
The correction changes only its typed plan-error branch; forbidden 403,
connection 503, and other execution/backend 500 mappings retain their behavior.

Production-route regressions verify identical client-plan responses on both
routes before storage access, with existing injected backend faults preserving
403/503/500. The saved release reproduced 24 status mismatches; the corrected
release passed 288 HTTP/Lua checks across resident/paged Native stores with zero
status mismatches and exact source/destination preservation after rejections.
See the [CG-38 verification report](../issues/query-error-status-2026-09-09.md)
for Rust gates, release evidence, and the scope of the runtime checks.

## Mutation backend-read restriction (CG-7, 2026-09-08)

Mutation execution does not install the read query's dynamic resolver. To
prevent silently committing null values or empty traversal results, semantic
validation now rejects `DOCUMENT()` and row-dependent traversal starts anywhere
in a mutation query. This includes nested read subqueries, mutation inputs,
untaken branches, and RETURN. Direct OLD/NEW projections and ordinary mutation
inputs remain available. Validation completes before materialization or any
write; no committed mutation is replayed to fetch dependencies.

This selects CG-7's explicit rejection remedy. Full support would require
defined read visibility and safe resolution. The shared-budget and backend-error
fixes in CG-15/CG-16 below do not enable mutation reads. Fetching data in a
separate read and passing mutation bind variables is supported, without a
transactional snapshot across those queries.

Verification: the pre-fix release committed null and retained it across restart
in resident and paged storage. The first fixed-release HTTP run revealed an
existing 500/400 mapping error; the query endpoint now maps plan errors to
HTTP 400. Final formatting, Clippy, all 893 tests, and 256 release HTTP/Lua
rejection cases passed, preserving complete stored documents after each case.
Ordinary mutation lifecycles, separate document reads, and restart checks also
passed. See [the regression report](../issues/mutation-backend-reads-2026-09-08.md).

## Shared backend analysis and accounting (CG-8 / CG-15, 2026-09-08)

Normal queries, precompiled backend plans, and EXPLAIN ANALYZE now share the
same materialization and dynamic document/traversal resolver. One query-wide
counter survives all head/tail retries and nested plans. Materialized source
rows count once; distinct backend document lookup attempts, correlated path
fetches, and runtime array/traversal expansions consume the same allowance.
Retries retain their charges. The precise [source-unit contract](../reference/cgql.md#execution-budgets)
clarifies that cached scan-site reuse does not charge another fetch, and that
these units do not count every intermediate join row or bound backend scan
effort. No configured limits are changed automatically.

Logical stage rows/runs exclude unresolved passes. Separate attempted counters
include all execution passes, while source/document fetch counters record real
backend work. This makes final result counts useful for analysis without hiding
the work needed to resolve them. COLLECT INTO projection reads are now included
in static backend-read detection. CG-7's mutation restriction remains in force;
document lookup error propagation is covered by the following CG-16 resolution.

The saved release reproduced zero-row analysis for a two-row result and
accepted five source units under a cap of three on both HTTP query routes and
Lua, in resident and paged storage. Final formatting, Clippy, all 901 tests,
60 release normal/analysis comparisons, 240 exact/one-below budget executions,
plain EXPLAIN, and restart-preservation checks passed. No final validation or
live check failed. See [the regression report and raw evidence](../issues/dynamic-query-2026-09-08.md).

## Document lookup failures (CG-16, 2026-09-08)

Dynamic document resolution now propagates backend failures instead of inserting
null. An actual absent document remains null; non-string values and strings
without a collection/key separator do not call the backend. The shared normal,
precompiled, and analyzed path aborts on a failed fetch without publishing
partial rows or a successful analysis report.

Forbidden and connection errors remain typed through the executor and public
HTTP/Lua layers, producing 403 and 503. Other backend execution errors remain
500. Lua may catch errors with pcall; error strings cannot impersonate typed
backend failures. Plain EXPLAIN and CG-7's mutation restriction are unchanged.
The OpenAPI reference records these response statuses.

Formatting, Clippy, all 907 tests, and the optimized build passed. Injected
query tests covered 210 expected failures and router tests covered 96 status
cases, including storage and connection faults. The real Native release
returned 403 in all 72 forbidden-lookup cases across both HTTP query routes and
Lua, normal and analyzed, in resident/paged storage; the saved release had
returned 200 for every case. Seventy-two controls, 36 plain EXPLAIN checks,
12 Lua pcall checks, and unchanged-document restarts also passed. An initial
live-test control used unsupported array indexing and was corrected to an
array FOR source. No final validation or live check failed. See
[the report and raw evidence](../issues/document-errors-2026-09-08.md).
