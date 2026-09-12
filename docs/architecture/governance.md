# Governance architecture

## Governance Model

This section is the short version. The normative contracts are the decision
records indexed under [governed operations](../decisions/README.md#governed-operations-and-authority-generations);
the operator's manual is the [DataOps guide](../dataops/README.md) and
[governance runbook](../operations/governance.md).

### Structural principles

- **Deterministic gates control storage.** Ordinary template grounding and
  recall/restraint scoring make no model calls. Directed construction calls a
  completion provider to nominate relations, then applies exact-ID, taxonomy,
  Unicode endpoint, quote and restraint gates before storage. Proposal, review,
  drafting and answer evaluation also use completion providers.
- **Managed collections are closed to generic writes.** `space_types`,
  `neurons`, `review_policies`, `eval_specs`, `entities`, `chunks`, `mentions`,
  and `facts` remain readable through generic document and parsed-CGQL reads,
  but every generic mutation path (documents, batch, graph, Lua, CGQL) rejects
  them before backend mutation. Typed construction routes retain narrow
  internal write access. Underscore-prefixed authority collections are hidden
  from both reads and writes. Generic edges cannot target a managed collection
  or point into a managed vertex collection.
- **Opaque backend-native query text is disabled for every role.** Text
  screening was shown not to be a security boundary (Unicode-escaped
  identifiers, M18); only parsed CGQL is accepted on public surfaces.
- **Human review is the default.** The legacy neuron workspace's measured,
  injection-gated two-stage judge (taint screener, then quality judge; versioned
  `POLICY_REV`) can auto-accept only an eligible `relation_hint` under an
  existing per-space `review_policies` document. Aliases, blockers, rank hints,
  and malformed output queue for a person. That policy is authoring aid, not
  signed model qualification.
- **Vocabulary drafts are structurally inert** in `space_type_drafts` until an
  attributed typed acceptance (`decision_ontology_drafter.md`).

### The signed chain, one generation per milestone

| Milestone | Boundary closed | Key property |
|---|---|---|
| M16–M17 | Durable, fair, bounded jobs | Idempotent, restart-safe, tenant-scoped; archive never purges |
| M18 | Evaluation → selection | Four-job evidence (candidate/baseline × original/replay), independent integer gates, immutable decisions, repairable `{space_type, channel}` head |
| M19 | Who may author, approve, promote | Externally pinned Ed25519 root; three distinct stable principals; server never holds a private key; prospective revocation |
| M20 | What bytes were claimed | Fourth `artifact-attestor` principal signs canonical exact-byte manifests for corpus, graph, oracle, scorer, verifier; server validates, never fetches |
| M21 | What bytes were consumed | Operator-staged tenant-incarnation local CAS; hash-read of every manifest blob; verified `graph.json`/`oracle.json` replace the live graph as evaluation input; receipt bound into signed intent |
| M22 | Prepared corpus → evaluation facts | Pinned grounder replay; attested `graph.json` must equal the reconstruction byte for byte |
| M23 | Raw UTF-8 text → prepared corpus | Pinned mechanical preparer (Unicode 17.0.0 NFC, fixed whitespace/paragraph/chunk rules, content-addressed chunk ids); reproduced `corpus.json` must match the signed one |
| M24 | CAS byte recovery | Admin-derived canonical recovery plan; offline closed bundle; absent-scope verified restore; unkeyed receipts, no custody claim |
| M25 | Repair candidate authority | PolicyAuthor-signed immutable revision of the exact M22 candidate; independent PolicyApprover review; resolves only when the existing promotion head selects that digest |
| M26 | Selected candidate → served graph | Synchronous Native-only generation build with exact impact receipt; separate Promoter-signed deployment intent; one atomic target-space switch; one generation served per space |

Authority generations never mix within a target: adopting a newer generation
uses a fresh target channel; older records keep their original meaning and
remain readable. Evidence and decisions are immutable; heads are rebuilt from
the decision chain after restart or snapshot import, and degraded authority
fences mutations until Admin recovery validates the tenant.

### What the chain does not claim

Signatures prove who authorized a frozen statement, not that it is true,
current, or complete. Receipts are server-produced unkeyed records bound by
later signed intent, not independent attestations or trusted timestamps.
Scorer/verifier blobs are matched against the executable-path digest pinned at
startup but are never executed. Staged code is never run; promotion never
deploys; building never activates; M26 adds no drift scheduler, automatic
healing, per-generation query routing, generation GC, distributed writer,
quorum, replication, or HA. Native atomic batches provide the required
all-or-nothing construction boundary.

### Multi-tenancy

One backend store per tenant, routed at a single choke point
(`server/src/tenancy.rs`): the auth middleware wraps each handler future in a
task-local tenant scope and the `RoutedBackend`/`RoutedCache` facades resolve
every call through it. Handlers are untouched, CGQL needs no rewriter, and no
cross-tenant read path exists for any role. Host-admin manages tenant records
under the `TenantAdmin` scope, never tenant data. Auth lives in a separate
control store. See `decision_multi_tenancy.md`.

---
