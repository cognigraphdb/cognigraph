# Railway Community deployment

The first database target is one Community instance in the existing CogniGraph
Railway project, alongside the independently deployed website. The owner
authorized source deployment with the console and deferred Docker Hub publishing.
[CG-71](../issues/CG-71.md) owns live acceptance; this guide alone is not evidence
that a deployment or restore succeeded.

## Service configuration

Create a separate `database` service in production, EU West. Use the verified
`cognigraphdb/cognigraph` main revision and the root Dockerfile, whose default
edition is Community. Build args may set the OCI version/revision labels; verify
the actual binary through `/health`. The image bundles the frozen console at
`/ui`; Rust serves the UI and API at one origin. No Bun process runs in production.

[railway-settings.json](../../deploy/railway-settings.json) is the reviewed
`ServiceInstanceUpdateInput` payload for the Railway API, not an automatically
loaded config file. Apply it only to the database service and read settings back.
Railway's former `railway.json` format is deprecated and cannot be enabled for
new services. Project-wide IaC is outside this service-scoped setup.

Use one replica, no serverless sleep, no overlapping writers, a 30-second
SIGTERM grace period and `/health/database` as the startup check. Volume-bound
deployments have downtime; this installation has no HA or failover. Expose only
Railway's HTTPS domain, targeting container port 3000. Do not create a public
raw TCP proxy. CI-required protected-main changes are the source gate; disable
unreviewed branch/PR deployments for this persistent instance.

Attach a persistent volume at `/data` before startup. Set:

| Variable | Value |
|---|---|
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
test an exact restore into a fresh empty Native store. Do not copy a live redb
file with ordinary file tools; see [recovery](recovery.md).

Railway volume snapshots are a separate platform mechanism. A successful hot
JSON import drill does not qualify a platform snapshot restore. For a first
platform drill, use only owned synthetic data, record a manual backup, perform
the restore, verify the readback and retain the original unmounted volume until
the result is accepted. Never restore over a live user workload as routine QA.
Backup schedules do not establish an independently verified restore, external
off-site copy, point-in-time recovery, or a guaranteed recovery time.

Official references checked 2026-09-12: [volumes](https://docs.railway.com/volumes),
[backups](https://docs.railway.com/volumes/backups),
[configuration migration](https://docs.railway.com/infrastructure-as-code).
