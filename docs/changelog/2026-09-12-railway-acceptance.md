# First Railway Community deployment accepted

- Date: 2026-09-12
- Status: Unreleased
- Kind: Deployment evidence and operations

## Changes

[CG-71](../issues/CG-71.md) is resolved: v2.7.7 from protected main is live on
Railway with authentication, persistent Native storage and the packaged console.
Local CI/Docker and GitHub PR/main CI passed. Hosted API/browser, restart,
exact application restore and actual Railway volume restore checks passed;
owned probe data was removed and a clean baseline backed up. Daily/weekly
schedules and unprivileged process identity were read back from the live service.

The [acceptance report](../issues/railway-community-2026-09-12.md) retains source,
runtime, failed-attempt and recovery evidence. The Railway guide/settings record
the executed start command and explicit port alignment. Recovery guidance now
includes temporary bootstrap-account cleanup for exact authenticated import.
Current plan/registry/instructions reflect the first live database. Docker Hub
remains deferred; the existing website remains unchanged.

This is post-deployment documentation, prepared after the published v2.7.7
candidate. It is not itself a new product deployment or registry publication.
