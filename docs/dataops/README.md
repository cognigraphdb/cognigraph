# DataOps Guide

How to prepare, load, query, and govern data in CogniGraph — written for
developers doing everything **manually**: a running server, `curl` (or any
HTTP client), the `cognigraph` CLI, and optionally the repo's example
binaries. No agentic tooling assumed anywhere in these pages.

Deployment, auth, backup, and env configuration live in
[../operations.md](../operations/README.md); this folder is about the **data**.

## The mental model

CogniGraph layers four kinds of state, each feeding the next:

```
documents ──► embeddings ──► search (vector / semantic / hybrid)
   │
chunks ──► grounding ──► facts (graph edges) ──► graph-augmented answers
                │                    ▲
          space type (ontology)      │
                │                    │
            Semantic Neurons ────────┘   (governed repairs & rankings)
```

- **Documents** are JSON in named application collections. M25 reserves generic
  mutation of its semantic-control and construction-derived collections; those
  remain readable but are written only by dedicated typed workflows.
- **Embeddings** make documents findable by meaning. You can compute them
  yourself or let the server do it in batches.
- **Chunks + a space type (ontology)** produce **facts**: evidence-bound
  graph edges, built only where a trigger phrase is affirmed in the text
  (negation-aware). No LLM writes an edge, ever.
- **Semantic Neurons** are the governance layer on top: reviewable,
  attributable units that repair missing facts, veto illegitimate ones, and
  re-rank retrieval. The mutable neuron lifecycle is an authoring workspace;
  M25 governed authority additionally requires a PolicyAuthor-signed exact
  candidate, independent PolicyApprover review, and selection of that digest by
  the existing promotion head. M26 can then verify a complete immutable
  occurrence generation from the selected CAS corpus and deploy it only through
  a separate Promoter-signed Native atomic switch.
  Retirement stops future influence. Targeted
  `POST /api/construct/ingest` reconciliation atomically replaces the supplied
  native chunks and their materialized evidence occurrences; degradation
  detection and repair proposals remain a separate invoked procedure. Review is
  human by default. Under an existing legacy policy, the measured LLM judge can
  auto-accept only the eligible relation-hint lane; malformed outputs and every
  other kind queue. That policy is not signed judge qualification (guide 04,
  §5).

## Reading order

| Guide | What you'll be able to do |
|---|---|
| [01 — Data preparation](01-data-preparation.md) | Model collections, chunk source material, load documents, batch-embed server-side, verify with CGQL |
| [02 — CGQL for DataOps](02-cgql-for-dataops.md) | Audit and reshape your data: filters, grouping, joins, EXPLAIN |
| [03 — Retrieval](03-retrieval.md) | Vector, semantic, hybrid, and graph-augmented search; the query cache |
| [04 — Semantic Neurons](04-semantic-neurons.md) | Author and measure a candidate, review neurons, sign and independently approve exact Semantic Repair authority, invoke governed ingest, or build and sign-deploy a verified M26 generation |
| [05 — Recurring routines](05-recurring-routines.md) | What to re-run when documents change; honest degradation triage, promotion, explicit governed ingest or M26 deployment, pruning, and snapshots |

## Prerequisites

A running server (see [../operations.md](../operations/README.md)):

```sh
COGNIGRAPH_PORT=3000 cognigraph-server
# or: docker compose up
```

Everything below assumes `http://localhost:3000` and, if auth is enabled,
a bearer token in `$TOKEN`:

```sh
export COGNIGRAPH_URL=http://localhost:3000
export COGNIGRAPH_TOKEN=$TOKEN     # the CLI reads both
```

`COGNIGRAPH_URL` is the server base; the application API lives under
`/api` (curl examples below spell it out), while `/health` and
`/metrics` stay at the root.

Semantic search and the embedding pipeline need an embedding provider
(`COGNIGRAPH_EMBEDDING_PROVIDER=openai|ollama|gemini` plus the provider's key),
and **proposing, judging, and answer evaluation need a completion provider**.
Documents, CGQL, grounding, and neuron *storage* run fully offline.

So "runs offline" is a property of a **deliberately local deployment** (e.g.
Ollama for both providers), not an intrinsic property of every configuration. If
you point it at OpenAI or Gemini, text leaves your perimeter. Choose the
providers accordingly and state which you chose.

## Two ways to drive everything

Product operations in these guides are shown as plain HTTP. Where the
`cognigraph` CLI has a shortcut, it is shown alongside. Library/example-only
diagnostics are labeled as such. The CLI is the same HTTP API with ergonomics —
nothing in it is privileged.
