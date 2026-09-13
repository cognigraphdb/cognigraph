# Multi-tenancy M3+M4: ops and docs

- Date: 2026-07-13
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:1465-1476` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Multi-tenancy M3+M4: ops and docs.** `/tenants` listing now reports
  which stores are open (`store_open` per tenant, `open_stores` total)
  via the registry's observability handle; CLI gains the full tenant
  lifecycle (`tenant list|create|suspend|activate|delete`) and
  `user create … --tenant T`. Per-tenant export/import needed no new
  code — the facade routes `/admin/export` to the requesting tenant's
  store by construction. operations.md gains the tenancy runbook
  (setup, lifecycle, per-tenant backup, single-store migration by file
  move, measured overhead); architecture.md documents the
  choke-point design. Milestones M1–M4 complete; deferred with
  recorded triggers: quota enforcement (D5), persistent query cache in
  multi-tenant mode, per-tenant metrics labels.
