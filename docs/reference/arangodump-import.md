# ArangoDB dump import contract

- Status: contract frozen 2026-09-23 ([CG-64](../issues/CG-64.md)); implemented in
  v2.7.31 by `cognigraph import --from-arangodump` ([CG-66](../issues/CG-66.md)).
- Decisions: [dump import contract](../decisions/decision_arangodump_import_contract.md),
  [importer](../decisions/decision_arangodump_importer.md)
- Executable form: [`scripts/arangodump_reader.py`](../../scripts/arangodump_reader.py),
  qualified against the [fixtures](../../fixtures/arangodump/README.md) by
  `python3 scripts/check-arangodump-fixtures.py`

This page states what an offline import from an `arangodump` directory into a
new Native store must do. Where this page and the reference reader disagree,
the reader and its fixtures win and this page is corrected.

## Supported input

| Input | Disposition |
|---|---|
| ArangoDB 3.11 and 3.12 single-database dumps, JSON lines | Supported |
| Plain `.data.json` and gzip `.data.json.gz` data files | Supported |
| 3.12 `--split-files` (`<name>_<md5>.<n>.data.json[.gz]`) | Supported; parts of one collection are read in part order as one collection |
| 3.11 `--envelope true` (`{"type":2300,"data":{…}}` per line) | Supported; any other marker type is `unsupported_marker` |
| `--include-system-collections true` | Supported; every `_`-prefixed collection is excluded and listed |
| `--dump-data false` | Imported as empty collections with the `no_documents` warning (3.11 writes empty data files, indistinguishable from empty collections; 3.12.11 ignores the option) |
| `--dump-vpack true` (`.data.vpack[.gz]` or `useVPack: true`) | `vpack_unsupported` |
| `--encryption.keyfile` (`ENCRYPTION` not `none`) | `encrypted_unsupported`; decrypt with ArangoDB first |
| `--all-databases true` (one subdirectory per database) | `multiple_databases`; point the importer at one database directory |

The qualified exporters are `arangodump` 3.11.14 (community image) and
3.12.11 (Enterprise image; no 3.12 community image is published). Other point
releases are expected to match but are not claimed until fixtures cover them.

## Command

```
cognigraph import --from-arangodump DUMP_DIR --output STORE [--dry-run] [--report REPORT.json]
                  [--max-record-bytes N] [--max-expanded-bytes N] [--max-files N]
```

- Offline: no server, `--url` or `--token`. Available in both editions.
- `STORE` is the Native redb path a server later opens as `COGNIGRAPH_NATIVE_PATH`.
  It must not exist. The importer never merges into or overwrites a store.
- `--dry-run` reads and validates everything and writes only the report. It
  creates nothing at or beside `STORE`, stages in the system temporary
  directory (removed afterwards) and opens no network connection. `STORE` must
  still be an absent path in an existing directory.
- The existing `cognigraph import SNAPSHOT.json` (HTTP snapshot restore) is
  unchanged; the two forms cannot be combined.

| Exit | Meaning |
|---|---|
| 0 | Accepted (dry run) or published |
| 2 | Usage error, including an existing `STORE` |
| 3 | Rejected by this contract; nothing published |
| 4 | I/O, staging or publication failure, or the dump changed during the run (`source_changed`); nothing published |
| 5 | Published, but the report could not be written; the store is complete |

## Limits

| Limit | Default | Error |
|---|---|---|
| One decompressed line | 16 MiB | `record_too_large` |
| All decompressed data | 64 GiB | `dump_too_large` |
| Files in the directory | 100 000 | `dump_too_large` |

Data files are streamed line by line. The importer never holds a whole
collection or dump as one JSON value.

## Layout rules

The directory must contain `dump.json` (`missing_dump_metadata`, or
`corrupt_dump_metadata` if unparseable) and may contain `ENCRYPTION`, structure
files, data files and `<name>.view.json` files. Anything else is refused:

- a symbolic link: `unsafe_path`;
- a subdirectory: `multiple_databases` if it holds `dump.json`, else `unknown_layout`;
- any other file, or a data file without a structure file: `unknown_layout`;
- files starting with `.` (for example `.DS_Store`) are not read and are listed as ignored.

Every non-system collection needs a data file, except in a split dump, where
ArangoDB writes none for an empty collection (`missing_data_file`).

## Mapping

- **Collections.** Structure `type` 2 is a document collection, 3 an edge
  collection; both are created, including empty ones. System collections
  (`_` prefix or `isSystem`) are excluded and listed.
- **Names.** A collection name must be a CGQL identifier
  (`[A-Za-z][A-Za-z0-9_]*`, so `order-lines` and extended Unicode names are
  `incompatible_collection_name`) and must not be a CGQL reserved word or a
  collection the server owns (`count`, `facts`, `entities`, `side_views`…:
  `reserved_collection_name`). The reader takes both lists from the server
  sources. V1 does not rename.
- **Documents.** `_key` must be a non-empty string (`invalid_document`) and
  unique per collection (`duplicate_key`). `_id`, when present, must equal
  `<collection>/<_key>` (`identity_mismatch`); the stored `_id` is that same
  value. `_rev` is dropped and counted. Every other field keeps its exact JSON
  value, including integers above 2^53, `1e+300`, empty objects, arrays and
  strings, and non-ASCII text. Imported documents are stored verbatim: no
  `created_at`/`updated_at` stamps and no edge `relation_type`/`confidence`
  defaults. Later writes through the API follow the ordinary stamping and
  edge-upsert rules.
- **Order.** Within a collection the first problem in file order is reported
  and the collection stops there; other collections are still checked.
- **Edges.** `_from` and `_to` must both be `<collection>/<key>` strings
  (`invalid_edge`) and resolve to an imported document (`unresolved_edge`).
  Edges into a collection that itself failed are not reported again.
- **Unique indexes.** A unique `persistent`, `hash` or `skiplist` index on
  plain field paths of a document collection becomes an enforced CogniGraph
  unique constraint with the same fields and `sparse` flag
  ([unique constraints](../decisions/decision_unique_indexes.md)); the data is
  checked against it first (`unique_violation`). A constraint keeps the
  ArangoDB index name. Unique indexes on edge collections are reported
  `edge_unique_unsupported`.
- **Everything else is reported, never enforced:** non-unique indexes
  (`non_unique_index`), TTL, geo, fulltext, inverted, multi-dimensional and
  array-expansion (`[*]`) indexes (`unsupported_index_type`), collection
  schemas, computed values and non-traditional key generators, views and
  analyzers (`unsupported_metadata`). Named graph definitions live in the
  excluded `_graphs` collection; their edges import normally.
- **Out of scope.** Users, permissions, Foxx services, queues, jobs and AQL
  user functions are not migrated. Create CogniGraph credentials separately.

## Report

`--report` writes `cognigraph-arangodump-report-v1` JSON, also on rejection
and failure; the same document is printed to stdout. It holds `importer`
version, `mode` (`dry_run` or `import`), `status` (`accepted`, `rejected`,
`failed`, `published`), `destination` once published, source `database`, every read
file with bytes and SHA-256, per-collection type and counts, carried
`constraints`, `not_carried` items with collection, kind, detail and reason,
`excluded` system collections, `ignored` files, `warnings`, `dropped._rev`, and
`errors` with code and, where known, collection, key, file and line.
Collections report their type and document count. The report never contains
document bodies or credentials. The reference reader returns the same
structure with the documents themselves in place of counts.

## Publication

The importer builds a paged staging store in `.STORE.cg-import-<uuid>/`
beside `STORE`, declares each collection's carried constraints, streams its
documents in bounded transactions, resolves every edge against the staged
store, re-hashes every source file and re-lists the directory against the
first read, closes the store, syncs it and hard-links it to `STORE`, which
fails rather than replacing a store that appeared meanwhile. A rejection, a
changed source or any I/O error removes this invocation's staging directory
and leaves `STORE` absent. A process killed mid-run leaves its staging
directory behind but never a `STORE`; delete the leftover by hand, the
importer never removes another run's staging.

## Qualification

`python3 scripts/check-arangodump-fixtures.py` runs in the shared CI suite. It
checks every fixture byte against the manifest, the committed expected results
against the [dataset definitions](../../scripts/arangodump_dataset.py), and
the reference reader against each fixture's recorded outcome. The Rust
importer is held to the same outcomes and additionally to the exact stored
documents and declared constraints by its fixture tests, and
[`arangodump-import-http.py`](../verification/harnesses/arangodump-import-http.py)
reads imported stores back through a real server in every persistent storage
mode, before and after a restart.
