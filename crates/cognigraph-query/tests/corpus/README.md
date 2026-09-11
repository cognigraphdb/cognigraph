# CGQL test corpus

Reusable `.cgql` files exercised by `tests/corpus.rs` (in-memory executor)
and by `crates/cognigraph-native/tests/cgql_corpus.rs` (the same queries
through `NativeBackend::query()` — dual-engine equivalence).

- `parse_ok/` — must parse, validate, and plan
- `parse_err/` — must fail parsing
- `validate_err/` — must parse but fail semantic validation
- `exec/` — executed against `dataset.json`; `<name>.json` holds the exact
  expected result array. Queries must be deterministic (use SORT for
  multi-row results) and must not return bare traversal path variables
  (path JSON shape legitimately differs between engines).
- Bind variables are declared per file with a `// binds: {...}` header line
  (bind matching is strict; the runner passes exactly what is declared).

The `sq_*.cgql` cases support the
[CG-28 subquery-position contract](../../../../docs/reference/cgql.md#subqueries-in-expression-position):
fourteen execution examples, two mutation-body forms that parse/validate/plan,
three parse rejections, and three validation rejections. The mutation forms are
also executed by the [release harness](../../../../docs/issues/evidence/subquery-contract-http.py).
`exec/sq_nested_binding_collision.cgql` now requires the corrected result for
[CG-37](../../../../docs/issues/CG-37.md), with four further nested/sibling,
correlated, bind-variable, and dynamic-document examples. `--expect-collisions`
in the release harness reproduces the old behavior with a saved pre-fix binary;
default runs require the correct results. CG-38 response-mapping checks now
require HTTP 400 on both query routes; `--expect-plan-errors-500` reproduces
the old read-only status defect with a saved pre-fix binary. The earlier
`--expect-collisions` option also enables that legacy status expectation.
