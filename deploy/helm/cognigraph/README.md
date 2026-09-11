# cognigraph Helm chart

Deploys one CogniGraph server (Native backend) as a StatefulSet with a
persistent volume. This is the Community tier: single node, single tenant,
under the [Functional Source License](../../../LICENSE). The default image
excludes Enterprise Components. `replicaCount`
other than 1 is refused at render time; read replicas and clustering are
described in the [read-replica design note](../../../docs/architecture/design-notes/read-replicas.md).

## Install

```sh
helm install cognigraph ./deploy/helm/cognigraph \
  --namespace cognigraph --create-namespace \
  --set auth.adminPassword="$(openssl rand -hex 16)" \
  --set auth.hostAdminPassword="$(openssl rand -hex 16)" \
  --set auth.jwtSecret="$(openssl rand -hex 32)" \
  --set persistence.size=50Gi
```

Or reference a Secret you manage:

```sh
kubectl -n cognigraph create secret generic cognigraph-auth \
  --from-literal=admin-password=... \
  --from-literal=host-admin-password=... \
  --from-literal=jwt-secret=...
helm install cognigraph ./deploy/helm/cognigraph -n cognigraph \
  --set auth.existingSecret=cognigraph-auth
```

## What you get

| Object | Purpose |
|---|---|
| StatefulSet (1 replica) | `cognigraph-server`, non-root, read-only root FS, `/data` on the PVC |
| Service + headless Service | ClusterIP on 3000; stable DNS for the pod |
| Secret (optional) | `admin-password`, `host-admin-password`, `jwt-secret` |
| CronJob + PVC (optional) | Hot JSON snapshots via `GET /api/admin/export`, with retention |
| Ingress, ServiceMonitor, NetworkPolicy (optional) | Off by default |

Probes: liveness `GET /health`, readiness `GET /health/database` (503 until the
store is open), startup probe allows five minutes for a large resident store.

## Sizing

`storage.mode=resident` keeps everything in RAM — fastest, and the default.
For datasets larger than memory set `storage.mode=paged` and
`storage.vectorMode=sidecar`, then size `storage.cacheBytes` and
`resources.limits.memory` together. The volume holds `cognigraph.redb` plus
rebuildable `*.sidecar` and `*.tantivy/` derivatives; budget roughly 2× the
raw JSON size.

## Upgrades

`helm upgrade` performs a rolling restart of the single pod; expect the
readiness window to equal the store's load time. The data volume is retained
across upgrades and uninstalls (`helm uninstall` does not delete the PVC).

## Enterprise values

`enterprise.multiTenant`, `enterprise.governanceRootPublicKey` and
`enterprise.artifactCas` require `edition: enterprise`; Community values with
these settings fail rendering. Build the image with
`--build-arg COGNIGRAPH_EDITION=enterprise`. Production use requires the
[Enterprise License](../../../LICENSE-COMMERCIAL). See
[LICENSING.md](../../../LICENSING.md).

`edition` defaults to `community`. Without an explicit `image.tag`, the chart
selects `appVersion` for Community and `appVersion-enterprise` for Enterprise.
An explicit tag takes precedence: the operator must supply the matching build.
The chart selects an image; it does not compile or publish one. `/health` reports
the running edition. Both editions remain single-writer deployments.

## Values

See [values.yaml](values.yaml); every key is commented.

## Backup behavior and verification

The optional job uses `python:slim` and the packaged Python standard-library
client. It safely encodes credentials, downloads to a temporary file, parses the
complete snapshot and publishes it before pruning older snapshots. Set a positive
`backup.retentionDays`. Failed exports leave completed backups untouched. Snapshot
validation loads decoded JSON into memory: increase `backup.resources.limits.memory`
for large stores. External artifact CAS and rebuildable indexes are not included;
follow the [recovery guide](../../../docs/operations/recovery.md).

Server selectors exclude backup pods. Custom `podLabels` apply to the server pod
and cannot replace chart identity labels. Set `image.repository` and `image.tag`
to an image available to the cluster; this chart does not publish the local
`cognigraph:2.7.0` image.

From the repository root, with Helm and Bun installed:

```sh
python3 scripts/check-helm.py
python3 scripts/verify.py --suite docker
# Also requires Docker and access to the configured backup image:
python3 scripts/check-helm.py --live
python3 scripts/check-helm.py --live --enterprise
```

The live check starts isolated containers with the rendered environment and
security settings, seeds a synthetic document and runs the packaged backup.
It removes its containers, network and temporary files. This is Docker/HTTP
verification, not a Kubernetes cluster deployment or restore drill.
