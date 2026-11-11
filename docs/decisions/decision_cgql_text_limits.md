# Decision: CGQL text is bounded before parsing

Status: accepted and implemented 2026-11-11 for [CG-105](../issues/CG-105.md).

## Context

The CGQL parser is recursive descent, and the AST builder, subquery hoisting,
validation, planning, execution and `Drop` all recurse over the expression
tree. Nothing bounded query length or nesting, so a 200 KB query of nested
parentheses on the read-only route overflowed a worker stack. A stack
overflow is not a panic: the process aborted and every tenant went down.

The dependency refresh of CG-192 brought a parser release that checks native
stack headroom and fails with "stack limit reached". That protects the
grammar only, depends on how much stack the calling thread happens to have,
and does nothing for the later passes or for long operator chains, which the
grammar matches in a loop and the builder folds into a deep tree.

## Decision

1. **A lexical pre-scan** runs at the top of `parse_query`. It is linear,
   keeps one counter per open bracket and never recurses. It skips string
   literals and comments.
2. **Three limits**, exported as constants and stated in the specification:
   262,144 bytes of text; nesting 32 (open brackets, pending conditionals,
   consecutive prefix operators); depth 256 (nesting plus chained binary
   operators at each open bracket).
3. **Width is not depth.** A comma or a top-level clause keyword ends the
   chains counted at that level.
4. **Overestimating is acceptable, underestimating is not.** A `-` counts as
   both prefix and binary; `=` counts as a binary operator.
5. **Sized from measurement.** On a 2 MiB thread, the server's worker stack:

   | Construct | Release build | Debug build |
   |---|---|---|
   | Nested arrays | 156 | 44 |
   | Nested parentheses | 183 | 49 |
   | Prefix `NOT` | 919 | 331 |
   | Conditional chain | 919 | 216 |
   | `+` chain | 6,258 | 217 |

   Measured on aarch64 macOS with rustc 1.99.1, as the deepest query that
   parses and executes. Nesting 32 leaves about a fourfold margin in release
   for the costliest construct. The worst accepted query (full nesting with a
   full chain inside) runs on a 1 MiB thread in a release build, which a
   release-only test asserts.
6. **Rejected: the parser's cumulative call limit.** It bounds total work,
   not depth, and is a process-wide setting.
7. **Rejected: relying on the parser's stack guard alone.** See Context.
8. **Rejected for now: running queries on a dedicated large stack or growing
   the stack on demand.** It would lift the limits but adds a thread hop or a
   dependency to every query; the limits are far above what written or
   generated queries use.

## Consequences

- Breaking: a query nested deeper than 32 or with a chain longer than about
  250 operators is refused. The error names the limit and, for chains,
  suggests a bind variable with `IN`.
- Debug builds use several times more stack per level. A debug server can
  still overflow on the worst accepted query; the tests run accepted queries
  on an explicit 8 MiB thread for that reason.
- The figures are for one platform. The margin is meant to absorb other
  targets; re-measure if the evaluator's frames grow or the worker stack size
  changes.
- Values supplied through bind variables and stored documents are not covered
  here. [CG-107](../issues/CG-107.md) bounds document nesting.
