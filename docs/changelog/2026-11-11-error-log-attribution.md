# v2.7.51 — Error log attribution

- Date: 2026-11-11
- Status: v2.7.51
- Kind: Security fix (Enterprise multi-tenancy)

## Changes

[CG-108](../issues/CG-108.md) is resolved under the
[error log attribution decision](../decisions/decision_error_log_attribution.md).

`GET /api/admin/logs` shows a tenant admin that tenant's recent failed
requests. A request the auth middleware rejected was stored without a
tenant, and such events were shown to every tenant's admin with their full
path and message. One tenant's collection names and document keys could
appear in another tenant's log view.

- A rejection issued after the caller is identified is stored under that
  caller's tenant and shown to that tenant's admins only.
- Requests with no identifiable caller (no token, an invalid token, an
  unknown route) are still listed. On a multi-tenant server a tenant admin
  sees their path reduced to the route group, for example
  `/api/documents/…`.
- Single-tenant servers and the stdout log are unchanged.

No storage or wire-format change. The `path` field of an unattributed event
can now end in `/…`.

## Validation

`cargo fmt --all -- --check`, both Clippy gates with `-D warnings`,
`cargo test --locked --all` (848 passed, 2 ignored) and
`cargo test --locked --all --features enterprise` (1,076 passed, 2 ignored)
pass. A release Enterprise server with two tenants was driven over HTTP: a
viewer's 403 in one tenant was invisible to the other tenant's admin, and
two 401s appeared there with the reduced path. The registry, docs and
decision-index checks pass.

Not run: the UI, Native, Helm, Docker, advisory and dependency suites; they
run on the finished P1 batch. Nothing was pushed, released or deployed.
