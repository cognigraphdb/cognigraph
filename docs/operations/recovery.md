# Recovery

## Backend backup and restore

Two complementary mechanisms:

1. **Hot JSON snapshot (authenticated Admin; auth must be enabled), works on a
   live server:**

   ```sh
   curl -H "Authorization: Bearer $TOKEN" :3000/api/admin/export > snapshot.json
   curl -H "Authorization: Bearer $TOKEN" -X POST :3000/api/admin/import \
        -H 'content-type: application/json' --data-binary @snapshot.json
   ```

   This is the Native application snapshot surface. Import is additive: it creates collections and overwrites matching keys,
   and clears the query cache. It does not delete documents absent from the
   snapshot — restore into a fresh Native instance for an exact copy. The snapshot is a
   trusted-root operator input and has no built-in signature verification;
   authenticate/sign the backup in the surrounding storage workflow.

2. **Cold file copy:** stop the server, copy `cognigraph.redb`, start. The
   redb file is the single source of truth. Do NOT copy it while the server
   runs (writes are transactional but the file may be mid-commit).

Derivative files next to the redb file — `*.sidecar` (int8 vectors) and
`*.tantivy/` (text indexes) — are rebuildable caches, safe to exclude from
backups and safe to delete; they regenerate on demand (first query pays the
rebuild).

Neither Native mechanism includes the M21-M24 external artifact CAS; pair it
with the matching M24 bundle and separately retained configuration, trust
anchor, and secrets. ArangoDB does not implement this CogniGraph application
snapshot surface: use tested `arangodump`/`arangorestore` or a platform-native
database backup, then pair that database recovery with the same CAS bundle and
external material.
