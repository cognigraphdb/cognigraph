# Railway Community deployment

> Public reading copy: environment-specific addresses and identifiers are omitted.
> The original is preserved in the private evidence catalog (artifact `f2d5b4e7a1220fd78833`).
> Dated acceptance describes that run; the shared service now follows on-demand QA.

The shared database follows the [on-demand QA decision](../decisions/decision_hosted_qa_lifecycle.md).
It stays stopped between hosted tests, with its existing volume preserved for now.
Applications deploy their own instances; this service is not a shared production
backend or public demo. The independently deployed website has its own lifecycle.
Historical [deployment acceptance](../issues/railway-community-2026-09-12.md) and
[v2.7.11 qualification](docker-hub/release-2.7.11.md) retain their dated scope.

## Hosted QA sessions

Use local disposable stores and CI for ordinary verification. Test Railway only
when the user explicitly requests a hosted test session. Routine builds, pushes,
releases and CI do not authorize a session, even if a change affects deployment.
Keep the database's automatic GitHub deployment trigger absent, so a
code push cannot start an idle QA service. The platform environment name does
not determine whether a service is production or QA.

Before starting a session, select the exact service and a qualified source/image
revision, inspect the retained volume and take an appropriate private backup.
Use Railway's manual deployment of that explicit revision. Redeploying an existing
deployment reuses its source; deploying latest source is a different operation.
Verify effective settings and source identity before exercising authenticated,
synthetic tests. Do not silently replace the retained store with an empty one.

After the checks, remove owned probes, record the results privately and stop only
the QA database, for example with `railway down --service <database-service-id>
--environment <environment-id> --project <project-id> --yes` (one command).
Check that it has no active deployment and that the public endpoint no longer
serves the application. Confirm the volume remains attached and the website's
active deployment is unchanged. Retained storage/backups may still incur charges.
Volume deletion and destructive recovery require their own explicit scope.

## Service configuration

Create a separate `database` service in the chosen QA environment. Use the verified
`cognigraphdb/cognigraph` main revision and the root Dockerfile, whose default
edition is Community. On Railway, leave OCI build labels at `dev`/`unknown` and
identify the release through `/health` plus Railway's Git commit/deployment
metadata. Fixed version/revision variables would become stale on later manual
deployments. The image bundles the frozen console at
`/ui`; Rust serves the UI and API at one origin. No Bun process runs in production.

[railway-settings.json](../../deploy/railway-settings.json) is the reviewed
`ServiceInstanceUpdateInput` payload for the Railway API, not an automatically
loaded config file. Apply it only to the database service and read settings back.
Use the explicit `/usr/local/bin/cognigraph-entrypoint` start command. Since
v2.7.34 that path is an alias of `/usr/local/bin/cognigraph-server`, which is
also the image entrypoint, so existing service settings keep working. In the
executed API setup, `null` did not clear a previous start-command or healthcheck
override; confirm the effective values before deploying. A redeploy uses the
previous deployment's code/configuration; create a fresh source deployment when
applying revised service settings.
Railway's former `railway.json` format is deprecated and cannot be enabled for
new services. Project-wide IaC is outside this service-scoped setup.

Use one replica, no serverless sleep, no overlapping writers, a 30-second
SIGTERM grace period and `/health/database` as the startup check. Volume-bound
deployments have downtime; this installation has no HA or failover. For a temporary authenticated HTTP test, expose only HTTPS targeting container
port 3000; do not create a public raw TCP proxy. Keep the source deployment
trigger absent and start the qualified revision manually. A future permanent
installation must separately qualify its access boundary and CI-gated lifecycle.

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

Railway supplies `RAILWAY_VOLUME_MOUNT_PATH=/data`. Started as root, the server
itself (the image has no shell since v2.7.34) creates only `/data/native`, owned
by UID/GID 10001, mode 0700, before it starts any thread. It rejects a symlink,
a wrong Native path or unexpected existing ownership/mode, and never
recursively repairs data. It then drops to UID/GID 10001 with no supplementary
groups, no capabilities in any set and privilege escalation disabled, and
verifies the result before serving. Normal Docker/Helm use stays non-root and
does not require this root-startup path.

Set secrets through Railway's protected variables interface or stdin to the
CLI. Never include values in source, command arguments, screenshots or logs.
Read the initial Admin credential in Railway to log in; create separate operator
accounts through the existing authenticated user-management flow. Changing the
bootstrap password variable does not rotate a user already stored in Native.
Keep actual operator identities and credential retrieval details in the private
runbook. Do not put passwords or authenticated snapshots in either repository.

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
