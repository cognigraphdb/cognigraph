# Decision: Native-first — own backend, own query language

Date: 2026-07-02 · Status: ACCEPTED, delivered (M1–M10)

## Context
ArangoDB (the original primary backend) changed its licensing model and broke
large API surfaces in v4, signaling third-party-backend risk. Maintaining
multiple query languages multiplied effort. The product had never shipped, so
breaking changes were free.

## Decision (owner: skitsanos)
CGQL becomes the product's only query language; `cognigraph-native` becomes
the strategic backend (persistence, full-text, vectors); ArangoDB is demoted
to maintenance as a conformance reference; no new third-party *database*
backends (SurrealDB dropped). Embedded, license-stable *libraries* (redb,
memmap2, tantivy) remain acceptable behind our own traits.

## Outcome
Delivered in one arc (M1–M10): the default install runs entirely on its own
stack — redb persistence, read-write CGQL, tantivy BM25, auth — with 269
tests and a live-ArangoDB-verified conformance suite. Feasibility concern
("can a small team own the whole stack?") answered empirically: yes, because
the scope excluded distributed consensus and cost-based optimization.
