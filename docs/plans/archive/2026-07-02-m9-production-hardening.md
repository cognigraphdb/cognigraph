# M9 — Production Hardening (Phase 10, first slice)

> Executed inline 2026-07-02.

- [x] Request timeout (tower-http TimeoutLayer, `REQUEST_TIMEOUT_SECS`, default 30) — also the v1 guard against runaway CGQL scans (per-query row caps deferred).
- [x] Per-IP fixed-window rate limiting, custom implementation (`RATE_LIMIT_PER_MINUTE`, 0 = disabled default), 429 on breach, /health and /metrics exempt.
- [x] `/metrics`: hand-rolled Prometheus text exposition — requests total, responses by status class, duration sum/count, uptime. Open endpoint (scraper-friendly), like /health.
- [x] JSON structured logging via `LOG_FORMAT=json` (tracing-subscriber json feature, already in workspace).
- [x] Graceful shutdown on SIGINT/SIGTERM.
- [x] Dockerfile (multi-stage, distroless-ish slim runtime) + .dockerignore.
- Deferred: circuit breaker/retry (Arango maintenance-mode), CLI tool, benchmarks vs Python, quantization/mmap/rayon performance work, per-query CGQL row/time budgets.
