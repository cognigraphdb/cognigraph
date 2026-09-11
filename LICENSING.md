# Licensing

CogniGraph is source-available. The single-node multi-model database is free
to run in production under the Functional Source License; the governance chain, Semantic
Neurons, multi-tenancy and clustering are Enterprise Components under a
commercial license. This page is the plain-language map; `LICENSE` and
`LICENSE-COMMERCIAL` are the operative texts.

## The two licenses

| | Community Components | Enterprise Components |
|---|---|---|
| License | [FSL-1.1-Apache-2.0](LICENSE) (Functional Source License) | [CogniGraph Enterprise License](LICENSE-COMMERCIAL) |
| Read the source | Yes | Yes |
| Run in production | Yes, free, including for commercial products | Requires a Subscription |
| Build your SaaS or product on it | Yes | Requires a Subscription |
| Embed or redistribute | Yes, under the same terms | No |
| Offer it as a managed database service or competing product | No (a "Competing Use") | No |
| Evaluate, develop, test, demo | Yes | Yes, free |
| Becomes Apache 2.0 | Each version, two years after its release | Never |

The FSL forbids exactly one thing: making CogniGraph itself available to others
as a commercial product or service that substitutes for CogniGraph. Running a
product *on* CogniGraph is not that. A produce-lookup site, a transcript
analysis platform or a forensic case tool that stores its data in CogniGraph is
a Permitted Purpose. A hosted "CogniGraph as a service" is not.

## What is in each tier

**Community** — everything a single-node ArangoDB Community Edition user
needs, without the dataset cap and without the commercial-use restriction:

- Native storage backend (redb primary, resident and paged modes, rebuildable
  derivatives), and the ArangoDB conformance backend
- CGQL v2: `FOR`/`FILTER`/`SORT`/`LIMIT`/`LET`/`COLLECT`, subqueries, joins,
  traversals, mutations, `EXPLAIN ANALYZE`, index-assisted planning
- Full-text (tantivy) and vector search, hybrid retrieval, semantic query cache
- Lua scripting with `graph.query()` and the instruction/time budgets
- Embedding providers (OpenAI-compatible, Ollama, Cloudflare, ONNX)
- Authentication, JWT, RBAC, rate limiting, request timeouts
- The HTTP API, OpenAPI document, CLI, management console, Docker image and
  Helm chart
- Backup and restore for a single tenant
- Read replicas for read scaling and manual standby with operator-controlled
  promotion for a single-writer store, when released

**Enterprise** — what a regulated buyer pays for:

- Governed operations M15–M26: signed policy authority and separation of duties,
  evaluation and promotion gates, content-addressed external artifact
  attestation, verified artifact consumption, reproducible corpus-to-graph
  derivation, durable CAS custody with verified restoration, governed semantic
  repair authority and verified materialization/deployment
- Semantic Neurons governed construction: grounding gates, relation blockers,
  recall/restraint evaluation, gate advisor, directed construction, side views
- Durable construction, evaluation, drafting and side-view jobs (all current job kinds)
- Multi-tenancy (more than one tenant per server)
- Sharded clustering, automatic failover and multi-writer deployment, when released
- Support and the warranties a Subscription provides

Read replicas and manual standby are assigned to Community (owner decision,
2026-09-10). They remain a design proposal; no read-replica capability ships in
the current binary. Automatic failover, sharding and multi-writer deployment
remain Enterprise scope.

## Crate map

| Crate | License | Notes |
|---|---|---|
| `cognigraph-core` | FSL-1.1-Apache-2.0 | Traits, document model, errors |
| `cognigraph-query` | FSL-1.1-Apache-2.0 | CGQL parser, AST, planner |
| `cognigraph-native` | FSL-1.1-Apache-2.0 | Native backend |
| `cognigraph-arango` | FSL-1.1-Apache-2.0 | ArangoDB conformance backend |
| `cognigraph-lua` | FSL-1.1-Apache-2.0 | Lua runtime and primitives |
| `cognigraph-cache` | FSL-1.1-Apache-2.0 | Semantic and paged caches |
| `cognigraph-embeddings` | FSL-1.1-Apache-2.0 | Embedding providers |
| `cognigraph-auth` | FSL-1.1-Apache-2.0 | Authentication and RBAC |
| `cognigraph-cli` | FSL-1.1-Apache-2.0 | Administration CLI |
| `cognigraph-server` | FSL-1.1-Apache-2.0 | HTTP server; see the build note below |
| `cognigraph-governance` | Enterprise | Signed authority, attestations, custody |
| `cognigraph-construct` | Enterprise | Semantic Neurons |
| `cognigraph-artifacts` | Enterprise | Artifact plans, consumption, derivation |
| `ui/` | FSL-1.1-Apache-2.0 | Management console |

Each crate's `Cargo.toml` declares its license. Enterprise crates carry a
`LICENSE` file that points at `LICENSE-COMMERCIAL`.

### Build note

The server and CLI build as **Community by default**. The optional `enterprise`
Cargo feature includes the Enterprise crates and governed/tenant execution
modules. Enable it on both binaries for Enterprise operations. Docker follows
the same default; Helm's `edition` value selects the matching image tag.
See [building and running editions](docs/operations/running.md#build-editions).

Community exposes the ordinary single-tenant database API and an edition-specific
OpenAPI document. Non-default identities and tenant lifecycle requests are
rejected with `enterprise_feature_required`. Governed routes are absent.
Existing ordinary stores and snapshots retain the same format; Community
refuses nonempty governed/generated state that needs Enterprise lifecycle
handling. Use an Enterprise build for those stores rather than deleting records
to force a downgrade.

Production use of Enterprise capabilities still follows `LICENSE-COMMERCIAL`.
There is no runtime license-key service in this change.

## Contributing

Contributions to either tier require the [Contributor License
Agreement](CLA.md), signalled by a `Signed-off-by:` trailer on every commit.
See [CONTRIBUTING.md](CONTRIBUTING.md).

## Trademarks

"CogniGraph", "CGQL" and "Semantic Neurons" are trademarks of Gedank Rayze,
LDA. CGQL is designed to be familiar to AQL users; it is not an ArangoDB
product and is not "AQL-compatible" in any certified sense. ArangoDB and AQL
are trademarks of their owner.

## Questions

Commercial licensing: info@skitsanos.com or https://cognigraphdb.com. This page is not legal advice; the
license texts govern.
