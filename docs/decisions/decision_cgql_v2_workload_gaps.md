# Decision: CGQL v2 measured against a 254k-document CRM workload — gaps and priorities

**Date:** 2026-07-21
**Owner:** skitsanos
**Status:** D1–D12 implemented by 2026-07-22; current status reconciled for
[CG-23](../issues/CG-23.md) on 2026-09-09 against `9fb4933`.
Extends [CGQL v2](decision_cgql_v2.md), including its approved amendment for
expression-position subqueries.

## Current outcome and evidence boundary (2026-09-09)

The work below is delivered. The initial four-of-eight completion result was
superseded by an eight-of-eight re-run with agreeing row counts. D1a/D1b removed
the measured scan bottleneck; D2 added correlated traversal; D3–D9 supplied the
expressions and helpers; D10/D11 reduced avoidable execution work. D12 is a
separate directed-construction capability, evaluated on legal contracts rather
than these CRM questions.

The timing tables retain the July 21–22 observations. They are not measurements
of the September revision, general performance guarantees, or proof of complete
AQL equivalence. Row-count agreement is distinguished from full-answer equality;
query 2a's intermediate reduced dimensions and query 2c's initial null-semantics
difference remain visible. The later release evaluation and its query-formulation
tradeoffs are recorded in [benchmarks](../research/benchmarks/native-backend.md#d10-move-calculations-down-2026-07-22).

The [CG-23 verification](../issues/roadmap-parity-2026-09-09.md) maps the delivery
claims to current code/tests and checks synthetic examples through the release
server. It does not reproduce the partner dataset or the CUAD quality scores;
[CG-25](../issues/CG-25.md) now recovers a partial scoring package, with the
execution-provenance gap still open.

**CUAD metric correction (CG-25, 2026-09-09):** the D12 row below is preserved
as the July report. Its P 0.744 / R 0.635 / F1 0.685 reproduces from recovered
accepted-evidence projections, but recall/F1 mix gold-entry true positives
with contract/category false negatives. With the same greedy matches,
122 matched and 208 unmatched gold entries give P 0.744 / R 0.370 / F1 0.494.
These are custom evidence-match diagnostics, not official CUAD scores or fact
truth judgments. The holdout was restarted after the redacted-endpoint runtime
fix, rather than executed untouched once. Raw nominations, full accepted fact
rows, and resolved model/settings remain absent; current defaults cannot
replace that history. See the [recovered package](../research/experiments/cuad-2026-07-22/README.md)
and [verification report](../issues/cuad-recovery-2026-09-09.md).
The [issue registry](../issues/README.md) is the active defect backlog.

## Historical context (2026-07-21)

Before this evaluation, CGQL exercises had run on corpora we authored. This was
the first run
against an **externally supplied dataset with externally specified questions**: a
pharma partner's CRM and contracting extract, evaluated head-to-head against
ArangoDB, which the partner already operates.

Shape of the workload (domain only — no partner, system or file names here):

- **254,076 documents**: ~27k people, ~15k organizations, ~14k contracts, ~15k
  fee tables, ~5k request forms, ~4.6k activities, ~4k engagement events, and
  ~168k edges across seven edge collections.
- Loaded from one backend-neutral extract into **both** backends, so any
  difference in results is the engine, not the ETL.
- **Eight questions** in three families set by the partner's own schema design:
  entity profiling ("everything about person X"), spend/compliance aggregation,
  and engagement planning.
- One edge collection is deliberately **heterogeneous** (`contract_party` →
  persons *or* organizations), because their design wants a single traversal to
  answer "everything about this party".

The initial result was poor. The measurements and subsequent corrections are
retained below so the original failure is not confused with current status.

## Initial measured result (2026-07-21; superseded)

| query | Arango (AQL) | CogniGraph (CGQL) |
|---|---|---|
| approval-gap aggregation | 14 ms | 179 ms ✅ identical counts |
| engagement coverage aggregation | 10 ms | 24 ms ✅ identical |
| unspent-budget scan | 241 ms | 138 ms ⚠️ semantics differ (see F3) |
| activities shared across contracts | 310 ms | **152,782 ms** ✅ same ranking |
| entity profiling | 94 ms | ❌ did not complete |
| profiling grouped by attribute | 89 ms | ❌ >10 min, aborted |
| spend by country/year | 549 ms | ❌ needs a second hop |
| coverage gap (has X but not Y) | 80 ms | ❌ >10 min, aborted |

Single-collection aggregation was competitive in this initial run.
**Every question needing a per-entity graph hop took minutes or did not finish.**
The completion result was superseded by the re-runs below.

## Historical findings and resolutions (2026-07-21–22)

Present-tense descriptions of missing behavior below describe the initial
revision. The resolution notes and delivery table establish what shipped.

### F1 — CORRECTED after implementing D1a

The original text of F1 said the blocker was that a correlated subquery "does
not use the edge index". Half right, and the wrong half mattered.

**What was true:** `list_documents_filtered` did not consult the adjacency index
at all, so ANY endpoint-equality filter cost a full collection scan. Fixed
(D1a): a warm endpoint-equality listing went **49 ms → 1 ms**, and the same
lookup through traversal is ~2 ms, so the two paths now agree.

**What was wrong:** fixing the backend did not move the correlated case at all
(142 ms → 79 ms for one outer row; still >240 s across all persons). The reason
is not the backend — it is that **the query engine never asks the backend**.
`materialize_backend_plan` recurses into `LetSubquery` and materializes the
subquery's scan **once**, up front, with only bind variables in scope. The
correlated value (`p._id`) cannot exist at that point, so the predicate is not
pushed, the whole collection is materialized, and the engine then re-filters
those rows **in memory, per outer row**. 17.6k rows × 27k outer rows is the
observed cost.

At that point, the remaining work was an engine change. The design was to index
the already-materialized site rather than re-fetch per outer row (which would
mean N backend calls): when a subquery's
scan carries `var.path == <expression correlated to an outer row>`, group the
materialized rows by `var.path` once, then serve each outer row by hash lookup.
One fetch, one grouping pass, O(1) per row — and it generalizes to any
collection, not only edges.

**Implemented (D1b).** `SiteRows::IndexedDocs` carries the documents plus a hash
index on one attribute path, built the first time a correlated equality filters
that site. Only STRING values are bucketed: a string can never equal a non-string
under `values_equal`, so the lookup may skip every other document without
changing which rows survive, and a non-string correlated key falls back to the
full expansion. The surviving `FILTER` still applies the whole expression, so the
index only ever narrows candidates — it never decides.

### Result after D1a + D1b + D3/D4/D5 (2026-07-21)

| query | Arango | CGQL before | CGQL after |
|---|---|---|---|
| entity profiling (1a) | 321 ms | ❌ did not complete | **301 ms** |
| profiling grouped by attribute (1b) | 94 ms | ❌ >10 min | **175 ms** |
| spend aggregation (2a) | 623 ms | ❌ | **220 ms** ⚠️ fewer dimensions |
| approval-gap aggregation (2b) | 15 ms | 179 ms | **89 ms** |
| unspent-budget scan (2c) | 193 ms | 138 ms | **137 ms** |
| engagement coverage (3a) | 12 ms | 24 ms | **11 ms** |
| coverage gap (3b) | 66 ms | ❌ >10 min | **244 ms** |
| activities shared across contracts (3c) | 316 ms | **152,782 ms** | **132 ms** |

Later re-run on July 21 after D3/D4/D5, with query 2c's CGQL arm restored to the
AQL semantics
(the ternary removed the reason it differed):

| query | Arango | CGQL |
|---|---|---|
| entity profiling (1a) | 106 ms | 285 ms |
| profiling grouped by attribute (1b) | 69 ms | 180 ms |
| spend aggregation (2a) | 435 ms | **232 ms** |
| approval-gap aggregation (2b) | 26 ms | 64 ms |
| unspent-budget scan (2c) | 210 ms | **134 ms** |
| engagement coverage (3a) | 4 ms | 10 ms |
| coverage gap (3b) | 68 ms | 241 ms |
| activities shared (3c) | 295 ms | **128 ms** |

8 of 8 on both, agreeing row counts, CGQL faster on three.

**8 of 8 complete, from 4 of 8.** In the first after table, the worst case improved
**1,157x** (152,782 ms → 132 ms) and beat Arango on that question.
Row counts agreed with Arango on every query, and the
independently-derived people-with-contracts total (5,576) matches Arango's
4,761 + 815 split exactly.

At this July 21 checkpoint, traversal still could not be correlated, but the
`LET`-subquery rewrite was index-assisted. D2 lifted that restriction on July 22,
as recorded under F2 below.

#### F1 as originally written (superseded by the correction above, kept for the record)
Three probes on the same data isolate it:

| operation | time |
|---|---|
| traversal from a **literal** start | **8 ms** |
| full scan of the edge collection | 62 ms |
| **correlated** subquery, ONE outer row (`FILTER e._to == p._id`) | **142 ms** |

The edge index exists and is ~18× faster than a scan. But an equality filter on
`_from`/`_to` inside a correlated subquery does **not** use it — it costs a full
collection scan per outer row. 142 ms × 27k persons ≈ 64 minutes, which is
precisely the observed behaviour.

*(The conclusion drawn from this — "teach the backend to use the index and the
correlated case gets fast" — was wrong. D1a did exactly that and the correlated
case barely moved. See the correction above.)*

### F2 — Traversal and correlation are mutually exclusive, and the workload needs both
`traversal sources are only allowed as the first FOR of the outermost query`:
the start expression is evaluated in an empty environment, so it may only be a
bind variable or a literal. "For every person, traverse their contracts" is not
expressible as a traversal at all. The expressible rewrite — joining `_from`/
`_to` in a `LET` subquery — is the one that materializes once and re-filters in
memory per outer row (F1, corrected).

**Traversal is indexed but cannot be correlated; correlation is possible but not
indexed.** Families 1 and 3 need exactly that intersection. This is the whole
gap; everything below is secondary.

**Resolved 2026-07-22 (D2), and the second half of the sentence turned out to be
the wrong worry.** By the time D2 was built, D1a/D1b had already made the
correlated rewrite indexed and fast, so D2 was never load-bearing for the
workload — it is expressiveness. The restriction came in two halves, both now
lifted: validation refused a traversal anywhere but the first `FOR`, and the
start expression was hard-limited to a bind variable or a string literal.

The design question was the same one `DOCUMENT()` had just answered: row
evaluation is synchronous and `traverse` is async. The measurement decided it —
a warm traversal against the backend's cached adjacency is **0.019 ms**, so
resolving per distinct start is affordable, and the second index the query
engine could have built over the edge collection was unnecessary. A correlated
traversal therefore reuses the same fetch-and-retry loop: the run records the
starts it could not resolve, they are traversed in one batch, and the run
repeats. Distinct starts dedupe, so the cost is one backend traversal per start
*value*, not per outer row.

Two things came along because the natural phrasing demanded them. AQL binds one,
two or three variables; CGQL required all three, so `FOR v IN 1..1 OUTBOUND x e`
was a parse error — another F5-class reflex, found by writing the first honest
test query. And an uncorrelated traversal in a non-first `FOR` had to start
merging with the rows above it rather than replacing them, which the old code
did not do because validation had guaranteed there were none.

The cost is recorded in `benchmarks.md` rather than summarized favourably here:
against the equivalent join the correlated form is about **2x** slower on a
whole-collection outer side, and the join rewrite remains the better tool for a
1-hop question at that scale. D2 earns its place on bounded outer sides, on
depths above 1 that no single join clause expresses, and on readability.

### F3 — No conditional expression, and real data is full of nulls
**Initial gap, resolved by D3 on 2026-07-21.**

There is no ternary, `IFNULL`, `COALESCE` or `NOT_NULL`. A comparison yields a
boolean, so `COLLECT flag = (x == null)` substitutes for bucketing — but
null-defaulting a numeric (`spent_hours ?? 0`) has no equivalent. The CGQL
variant of the unspent-budget query therefore had to *exclude* rows with a null
value, which is why its answer differs from the AQL one. On externally supplied
data, nulls are not an edge case.

### F4 — A heterogeneous edge collection has no clean discriminator
Their schema deliberately points one edge collection at two vertex collections.
CGQL has no `IS_SAME_COLLECTION`, `PARSE_IDENTIFIER` or `DOCUMENT()`, so the only
way to tell the target apart is `STARTS_WITH(e._to, "persons/")` — string
surgery on an identifier. It works, and it will silently break the day a
collection is renamed.

**Resolved 2026-07-22.** The question was how a synchronous row evaluator calls
an async backend. Three options were considered: make evaluation async (touches
every expression path, and costs a future per row for a feature most queries do
not use); give the function registry a backend handle (forces `fn(&[Value])` to
become async and re-entrant, for one function); or **fetch and retry**, which is
what shipped. A run records the ids it could not resolve, they are fetched in one
batch, and the run repeats with them available — reads are side-effect free, so
repeating is safe, and only the ids change between rounds. Nested lookups
converge because a later round sees the previous round's ids; four rounds are
allowed before the query is rejected rather than looped on.

The cost is that a plan containing `DOCUMENT()` executes twice in the common
case. That is why the retry path is entered only when a plan actually calls
`DOCUMENT` — the walker checks every expression position including field access
over a call, which is the form nearly everyone writes. Measured on the 254k-node
extract: `COLLECT` over a looked-up field across the whole party edge collection
is 266 ms, and the `FILTER` form returns the same 888 rows as the explicit join
that was the only way to write it before.

### F5 — Three syntax reflexes fail, and evaluators do not re-probe
**Initial ergonomics gaps, resolved by D5 on 2026-07-21.** The supported
function form uses a parenthesized query: `LENGTH((FOR … RETURN 1))`.

All three cost us a false "CGQL can't do this" during the run, corrected only
because we deliberately re-probed:

- `RETURN {role, n}` shorthand → must be `{role: role, n: n}`
- `LENGTH(FOR … RETURN 1)` → the subquery must be `LET`-bound first
- bare `COLLECT WITH COUNT INTO n` → requires a grouping binding

Each is a parse error against a reflex an AQL user has. An evaluator on a
timebox reads a parse error as absence of capability and moves on. The
capability is present in all three cases.

### F6 — `LIMIT` does not push past `COLLECT`, so "try it on a subset" is unavailable
**Still a limitation.** D10 defers projection-only calculations; it does not
move a result limit ahead of aggregation or enable input sampling.

Documented behaviour (pushdown requires no `SORT`/`COLLECT`), but worth stating
as a usability consequence: adding `LIMIT 100` to a grouped query does not reduce
the work scanned. When a query is too slow, the natural next move — shrink it and
see — silently measures the same thing. It cost us a wasted scaling run.

### F8 — A gap claimed from the docs that the code did not have

D7 asserted `DATE_DIFF` was missing. It was not: implemented, registered, and
documented — in a **second** function table further down the spec than the one
this analysis read. Only `DATE_ADD` was genuinely absent.

At that July checkpoint, all 46 registered functions were documented (the
`DATE_*` components share one slash-combined row, which is easy to miss with a
naive search but is not drift). So the spec was fine and the analysis was not.

The lesson is narrow and worth keeping: **enumerate the registry, not the
prose.** Two of this document's claims — this one and the original F1 — were
inferred from a reading rather than from the code, and both were wrong in a way
that would have sent someone off to build something that already existed.

### F7 — What was NOT exercised
Vector search, mutations, `EXPLAIN`, `DISTINCT`, deeper subquery nesting, and the
date functions. Treat this as "what a structured-CRM workload surfaced", not an
audit. In particular the differentiator we care about — governed construction
from narrative text — is not exercised by any of these eight questions, because
the dataset is structured records with almost no prose.

## Delivered decisions (2026-07-21–22)

This is the historical delivery register, not the current issue queue. D1 is
split into D1a and D1b; both are implemented. Current defects live in the
[CG registry](../issues/README.md).

| # | change | where | why now |
|---|---|---|---|
| **D1a** | Use the adjacency index for an `_from`/`_to` equality in `list_documents_filtered` | native backend | **DONE 2026-07-21.** Endpoint-equality listing **49 ms → 1 ms warm** (49x) on a 17.6k-edge collection. Benefits every caller, not just CGQL. |
| **D1b** | Make a correlated equality inside a subquery index-assisted | query engine (`run.rs`) | **DONE 2026-07-21.** Group-once hash index over the materialized site. The workload went from 4 of 8 queries not completing to **8 of 8**, with the worst case **152,782 ms → 132 ms**. |
| **D2** | Allow a traversal source to be correlated — start expression resolvable against the enclosing row | parser + executor | **DONE 2026-07-22.** Correlated starts resolve per distinct start through the same fetch-and-retry loop as `DOCUMENT()`, chosen after measuring a warm backend traversal at 0.019 ms. Brought two dependants: one- and two-variable traversal forms (three were required, so the AQL reflex was a parse error), and merging rather than replacing outer rows when an uncorrelated traversal is not the first `FOR`. As predicted, D1 landing first made this ergonomics, not survival: it is ~2x the equivalent join on a whole-collection outer side. EXPLAIN now reports `correlated` per traversal. |
| **D3** | Conditional expression: ternary or `COALESCE`/`IFNULL` | grammar + functions | **DONE 2026-07-21.** `cond ? then : else` plus `COALESCE`/`NOT_NULL`. A non-boolean condition takes the else branch — the same rule `FILTER` uses, rather than inventing truthiness. Query 2c's CGQL arm now matches its AQL arm exactly instead of excluding null-valued rows. |
| **D4** | Identifier helpers: `IS_SAME_COLLECTION`, `PARSE_IDENTIFIER`, `DOCUMENT()` | functions | **DONE 2026-07-22.** `IS_SAME_COLLECTION` and `PARSE_IDENTIFIER` shipped 07-21; both accept an id string or a document. `DOCUMENT()` landed 07-22 via fetch-and-retry (below) rather than by giving the function registry a backend handle. Postfix field access (`DOCUMENT(x).name`) came with it — the grammar previously allowed `.field` only off a variable. |
| **D5** | Accept the three AQL reflexes: shorthand object fields, function-over-subquery, bare `COLLECT WITH COUNT` | grammar | **DONE 2026-07-21.** Shorthand object fields, bare `COLLECT WITH COUNT INTO n`, and function-over-subquery shipped. The latter uses desugaring, after the amendment to `decision_cgql_v2.md` was approved: a subquery in expression position is lifted into a synthetic `LET` before the clause that uses it, so the planner and executor still see only the LET-position shape. Refused in `SORT`/`RETURN` after `COLLECT`, where hoisting would change scope. |
| **D6** | Array helpers: `SLICE` at minimum, then `FLATTEN`/`INTERSECTION`/`MINUS` | functions | **DONE 2026-07-21.** All four. Set operations use `UNIQUE`'s equality rule, so `1` and `1.0` are one value; a non-array argument is null rather than a silently empty result. |
| **D7** | Date arithmetic: `DATE_DIFF`, `DATE_ADD` | functions | **DONE 2026-07-21 — and the gap was smaller than stated.** `DATE_DIFF` already existed, registered AND documented; the claim that it was missing came from reading a partial function table in the spec rather than the code. Only `DATE_ADD` was genuinely absent. Calendar units are deliberately excluded; `DATE_ADD` uses fixed-duration units. |
| **D8** | Numeric/string casts (`TO_NUMBER`, `TO_STRING`); regex (`REGEX_TEST`, `REGEX_REPLACE`) | functions | **DONE 2026-07-21.** Plus `TO_BOOL`. Casts return **null** for unconvertible input rather than AQL's 0/"": a silent 0 inside a `SUM` is a wrong answer, not a missing one. Regex patterns are compiled once into a bounded cache — a `FILTER` over 27k rows calls the function once per row. |
| **D9** | `SORTED` / `SORTED_UNIQUE` | functions | **DONE 2026-07-22.** Surfaced while making the evaluation queries deterministic: `SLICE(UNIQUE(xs), 0, 3)` takes three arbitrary members of an unordered set, so the query had no single right answer. The design constraint was self-consistency, not novelty: same-type ordering is exactly the SORT clause's (a test asserts `SORTED(g)` equals the clause's output), dedup equality is exactly `UNIQUE`'s. The clause leaves mixed types unordered; a function returning an array cannot, so they take AQL's ladder (null < bool < number < string < array < object). The evaluation's person-360 query got its affiliation *names* back. One finding worth keeping: `SLICE(SORTED_UNIQUE(...), 0, 3)` is deterministic on each engine but NOT comparable across engines — the reference backend collates strings with ICU (measured: `a < B < b`, and `Guy?s B` before `Guy's A`), CGQL byte-wise with `COLLATE` as the opt-in. Same sets, different windows. The evaluation compares the full set. |
| **D10** | Move-calculations-down: defer RETURN-only `LET`s past SORT/LIMIT | planner + runner | **DONE 2026-07-22.** Found by decomposing the worst evaluation query (person 360, 6.7x behind): three subqueries referenced only by RETURN ran for 651 rows to return 5. The reference engine applies exactly this rule, by this name. Deferral skips work, never adds it: COLLECT/mutations disable the pass, blocking over-approximates. Person 360 went 428 → 99 ms (with the count-only reformulation the rule makes worthwhile), the board total from 1.6x behind to 0.86x — net faster. |
| **D11** | Fetch-and-retry re-runs only the phase that missed | executor | **DONE 2026-07-22.** The loop cloned every site and re-ran the whole plan per round even when every miss came from the deferred suffix. Now the head (body before the deferral split + COLLECT/SORT/LIMIT) retries only if it can miss — known statically — and tail rounds borrow sites and replay just the deferred LETs + projection over the surviving rows. Scan-count tests pin it: one head run for tail-only reads. Person 360 with `DOCUMENT` 259 → 81 ms; the `DOCUMENT`-bearing idiom now beats the join rewrite written to avoid it. |
| **D12** | Directed construction: extract per-document facts for a GIVEN taxonomy through the grounding gates | construct | **DONE 2026-07-22.** Found on CUAD (510 public expert-labeled contracts): auto-drafted spaces extract recurring corpus facts (97% precision there) but rules are instance-anchored, so per-document clause detection was inexpressible. Now: `POST /api/construct/directed` — the model nominates facts constrained to the caller's taxonomy, deterministic gates decide (verbatim evidence with offsets recovered into the original text, endpoints present in the evidence sentence, restraint vocabulary affirmed — negated vocabulary grounds nothing), and survivors flow through the SAME writer as rule grounding: same occurrence-v1 rows, same delete-and-rebuild reconciliation, same semantics sidecar. Attribution via `reviewed_by = directed:<model>@directed-policy-v1`, not a parallel schema. One request = one completion call (32-chunk cap); slices are idempotent per chunk. **Holdout result** (40 held-out CUAD contracts, taxonomy frozen after 3 design-set rounds, single pass): **P 0.744 / R 0.635 / F1 0.685** against expert clause labels — six of eight categories at 0.81–0.90 precision; the two weak ones have a named failure mode each (the non-exclusive-grant trap; retention-vs-assignment). Also measured: the semantics-v1 quarantine is NOT precision-selective on directed legal facts (P flat, R halved) — its self-flagging success on auto-drafted spaces does not transfer. |

Explicitly **not** revisited: the `decision_cgql_v2.md` exclusions (JOIN keywords,
hash joins, UDFs, named views, wildcard projections) remain correct scope calls.
Cost-based join reordering stays excluded, but note that with nested-`FOR` joins
the join order is currently the query author's responsibility, and on a 254k
graph that is a sharp edge.

## Outcome after delivery

The original missing-optimization conclusion is superseded: D1a/D1b and D2 are
implemented, as are D3–D12. All eight CRM questions completed in the recorded
re-run. The July 22 release board later recorded full-answer equality for its
selected formulations and, after D11, a **780 ms CGQL / 927 ms reference** total;
see the [dated measurements](../research/benchmarks/native-backend.md#d11-retry-only-the-phase-that-missed-2026-07-22).

Those results establish progress on this workload. They do not establish a
general speed advantage: correlated traversal cost about twice the equivalent
join on the measured whole-collection outer side, query authors still choose
join order, and `LIMIT` does not reduce pre-aggregation input work. Vector search,
mutations, and governed construction were outside these eight CRM questions.
The September source/example verification confirms delivered capabilities;
remaining correctness and documentation work is tracked in the
[open issue registry](../issues/README.md).
