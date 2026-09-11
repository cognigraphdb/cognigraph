# 2026-07-03 — Semantic Neurons hardening (post-M14)

- Date: 2026-07-03
- Status: Historical
- Kind: History
- Date source: Original section heading
- Source: `CHANGELOG.md:2261-2273` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- relation_rank_hint (retrieval-trace ranking boosts) and
  relation_blocker (extraction-time veto, no precedence engine) neuron
  kinds; blocker acceptance suite closes a manufactured violation with
  recall held.
- Proposer reliability: skip reports, retries, verbatim trigger
  self-check, both-endpoint evidence ranking; generalization experiment
  closes 2/6 → 6/6 recall on the untuned CrowdStrike article (restraint
  0/3), pinned as a deterministic regression test.
- Graph-augmented search returns ranked `graph_facts`, reweighted by
  accepted rank hints read from the neurons collection.
