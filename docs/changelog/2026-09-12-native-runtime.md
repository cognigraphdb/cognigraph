# Use Native storage as the only runtime backend

- Date: 2026-09-12
- Status: Unreleased
- Kind: Runtime and configuration

## Change

[CG-67](../issues/CG-67.md) removes the Arango adapter, workspace member and
dependency paths from both editions. The server opens Native directly, with
the existing memory, resident and paged/sidecar modes. Docker, Compose and Helm
use the same direct startup path. The edition dependency check now also guards
against restoring an Arango adapter or driver.

Remove the backend selector, Arango connection settings and its vector-search
setting. Hybrid search uses Native BM25 and no longer has a `search_view`
parameter or an ArangoSearch branch. Promotion history uses typed filtered scans.
HTTP and Lua queries remain parsed CGQL with read/write authorization, budgets,
tenant scope and collection guards. Remove opaque-query passthrough from the Lua
embedding API and replace the guarded AQL text filter with unconditional denial
for a backend that does not declare parsed CGQL support.

These are owner-authorized breaking changes before first delivery. There is no
compatibility shim or last-Arango release. The Lua `with_backend_permissions`
and `register_graph_bindings_with_permissions` constructors are removed;
`with_backend_control` takes the write permission and execution control without
an opaque-query permission flag. `QueryLanguage::Aql` is removed. Useful generic
capabilities, wrappers and failure doubles remain.

Update the runtime configuration, architecture, authentication, query and Lua
guides and their examples. Preserve the external AQL-to-CGQL comparison and move
the adapter's historical vector fixture, byte-for-byte, into issue evidence.
The [runtime verification report](../issues/native-runtime-2026-09-12.md) records
the coverage and retained history. Full Native-only packaging/readiness remains
CG-68; optional dump import remains deferred. Versioning/publication follows the
next authorized push.

## Validation

Formatting, strict Clippy and full Rust suites passed for both editions: 722/913
reported passing tests, with two ignored provider tests and two early-returning
live-loop cases per suite. Release binaries passed 514 HTTP/Lua assertions across
four Native modes and cross-edition CLI/snapshot/tenant probes. All 161 UI tests
and nine real-browser journeys passed, as did script, edition, modularity, Helm
rendering and documentation checks. The linked report preserves source/capture
hashes, detailed results and exclusions. Docker images and live Helm backups
remain part of CG-68's final readiness gate.
