# cognigraph-query

Parser, validator, planner, and executors for CogniGraph Query Language
(CGQL).

This crate owns the CGQL language pipeline used by the native backend, server,
and Lua runtime: parsing, validation, planning, read execution, mutations,
EXPLAIN/EXPLAIN ANALYZE, and backend pushdowns. The file-driven corpus also
runs the in-memory executor and native backend executor against the same
expected results.

The language contract is documented in
[`docs/cgql-v1.md`](../../docs/reference/cgql.md).

## Query Examples

Collection query:

```cgql
FOR d IN documents
FILTER d.category == @category
SORT d.created_at DESC
LIMIT 20
RETURN d
```

Traversal query:

```cgql
FOR v, e, p IN 1..3 OUTBOUND @start relationships
FILTER e.confidence >= @min_confidence
RETURN { vertex: v, edge: e, path: p }
```

Vector source:

```cgql
FOR d IN VECTOR_SEARCH(documents, @embedding)
LIMIT 10
RETURN { document: d, score: d._score }
```
