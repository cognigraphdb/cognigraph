# Migrating from ArangoDB: AQL to CGQL

CGQL is designed to be familiar to AQL users: the same `FOR … FILTER … SORT …
LIMIT … RETURN` shape, the same `_key`/`_id`/`_from`/`_to` document
conventions, the same `@bind` variables, the same traversal syntax, and the
same names for most built-in functions. It is not AQL. This page lists, clause
by clause, what carries over unchanged, what needs a rewrite, and what has no
equivalent. Every "supported" row is backed by the [CGQL specification](cgql.md)
or the [query corpus](../../crates/cognigraph-query/tests/corpus/); every gap
names the workaround.

Scope: ArangoDB 3.11/3.12 AQL against CogniGraph 2.6 with the Native backend.
CogniGraph's ArangoDB backend (`cognigraph-arango`) exists for conformance
testing, not as a migration bridge.

## Ten things to know before you start

1. **Strings are double-quoted only.** `'single'` quotes and backtick
   identifiers are parse errors. Object keys in literals are bare identifiers
   (`{ title: d.title }`), not quoted strings.
2. **No `LIKE`, no `=~`, no `%`.** Use `CONTAINS`, `STARTS_WITH`, `REGEX_TEST`;
   there is no modulo operator.
3. **`LIMIT` takes literal integers**, not bind variables or expressions.
4. **No `@@collection` bind variables.** Collection names are identifiers in
   the query text.
5. **No bracket access.** `d.attr` and `d.a.b` work; `d["attr"]`, `d[@key]` and
   `arr[0]` do not. Use `FIRST`, `LAST`, `SLICE` for arrays.
6. **No array operators.** `arr[*]`, `arr[* FILTER …]`, `ALL IN`, `ANY IN`,
   `NONE IN`: rewrite as a subquery or `FOR` over the array.
7. **One `LIMIT`, one `SORT`, and `FILTER` may not follow them.** AQL lets you
   filter after a limit; CGQL rejects it. Use a subquery.
8. **Results come back whole.** There is no cursor API with `batchSize`; a
   query returns its full result set. Bound it with `LIMIT` and the
   server-side row/time budgets.
9. **No named graphs, no `PRUNE`, no `OPTIONS`.** Traversals name one edge
   collection and a depth range; that is the whole surface.
10. **Mutations are gated.** `POST /api/query` must have
    `COGNIGRAPH_CGQL_MUTATIONS_ENABLED=true` and, with auth, the
    `documents:write` scope. Read-only `POST /api/search/query` never mutates.

## Endpoints and request shape

| ArangoDB | CogniGraph | Notes |
|---|---|---|
| `POST /_api/cursor` `{query, bindVars}` | `POST /api/query` `{query, bind_vars}` (read-write) or `POST /api/search/query` (read-only) | `bind_vars`, not `bindVars`. No `batchSize`, `count`, `ttl` or cursor follow-up calls. |
| `PUT /_api/cursor/{id}` | none | Whole result returned; see item 8 above. |
| `POST /_api/explain` | prefix the query with `EXPLAIN` / `EXPLAIN ANALYZE` | Static plan without bind values; `ANALYZE` executes reads. |
| `/_api/document/{coll}` CRUD | `/api/documents`, `/api/documents/{coll}/{key}` | `PATCH` merges, `PUT` replaces, same as Arango. |
| `/_api/collection` | `GET/POST /api/collections`, `DELETE /api/collections/{name}` | Types are `document` / `edge`. |
| `/_api/gharial` (named graphs) | `/api/graph/relationships`, `/api/graph/traverse` | Edge collections, not named graphs. |
| `/_api/transaction` | `POST /api/batch` | Atomic multi-op write batch on Native; not a general transaction with reads. |
| ArangoSearch views, `SEARCH` clause | `/api/search/text` (BM25), `/api/search/hybrid` (BM25 + vector, RRF) | Full-text is an HTTP endpoint, not a CGQL clause. |
| `/_api/database` | tenants (Enterprise) or one server per database | The Community build runs one tenant. |
| `arangodump` / `arangorestore` | `GET /api/admin/export`, `POST /api/admin/import` (JSON snapshot); `cognigraph-cli` backup/restore | See [recovery](../operations/recovery.md). |
| `arangojs` | HTTP + `fetch`; a TypeScript SDK is planned | Bind variables and result arrays map directly. |

## Query structure

| AQL | CGQL | Status |
|---|---|---|
| `FOR d IN coll` | same | Supported |
| `FOR d IN coll FILTER … SORT … LIMIT … RETURN …` | same order; `FILTER`/`LET` may interleave before `COLLECT`/`SORT`/`LIMIT` | Supported |
| Multiple `FOR` (joins, nested loops) | same; no `JOIN` keyword in either | Supported (v2) |
| `FOR x IN d.tags` (array un-nesting) | same | Supported |
| `FOR x IN [1,2,3]`, `FOR x IN someVar` | same | Supported |
| `FOR i IN 1..10` (numeric range) | not supported; `..` is traversal-only | Use an array literal or a stored list |
| `LET x = expr` | same; positional — a `FILTER` cannot reference a later `LET` | Supported |
| `LET rows = (FOR … RETURN …)` (subquery) | same; subquery always yields an array, nesting ≤ 4 | Supported |
| Subquery in `FILTER`/`RETURN`/`SORT` expression | same, lifted into a `LET` automatically | Supported except in `SORT`/`RETURN` after `COLLECT`, `COLLECT … INTO x = expr`, and mutation operands |
| `RETURN 1` with no `FOR` | same | Supported |
| `RETURN DISTINCT expr` | same | Supported |
| `SORT a ASC, b DESC` | same | Supported |
| `SORT … ` locale-aware | `SORT d.name COLLATE "de"` | CogniGraph-only addition |
| `SORT RAND()` | none | Not supported |
| `LIMIT n`, `LIMIT offset, n` | same, literal integers only; `LIMIT 0` rejected | Supported |
| `LIMIT @n` | not supported | Interpolate client-side |
| `FILTER` after `SORT`/`LIMIT` | rejected | Wrap the sorted/limited part in a subquery, filter outside |
| `COLLECT k = expr` | same | Supported |
| `COLLECT … AGGREGATE s = SUM(x), m = MAX(x)` | same; `SUM`, `MIN`, `MAX`, `AVG` only | `COUNT`, `UNIQUE`, `SORTED_UNIQUE`, `LENGTH`, `COUNT_DISTINCT`, `VARIANCE`, `STDDEV` in `AGGREGATE`: collect `INTO g = x` and apply the function in `RETURN` |
| `COLLECT … WITH COUNT INTO n` | same, with or without a grouping key | Supported |
| `COLLECT … INTO groups` | same; bare `INTO` captures all in-scope variables, `INTO g = expr` a projection | Supported |
| `COLLECT … INTO g KEEP a, b` | not supported | Use `INTO g = { a, b }` |
| `COLLECT … OPTIONS { method: … }` | not supported | Ignore; one grouping method |
| `WINDOW` | none | Not supported |
| `SEARCH` (ArangoSearch) | none in CGQL | `/api/search/text` or `/api/search/hybrid`, then query by returned keys |
| `WITH coll1, coll2` (cluster locking hint) | not needed | Drop |
| `// comment` | same | Supported |
| `/* block */` | not supported | Use line comments |

## Expressions and operators

| AQL | CGQL | Status |
|---|---|---|
| `==` `!=` `<` `<=` `>` `>=` | same; non-associative (`a == b == c` is a parse error) | Supported |
| `IN` | same | Supported |
| `NOT IN` | `NOT (x IN arr)` | Rewrite |
| `AND` `OR` `NOT`, `&&` `\|\|` `!` | word forms only | Rewrite symbols to words |
| `+ - * /` | same | Supported |
| `%` | none | Not supported; compute client-side or store the remainder |
| `? :` ternary | same | Supported |
| `??` (null coalescing) | `COALESCE(a, b)` / `NOT_NULL(a, b)` | Rewrite |
| `LIKE "a%"` | `STARTS_WITH(s, "a")`, `CONTAINS(s, "x")`, or `REGEX_TEST(s, "^a")` | Rewrite |
| `=~` `!~` | `REGEX_TEST(s, pattern)` / `NOT REGEX_TEST(…)` | Rewrite |
| `ALL ==`, `ANY ==`, `NONE ==`, `ALL IN`, `ANY IN`, `NONE IN` | none | `LENGTH((FOR x IN arr FILTER x == v RETURN 1)) > 0` |
| `arr[*]`, `arr[*].attr`, `arr[* FILTER …]` | none | `(FOR x IN arr RETURN x.attr)` |
| `arr[**]` | `FLATTEN(arr, depth)` | Rewrite |
| `arr[0]`, `arr[-1]`, `arr[1..3]` | `FIRST(arr)`, `LAST(arr)`, `SLICE(arr, 1, 3)` | Rewrite |
| `doc["attr"]`, `doc[@key]` | none; only `doc.attr` paths | Dynamic attribute access is not expressible; project explicitly |
| `doc.`attr with spaces`` | none | Not supported |
| `{ "quoted key": 1 }` | `{ key: 1 }` identifiers only | Rewrite keys |
| `{ x }` shorthand | same | Supported |
| `'string'` | `"string"` | Rewrite |
| `1e6`, `0x1F`, `.5` | not supported; digits with optional fraction only | Rewrite |
| `1..5` range value | none outside traversal depth | Not supported |
| Null semantics: missing attribute → `null`, `null` is falsy | same | Supported |
| Type-mixed comparison (`1 < "a"`) | `false` for ordering ops; `==`/`!=` by type | Differs from AQL's total type order |
| Numeric equality `2 == 2.0` | true | Same |
| Unicode: `é` composed vs decomposed | compare unequal; use `NORMALIZE_NFC` | Differs: AQL normalizes some inputs |

## Traversals

| AQL | CGQL | Status |
|---|---|---|
| `FOR v, e, p IN 1..3 OUTBOUND @start edges` | same; `v` alone or `v, e` also legal | Supported |
| `INBOUND`, `ANY` | same | Supported |
| `0..n` (include start vertex) | same; `e` is `null`, path has zero edges | Supported |
| Start as bind variable or string literal | same | Supported |
| Start from the enclosing row (`OUTBOUND p._id`) | same; correlated, traversed once per distinct start | Supported (v2); see the cost note in the spec |
| Start from `DOCUMENT(…)` | not supported in the start position | Bind or `LET` it first |
| Several edge collections: `OUTBOUND s edges1, edges2` | one edge collection per `FOR` | Two traversals + `UNION` is also absent; run two `FOR`s |
| `OUTBOUND s GRAPH "name"` (named graph) | none | Name the edge collection |
| `PRUNE cond` | none | `FILTER` on `p`, or lower the depth |
| `OPTIONS { uniqueVertices, bfs, order, … }` | none | One traversal strategy |
| `SHORTEST_PATH`, `K_SHORTEST_PATHS`, `K_PATHS`, `ALL_SHORTEST_PATHS` | none | Bounded-depth traversal + client-side selection, or `/api/search/graph-augmented` |
| `p.vertices`, `p.edges` | path object with vertices and edges | Supported |
| Edge attributes `e.confidence` | same; Native traversals also carry a path score | Supported |

## Functions

Same name, same meaning (differences noted):

`LENGTH`, `UPPER`, `LOWER`, `TRIM`, `CONTAINS`, `STARTS_WITH`, `SUBSTRING`
(character-based), `CONCAT`, `SPLIT`, `ABS`, `FLOOR`, `CEIL`, `ROUND`
(half-away-from-zero), `MIN`, `MAX`, `SUM`, `AVG` (AQL also spells it
`AVERAGE`), `FIRST`, `LAST`, `UNIQUE`, `SORTED`, `SORTED_UNIQUE`, `SLICE`,
`FLATTEN`, `INTERSECTION`, `MINUS`, `TO_NUMBER` (non-numeric → `null`, not
`0`), `TO_STRING`, `TO_BOOL`, `REGEX_TEST`, `REGEX_REPLACE`, `NOT_NULL`,
`IS_SAME_COLLECTION`, `PARSE_IDENTIFIER`, `DOCUMENT`, `HAS`, `COSINE_SIMILARITY`,
`TYPENAME`, `DATE_YEAR`, `DATE_MONTH`, `DATE_DAY`, `DATE_HOUR`, `DATE_MINUTE`,
`DATE_SECOND`, `DATE_TIMESTAMP`, `DATE_DIFF`, `DATE_ADD`.

Function names are case-insensitive. Unknown functions and wrong arity fail
validation; wrong argument *types* yield `null` at run time rather than an
error, which matches AQL's lenient behaviour in most cases.

Rewrites:

| AQL | CGQL |
|---|---|
| `COUNT(x)` | `LENGTH(x)` |
| `COUNT_DISTINCT(x)`, `COUNT_UNIQUE(x)` | `LENGTH(UNIQUE(x))` |
| `LENGTH(collectionName)` (document count) | `LENGTH((FOR d IN coll RETURN 1))` or `COLLECT WITH COUNT INTO n` |
| `AVERAGE(x)` | `AVG(x)` |
| `CONCAT_SEPARATOR(sep, a, b)` | `CONCAT(a, sep, b)` |
| `LIKE(s, pattern)` | `REGEX_TEST` / `CONTAINS` / `STARTS_WITH` |
| `POSITION(arr, v)` | `v IN arr` (boolean only; no index) |
| `APPEND(a, b)`, `PUSH`, `UNSHIFT`, `POP`, `SHIFT`, `REMOVE_VALUE`, `REMOVE_NTH`, `UNION`, `UNION_DISTINCT`, `OUTERSECTION` | none — build the array with a subquery: `(FOR x IN a RETURN x)`; `UNION` of two sources is two `FOR`s or a `LET` per side and `FLATTEN([a, b])` |
| `MERGE(a, b)`, `MERGE_RECURSIVE`, `UNSET`, `KEEP`, `ATTRIBUTES`, `VALUES`, `ZIP`, `MATCHES` | none — project the object explicitly |
| `DATE_NOW()` (ms) | `DATE_TIMESTAMP(NOW())` |
| `DATE_ISO8601(x)` | `NOW()` is already RFC 3339; there is no formatter for arbitrary inputs |
| `DATE_FORMAT`, `DATE_TRUNC`, `DATE_ROUND`, `DATE_DAYOFWEEK`, `DATE_ISOWEEK`, `DATE_DAYS_IN_MONTH` | none |
| `DATE_DIFF(a, b, "d")` | `DATE_DIFF(a, b, "days")` — units are `days`, `hours`, `minutes`, `seconds` only; no `y`, `m`, `w`, `f` |
| `DATE_ADD(d, 1, "month")` | none — calendar units are absent by design; `days`/`hours`/`minutes`/`seconds` only |
| `RAND()`, `RANGE(a, b)`, `SHUFFLE` | none |
| `SQRT`, `POW`, `EXP`, `LOG`, `SIN`, … , `MEDIAN`, `PERCENTILE`, `VARIANCE`, `STDDEV` | none — aggregate client-side or in Lua |
| `SUBSTITUTE`, `REVERSE`, `LEFT`, `RIGHT`, `LTRIM`, `RTRIM`, `FIND_FIRST`, `CHAR_LENGTH`, `MD5`, `SHA1`, `UUID`, `TO_BASE64`, `JSON_PARSE`, `JSON_STRINGIFY` | none — `REGEX_REPLACE` covers substitution and trimming |
| `IS_NULL`, `IS_NUMBER`, `IS_STRING`, `IS_ARRAY`, `IS_OBJECT`, `IS_BOOL` | `x == null`, `TYPENAME(x) == "number"`, etc. |
| `NOOPT`, `V8`, `SLEEP`, `ASSERT`, `WARN`, `FAIL` | none |
| Geo functions (`GEO_DISTANCE`, `GEO_CONTAINS`, …) | none — store and filter bounding boxes explicitly |
| Fulltext functions (`FULLTEXT`, `TOKENS`, `NGRAM_*`, `BM25`, `TFIDF`) | none in CGQL — `/api/search/text` |
| User-defined functions (`aqlfunctions`) | none — Lua scripts via `POST /api/lua/execute` with `graph.query()` |

## Mutations

| AQL | CGQL | Status |
|---|---|---|
| `INSERT {…} INTO coll RETURN NEW` | same | Supported |
| `UPDATE key WITH {…} IN coll RETURN { OLD, NEW }` | same; key is a `_key` or full `_id`, may be an expression such as `d._key` | Supported |
| `UPDATE doc IN coll` (key inside the doc) | `UPDATE d._key WITH d IN coll` | Rewrite |
| `REPLACE key WITH {…} IN coll` | same | Supported |
| `REMOVE key IN coll RETURN OLD` | same | Supported |
| `UPSERT {search} INSERT {…} UPDATE {…} IN coll` | same; `_key` in the search object short-circuits, all search fields must match | Supported |
| `UPSERT … REPLACE {…}` | none | Use `UPDATE` branch, or `REMOVE` + `INSERT` in a batch |
| `FOR d IN coll FILTER … REMOVE d._key IN coll` (bulk) | same; one `FOR`, one mutation per query | Supported; atomicity is per document |
| Bulk mutation driven by two `FOR`s | not supported | Compute the set in a `LET` subquery first |
| `OPTIONS { ignoreErrors, waitForSync, overwrite, keepNull, mergeObjects, exclusive }` | none | `UPDATE` merges (like `mergeObjects: true`); nothing else is configurable |
| `DOCUMENT()` inside a mutation query | rejected (CG-7) | Read first, pass the value as a bind variable |
| Traversal from a row variable inside a mutation | rejected (CG-7) | Same workaround |
| Several mutations in one query, `LET x = (INSERT …)` | none; one mutation per query, none inside subqueries | `POST /api/batch` for an atomic group |
| Stream transactions | none | `POST /api/batch` |
| `_rev` / `If-Match` optimistic locking | none | Not supported |

## Search and vectors

| AQL / ArangoSearch | CogniGraph | Status |
|---|---|---|
| `FOR d IN view SEARCH ANALYZER(d.text IN TOKENS(@q, "text_en"), "text_en") SORT BM25(d) DESC` | `POST /api/search/text` `{query, collection, fields, limit}` | Endpoint, not a clause |
| Hybrid ranking | `POST /api/search/hybrid` (BM25 + vector, reciprocal rank fusion) | Endpoint |
| `APPROX_NEAR_COSINE(d.embedding, @v, 10)` (3.12 vector index) | `FOR d IN VECTOR_SEARCH(coll, @embedding) LIMIT 10 RETURN { d, score: d._score }` | Supported; `VECTOR_SEARCH` must be the outermost first `FOR` |
| `COSINE_SIMILARITY(a, b)` | same | Supported |
| Embedding at write time | `POST /api/documents/embed` | Server-side embedding with configured provider |
| Semantic search from text | `POST /api/search/semantic` | Embed → vector search → fetch |

## Worked rewrite

A typical lookup service on ArangoDB — a code table, a producer join, a grouped
count — and the same three queries in CGQL.

AQL:

```aql
FOR p IN plu
  FILTER p.code == @code
  RETURN p

FOR p IN plu
  FILTER LIKE(p.name, CONCAT("%", @q, "%"), true)
  FOR v IN 1..1 OUTBOUND p._id grown_by
    RETURN { code: p.code, name: p.name, grower: v.name }

FOR p IN plu
  COLLECT cat = p.category WITH COUNT INTO n
  SORT n DESC
  LIMIT @top
  RETURN { cat, n }
```

CGQL:

```cgql
FOR p IN plu
  FILTER p.code == @code
  RETURN p

FOR p IN plu
  FILTER CONTAINS(LOWER(p.name), LOWER(@q))
  FOR v IN 1..1 OUTBOUND p._id grown_by
    RETURN { code: p.code, name: p.name, grower: v.name }

FOR p IN plu
  COLLECT cat = p.category WITH COUNT INTO n
  SORT n DESC
  LIMIT 20
  RETURN { cat, n }
```

Three edits: `LIKE` with a case-insensitive flag became `CONTAINS(LOWER(…))`;
`LIMIT @top` became a literal; everything else is byte-identical. The join
variant `FOR e IN grown_by FILTER e._from == p._id` is also valid and is
measured faster than the correlated traversal at collection scale (see the
[benchmarks](../research/benchmarks/native-backend.md)).

## What has no equivalent and no workaround

- Cursor streaming and `batchSize`
- `@@collection` bind variables and dynamic attribute access
- Named graphs, `PRUNE`, traversal `OPTIONS`, shortest-path functions
- `WINDOW`, geo functions, ArangoSearch analyzers inside a query
- Multi-statement and stream transactions
- `_rev`-based optimistic concurrency
- User-defined AQL functions (Lua fills this role with a different model)
- Foxx microservices

If a query on this list is load-bearing for your application, it is a
reason to keep that path on ArangoDB or to redesign it around CogniGraph's
batch and Lua surfaces, not to expect a CGQL release to close it soon.
