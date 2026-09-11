# Tenant derivative isolation — 2026-09-08

This batch addresses [CG-11](CG-11.md). Changes remain local and uncommitted;
unrelated shared work was preserved.

## Behavior

Native schema 2 adds an immutable database UUID and a unique commit-revision
UUID. The schema-1 upgrade initializes this metadata atomically without
rewriting user rows or advancing the numeric data generation. Every data
transaction changes the revision, and failed writes leave it unchanged.
Schema-2 stores with missing or malformed identity metadata refuse to open.

Text-index identities now include the database UUID, collection, and field
list; warm loading additionally requires an exact revision marker and schema.
Vector filenames include database UUID and collection, while the `CGVEC2`
header binds the exact base revision. Legacy or mismatched derivatives rebuild
from redb. Unchanged restarts retain identity and can reuse warm indexes.

The revision token closes an additional restore case within CG-11: two
branches of the same physical backup can have identical database UUIDs, keys,
and numeric generations, yet contain different data. Their revisions differ,
so neither branch's derivative is valid for the other.

Tenant retirement now quarantines Tantivy directories and unfinished vector
builds as well as the database and vector sidecars, including legacy names.
Tests check preservation of other tenants, unrelated files, already quarantined
entries, and the contents of moved index directories.

## Verification

- Before implementation, both database-replacement and divergent-restore
  regressions failed: the new vocabulary was absent because the old text index
  was reused.
- Focused Native tests passed, including resident/paged recreation and
  divergent physical restore, vector candidate correctness with 40 distractors,
  atomic metadata migration, failed-write revision preservation, corrupt
  identity rejection, warm restart reuse, and legacy vector-format rejection.
- Two focused retirement tests passed with actual text/vector indexes plus
  synthetic legacy and temporary entries.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo test --all`: passed, 863 tests reported passed, 0 failed, 0 ignored
  across 66 result summaries. The focused Native run passed 51 tests.
- `cargo build --release -p cognigraph-server`: passed.

Live verification used the previous and updated release binaries with
synthetic users and disposable multitenant stores over loopback HTTP. Both
resident and paged modes passed the following sequence:

1. Create a schema-1 tenant with one text-bearing document and 40 vector
   distractors; build its legacy text and vector derivatives.
2. Start the updated binary, verify the original document is unchanged, and
   retrieve its text/vector results after metadata upgrade.
3. Delete the tenant. The response reports five quarantined entries: the
   database, two text directories, and two vector files (legacy and new).
   Every reported entry exists; the old token returns 401.
4. Recreate the same tenant name. GET initially returns 404. Insert the same
   document key with new vocabulary and a different vector: the new term and
   vector each return one hit, and the old term returns zero. The physical
   database UUID differs from the retired tenant's UUID.
5. Restart and repeat those checks; the recreated database keeps its UUID.
6. Attempt to open the upgraded store with the previous binary. It exits
   before serving requests with `database schema version 2 is newer than
   supported 1`.

The [sanitized observations](evidence/derivative-isolation-http-2026-09-08.json)
record both modes. All live checks passed on their first run. During local
test development, the old persistence test still expected `GENERATION`
instead of `REVISION`, and the new server test assumed an unavailable UUID
dependency and `VectorSearchOpts::default()`. Those test errors were corrected
before the final gates; no dependencies were added.

## Compatibility and scope

**Opening a schema-1 store upgrades it to schema 2; older binaries refuse the
upgraded file.** Downgrade uses a pre-upgrade backup or JSON export/import into
a separate database with the older binary. Public JSON snapshots keep their
existing shape and omit physical identity. See the
[migration decision](../decisions/decision_native_derivative_identity.md).

Ordinary upgrades retain older derivative files, which may consume disk until
operator cleanup or tenant retirement. New derivatives rebuild once, with a
corresponding cold-start cost. Quarantine remains recoverable and permanent
deletion remains an operator action. Only temporary test databases were
migrated or retired during this work.

This does not bind in-flight requests to tenant incarnations. That remains
[CG-12](CG-12.md), the next priority. No live ArangoDB or external provider
coverage is claimed; environment-gated tests may return early.
