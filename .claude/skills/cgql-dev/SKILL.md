---
name: cgql-dev
description: Work on CogniGraph Query Language in cognigraph-query. Use when changing CGQL grammar, AST, validation, planning, execution, parser edge cases, or docs/reference/cgql.md.
argument-hint: "[optional-focus]"
user-invocable: true
allowed-tools: Bash Read Grep Glob Edit MultiEdit Write
---

# CGQL Development

Use this workflow for changes under `crates/cognigraph-query/` and CGQL-facing documentation.

## Scope

- Keep grammar, validation, planner and executor ownership in `cognigraph-query`.
  Update Native, server and Lua integration as required by the requested behavior.
- Keep `graph.query()` as the Lua query entry point. Public HTTP and Lua query
  text uses parsed CGQL within each surface's capabilities and authorization;
  a backend's AQL declaration does not enable public opaque query passthrough.
- Preserve AQL/XQuery-style keyword handling: keywords are case-insensitive; identifiers are case-sensitive.
- Multiple `FOR` clauses, joins and read subqueries are delivered capabilities.
  Preserve the tested restrictions in [the current specification](../../../docs/reference/cgql.md),
  including its mutation and dynamic-read boundaries. Update the specification,
  tests and implementation plan when extending the supported contract.

## Workflow

1. Read the relevant existing files before editing:
   - `crates/cognigraph-query/src/`
   - `crates/cognigraph-query/tests/`
   - `docs/reference/cgql.md`
   - `docs/implementation-plan.md`
2. For grammar changes, update parser tests first or alongside the grammar.
3. For semantic changes, update validation tests and planner/executor tests at the same time.
4. Keep error messages deterministic enough for tests and user-facing diagnostics.
5. Update `docs/reference/cgql.md` when syntax or behavior changes.
6. Update `docs/implementation-plan.md` when status, scope, or known limitations change.

## Validation

For Rust query changes, run at least the focused query crate tests:

```bash
cargo test -p cognigraph-query
```

For completed Rust changes, finish with the repository standard:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
```

For documentation-only changes, follow the documentation validation in
[AGENTS.md](../../../AGENTS.md#documentation-validation). Exercise changed runnable
query examples against the real implementation. Behavioral changes also require
the root instructions' live verification and accurate coverage reporting.
