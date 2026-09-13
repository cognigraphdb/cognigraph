# Verify private Helm backups with Linux ownership semantics

- Date: 2026-09-12
- Status: v2.7.5
- Kind: CI portability

## Change

The [CG-69](../issues/CG-69.md) GitHub run passed Rust, UI, Native release
acceptance and both packaged Docker runtimes, then exposed a host-side backup
inspection error. Linux correctly retained the backup container's UID 10001
and file mode `0600`; the CI host user could not read that snapshot.

The live Helm harness now uses a disposable Docker volume and reads the backup
as its container owner. It checks mode `0600`, exact snapshot contents, absence
of partial files and denial for UID 10002. The volume makes Docker Desktop and
native Linux exercise the same file ownership rules. A bounded provisioner
initializes only that temporary target; backup and inspection run non-root,
without capabilities, and the volume is removed afterward.

Production chart and backup behavior are unchanged. Deployment and image
publication remain on hold. This correction updates the existing CI PR.

## Verification

The backup client regression passes with its private-file assertion. Live
Community and Enterprise probes passed owner access and non-owner denial. The installed
pre-push hook qualifies the final commit with both full suites; the PR records
the subsequent GitHub result without rewriting earlier failed runs.
