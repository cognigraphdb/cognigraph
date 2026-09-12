# Decision: shared per-edge traversal confidence

> Current storage scope (2026-09-12): [Native-only storage](decision_native_only.md)
> supersedes this record's runtime-backend choices and adapter-specific paths.
> Both editions now use Native; public HTTP/Lua queries are parsed CGQL.
> Storage-independent contracts below remain applicable. Earlier backend
> behavior, configuration and verification are retained as dated history,
> not current setup instructions. Use the [operator guides](../operations/README.md).

Status: accepted and implemented 2026-09-09 for CG-19.

## Contract

`GraphBackend::traverse` applies `TraversalOpts.min_confidence` inclusively to
every edge in a candidate path. An edge below the threshold excludes its path
and all extensions, even when that edge occurs below the requested `min_depth`.
Without a threshold, confidence does not restrict traversal.

Missing, null, and nonnumeric confidence have effective confidence 1.0, matching
Native traversal and both backends' existing score calculation. Numeric zero
and negative values retain their numeric meaning. A depth-zero path has one
vertex, no edges, and score 1.0; the confidence threshold cannot exclude it.

The path score remains the product of effective edge confidences, multiplied
by `path_decay` once per hop. Neither the product nor the decayed score is
compared to `min_confidence`: two edges with confidence 0.6 meet a 0.5 threshold,
even though their combined score is lower. These rules apply to outbound,
inbound, and any-direction traversal through the Rust API, HTTP traversal,
and Lua `graph.traverse`.

## Implementation and compatibility

Arango uses `PRUNE rejected = ...` to stop expansion, followed by
`FILTER !rejected` to exclude the failing endpoint. The expression guards
`e != null` and uses `IS_NUMBER` to select the effective confidence. Arango
evaluates pruning below the minimum depth, including the null edge at depth
zero; pruning alone still permits the stopping endpoint in returned results.
See the [official traversal documentation](https://docs.arango.ai/arangodb/3.12/aql/graph-queries/traversals/#pruning).

Native already implements these rules in resident and paged traversal. Its
algorithm and scoring remain unchanged. Arango callers can now receive fewer
paths that crossed a failing intermediate edge, and more paths that contain
unannotated/default-confidence edges or include depth zero. No data migration
is needed.

This is the typed traversal contract. Backend query language selection and
CGQL expression filters retain their existing semantics; an expression on a
traversal's final edge is not automatically a per-path confidence constraint.

## Verification

The shared Rust fixture contains 15 cases with explicit expected paths and
scores. Both backend conformance suites and the release HTTP/Lua harness use
that fixture. The pre-fix implementation fails against disposable ArangoDB
3.12.11, while the corrected focused Arango suite passes all six tests.
Formatting, strict Clippy, all 923 workspace tests, the release build, and 240
HTTP/Lua checks across Arango and all persistent Native configurations passed,
including four CogniGraph server restarts. The corrected release eliminated all
48 Arango mismatches reproduced across two lifetimes of the saved release.
The disposable Arango container and its volumes were removed afterward.
Final release and workspace validation are recorded in the
[CG-19 remediation report](../issues/traversal-confidence-2026-09-09.md).
