# CGQL Specification

(Referenced elsewhere as "cgql-v1" — the filename is stable; the language
has grown well past its v1 read-only scope and this document tracks it.)

CogniGraph Query Language (CGQL) is the native query language for CogniGraph:
an executable, read-write language covering iteration, filtering, LET
bindings, graph traversal, vector search, grouping (COLLECT with AGGREGATE,
INTO, and WITH COUNT), sorting, projection, 30+ built-in functions, and
permission-gated mutations. It runs identically on the in-memory test
executor and on any `GraphBackend`; a file-driven corpus
(`crates/cognigraph-query/tests/corpus/`) holds both engines to the same
results.

CGQL is inspired by AQL/XQuery-style iteration:

```cgql
FOR d IN documents
FILTER d.category == @category
RETURN { id: d._id, title: d.title }
```

## Case Rules

Keywords and built-in source names are case-insensitive:

```cgql
for d in documents return d
FoR d In documents ReTuRn d
FOR d IN documents RETURN d
```

Identifiers are case-sensitive. `d`, `D`, `documents`, and `Documents` are
distinct.

## Query Shape

The original v1 shape had exactly one `FOR` clause and one `RETURN` clause.
Current CGQL keeps that form valid and extends it with zero-FOR expressions,
multiple `FOR` clauses, positional `LET`/`FILTER` semantics, LET subqueries,
grouping, and mutations.

```text
FOR ...
FILTER ...     // optional, repeatable before SORT/LIMIT
SORT ...       // optional
LIMIT ...      // optional
RETURN ...
```

Clause order is fixed:

1. `FOR`
2. zero or more `LET` and `FILTER`, in any interleaving
3. optional `COLLECT` (grouping)
4. optional `SORT` (one or more keys)
5. optional `LIMIT`
6. `RETURN` (optionally `RETURN DISTINCT`)

`LET name = expr` binds a per-row variable. The body executes in written
order: a `LET` sees variables bound above it, and a following `FILTER` sees
that binding. A filter cannot reference a later `LET`; validation reports
that as use-before-definition. Later sort keys, projections, and `LET`s can
reference earlier bindings.

`COLLECT name = expr [, ...] [AGGREGATE name = SUM|MIN|MAX|AVG(expr) [, ...]] [WITH COUNT INTO total]` groups rows
by the evaluated key tuple (numeric-aware equality), preserving
first-occurrence order. After `COLLECT`, only the binding names and the
count variable are in scope — `SORT` and `RETURN` cannot reference the
original row variable. `INTO name [= expr]` captures each group's rows as an array: with an expression, the projected value per row; bare, an object of all in-scope variables per row (keys sorted). `AGGREGATE` computes per-group aggregates (the array built-ins applied to each group's collected values, so null handling matches SUM/MIN/MAX/AVG). `WITH COUNT INTO` binds the group size.

`RETURN DISTINCT expr` deduplicates the projected results, preserving
first-occurrence order, using numeric-aware equality.

`FILTER` after `SORT` or `LIMIT` is invalid. Repeated `SORT` or repeated
`LIMIT` is invalid.

## Collection Queries

Collection queries bind one variable to documents from a collection:

```cgql
FOR d IN documents
RETURN d
```

The variable and collection are identifiers. Collection names are case-sensitive.

## Traversal Queries

Traversal queries bind a vertex, and optionally an edge and a path:

```cgql
FOR v, e, p IN 1..3 OUTBOUND @start relationships
FILTER e.confidence >= @min_confidence
RETURN { vertex: v, edge: e, path: p }
```

Only the vertex is required — `FOR v IN …` and `FOR v, e IN …` are both legal.
Nothing is bound for a variable that is not named, and the engine skips building
it, which matters most for the path: it carries every vertex and edge along the
way (see `benchmarks.md`).

Traversal directions:

- `OUTBOUND`
- `INBOUND`
- `ANY`

The depth range is inclusive. `1..3` means minimum depth 1 and maximum depth 3.
The parser rejects ranges where the minimum exceeds the maximum.

A range starting at `0` includes the start vertex itself: the vertex variable
binds the start vertex, the edge variable binds `null`, and the path has zero
edges with a score of `1.0`.

### Where a traversal may appear, and what it may start from

A traversal is an ordinary `FOR`: it may be the first one or a later one, and it
may appear inside a `LET` subquery. The start is an ordinary expression,
validated against the variables bound above it, so it can read the enclosing
row:

```cgql
FOR p IN persons
FILTER p.specialty == "Oncology"
FOR c IN 1..1 INBOUND p._id contract_party
COLLECT person = p.full_name WITH COUNT INTO n
RETURN { person: person, contracts: n }
```

Rejected at validation: a start that could never be an identifier — a number,
boolean, null, array or object literal. Anything that might evaluate to a string
is left to the run, where a start that is absent or not a string contributes no
rows for that row rather than failing the query.

Two costs follow from how a **correlated** start executes, and they are worth
knowing before reaching for it:

- The backend is traversed once per **distinct start value**, not once per outer
  row, and the plan runs twice — once to discover the starts, once to answer.
  Over a whole collection that is one call per member.
- The join rewrite (`FOR e IN edges FILTER e._to == p._id`) is roughly **twice
  as fast** for a 1-hop question at that scale, because the edge site is grouped
  once and answered by hash lookup. Prefer the traversal when the outer side is
  bounded, when the depth exceeds 1 — which no single join clause expresses — or
  for readability. Measured numbers are in `benchmarks.md`.

`EXPLAIN` reports `correlated` on each traversal source, which is the quickest
way to tell which of the two costs a query has.

A start that names no variable — a bind variable such as `@start`, or a string
literal such as `"documents/abc"` — is uncorrelated and is fetched once before
the run, as it always was, wherever the `FOR` sits.

## Vector Search Source

Vector search is a first-class source:

```cgql
FOR d IN VECTOR_SEARCH(documents, @embedding)
LIMIT 10
RETURN { document: d, score: d._score }
```

The vector input is semantically valid when it is:

- a bind variable
- a numeric array literal, for example `[0.1, 0.2, -0.3]`

`VECTOR_SEARCH` is reserved as a source name. It is not valid as a normal
function call in v1.

## Filters

Filters accept expressions:

```cgql
FILTER d.category == @category
FILTER d.active == true AND d.score >= 0.75
```

Multiple `FILTER` clauses are allowed and are evaluated cumulatively.

## Sort

Sort accepts one or more expressions, each with an optional direction:

```cgql
SORT d.created_at
SORT d.created_at DESC
SORT d.category ASC, d.score DESC
```

Multiple keys compare lexicographically, each with its own direction.
If no direction is provided, a key sorts ascending.

## Limit

Limit accepts either count or offset plus count. Each operand is an unsigned
integer literal or a bind variable; the two may be mixed:

```cgql
LIMIT 20
LIMIT 10, 20
LIMIT @count
LIMIT @offset, @count
LIMIT 10, @count
```

Validation rejects `LIMIT 0`. Validation options can set a maximum accepted
limit. A bound operand must be supplied as a non-negative integer JSON
number; strings, floats (including `2.0`), negatives, booleans and null are
rejected before execution with
`LIMIT count bind variable `@count` must be a non-negative integer` (or
`offset`). A bound count of zero or above the maximum raises the same errors
as a literal. Bound values are resolved after the bind-variable presence
check and before any storage access, including inside `LET` subqueries, so
an invalid value never starts a scan. Expressions (`@n + 1`, `-@n`,
`d.count`) are not operands. Plain `EXPLAIN` shows the bind names
(`LIMIT @offset, @count`); pushdown behaves as for literals with the resolved
values. See the [decision record](../decisions/decision_limit_bind_variables.md).

## Return

### Conditional expression

`cond ? then : else`. Only `true` takes the then-branch — a non-boolean condition
takes the else-branch, matching how `FILTER` decides truth. `COALESCE`/`NOT_NULL`
express the common null-defaulting case without a branch.

```cgql
FOR f IN fee_tables
RETURN { spent: f.spent == null ? 0 : f.spent, same: COALESCE(f.spent, 0) }
```

### Subqueries in expression position

In supported expression positions, a parenthesized read subquery is lifted into
a synthetic `LET` before the clause that uses it. `LENGTH((FOR … RETURN …))`
therefore uses the same subquery execution as a hand-written `LET`. The inner
parentheses enclose the query; the outer pair belongs to the function call:

```cgql
FOR d IN documents
FILTER LENGTH((FOR e IN relationships FILTER e._from == d._id RETURN 1)) > 1
RETURN d._key
```

This is the executable [filter corpus case](../../crates/cognigraph-query/tests/corpus/exec/sq_expr_filter.cgql).
The current positions and restrictions are:

| Position | Support and checked example |
|---|---|
| `FOR` source, `LET` expression, `FILTER` | Supported: [iteration](../../crates/cognigraph-query/tests/corpus/exec/sq_expr_for.cgql), [LET and object projection](../../crates/cognigraph-query/tests/corpus/exec/sq_expr_let_return.cgql), and the filter above |
| `SORT` or `RETURN` in a read query without `COLLECT` | Supported: [sorting](../../crates/cognigraph-query/tests/corpus/exec/sq_expr_sort.cgql) and [nested read queries](../../crates/cognigraph-query/tests/corpus/exec/sq_expr_nested.cgql) |
| `COLLECT` grouping expressions and `AGGREGATE` arguments | Supported; computed before grouping: [group key](../../crates/cognigraph-query/tests/corpus/exec/sq_expr_collect.cgql), [aggregate](../../crates/cognigraph-query/tests/corpus/exec/sq_expr_aggregate.cgql) |
| `SORT` or `RETURN` after `COLLECT` in the same query | Rejected during parsing/desugaring, even for an uncorrelated subquery: [SORT rejection](../../crates/cognigraph-query/tests/corpus/parse_err/sq_postcollect_sort.cgql), [RETURN rejection](../../crates/cognigraph-query/tests/corpus/parse_err/sq_postcollect_return.cgql) |
| Inline subquery in `COLLECT … INTO name = expression` | Not lifted; rejected by validation: [rejection](../../crates/cognigraph-query/tests/corpus/validate_err/sq_into_inline.cgql). Precompute it with `LET`, then project the variable: [working form](../../crates/cognigraph-query/tests/corpus/exec/sq_precollect_into.cgql) |
| Inline subquery in a mutation selector or payload (`INSERT`, `UPDATE`, `REPLACE`, `REMOVE`, or an `UPSERT` operand) | Not lifted; rejected by validation: [payload rejection](../../crates/cognigraph-query/tests/corpus/validate_err/sq_mutation_operand.cgql). Use the supported body forms described under [Mutations](#mutations) |
| Mutation inside a subquery | Rejected by the read-query grammar: [rejection](../../crates/cognigraph-query/tests/corpus/parse_err/sq_contains_mutation.cgql) |

For a post-`COLLECT` result, precompute the subquery before grouping **and carry
its value through `COLLECT`** using a group binding, aggregate, or `INTO`
projection. Merely moving it to a `LET` does not keep that variable in scope:

```cgql
FOR d IN documents
LET size = LENGTH((FOR x IN documents RETURN 1))
COLLECT size = size WITH COUNT INTO n
RETURN {size, n}
```

The [rewrite corpus case](../../crates/cognigraph-query/tests/corpus/exec/sq_postcollect_rewrite.cgql)
returns `[{"size":4,"n":4}]` on the shared corpus. The rejected inline positions
currently report `subqueries are only allowed as LET values`; that diagnostic
describes the validator's internal form, not a ban on supported expression
syntax. Both `POST /api/query` and the read-only `POST /api/search/query`
return 400 for parse/validation failures, including EXPLAIN and EXPLAIN ANALYZE.
Their error bodies retain the same diagnostic with a `Validation error:` prefix.
This response-mapping correction is recorded in [CG-38](../issues/CG-38.md).

Nested read subqueries are supported up to the depth limit below. Generated
`$sqN` bindings are unique across the complete parsed query, including nested
and sibling bodies; numbering is deterministic for each parse. The former
collision defect is resolved in [CG-37](../issues/CG-37.md), with the
[original reproducer](../../crates/cognigraph-query/tests/corpus/exec/sq_nested_binding_collision.cgql)
and [correlated explicit-LET example](../../crates/cognigraph-query/tests/corpus/exec/sq_nested_correlated.cgql)
now in the execution corpus. User-variable shadowing remains invalid.

### Object shorthand

`{ x }` is `{ x: x }`, and may be mixed with explicit fields: `{ title, id: d._id }`.

### Grouping without a binding

`COLLECT WITH COUNT INTO n` is valid with no grouping binding — it counts the
rows reaching the clause.

`RETURN` accepts any expression:

```cgql
RETURN d
RETURN d.title
RETURN { id: d._id, title: d.title }
RETURN [d._id, d.title]
```

Object projection field names must be unique after validation.

## Expressions

Supported literals:

- strings with the full JSON escape set
  (`\"`, `\\`, `\/`, `\b`, `\f`, `\n`, `\r`, `\t`, `\uXXXX`)
- numbers with required leading digits, for example `1`, `1.5`, `-1`
- booleans: `true`, `false`
- `null`
- arrays
- objects

Supported references:

- identifiers: `d`
- paths: `d.title`, `d.metadata.source`
- bind variables: `@category`

A field may also be read off a parenthesised expression or a function result,
not only off a variable:

```cgql
RETURN DOCUMENT(e._to).name
RETURN (cond ? a : b).label
```

Supported operators, from highest to lowest precedence:

| Precedence | Operators |
|------------|-----------|
| 1 | `NOT`, unary `-` |
| 2 | `*`, `/` |
| 3 | `+`, `-` |
| 4 | `==`, `!=`, `<`, `<=`, `>`, `>=`, `IN` |
| 5 | `AND` |
| 6 | `OR` |

Numbers compare by numeric value regardless of their JSON representation:
a stored integer `2` equals the literal `2` (and `2.0`). `==`, `!=`, `IN`,
and `UNIQUE` all follow this rule, including inside nested arrays and
objects.

Comparison operators are non-associative: `a == b == c` is a parse error and
must be written with explicit parentheses or logical operators.

Parentheses override precedence:

```cgql
FILTER (d.a == 1 OR d.b == 2) AND d.c == 3
```

Function calls are parsed:

```cgql
FILTER LENGTH(d.title) > 3
```

`VECTOR_SEARCH` is reserved for source syntax.

## Built-in Functions

Function names are case-insensitive. Unknown function names and wrong
argument counts are rejected at validation time. Argument *types* are
lenient at execution time: a wrong type yields `null` (which is falsy in
`FILTER`).

| Function | Arity | Semantics |
|----------|-------|-----------|
| `LENGTH(v)` | 1 | string → char count; array/object → element count; else null |
| `NORMALIZE_NFC(s)` | 1 | explicit NFC text normalization; non-string → null; opaque identifiers should retain their exact form |
| `UPPER(s)` / `LOWER(s)` / `TRIM(s)` | 1 | string transforms; non-string → null |
| `CONTAINS(s, needle)` | 2 | substring test → bool |
| `STARTS_WITH(s, prefix)` | 2 | prefix test → bool |
| `SUBSTRING(s, start[, len])` | 2–3 | char-based; negative arguments → null; clamps at the end |
| `CONCAT(...)` | 1+ | strings/numbers/bools concatenated; nulls skipped; arrays/objects → null |
| `SPLIT(s, sep)` | 2 | array of strings; empty separator → null |
| `ABS(n)` / `FLOOR(n)` / `CEIL(n)` / `ROUND(n)` | 1 | non-number → null; ROUND is half-away-from-zero |
| `MIN(arr)` / `MAX(arr)` | 1 | over numeric elements; none → null |
| `SUM(arr)` | 1 | sum of numeric elements; empty → 0 |
| `AVG(arr)` | 1 | mean of numeric elements; empty → null |
| `FIRST(arr)` / `LAST(arr)` | 1 | element or null |
| `UNIQUE(arr)` | 1 | dedupe preserving first occurrence |
| `SORTED(arr)` | 1 | ascending; same-type order matches the SORT clause, mixed types take the ladder null < bool < number < string < array < object; non-array → null |
| `SORTED_UNIQUE(arr)` | 1 | `SORTED` then deduped under `UNIQUE`'s equality (`1` and `1.0` are one value), first of each tie kept |
| `SLICE(arr, start[, length])` | 2–3 | negative `start` counts from the end; negative `length` drops that many from the end; out-of-range clamps |
| `FLATTEN(arr[, depth])` | 1–2 | depth defaults to 1 |
| `INTERSECTION(a, b, ...)` | 2+ | values in EVERY argument, deduped, first-argument order; non-array → null |
| `MINUS(a, b, ...)` | 2+ | values in the first and in none of the others, deduped |
| `TO_NUMBER(v)` | 1 | number as-is, numeric string parsed, bool → 1/0; otherwise **null** (not 0) |
| `TO_STRING(v)` | 1 | scalars rendered (integral numbers without a fraction); containers → null |
| `TO_BOOL(v)` | 1 | bool as-is; number ≠ 0; non-empty string; otherwise null |
| `REGEX_TEST(s, pattern[, ci])` | 2–3 | bool; non-string or uncompilable pattern → null |
| `REGEX_REPLACE(s, pattern, repl[, ci])` | 3–4 | replaces every match; `$1` / `${name}` reference groups |
| `COALESCE(...)` / `NOT_NULL(...)` | 1+ | first non-null argument; all null → null |
| `IS_SAME_COLLECTION(collection, doc_or_id)` | 2 | does the id (or the document's `_id`) live in `collection`? non-identifier → null |
| `PARSE_IDENTIFIER(doc_or_id)` | 1 | `{ collection, key }` from a `collection/key` id; non-identifier → null |
| `DOCUMENT(id_or_array)` | 1 | fetch by `collection/key`; array → array of results, positions preserved. Missing or non-identifier → null. See below |
| `HAS(obj, key)` | 2 | attribute existence → bool |
| `COSINE_SIMILARITY(a, b)` | 2 | cosine similarity of two numeric arrays; null on mismatch/zero norm |
| `TYPENAME(v)` | 1 | "null", "bool", "number", "string", "array", "object" |
| `NOW()` | 0 | current UTC time as an RFC 3339 string |
| `DATE_YEAR/MONTH/DAY/HOUR/MINUTE/SECOND(s)` | 1 | integer component; accepts RFC 3339 or `YYYY-MM-DD`; bad input → null |
| `DATE_TIMESTAMP(s)` | 1 | unix epoch milliseconds |
| `DATE_DIFF(from, to, unit)` | 3 | signed, fractional; unit: "days", "hours", "minutes", "seconds" |
| `DATE_ADD(date, amount, unit)` | 3 | RFC 3339 string; same units as `DATE_DIFF`, so the two are inverses. Calendar units (months, years) are absent: they are not a fixed number of seconds |

Arithmetic producing non-finite numbers (for example division by zero)
yields `null`.

### DOCUMENT()

`DOCUMENT("persons/p1")` resolves a document by its full identifier. It is the
way to read across a **heterogeneous edge collection** — one whose `_to` points
into more than one collection — because no single `FOR` can range over them:

```cgql
FOR e IN contract_party
LET party = DOCUMENT(e._to)
RETURN {
  kind:  IS_SAME_COLLECTION("persons", e._to) ? "person" : "organization",
  label: COALESCE(party.full_name, party.name)
}
```

In read queries it works in expression positions such as `LET`, `FILTER`,
`SORT`, `COLLECT`, and `RETURN`, and accepts an array of ids, returning results
in the same order. Queries containing a mutation reject `DOCUMENT()` during
validation, including calls in nested read subqueries and mutation `RETURN`.
See [Mutations](#mutations) for the current limitation.

An absent document (`Ok(None)` from the backend) returns `null`. Non-string
values and strings without a collection/key separator also return `null`
without a backend lookup. Backend errors abort the whole query, including
array lookups and `EXPLAIN ANALYZE`; they never become successful null values
or partial result rows. Public HTTP query routes and uncaught Lua
`graph.query()` errors return 403 for forbidden access, 503 for backend
connection failures, and 500 for other backend execution failures. A Lua
script may catch the error with `pcall` and handle it; ordinary script error
text cannot impersonate a typed backend failure. Plain `EXPLAIN` remains
inert. See [CG-16 verification](../issues/document-errors-2026-09-08.md).

Two behaviours follow from how it executes, and are worth knowing:

- **Fetches are batched, not per row.** Row evaluation is synchronous, so a
  query runs, records the ids it could not resolve, fetches those in one batch,
  and runs again with them available. Each distinct id costs one fetch for the
  whole query regardless of how many rows ask for it. Reads are side-effect
  free, so the repeat is safe.
- **Nesting is bounded.** `DOCUMENT(DOCUMENT(x)._id)` resolves, because a later
  round sees the ids the earlier one produced. Four rounds are allowed; a query
  that keeps deriving fresh ids from fetched documents past that is rejected
  rather than looped on. Any wall-clock budget spans all rounds.

A traversal's start expression is evaluated before the query runs, so
`DOCUMENT()` is not available there — that position still takes a bind variable
or a literal.

## Bind Variables

Bind variables start with `@` followed by an identifier:

```cgql
@category
@embedding
@min_confidence
```

Bind variable names are collected into the parsed AST in sorted unique order.

Execution matches provided bind variables against the declared set strictly and
eagerly, before any storage access: a missing bind variable fails with
`missing bind variables: <names>`, and an extra one fails with
`unexpected bind variables: <names>`.

## Unicode Semantics

Strings are Unicode throughout. `LENGTH` and `SUBSTRING` operate on
characters (codepoints), never bytes; `UPPER`/`LOWER` use full Unicode case
mapping (`UPPER("straße")` is `"STRASSE"`).

- **Collation**: `SORT` orders strings by codepoint by default; append
  `COLLATE "de"` (any BCP-47 locale) to a SORT clause for locale-aware
  comparison of its string keys (ICU4X). Invalid locales are rejected at
  validation time.
- **Exact identity**: literals decode JSON escapes without normalization;
  bind variables and stored caller-owned strings retain the same exact Unicode
  representation. Composed `é` (U+00E9) and decomposed `e`+U+0301 compare unequal
  and can name different documents. This applies to `DOCUMENT()` literals,
  endpoint predicates, model names, and nested payloads.
- **Explicit text normalization**: `NORMALIZE_NFC(s)` produces NFC for a string
  and null for other inputs. Use `NORMALIZE_NFC(d.title) == NORMALIZE_NFC(@title)`
  when canonical text comparison is intended. Do not normalize opaque handles
  to choose a target. This supersedes implicit literal/write normalization;
  existing data is not migrated. Construction evidence still uses its explicit
  NFC boundary before hashes and byte spans. See the
  [CG-33 decision and compatibility notes](../decisions/decision_exact_reference_identity.md).

## Null Semantics

Accessing a missing attribute path on a document yields `null` rather than an
error. Only an out-of-scope root identifier is an error, and validation rejects
that before execution.

`null` is falsy in every boolean context: a filter expression evaluating to
`null` excludes the row, `NOT null` is `true`, and `null` behaves as `false`
as an `AND`/`OR` operand. Any filter result other than `true`, `false`, or `null` is a
type error. Comparisons other than `==`/`!=` between values of different types
(including `null`) evaluate to `false`.

## Execution Budgets

Callers can bound a query's cost: a cap on rows materialized from the
source (scan, traversal, or vector search) and a wall-clock budget checked
per row through filters, projections, and mutation loops. Exceeding either
fails the query with a deterministic error. The HTTP CGQL endpoints apply
`COGNIGRAPH_CGQL_MAX_SOURCE_ROWS` / `COGNIGRAPH_CGQL_TIME_BUDGET_MS` (0 = unlimited, the
default); the whole-request timeout remains the outer guard.

The row cap uses one counter across initial materialization, backend resolution,
query-head and deferred-tail passes, and nested plans. Its units are:

- Collection-scan, vector-search, and static-traversal rows count when
  materialized. Reusing these cached sites does not count as another fetch.
- Each distinct backend `DOCUMENT()` lookup counts once, including a missing
  document. Duplicate IDs share the lookup. Non-string values and strings
  without a collection/key separator cause no backend lookup and cost no
  lookup unit. The charge happens before sending the lookup.
- A correlated traversal counts its fetched paths once per distinct
  site/start pair, plus the paths expanded into rows on every execution pass.
- Array/variable source expansion counts its elements on every pass, including
  nested plans. Work performed during an unresolved speculative pass remains
  charged, even though its intermediate results are discarded.

`EXPLAIN ANALYZE` follows these same backend budget rules. Configured limits may
reject dynamic queries that previously passed because each phase received a
fresh allowance. Source-row units describe fetched data and runtime source
expansions, not every intermediate join row or a bound on backend scan effort.

Lua `graph.query()` applies the same source-row cap in read-only and writable
modes. For Lua, the configured time allowance is shared across one script's
execution and all graph callbacks; another query does not renew it. Parsing
and planning consume the remaining allowance. Cancellation and timing are
cooperative: a synchronous backend operation must return or yield before its
future can be stopped, and completed writes are not rolled back.

## Parse Errors

Parse errors report a 1-based line and column position when the failure comes
from the grammar, formatted as `parse error at line L, column C: <message>`.

## Comments

Line comments are supported:

```cgql
// only published documents
FILTER d.published == true
```

## Semantic Validation

The validator checks:

- referenced root identifiers are in scope
- traversal variables are distinct
- variable names do not use reserved words
- object projection fields are unique
- collection names do not use reserved words
- `VECTOR_SEARCH` is not used as a normal function name
- `LIMIT` is greater than zero and below the configured maximum
- traversal depth is below the configured maximum
- vector search input is a bind variable or numeric array literal

## Logical Planning

The non-executing planner validates a parsed query and converts it into a
logical plan. A logical plan contains:

- one source:
  - collection scan
  - vector search
  - traversal
- zero or more filter expressions
- optional sort expression
- optional limit
- one projection expression from `RETURN`
- sorted unique bind variable names

Planning does not evaluate expressions, access storage, optimize indexes, or
translate CGQL into another query language.

## In-Memory Test Execution

The `cognigraph-query` crate includes an in-memory executor for correctness
testing. It is not a production storage backend.

The executor supports the same language semantics exercised by the
file-driven dual-engine corpus, including:

- collection scans over named in-memory collections
- filters
- sorting
- limit/offset
- object and array projections
- bind variables
- the full built-in function registry and expression semantics
- vector search over document `embedding` arrays using cosine similarity
- traversal over edge collections with `_from` and `_to` fields
- LET bindings and subqueries, multiple FOR sources, COLLECT/AGGREGATE/INTO,
  DISTINCT, EXPLAIN/EXPLAIN ANALYZE, and mutations under `ReadWrite` mode

The executor is the correctness reference used to keep native backend
execution byte-identical for the shared corpus; it is not a storage engine.

## GraphBackend Execution

CGQL can also execute against any `GraphBackend` implementation through the
query crate's backend executor. This path maps logical sources to backend trait
calls:

- collection scan -> `list_documents`, `list_documents_projected`, or
  `list_documents_filtered` as the plan permits
- vector search -> `vector_search`
- traversal -> `traverse`

Sorting, residual filters, limits, and projections are evaluated by the CGQL
layer after the backend source operation returns rows. Projection, supported
filter conjuncts, scan limits, vector thresholds, and exact-depth-1 traversal
confidence can be pushed into backend capabilities; the detailed semantic
guards are in the Pushdown section below.

Lua uses one query entry point: `graph.query(query, bind_vars)` executes parsed
CGQL on Native storage. `graph.query_language` reports `cgql`. Query text is
parsed under the caller's read/write permissions and execution budget; there is
no opaque query passthrough.

## Mutations

CGQL supports data modification, executed only in read-write mode
(`QueryMode::ReadWrite`; the `POST /api/query` endpoint, gated by
`COGNIGRAPH_CGQL_MUTATIONS_ENABLED`). `/api/search/query` always parses CGQL in
read-only mode, whether `language` is omitted or explicitly `cgql`. Lua
`graph.query()` is read-only without auth and is lifted to CGQL mutation mode
only for callers with the write scope. With authentication disabled, Lua remains
read-only. Every query enforces the system-collection boundary.

```cgql
INSERT { title: "New" } INTO documents RETURN NEW
UPDATE "key" WITH { reviewed: true } IN documents RETURN { old: OLD, new: NEW }
REPLACE "key" WITH { title: "Rewritten" } IN documents
REMOVE "key" IN documents RETURN OLD
FOR d IN documents FILTER d.stale == true REMOVE d._key IN documents
```

- **UPDATE merges (partial update); REPLACE swaps the whole document.**
- `UPSERT search INSERT doc UPDATE merge IN collection`: the search object
  selects a candidate by `_key` when present, then checks every search field.
  Otherwise it selects the first document (key order) whose fields all equal
  the search fields. Both paths use numeric-aware equality; an absent field
  does not match an explicit null.
  Found → partial UPDATE merge (`OLD` = prior document); absent → the
  INSERT document is created as-is (`OLD` = null).
- Keys accept a plain `_key` or a full `collection/key` id.
- `NEW` and `OLD` are in scope only in a mutation's `RETURN`; `OLD` is null
  for INSERT, `NEW` is null for REMOVE.
- Mutation operands are additive expressions (parenthesize to use
  comparison operators inside them).
- Mutations cannot be combined with `COLLECT`; one mutation per query.
- **Read subqueries may supply mutation inputs through the body.** A mutation
  keeps one optional top-level `FOR`, followed by its `LET`/`FILTER` clauses.
  Its `FOR` source may be a read subquery, or a body `LET` may compute one for
  the mutation payload. See the checked [FOR-source](../../crates/cognigraph-query/tests/corpus/parse_ok/sq_mutation_for.cgql)
  and [LET-input](../../crates/cognigraph-query/tests/corpus/parse_ok/sq_mutation_let.cgql)
  forms. Inline subqueries in mutation selectors/payloads are not lifted;
  compute the value in the body and reference its variable instead. Subqueries
  contain only reads; no mutation can occur inside them. The dynamic-read
  restriction below still applies throughout the enclosing mutation.
- **Dynamic backend reads are rejected before execution (CG-7).** A mutation
  query cannot contain `DOCUMENT()` or a traversal start that references a row
  variable. The restriction covers its source, filters, LETs, nested read
  subqueries, selectors, payloads, UPSERT branches, and `OLD`/`NEW` return
  expressions, including unused branches and `EXPLAIN`. Validation returns
  `DOCUMENT() is not supported in mutation queries` or
  `correlated traversal starts are not supported in mutation queries` before
  materialization or any write. Direct `OLD`/`NEW` access, ordinary subqueries,
  and traversals starting from literals or bind variables remain supported.
  Read the required document separately and pass the value as a mutation bind
  variable when appropriate; separate queries do not guarantee a shared
  transactional snapshot. No committed mutation is replayed to resolve reads.
- **CGQL bulk-mutation atomicity is per document**: a FOR-driven mutation that
  fails midway leaves earlier operations applied. For explicitly atomic groups
  of heterogeneous writes, use `GraphBackend::execute_batch`, `POST /api/batch`,
  or `graph.batch`; the native backend commits the whole group in one redb
  transaction.

## v2: Multiple FOR, positional semantics, subqueries (2026-07-04)

Approved design: [CGQL v2](../decisions/decision_cgql_v2.md). The iteration
model follows AQL conventions; the CGQL rules below and its fixed corpus define
the implemented contract. No external database is needed to execute or test it.

### Positional body semantics (breaking change vs v1)

The body — `FOR` / `LET` / `FILTER` in any order — executes **in written
order**; each clause sees exactly the variables bound above it. A FILTER
referencing a later LET is a use-before-definition validation error (v1
evaluated all LETs first). A body with zero FORs runs on one synthetic row:
`RETURN 1` and `LET x = (…) RETURN x` are valid queries.

### Multiple FOR: joins and array iteration

There is no JOIN keyword. `FOR x IN <source>` nests loops, and the source
is anything that evaluates to an array:

```
FOR d IN documents                       // collection scan
  FILTER d.category == "research"        // applies BEFORE the next FOR
  FOR e IN relationships                 // nested loop (join)
    FILTER e._from == d._id              // join predicate
    FOR t IN d.tags                      // array un-nesting
      RETURN [d._key, e._key, t]
```

Bare identifiers resolve as in-scope variables first, then as collection
names; shadowing is rejected. `FOR x IN [1, 2, 3]` and `FOR x IN someVar`
work. Iterating `null` yields no rows (missing document fields); iterating
any other non-array is an error.

Execution: every collection-scan site is **materialized once** per query
with its own projection/filter pushdown; nested loops run in memory.
Correlated join predicates are evaluated in-engine (no hash joins yet —
EXPLAIN shows each source's strategy). LIMIT pushdown applies only to
single-FOR bodies.

### Subqueries (LET and expression positions)

```
FOR d IN documents
  LET related = (FOR e IN relationships FILTER e._from == d._id LIMIT 5 RETURN e._to)
  RETURN {doc: d._key, related}
```

The parenthesized body is the full read grammar (COLLECT/SORT/LIMIT/
DISTINCT included) and always evaluates to an array. Subqueries referencing
outer variables are correlated and run per row; uncorrelated ones run once.
Expression-position subqueries are lowered to this LET form as described
[above](#subqueries-in-expression-position); the remaining placement restrictions
are listed there. Subqueries contain no mutations and nesting depth is at most
4. `VECTOR_SEARCH` is restricted to the outermost first FOR. In read queries,
a traversal may appear inside a subquery and start from the enclosing row;
mutation queries retain the [CG-7 dynamic-read restrictions](#mutations).

Budgets: `max_source_rows` counts all materialized source rows — scans,
array expansions, and each correlated re-execution — against one shared
budget.

## EXPLAIN

Prefix any query — read or mutation — with `EXPLAIN` to get a static,
engine-independent description of the logical plan instead of executing it.
`EXPLAIN` never touches data, never resolves bind variables (values need not
be provided), and never executes a mutation, so it is safe on read-only
endpoints.

```
EXPLAIN FOR d IN documents FILTER d.score >= 2 AND d.category == @cat LIMIT 3 RETURN d.title
```

returns a single row:

```json
{
  "sources": [{
    "kind": "collection_scan",
    "collection": "documents",
    "var": "d",
    "depth": 0,
    "pushdown": {
      "projection": ["category", "score", "title"],
      "predicates": [".score >= 2.0", ".category == @cat"],
      "all_filters_pushed": true,
      "fetch_limit": 3
    }
  }],
  "pipeline": ["FOR d", "FILTER", "LIMIT 3", "RETURN"],
  "bind_vars": ["cat"]
}
```

`sources` lists every FOR site in execution (pre-)order — kinds:
`collection_scan`, `var_ref`, `expression`, `vector_search`, `traversal`;
`depth` > 0 marks subquery nesting. `pipeline` lists the clause stages in
written order, including `LET x = subquery (correlated, per row)` entries.
Both executors return the identical document. `EXPLAIN` and `ANALYZE` are
reserved words.

### EXPLAIN ANALYZE

`EXPLAIN ANALYZE <query>` EXECUTES the query — bind variables are required
and budgets apply — and returns a statistics report instead of rows:

```json
{
  "analyze": true,
  "sources": [{ "kind": "collection_scan", "...": "...", "rows": 1250, "fetches": 1, "fetch_ms": 0.4 }],
  "stages": [
    {"stage": "FOR d", "depth": 0, "rows": 1250, "runs": 1, "attempted_rows": 1250, "attempted_runs": 1},
    {"stage": "FILTER", "depth": 0, "rows": 1250, "runs": 1, "attempted_rows": 1250, "attempted_runs": 1},
    {"stage": "LET related = subquery (correlated, per row)", "depth": 0, "rows": 1250, "runs": 1, "attempted_rows": 1250, "attempted_runs": 1},
    {"stage": "FOR e", "depth": 1, "rows": 12500, "runs": 1250, "attempted_rows": 12500, "attempted_runs": 1250},
    {"stage": "RETURN", "depth": 0, "rows": 10, "runs": 1, "attempted_rows": 10, "attempted_runs": 1}
  ],
  "stats": {"total_ms": 2.1, "source_rows": 1260, "result_rows": 10, "document_fetches": 0, "document_fetch_ms": 0.0}
}
```

Normal backend execution and analysis use the same `DOCUMENT()` and correlated
traversal resolver, including dependent LETs, nested plans, and deferred
projections. `stats.result_rows` counts only the settled result. Stage `rows`
and `runs` exclude speculative resolution passes; correlated subqueries still
aggregate across their per-row executions. Each stage also reports
`attempted_rows` and `attempted_runs`, including all discarded passes.

Materialized sources report actual `rows`, `fetches`, and `fetch_ms`; a
correlated traversal aggregates fetches for its distinct start values.
`stats.document_fetches` counts distinct backend lookup attempts, including
absent documents, and `stats.document_fetch_ms` measures their combined wall
time. `stats.source_rows` is the cumulative budget-unit count defined above.
These work counts may differ from the in-memory engine, which resolves
documents immediately; every `*_ms` field is wall time, not an engine-parity
metric. Static sources remain materialized once across resolution retries.

Mutations are REJECTED under EXPLAIN ANALYZE — running
a write behind an analysis prefix is a footgun this language refuses
(unlike Postgres).

### Pushdown (collection scans)

The executor delegates work to the backend when semantics allow:

- **Projection pushdown** — when the plan only touches `var.attr` paths,
  only those top-level fields (plus `_key`/`_id`) are cloned. Any
  whole-document use (`RETURN d`, bare `INTO`) disables it.
- **Filter pushdown** — `AND`-conjuncts of the form
  `var.path <op> literal-or-@bind` with `==  !=  <  <=  >  >=  IN` are
  applied by the backend during the scan, so non-matching documents are
  never materialized. `IN` takes a fully-literal array or a bind variable
  (membership by CGQL value equality; the flipped form — `literal IN
  var.path` — is array-contains on a document field and stays in-engine). Semantics are identical to in-engine evaluation (numbers
  compare by value, strings lexicographically, mixed types never order,
  missing paths read null); the engine re-applies filters after the fetch
  as a correctness belt. Anything else (OR, functions, LET references)
  stays in-engine as a residual filter.
- **LIMIT pushdown** — without `SORT` or `COLLECT`, and with all filters
  either absent or fully pushed, only the first `offset + count` matching
  rows are fetched.
- **Vector threshold pushdown** — a `var._score >= x` (or `> x`) conjunct
  on a `VECTOR_SEARCH` source becomes the backend search threshold
  (top-K-above-threshold equals filtering top-K). `model_name` is
  deliberately NOT pushed from filters: scoping the search to a model
  changes results from "top-K then filter" to "top-K within model" — that
  needs explicit syntax, deferred.
- **Traversal confidence pushdown** — an `edge.confidence >= x` conjunct
  becomes `min_confidence` ONLY for exact depth `1..1` traversals: at
  deeper ranges, expansion-time pruning also removes paths THROUGH weak
  edges, which a post-`FILTER` does not.

Execution budgets count **materialized** source rows, so a pushed filter
also shrinks budget pressure.

## Explicitly Unsupported

CGQL v2 (2026-07-04) added multiple `FOR` (joins, array iteration), LET subqueries,
and positional body semantics. The approved July 21 amendment added
[expression-position subqueries](#subqueries-in-expression-position) through
desugaring. See the [decision and current contract](../decisions/decision_cgql_v2.md).
Still unsupported:

- JOIN/OUTER keywords (nested FOR + FILTER is the join syntax)
- inline subqueries in `SORT`/`RETURN` after `COLLECT` in the same query
- inline subqueries in a `COLLECT … INTO` projection or mutation selector/payload
  (precompute with a body `LET` instead)
- mutations inside subqueries; dynamic `DOCUMENT()` or row-dependent traversal
  reads anywhere in a mutation query (CG-7)
- mutations driven by more than one FOR
- cost-based join reordering / hash joins
- graph named views
- user-defined functions
- string interpolation
- block comments
- quoted identifiers
- wildcard projections
- backend-specific storage hints
