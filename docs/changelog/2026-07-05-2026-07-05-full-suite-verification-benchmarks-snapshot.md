# 2026-07-05 — Full-suite verification + benchmarks snapshot

- Date: 2026-07-05
- Status: Historical
- Kind: History
- Date source: Original section heading
- Source: `CHANGELOG.md:2204-2216` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- Complete re-run after the Arc A + Arc B push: 324/324 tests green
  across all 11 crates, clippy `-D warnings` and fmt clean, all three
  benchmark harnesses (native engine, grounding, server concurrent load)
  within noise of their recorded baselines — no regressions anywhere.
- Headline figures: CGQL flagship scan 0.83 ms (10x cumulative vs three
  days prior), vector search 0.36 ms at 100% recall@10, upsert_edge
  0.001 ms, 139k point reads/s and 126k CGQL queries/s at 128 concurrent
  clients with p99 < 3 ms.
- docs/benchmarks.md gains an authoritative "Current numbers" snapshot
  section; the dated delta log stays intact as the optimization history.
