# Multi-tenancy M1: tenant identity

- Date: 2026-07-13
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:1498-1515` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **Multi-tenancy M1: tenant identity** (design session D1–D6,
  decision_multi_tenancy.md — isolation is structural, never
  predicate-discipline). Tenants are first-class records
  (`_tenants`, name/status/reserved-quota schema) with lifecycle over
  `POST|GET /tenants` + `/tenants/{name}` under a NEW dedicated
  `TenantAdmin` scope; the new `host-admin` role holds that scope and
  deliberately nothing else — host-admin manages tenants, it does not
  read their graphs. `User.tenant` (v1: exactly one; legacy user
  documents and JWT sessions default to the implicit `default` tenant —
  back-compat absolute), carried through tokens and JWT claims, and
  gated in the auth middleware before any route logic: suspended or
  unknown tenants are refused at the door. Users can be created into a
  tenant (`tenant` on POST /users). M2 (per-tenant backend stores +
  request routing) is next; until then all tenants share the single
  store, and single-tenant deployments never notice tenancy exists.
  Tests pin the gate matrix, host-admin's scope isolation, legacy-doc
  defaults, JWT tenant carry, and route CRUD. 376 tests.
