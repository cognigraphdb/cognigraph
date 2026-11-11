# Decision: a rejected request belongs to its caller's tenant, and unattributed requests are redacted

Status: accepted and implemented 2026-11-11 for [CG-108](../issues/CG-108.md).

## Context

`GET /api/admin/logs` shows a tenant admin the recent non-2xx responses of
that tenant. The auth middleware stamped the tenant onto a response only
after the request had been admitted and served. A rejection (wrong scope,
suspended tenant, stale identity) carried no tenant, was stored as
untenanted, and untenanted events were shown to every tenant's admin on the
assumption that they hold nothing tenant-specific. They hold the method, the
path and the message. A viewer in one tenant who was refused
`DELETE /api/documents/patients/p-123` put that path into every other
tenant's log view, and a suspended tenant's name appeared in the message.

## Decision

1. **Attribute at identification, not at admission.** As soon as the caller
   is known, by a valid API token or a signature-verified session token, a
   rejection is tagged with that caller's tenant. It is then visible to that
   tenant's admins only.
2. **Unattributed events stay listed.** A request with no token or an invalid
   one has no tenant. It is still useful to an admin (failed logins, probes),
   so it is not hidden.
3. **Redacted on a multi-tenant server.** Such a request may have been aimed
   at another tenant's data, so a tenant admin sees its path reduced to the
   route group: the first two segments followed by an ellipsis
   (`/api/documents/…`). Method, status, time and message are kept.
4. **Not redacted on a single-tenant server.** Without isolated tenant
   stores the admin is the operator, and with authentication off there is no
   caller tenant at all.
5. **Rejected: hiding unattributed events from tenant admins.** An admin
   would lose sight of failed sign-ins against their own tenant.
6. **Rejected: keeping three path segments.** `/api/documents/patients/…`
   still discloses a collection name.

## Consequences

- The stored event is unchanged; redaction happens when a tenant admin
  reads it. The server's own log output keeps the full path for the
  operator.
- The error log remains an in-memory, lossy aid and not an audit record.
- A host administrator sees the same redacted view as a tenant admin. A
  full cross-tenant view, if wanted, is a separate control-plane feature.
