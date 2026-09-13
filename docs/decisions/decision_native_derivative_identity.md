# Decision: bind Native derivatives to database and commit identity

Status: accepted and implemented 2026-09-08 for CG-11.

## Contract

Numeric data generations can repeat across recreated databases and divergent
restores of the same backup. Persistent derivative validity therefore needs
both an immutable database identity and the exact committed data revision.

Native storage schema 2 adds an `identity` table containing `database_id` and
`data_revision`, each a UUID. Opening a schema-1 store initializes both and
upgrades the schema marker in one transaction, without rewriting documents
or changing their numeric data generation. Every data mutation assigns a new
revision UUID in the same transaction as its effects. Failed writes leave the
revision unchanged. Reopening an unchanged database preserves both identities.

Derivative filenames hash a structured identity containing the database UUID
and the collection/index specification. Tantivy warm-open metadata also binds
that identity and the revision UUID. Vector format `CGVEC2` includes the base
revision UUID alongside its existing generation, dimension, and row count.
Warm loading rejects older or mismatched metadata and rebuilds from redb.
In-memory vector deltas keep their existing ordered-publication behavior.

Tenant retirement quarantines the database, matching Tantivy directories,
vector files, and unfinished vector temporary files, including legacy names.
Unrelated files and already quarantined entries stay untouched. Quarantine
remains recoverable; this change does not delete retained data permanently.

## Migration and restore

This metadata-only schema-1-to-2 upgrade is an explicit exception to the
original JSON-only migration plan. It is atomic and preserves all user rows.
Older schema-1 binaries reject schema-2 databases instead of making writes
without updating the revision. Downgrade uses a pre-upgrade backup or a JSON
export imported into a separate database by the older binary.

Existing derivatives rebuild once under the new identity. Ordinary upgrades
leave old derivative files in place; tenant retirement also recognizes those
legacy files. Unchanged restarts can still reuse current derivatives. JSON
exports keep their existing public shape and omit physical database identity;
imports mutate the destination's own revision. A physical backup preserves
its database and revision IDs, while a subsequent divergent write creates a
different revision even when its numeric generation repeats.

The work does not pin in-flight HTTP requests to tenant incarnations; CG-12
tracks that separate boundary. Only synthetic temporary stores are used for
migration and restore verification.

## Verification

Formatting, Clippy, and all 863 Rust tests passed. Deterministic regressions
cover database replacement and divergent physical restores at a reused path,
schema-1 migration, failed writes, invalid identity metadata, and warm reuse.
Release HTTP lifecycles in resident/paged modes preserved schema-1 data during
upgrade, quarantined old and new derivatives, isolated recreated tenants, and
preserved identity on restart. The previous binary rejected schema 2 before
serving requests. See [the remediation report](../issues/derivative-isolation-2026-09-08.md).
