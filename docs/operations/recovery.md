# Recovery

Native supports hot application snapshots and cold database copies. For first
deployment, follow the [setup and readiness guide](first-deployment.md).

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
   snapshot. An exact copy requires a fresh Native instance and removal of its
   newly generated bootstrap account as described below. The snapshot is a
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
anchor, and secrets. The CAS bundle and Native database backup must belong to
the same recovery plan.

## Exact authenticated restore

A fresh authenticated target creates its own Admin with a newly generated key.
Importing a snapshot containing the source Admin does not remove that target
account. A plain import therefore does not produce an exact copy even when the
target has no user collections. The CLI import delegates to authenticated HTTP;
there is no separate offline CLI restore path.

The following procedure was [executed for Community v2.7.7](../issues/railway-community-2026-09-12.md)
on a disposable target isolated from users and other writers:

1. Start a fresh authenticated Native instance with independently retained
   target bootstrap/JWT secrets. Verify it contains only the expected bootstrap
   Admin and no user data. Retain the source Admin credential separately from
   the snapshot; password hashes in a backup cannot recover that credential.
2. Create a temporary Admin with a unique username and generated password.
   Record the exact keys of this account and the target bootstrap Admin.
3. Log in as the temporary Admin, then delete only the target bootstrap Admin
   by its captured key. Do not delete source accounts or work against an existing
   workload. The temporary account preserves authenticated import access.
4. Import the complete source snapshot using the temporary Admin. Log in as the
   restored source Admin, then delete only the temporary account by its key.
5. Export again and compare the entire parsed snapshot with the source, including
   internal collections. Restart the target, authenticate again and repeat the
   comparison before accepting recovery or directing traffic to it.

Keep all snapshots private: they contain user password hashes and may contain
other sensitive records. Retain secrets/configuration separately and remove
temporary restore credentials and disposable targets after the drill. This
Community result does not qualify Enterprise tenant or external-CAS recovery.
