# 2026-07-04 — Packaging, ops, administration

- Date: 2026-07-04
- Status: Historical
- Kind: History
- Date source: Original section heading
- Source: `CHANGELOG.md:2248-2260` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- Backup/restore: GET /admin/export, POST /admin/import (+ snapshot
  capability on GraphBackend); hardened Dockerfile + compose profile;
  OpenAPI served at /openapi.yaml; docs/operations.md runbook.
- `cognigraph` admin CLI (health, export/import, query, doc/user/token
  management, cache, JWT login); ships in the Docker image.
- GitHub Actions CI running the exact commit gates; live-LLM tests
  opt-in via COGNIGRAPH_LIVE_LLM.
- Concurrent-load benchmark: 135k point reads/s, 121k pushdown CGQL
  queries/s at c=128 (p99 < 3 ms); mixed 90r/10w indistinguishable from
  pure reads.
