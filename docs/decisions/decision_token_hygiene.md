# Decision: API-token expiry and rotation (H6)

**Status:** Decided and landed 2026-07-06.

## Context

API tokens (`_tokens` collection, SHA-256 hash only) lived forever until
explicitly revoked, and "rotation" meant revoke + recreate — a new token
key, breaking anything that referenced the old one. H6 asked for an
expiry/rotation policy before the auth surface calcifies.

## Decision (owner: agent, within the approved H6 scope)

1. **Expiry is opt-in, not silently imposed.** Tokens carry `created_at`
   and optional `expires_at` (unix secs). `COGNIGRAPH_TOKEN_TTL_SECS`
   (default 0 = never) sets the server-wide default for NEW tokens;
   per-request `expires_in_secs` overrides it in both directions
   (`0` = explicitly non-expiring). Default-never because a
   default-expiring credential in a deployment where nobody read the docs
   produces mysterious 401s months later; the runbook states the
   recommended production policy (90 days) instead.
2. **Expired ≠ deleted.** An expired token fails validation with the SAME
   error as a revoked/unknown token (no oracle), but its record stays in
   `list_tokens` with `"expired": true` for audit. Removal stays an
   explicit revoke.
3. **Rotation is in-place.** `rotate_token` keeps the record and key,
   replaces the hash, and stamps a fresh `created_at`/`expires_at` window
   (fresh-grant semantics: rotation without a TTL falls back to the
   server default, and clears any previous expiry when that default is
   0). The old secret dies the moment the update lands. Exposed as
   `POST /api/users/{key}/tokens/{token_key}/rotate` (admin) and
   `cognigraph token rotate USER TOKENKEY [--ttl-secs N]`.
4. **No `last_used_at`.** Touching the token record on every request is a
   write per request — in persistent mode that is one fsync per request
   against a measured ~255 commits/s ceiling (benchmarks.md H4). Revisit
   with an in-memory counter + periodic flush if usage auditing is ever
   actually needed.
5. **No auto-purge.** Expired records are inert and small; deleting them
   automatically would erase the audit trail point 2 exists to keep.

## Outcome

Landed 2026-07-06: `TokenGrant` return type, expiry check in
`validate_token`, `rotate_token`, `created_at`/`expires_at`/`expired` in
`list_tokens`, `COGNIGRAPH_TOKEN_TTL_SECS`, route + CLI + OpenAPI (drift
test enforced the spec entry), lifecycle test covering expiry, rotation
revival, and TTL clearing.
