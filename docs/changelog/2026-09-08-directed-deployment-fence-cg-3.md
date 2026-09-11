# Directed deployment fence (CG-3)

- Date: 2026-09-08
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:394-402` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Directed deployment fence (CG-3).** Directed construction checks tenant
  health and M26 deployment authority before space creation or provider calls,
  and holds the promotion transition lock through reconciliation. An admitted
  ingestion finishes before deployment; later writes to a deployed space
  return 409. Signed lifecycle tests and release-server HTTP checks passed in
  resident/paged Native modes, including restarts, zero provider calls on
  rejection, and complete snapshot preservation. See
  [verification and scope](../issues/directed-fence-2026-09-08.md).
