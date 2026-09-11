# Decision: Custom implementations first, libraries where measurement or safety demands

Date: 2026-07-02 · Status: ACCEPTED (standing preference, with named exceptions)

## Context
Project preference: custom implementations over third-party crates when
practical (supply-chain and understanding benefits).

## Decision
Custom: BM25 v1, rate limiter, Prometheus text exposition, bench harness,
RRF fusion, quantization. Library exceptions, each with a reason on record:
argon2/sha2 (never hand-roll crypto), redb (storage engines are where
hand-rolling buys bugs), memmap2 (thin safe wrapper), rayon, tantivy
(adopted only after measurement — see Outcome), pest (grammar).

## Outcome
The custom exact BM25 served as the correctness oracle; when profiling showed
it tokenization-bound (two independent measurements), tantivy replaced it for
a measured 115× — with the custom implementation's tests proving the swap
changed nothing semantically. The pattern generalizes: build the honest
simple thing, measure, then buy the index.
