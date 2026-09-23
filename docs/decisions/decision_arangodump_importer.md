# Decision: the dump importer stages verbatim into a paged store and publishes by hard link

Status: accepted and implemented 2026-09-23 for [CG-66](../issues/CG-66.md),
under the [dump import contract](decision_arangodump_import_contract.md).

## Context

The contract fixed what an import must accept, refuse and report. Building it
exposed three things the contract did not settle: the ordinary Native write
path stamps documents (it overwrites `updated_at`, adds `created_at`, and gives
edges default `relation_type` and `confidence`), snapshot import writes one
redb transaction per document, and a streaming importer cannot reproduce a
reader that checks unique constraints only after reading a whole collection.

## Decision

1. **Verbatim bulk insert in Native.** `NativeBackend::bulk_insert(collection,
   docs)` stores documents exactly as given, deriving only `_id`. All
   documents of a call are checked under the write lock (shape, existing and
   repeated keys, unique constraints against the store and earlier documents
   in the call) and then persisted in one transaction; a refusal writes
   nothing. It is an inherent Native method for offline tools, not part of the
   `GraphBackend` contract or any HTTP route.
2. **Paged staging, bounded chunks.** The importer stages into a paged,
   sidecar-vector store (key sets plus a 64 MiB document cache in memory) and
   writes chunks of at most 1,000 documents or 8 MiB. The dry run stages the
   same way in the system temporary directory, so it detects every problem an
   import would, including unique violations and dangling edges.
3. **First problem in file order.** Within a collection the first problem in
   file order is reported. Before a record problem is reported, the pending
   chunk (earlier lines) is flushed; a refused chunk is replayed document by
   document to name the first offender. The reference reader was changed to
   check unique constraints inline so both implementations agree exactly.
4. **Publication.** Re-hash every source file and re-list the directory, close
   and sync the staging store, then hard-link it to the destination (which
   fails instead of replacing a store that appeared meanwhile), sync the
   directory and remove the staging directory. A killed process leaves its
   staging directory but never a destination; the importer never deletes
   another run's staging.
5. **Exit codes and reports.** 0/2/3/4/5 as in the contract, with a `failed`
   status and `source_changed` code for exit 4. A report that cannot be
   written after publication is exit 5, and the message says the store is
   complete; a retry then refuses the existing destination.
6. **CLI placement and dependencies.** The importer is a module tree in the
   CLI crate, which now depends on `cognigraph-core`, `cognigraph-native`,
   `tokio` (current-thread runtime for the offline run) and `flate2` for gzip.
   The workspace stays at twelve crates.

## Consequences

- An imported document reads back identical to its source, minus `_rev`,
  through every storage mode. Documents written later through the API are
  stamped as usual.
- The CLI binary grows by the storage engine; it no longer needs a server for
  this command.
- Revisit for renaming on import, import into an existing store, parallel
  collection loading, or a lower staging cache on small machines.
