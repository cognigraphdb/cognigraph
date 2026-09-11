# Decision: Auth data lives in graph collections via GraphBackend

Date: 2026-07-02 (M8) · Status: ACCEPTED

## Context
The original Phase 8 plan assumed per-backend AuthProvider implementations.

## Decision
Users/tokens are documents in `_users`/`_tokens`, accessed through the
existing trait — ONE implementation, every backend (in-memory, persistent,
Arango) gets auth for free. argon2 passwords; tokens stored as SHA-256
hashes, plaintext shown exactly once. Fixed role→scope matrix; read scope
for GET/HEAD, write scope otherwise; auth off by default.

## Outcome
The whole of Phase 8 fit in one crate + middleware, with a full lifecycle
test on the native backend. JWT sessions delivered 2026-07-03: HS256 via hmac/sha2 (library-backed crypto per decision_custom_vs_library.md), POST /api/auth/login, middleware accepts cg_ tokens or JWTs; stateless by design (short TTL, no revocation — API tokens cover revocable access).
