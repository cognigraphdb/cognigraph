# Helm chart for single-node deployment and read-replica design note

- Date: 2026-09-10
- Status: Unreleased
- Kind: Packaging

## Change

Add `deploy/helm/cognigraph`, a chart that deploys one `cognigraph-server`
as a StatefulSet with a persistent volume, ClusterIP and headless Services,
an optional auth Secret (or `auth.existingSecret`), and optional Ingress,
ServiceMonitor, NetworkPolicy and a backup CronJob. The CronJob takes hot
JSON snapshots through `POST /api/auth/login` and `GET /api/admin/export`
onto a second PVC with day-based retention, matching the
[recovery guide](../operations/recovery.md). Probes use `GET /health`
(liveness, startup) and `GET /health/database` (readiness). The pod runs
non-root with a read-only root filesystem and all capabilities dropped.

The chart refuses to render `replicaCount` other than 1, `storage.mode=paged`
without `storage.vectorMode=sidecar`, `auth.enabled=true` without
credentials, and `backup.enabled=true` without auth. `enterprise.*` values
(multi-tenant `COGNIGRAPH_DATA_DIR`, governance root public key, read-only
artifact CAS mount) render but are marked as Enterprise Components in
[LICENSING.md](../../LICENSING.md).

Add the [read-replica design note](../architecture/design-notes/read-replicas.md):
a log-shipping follower built on the existing single `Store::apply`
commit point and `data_generation` counter, with a `replication_log` table,
a scoped `GET /api/replication/log` endpoint, snapshot bootstrap, write-route
refusal on replicas, and an opt-in read-your-writes fence. Proposed, not
implemented; manual promotion only. Sharding and automatic failover remain
Enterprise scope.

## Validation

`helm lint` passes with required auth values (one INFO: icon recommended).
`python3 scripts/check-helm.py --live` passes eight render variants, seven
invalid-value rejection cases and a real HTTP backup using the rendered
configuration in isolated Docker containers. It asserts service/controller
selector separation and exports a seeded synthetic document with a password
containing JSON special characters. [CG-46](../issues/CG-46.md) records these
pre-publication fixes and failure-path tests. No cluster deployment or restore
drill was performed; the local server image is `cognigraph:ci` at version 2.6.1.
Deployment requires making the chosen image available to the cluster.
