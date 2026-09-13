# Railway Community deployment

The first database target is one Community instance in the existing CogniGraph
Railway project, alongside the independently deployed website. The owner
authorized source deployment with the console and deferred Docker Hub publishing.
[CG-71](../issues/CG-71.md) owns live acceptance; this guide alone is not evidence
that a deployment or restore succeeded. The [2026-09-12 acceptance record](../issues/railway-community-2026-09-12.md)
qualifies v2.7.7 at [the hosted console](https://database-production-fe77.up.railway.app).

## Service configuration

Create a separate `database` service in production, EU West. Use the verified
`cognigraphdb/cognigraph` main revision and the root Dockerfile, whose default
edition is Community. On Railway, leave OCI build labels at `dev`/`unknown` and
identify the release through `/health` plus Railway's Git commit/deployment
metadata. Fixed version/revision variables would become stale on automatic
main deployments. The image bundles the frozen console at
`/ui`; Rust serves the UI and API at one origin. No Bun process runs in production.

[railway-settings.json](../../deploy/railway-settings.json) is the reviewed
`ServiceInstanceUpdateInput` payload for the Railway API, not an automatically
loaded config file. Apply it only to the database service and read settings back.
Use the explicit `/usr/local/bin/cognigraph-entrypoint` start command. In the
executed API setup, `null` did not clear a previous start-command or healthcheck
override; confirm the effective values before deploying. A redeploy uses the
previous deployment's code/configuration; create a fresh source deployment when
applying revised service settings.
Railway's former `railway.json` format is deprecated and cannot be enabled for
new services. Project-wide IaC is outside this service-scoped setup.

Use one replica, no serverless sleep, no overlapping writers, a 30-second
SIGTERM grace period and `/health/database` as the startup check. Volume-bound
deployments have downtime; this installation has no HA or failover. Expose only
Railway's HTTPS domain, targeting container port 3000. Do not create a public
raw TCP proxy. Enable the source trigger's **Wait for CI** setting. CI-required
protected-main changes are the source gate; disable
unreviewed branch/PR deployments for this persistent instance.

Attach a persistent volume at `/data` before startup. The Dockerfile deliberately
has no `VOLUME` declaration because Railway rejects that instruction. Ordinary
Docker runs must also attach `/data` explicitly, as in the root README. Set:

| Variable | Value |
|---|---|
| `PORT` | `3000`, Railway's healthcheck target |
| `COGNIGRAPH_PORT` | `3000`, the Rust listener |
| `RAILWAY_RUN_UID` | `0` for volume provisioning only |
| `COGNIGRAPH_NATIVE_PATH` | `/data/native/cognigraph.redb` |
| `COGNIGRAPH_AUTH_ENABLED` | `true` |
| `COGNIGRAPH_EMBEDDING_PROVIDER` | `none` |
| `COGNIGRAPH_CGQL_MAX_SOURCE_ROWS` | `100000` |
| `COGNIGRAPH_CGQL_TIME_BUDGET_MS` | `5000` |
| `COGNIGRAPH_CGQL_MUTATIONS_ENABLED` | `false` |
| `COGNIGRAPH_ADMIN_PASSWORD` | Unique generated secret, retained in Railway |
| `COGNIGRAPH_JWT_SECRET` | Independent generated secret, retained in Railway |

Railway supplies `RAILWAY_VOLUME_MOUNT_PATH=/data`. The entrypoint creates only
`/data/native`, owned by UID/GID 10001, mode 0700. It rejects a symlink, a wrong
Native path or unexpected existing ownership/mode. It never recursively repairs
data. The server then becomes PID 1 as UID 10001, with no effective capabilities
and privilege escalation disabled. Normal Docker/Helm use stays non-root and
does not require this root-startup path.

Set secrets through Railway's protected variables interface or stdin to the
CLI. Never include values in source, command arguments, screenshots or logs.
Read the initial Admin credential in Railway to log in; create separate operator
accounts through the existing authenticated user-management flow. Changing the
bootstrap password variable does not rotate a user already stored in Native.
For this installation, the bootstrap username is `admin`; retrieve its password
from CogniGraph → production → database → Variables → `COGNIGRAPH_ADMIN_PASSWORD`.
Do not put the password or an authenticated snapshot in this repository.

## Verification and recovery

Run `python3 scripts/verify.py --suite ci` and the Docker suite before publishing.
The Docker suite includes the root-owned-volume/asset regression. For the
packaged browser journeys, install Chromium as in [UI testing](ui-testing.md)
and run `python3 scripts/check-container-startup.py --browser`; this creates
and removes its own loopback containers and volumes in both editions.

For live acceptance, record the deployed commit and deployment ID, edition,
single replica, mount and process identity. Check HTTPS readiness, anonymous
API denial, login, console assets/deep links, a synthetic CRUD/CGQL round trip,
and persistence after a real service restart. Clean only the synthetic records
owned by the check. A local test does not qualify the hosted environment.

Enable daily and weekly Railway volume backups and confirm the schedule by API
readback. Railway currently retains these for six and 27 days respectively.
Also export an authenticated application snapshot before planned changes and
test an exact restore into a fresh Native store, including the
[temporary bootstrap-account cleanup](recovery.md#exact-authenticated-restore).
Do not copy a live redb
file with ordinary file tools; see [recovery](recovery.md).

Railway volume snapshots are a separate platform mechanism. A successful hot
JSON import drill does not qualify a platform snapshot restore. For a first
platform drill, use only owned synthetic data, record a manual backup, perform
the restore, verify the readback and retain the original unmounted volume until
the result is accepted. Never restore over a live user workload as routine QA.
The executed procedure was:

1. Stop the writer and verify its instance is `EXITED`. Railway can retain
   `SUCCESS` on the deployment record while the service is stopped; that status
   alone does not prove a running process or a stopped writer.
2. Create a manual volume backup and confirm it in the backup catalog. Snapshot
   size and incremental storage size are distinct; a nullable `usedMB` field is
   not by itself a failed backup. Restart and verify application readback.
3. For an isolated drill, add a post-backup marker, stop the writer again, and
   restore the selected backup. Railway creates a new volume and stages the
   mount replacement. Inspect the `READY` volume and exact staged patch before
   committing it; unrelated service changes must not be included.
4. Commit the mount change with deployment enabled. `skipDeploys: true` cannot
   apply a volume-mount replacement. Verify readiness, exact snapshot equality,
   and absence of the post-backup marker on the replacement volume.
5. Confirm copied backups and daily/weekly schedules on the active volume.
   Remove only owned probes, take a clean baseline backup, and verify a restart.
   Retire the original unmounted volume only after accepting the restore.

The account could execute backup/restore operations but could not read
`workflowStatus`. Catalog readback, staged-volume inspection and actual restored
data supplied the acceptance evidence; a returned workflow ID alone did not.

Backup schedules do not establish an independently verified restore, external
off-site copy, point-in-time recovery, or a guaranteed recovery time.

Official references checked 2026-09-12: [volumes](https://docs.railway.com/volumes),
[backups](https://docs.railway.com/volumes/backups),
[healthchecks](https://docs.railway.com/deployments/healthchecks),
[start commands](https://docs.railway.com/deployments/start-command),
[deployment actions](https://docs.railway.com/deployments/deployment-actions),
[configuration migration](https://docs.railway.com/infrastructure-as-code).
