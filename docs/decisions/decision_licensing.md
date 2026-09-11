# Decision: licensing — FSL community core, commercial enterprise components

**Status:** Decided 2026-09-10 (owner: user). Executed the same day for the
license texts, crate manifests and the build boundary in
[CG-45](../issues/CG-45.md).

## Context

The workspace declared `license = "MIT"` since the first commit while the
repository stayed private. That was never a decision, only a template default,
and it contradicted the intent to keep multi-node and governed operations
proprietary. The commercial trigger is external: ArangoDB moved its Community
Edition source to BSL 1.1 from 3.12 and its binaries to a Community License
with a 100 GB cap and no commercial use, so developers who built products on
ArangoDB CE now need either a contract or a replacement. CGQL is
AQL-familiar and the Native backend is single-node, which is how most of
those deployments ran.

Three options were weighed for the community core: FSL-1.1-Apache-2.0,
BSL 1.1 with a custom Additional Use Grant, and Elastic License 2.0. FSL was
chosen: fixed text with one prohibition (Competing Use), an explicit
Permitted Purpose for building products on the software, and an irrevocable
two-year conversion to Apache 2.0 per version, which is the trust signal the
ex-ArangoDB audience is looking for after a BSL change. MIT was rejected for
the whole workspace because it would give away clustering and governance the
day they exist.

## Decision

1. **Community Components** are licensed under the Functional Source License,
   Version 1.1, Apache 2.0 Future License (`LICENSE`): `cognigraph-core`,
   `-query`, `-native`, `-arango`, `-lua`, `-cache`, `-embeddings`, `-auth`,
   `-cli`, `-server` and `ui/`. Production use, including commercial products
   built on CogniGraph, is a Permitted Purpose. Offering CogniGraph itself as
   a managed or competing database is a Competing Use.
   Read replicas for read scaling and manual standby with operator-controlled
   promotion for a single-writer store are Community capabilities when released.
2. **Enterprise Components** are licensed under the CogniGraph Enterprise
   License (`LICENSE-COMMERCIAL`): `cognigraph-governance`,
   `cognigraph-construct`, `cognigraph-artifacts`, and by capability the
   M15–M26 governance chain, Semantic Neurons, multi-tenancy, sharding,
   automatic failover and multi-writer deployment. Free for evaluation and
   development; production requires
   a Subscription. Never converts to open source.
3. **Build editions follow the capability boundary.** The default Community
   server and CLI exclude the three Enterprise crates and tenant execution
   machinery. `--features enterprise` includes them. The generic graph edge
   ranker belongs in Core; accepted neuron rank boosts remain in Construction.
   All current durable job kinds perform Enterprise construction or evaluation,
   so their scheduler, recovery and routes are Enterprise too. Ordinary snapshot
   backup/restore remains Community. The capability license still applies to
   Enterprise modules living inside the server crate.
4. **Contributions** require the CLA in `CLA.md`, accepted by a
   `Signed-off-by:` trailer. The CLA grants Gedank Rayze the right to
   relicense contributions under all three licenses; without it the Apache
   conversion and the commercial tier would both be encumbered.
5. **Naming:** CGQL is described as "familiar to AQL users", never
   "AQL-compatible". ArangoDB and AQL are third-party marks.

## Consequences

- `Cargo.toml` no longer says MIT; every crate manifest declares its own
  license. The README's license section points at `LICENSING.md`.
- The delivered free-tier Kubernetes scope is single-node. Read replicas remain
  a [design proposal](../architecture/design-notes/read-replicas.md), assigned
  to Community together with manual standby by the owner's 2026-09-10 decision.
  Sharding, consensus clustering, automatic failover and multi-writer deployment
  remain Enterprise scope.
- The Enterprise License is a draft for legal review. Subscription terms,
  pricing and any production grant for small deployments are not decided
  here.
- Copyright year is 2026 (first commit 2026-03-09).
- This record does not make the repository public. Visibility is a separate
  decision that now has a license to go with it.

## CG-45 build and data boundary

Community keeps tenant rejection endpoints for a stable structured error, but
omits them and governed operations from its generated OpenAPI document. Each
build reports its edition at `/health`. The specification is generated from the
canonical YAML during Cargo compilation, with the package version and only
reachable Community schemas. It is served as JSON, which is valid YAML 1.2.

Community refuses non-default identities and Enterprise deployment configuration.
It also refuses stores and snapshot imports with nonempty managed/generated or
`_cognigraph_*` collections. Without the Enterprise cascade, recovery and
promotion fences, mutating such records would be unsafe. This is admission
validation, not an on-disk format change or a conversion tool. Ordinary stores
can move between editions. A Community-to-Enterprise start may create governed
state; subsequent Community admission depends on that state remaining empty.

The [CG-45 evidence](../issues/CG-45.md) records local checks and real-binary
verification. CG-45 does not implement replicas.

## Read-replica tier amendment — 2026-09-10

The owner confirmed that read replicas and manual standby belong in Community.
This resolves the earlier disagreement between the licensing map and replica
design. Manual promotion keeps one writer and requires operator action;
automatic failover, sharding, consensus clustering and multiple writers remain
Enterprise capabilities. Replication does not make other Enterprise capabilities,
including multi-tenancy and governed construction, available under Community.

The draft Enterprise license definition and active product documentation follow
this boundary. The runtime design remains proposed; no replication capability
ships yet.
