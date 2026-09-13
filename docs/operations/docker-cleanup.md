# Local Docker test cleanup

Every CogniGraph test session ends with cleanup, including after a failed check.
The scope is CogniGraph test resources. Preserve running services, other
projects' resources and host bind-mounted data. Do not use machine-wide container,
image, volume or builder pruning for this project-scoped task.

## Resource ownership

Record the containers, image tags and IDs, networks and volumes created or pulled
for the session. Give manually created resources a `cognigraph-qa-` name or a
project/session label. Capture the pre-existing inventory before testing when
shared fixed image tags are required. A resource's age, anonymous name or unused
status alone does not establish CogniGraph ownership.

## Session completion

1. Save useful logs and reviewed results to the private evidence repository.
2. Remove the session's test containers and temporary networks. Identify mounted
   volumes first; container removal must not delete retained Evidence data.
3. Remove disposable test volumes. The only retention exception is explicitly
   identified reusable Evidence data that avoids a later reimport. Record its
   volume name, dataset identity and reason for retention privately; use a clear
   name such as `cognigraph-evidence-<dataset>` for new retained volumes. Keep the
   volume without retaining a test container or image.
4. Remove temporary build/check tags and images pulled only for the tests after
   their final required consumer finishes. Verify their current IDs against the
   session inventory, check for other consumers and use scoped removal without
   force. The Docker suite uses `cognigraph:ci` and `cognigraph:ci-enterprise`;
   it currently leaves those images for the caller to clean up after testing or
   an explicitly authorized publication step.
5. Read back Docker's inventory and report any owned resources that could not be
   removed. Preserve resources whose ownership or data-retention status is unclear
   until identified; do not classify an unknown volume as Evidence automatically.

The existing container and Helm probes remove their owned disposable stores in
cleanup handlers. This procedure also covers manual probes and leftover images.
Build cache is separate from runnable images and data volumes: reclaim cache
through a dedicated test builder when one was used. A shared builder cache has
no reliable project boundary and requires separate scope before broad pruning.

[Railway QA](railway.md#hosted-qa-sessions) has its own explicitly requested
lifecycle and volume-retention decision; local testing does not start it.
