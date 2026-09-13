# 02 — CGQL for DataOps

CGQL is the audit tool for everything the other guides build. The full
language spec is [../cgql-v1.md](../reference/cgql.md); this page is the working
subset a data operator reaches for, with runnable examples. All queries
go through:

```sh
curl -s -X POST $COGNIGRAPH_URL/api/search/query -H "content-type: application/json" \
  -d '{"query": "...", "bind_vars": {"name": "value"}}'
# or:
cognigraph query --bind year=2026 'FOR d IN docs FILTER d.year == @year RETURN d.title'
```

`/api/search/query` is read-only. Mutations (`INSERT`/`UPDATE`/`REPLACE`/
`REMOVE`) require the separately mounted `POST /api/query` route
(`COGNIGRAPH_CGQL_MUTATIONS_ENABLED=true`) and write scope.

## Scan, filter, project

```cgql
FOR d IN docs
  FILTER d.status == "final" AND d.year >= 2025
  SORT d.year DESC, d.title
  LIMIT 20
  RETURN { title: d.title, year: d.year }
```

Numbers compare by value (`d.year == 2026` matches `2026.0`), missing
paths read as `null`, and mixed types never order — the engine is strict
so audits don't lie.

Membership beats OR-chains, and pushes down to the storage scan:

```cgql
FOR d IN docs
  FILTER d.status IN ["draft", "review"]
  RETURN d._key
```

## Counting and grouping (COLLECT)

Per-group counts:

```cgql
FOR d IN docs
  COLLECT status = d.status WITH COUNT INTO n
  RETURN { status: status, n: n }
```

Aggregates:

```cgql
FOR c IN chunks_raw
  COLLECT doc = c.doc_key
  AGGREGATE chars = SUM(LENGTH(c.text)), longest = MAX(LENGTH(c.text))
  RETURN { doc: doc, chars: chars, longest: longest }
```

Group members, when you need the rows themselves:

```cgql
FOR d IN docs
  COLLECT year = d.year INTO titles = d.title
  RETURN { year: year, titles: titles }
```

After `COLLECT`, only the collect bindings are in scope — the row
variable `d` is gone by design.

Deduplication:

```cgql
FOR c IN chunks_raw
  RETURN DISTINCT c.doc_key
```

## LET, joins, subqueries

Per-row bindings:

```cgql
FOR d IN docs
  LET age = 2026 - d.year
  FILTER age <= 1
  RETURN { title: d.title, age: age }
```

Joins are nested FOR + FILTER (no special syntax):

```cgql
FOR d IN docs
  FOR c IN chunks_raw
    FILTER c.doc_key == d._key AND d.status == "final"
    RETURN { doc: d.title, chunk: c._key }
```

Subqueries in LET — correlated ones run per row:

```cgql
FOR d IN docs
  LET chunk_count = (
    FOR c IN chunks_raw FILTER c.doc_key == d._key RETURN 1
  )
  RETURN { title: d.title, chunks: LENGTH(chunk_count) }
```

Un-nesting arrays:

```cgql
FOR d IN docs
  FOR tag IN d.tags
    RETURN DISTINCT tag
```

## Graph sources

Traversal as a query source:

```cgql
FOR v, e, p IN 1..3 OUTBOUND @start document_relations
  FILTER e.confidence >= 0.8
  RETURN { vertex: v._id, relation: e.relation_type }
```

(bind `start` to e.g. `"docs/a1"`; directions: `OUTBOUND`, `INBOUND`,
`ANY`). Vector search as a source:

```cgql
FOR d IN VECTOR_SEARCH(chunks_raw, @embedding)
  FILTER d._score >= 0.75
  LIMIT 10
  RETURN { key: d._key, score: d._score }
```

The `_score >=` filter is pushed into the backend search as its
threshold.

## EXPLAIN — check what the engine will actually do

Prefix any query with `EXPLAIN` for the static plan (sources, pushdowns,
pipeline) without executing:

```sh
cognigraph query 'EXPLAIN FOR d IN docs FILTER d.status == "final" LIMIT 5 RETURN d.title'
```

Look for the filter and limit attached to the *source* — that means the
scan itself is doing the work (projection/filter/limit pushdown) instead
of materializing the collection. `EXPLAIN ANALYZE` executes and adds
per-stage row counts and per-source fetch stats:

```sh
cognigraph query 'EXPLAIN ANALYZE FOR d IN docs FILTER d.year >= 2025 RETURN d._key'
```

Row counts are engine-comparable; timings are indicative only. Mutations
are rejected under ANALYZE.

## Budgets

Long scans on production data should run with budgets on
(`COGNIGRAPH_CGQL_MAX_SOURCE_ROWS`, `COGNIGRAPH_CGQL_TIME_BUDGET_MS` —
see the env table in [../operations.md](../operations/README.md)). Budget errors
are a signal to add a `FILTER` the engine can push down, not to raise the
cap first.

## DataOps queries worth keeping around

(Note the shape: a subquery is only valid as a `LET`'s whole value —
`LENGTH(FOR ...)` inline does not parse. Bind the subquery first, then
measure it.)

```cgql
// collections' health after a load: docs with no chunks
FOR d IN docs
  LET matches = (FOR c IN chunks_raw FILTER c.doc_key == d._key RETURN 1)
  FILTER LENGTH(matches) == 0
  RETURN d._key
```

```cgql
// chunks that never grounded a fact (after guide 04)
FOR c IN chunks
  LET used = (FOR f IN facts FILTER f.evidence_chunk_id == c.id RETURN 1)
  FILTER LENGTH(used) == 0
  RETURN c._key
```

```cgql
// fact edges per relation type
FOR f IN facts
  COLLECT rel = f.relation_type WITH COUNT INTO n
  SORT n DESC
  RETURN { relation: rel, n: n }
```

Next: [03 — Retrieval](03-retrieval.md).
