# Preserve Native conformance coverage before adapter removal

- Date: 2026-09-12
- Status: Unreleased
- Kind: Verification

## Change

Resolve [CG-65](../issues/CG-65.md) by replacing four Arango-dependent capability-boundary
tests with a counted no-access backend double. Rejections must occur before
storage access, even when an attempted operation's error could be caught. This
also removes the construct crate's Arango dev-dependency.

The unchanged 126-case CGQL execution corpus now runs in each persistent Native
mode before and after reopen, alongside its existing memory run. Resident
storage with sidecar vectors gains the full shared contract suite. Preserve
targeted mutation, error, budget, cancellation, identity and authorization tests.
The [coverage disposition](../issues/native-conformance-2026-09-12.md) records
which adapter-specific tests can be deleted in CG-67 and the retained coverage.

No query fixture or historical measurement was regenerated. Arango runtime
removal remains CG-67; complete Native-only readiness remains CG-68. The optional
importer backlog is still deferred. This change does not bump or publish 2.7.1.

## Validation

Formatting and strict Clippy pass for both editions. Workspace tests report
780 passing tests for Community and 971 for Enterprise, with two provider tests
ignored in each suite. Each total includes eight Arango and two live-loop cases
that returned early; they are not executed service coverage.

Both freshly built release binaries pass 450 authenticated HTTP/Lua/query/vector
and restart checks each, with disposable Native stores and no provider calls.
Edition, server modularity, documentation, decision-index and issue-identity
checks pass. The [coverage report and captures](../issues/native-conformance-2026-09-12.md)
record commands, source/binary hashes, retained contracts and exclusions. CG-67
is next; final Native-only readiness remains CG-68.
