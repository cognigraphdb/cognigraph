# Tenant derivative isolation (CG-11)

- Date: 2026-09-08
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:375-384` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Tenant derivative isolation (CG-11).** Native schema 2 adds a database
  UUID and unique commit revision; persistent text/vector derivatives bind
  both identities. Tenant retirement also quarantines text directories and
  unfinished vector builds, including legacy names. Database replacement,
  divergent restore, and resident/paged release HTTP upgrade/recreation/restart
  checks passed. **Opening schema 1 upgrades metadata atomically; older
  binaries refuse schema 2.** Downgrade uses a pre-upgrade backup or JSON
  export/import into a separate older database. See
  [verification and migration details](../issues/derivative-isolation-2026-09-08.md).
