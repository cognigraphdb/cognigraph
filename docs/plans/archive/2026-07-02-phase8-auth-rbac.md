# Phase 8 — Auth & RBAC Implementation Plan

> Executed inline 2026-07-02 (superpowers:executing-plans).

**Goal:** Bearer-token auth with role-based scopes, backend-agnostic, off by default.

**Decisions:**
- Auth storage = `_users` / `_tokens` collections via `GraphBackend` — ONE implementation for all backends (no per-backend AuthProvider impls).
- argon2 password hashing; API tokens are `cg_<random>`, stored as SHA-256 hashes only.
- Roles → scopes: admin → all; editor → documents/graph read+write, search, lua:execute; viewer → documents:read, graph:read, search; script-runner → lua:execute, search, documents:read, graph:read.
- `AUTH_ENABLED=false` default; when enabled, `COGNIGRAPH_ADMIN_PASSWORD` bootstraps the admin user; `/health` stays open.
- Route scopes: documents GET→documents:read, writes→documents:write; graph read/write; search→search; lua→lua:execute; POST /query→documents:write; cache+users→admin.
- Lua `graph.query()` read-only restriction lifts per M7 decision 3: callers with documents:write scope get ReadWrite mode.
- JWT sessions deferred (optional in the original plan).

- [x] Task 1 — `cognigraph-auth` crate: Role/Scope types, AuthProvider over `Arc<dyn GraphBackend>` (user CRUD, password verify, token create/revoke/validate, admin bootstrap), unit tests.
- [x] Task 2 — Server: config (AUTH_ENABLED, COGNIGRAPH_ADMIN_PASSWORD), AppState.auth, scope middleware, users/tokens routes, per-router scope layers.
- [x] Task 3 — Lua mode plumbing: bindings accept QueryMode; route selects by scope.
- [x] Task 4 — Tests, docs (README env vars, implementation-plan), validation, commit.
