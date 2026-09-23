# Decision: ArangoDB dump import is an offline, fixture-qualified, fail-closed contract

Status: accepted 2026-09-23 for [CG-64](../issues/CG-64.md); implementation is
[CG-66](../issues/CG-66.md). Supersedes the unqualified
[deferred design](../plans/arangodump-import-design.md) where they differ.

## Context

ArangoDB users evaluating CogniGraph had a query migration guide but no way
to bring their data. The earlier design was written before any dump had been
captured and before CogniGraph had unique constraints. Generating real dumps
from ArangoDB 3.11.14 and 3.12.11 on 2026-09-23 showed layouts the design did
not anticipate: 3.12 has no envelope option, split dumps omit empty
collections' data files, VPack uses its own file suffix, encryption makes
even `dump.json` unreadable, and a structure-only dump cannot be told apart
from a dump of empty collections.

## Decision

1. **Scope (owner choice).** ArangoDB 3.11 and 3.12 single-database JSON
   dumps: plain, gzip and 3.12 split files, with and without the 3.11
   envelope. VPack, encrypted and multi-database dumps are rejected with an
   actionable code.
2. **Unique indexes carry over (owner choice).** Unique persistent, hash and
   skiplist indexes on plain field paths of document collections become
   enforced unique constraints from CG-86, checked against the data before
   publication. Everything else ArangoDB can express (non-unique and special
   index types, edge-collection unique indexes, schemas, computed values, key
   generators, views, named graphs) is listed in the report and never
   silently presented as enforced.
3. **Fail closed, never rename.** Names that are not CGQL identifiers, CGQL
   reserved words and server-owned collection names are rejected, not
   remapped. The reserved lists are read from the server sources so the
   contract follows the server.
4. **Structure-only is imported, not rejected.** Because 3.11 writes empty
   data files for it and 3.12.11 ignores `--dump-data false`, the importer
   creates empty collections and emits a `no_documents` warning. Rejecting it
   would also reject genuine dumps of empty databases.
5. **The reference reader is the contract's executable form.** A small
   Python reader and 25 fixtures (15 from real exporter runs, 10 recorded
   corruptions of them) define outcomes, error codes, documents and
   dispositions; CI runs it. The Rust importer is qualified by matching it,
   not by reading this record.
6. **Placement (owner choice).** The importer is a self-contained module
   tree in the CLI crate that writes a Native store directly; the workspace
   stays at twelve crates.
7. **Publication.** Stage beside the destination, verify, re-hash the source,
   close durably and rename to a path that did not exist. Rejection or
   failure leaves the destination absent.
8. **Fixtures from the Enterprise 3.12 image.** No 3.12 community image is
   published. Dumps of our own synthetic data from the unlicensed evaluation
   mode are fixtures, not redistributed ArangoDB software.

## Consequences

- CG-66 can start from frozen error codes, a report schema and passing
  fixtures instead of open questions.
- A real private dump is used only as sealed private evidence; public
  fixtures stay synthetic.
- Revisit for explicit renaming (`--rename SRC=DST`), VPack decoding, a
  newer 3.12 point release or 3.13, and importing into an existing store.
