# Hosted QA is an on-demand environment

- Date: 2026-09-13
- Status: Active
- Related: [CG-76](../issues/CG-76.md), [Railway guide](../operations/railway.md)

## Decision

The shared Railway database is a disposable engineering QA environment. It is
neither a public demo nor a central production database for other applications.
Applications deploy and operate their own CogniGraph instances with their own
data, credentials, lifecycle and recovery requirements.

Use local disposable Native instances for routine tests and CI. Run hosted QA
only when the owner explicitly requests a Railway test session. A build, CI run,
push, release or agent assessment that hosted testing would help does not
authorize starting or testing Railway. An authorized session may cover deployment,
networking, persistent volumes, restart behavior or platform recovery. Describe
each result by the version and environment actually tested.

The owner chose to stop the current database between hosted test sessions,
preserve its existing volume for now, and remove the unfinished permanent
Access/tunnel setup. Remove automatic source deployment triggers from the QA
database so ordinary code pushes cannot restart it. The independently deployed
website keeps its own lifecycle.

Local test sessions end with [Docker cleanup](../operations/docker-cleanup.md).
Only explicitly identified reusable Evidence data volumes may be retained from
those tests; temporary containers and images are removed even when their data
volume is retained. This does not change the separate decision to preserve the
current Railway volume.

## Access and data

Use synthetic test data and retain application authentication during hosted
tests. A temporary public address still requires deliberate access and shutdown
controls; its obscurity is not authorization. A permanent private ingress service
is not required by this QA model. If future work needs sustained private remote
access, qualify that separately before exposing application data.

Stopping a process is not deleting its database. Preserve the attached volume
until a separate reset or deletion is authorized. Retained volumes and backups
may still incur storage charges. Verify the service is stopped after the session;
the historical deployment status alone is insufficient.

## Evidence ownership

Keep exact environment identifiers, addresses, inventories and operational
receipts in the private evidence repository. Public guides use examples and
publish bounded acceptance summaries. Earlier successful deployment and restore
records remain historical evidence; they do not imply an always-running service.
