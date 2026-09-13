# CG-2: Collection names can escape the Native text-index directory

- Status: Resolved
- Priority: P1
- Area: Native storage / collection validation
- Found: 2026-09-08
- Reviewed revision: `bc4bff1` (workspace 2.6.0)
- Verification: Live HTTP and filesystem reproduction

## Problem

Document creation accepts collection names containing path separators. Native BM25 index paths embed that name directly in a filename, and index rebuild recursively removes the resulting directory before recreating it. A document writer can therefore cause derivative files to be created or rebuilt outside the configured database directory. The affected directory still has the generated hash and `.tantivy` suffix; this finding does not claim arbitrary filename deletion.

## Evidence

- `crates/cognigraph-server/src/routes/documents.rs:80-100` — forwards the collection unchanged.
- `crates/cognigraph-native/src/memory/search.rs:190-226` and `memory/paged.rs:96-121` — construct paths from raw collection names.
- `crates/cognigraph-native/src/text_index.rs:70-82` — removes an existing index directory during rebuild.

## Reproduction / failure sequence

POST `/api/documents` with collection `probe/../../escaped`, key `a`, and content `uniqueprobe`; then POST `/api/search/text` for that collection, query `uniqueprobe`, fields `["content"]`. Both returned HTTP 200. With the database under the disposable `http-basic` directory, the index appeared one level outside it as `escaped.420c75b526b35282.tantivy`.

## Acceptance criteria

- [x] Treat collection identifiers as opaque data at a shared derivative boundary reached through CRUD, batches, Lua, queries, embeddings, and import.
- [x] Encode or hash collection identity when naming derivatives and prove resolved paths remain inside the intended store directory.
- [x] Cover resident and paged text search and vector sidecar paths with separator-bearing-name regression cases.

## Resolution — 2026-09-08

`crates/cognigraph-native/src/derivative.rs` supplies safe hashed filenames to
both Tantivy and vector sidecars. This implements the isolation requirement at
the filesystem boundary instead of imposing a new collection-name grammar;
existing names remain usable. Resident/paged integration tests and real release
server HTTP checks verified search and restart behavior with `probe/../../escaped`
and confirmed all derivative files remained inside the store directory. Legacy
derivatives are left untouched and rebuilt under the new names on demand.
All Rust gates passed; see [batch verification](fixes-2026-09-08.md).

Run the repository Rust gates and a focused runtime regression before closing this issue. See [review evidence](review-2026-09-08.md).
