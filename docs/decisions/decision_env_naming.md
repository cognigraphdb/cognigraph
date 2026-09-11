# Decision: environment variable naming — the pre-2.0 sweep

**Status:** Decided 2026-07-05. Executed the same day (H5).

## Context

Config grew organically: backend/storage knobs arrived prefixed
(`COGNIGRAPH_BACKEND`, `COGNIGRAPH_NATIVE_PATH`, ...) while server knobs
arrived bare (`AUTH_ENABLED`, `JWT_SECRET`, `CACHE_ENABLED`,
`LOG_FORMAT`, ...). Bare generic names are collision-prone in shared
container/compose environments, and the split already caused one real
incident: docker-compose set `ADMIN_PASSWORD` while the server read
`COGNIGRAPH_ADMIN_PASSWORD` — auth bootstrap silently never ran. The
roadmap (H5) required either a sweep before anything ships or a recorded
keep-split decision. With v2.0.0 about to tag and nothing shipped, this
was the one moment a breaking rename costs nothing.

## Decision (owner: user, via H5 approval; naming rule: agent)

Sweep, with one classification rule:

1. **CogniGraph-owned knobs are `COGNIGRAPH_*`.** Renamed:
   `AUTH_ENABLED`, `JWT_SECRET`, `JWT_TTL_SECS`, `CGQL_MUTATIONS_ENABLED`,
   `CGQL_MAX_SOURCE_ROWS`, `CGQL_TIME_BUDGET_MS`, `LUA_INSTRUCTION_LIMIT`,
   `RATE_LIMIT_PER_MINUTE`, `REQUEST_TIMEOUT_SECS`, `LOG_FORMAT`,
   `EMBEDDING_PROVIDER`, `EMBEDDING_MODEL`, `VECTOR_SEARCH_MODE` — each
   gains the `COGNIGRAPH_` prefix verbatim.
2. **The semantic query cache family is `COGNIGRAPH_QUERY_CACHE_*`**
   (not `COGNIGRAPH_CACHE_*`): `COGNIGRAPH_CACHE_BYTES` already means the
   native backend's paged DOCUMENT cache, and overloading one family name
   for two caches is exactly the ambiguity a breaking window exists to
   fix. `CACHE_ENABLED`/`CACHE_TTL_SECS`/`CACHE_MAX_ENTRIES`/
   `CACHE_SIMILARITY_FLOOR`/`CACHE_STRONG_THRESHOLD`/`CACHE_BACKEND` →
   `COGNIGRAPH_QUERY_CACHE_*`.
3. **Ecosystem-standard third-party names stay bare:** `OPENAI_API_KEY`,
   `OPENAI_BASE_URL`, `GEMINI_API_KEY`, `OLLAMA_BASE_URL`, `ARANGO_URL`,
   `ARANGO_DB`, `ARANGO_USER`, `ARANGO_PASSWORD`, `RUST_LOG`. These are
   names the wider ecosystem sets for us; prefixing them would break
   every standard credential-injection setup for zero gain.
4. **No fallback reads, loud failure instead.** Legacy names are NOT
   read (a fallback perpetuates both spellings forever). Instead
   `Config::from_env` warns on stderr for every legacy name found set —
   set-but-ignored config (auth silently off) is the worst failure mode,
   so a stale `.env` announces itself at startup.
5. Historical documents (CHANGELOG entries, decision records, archived
   plans) keep the old names — they describe what was true then.

## Outcome

Executed 2026-07-05 across config.rs, cognigraph-cache, route messages,
openapi.yaml, Dockerfile, docker-compose.yml, and all live docs; gates
green. This record closes the question — do not revisit naming again
unless a third naming class genuinely appears.
