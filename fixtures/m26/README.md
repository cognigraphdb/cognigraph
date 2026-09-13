# M26 verified Semantic Repair request templates

These files document the checked public request shapes for the M26 generation
and deployment surface. They are deliberately nonfunctional authoring
templates, **not signed authority or release evidence**. Every
`REPLACE_WITH_...` value and the epoch-millisecond value `1` is a placeholder.
No private key, secret seed, credential, or production signature belongs in
this directory, `.env`, an HTTP request, a Native snapshot, or server
configuration.

- `generation-build.request.json` is the unsigned body for
  `POST /api/semantic-repairs/generations`. The authenticated caller must have
  the Promoter role and supply an `Idempotency-Key` header. The expected
  promotion decision must be the exact current decision returned for the same
  `{space_type, channel}` target.
- `deployment-intent.request.json` is the public signed envelope for
  `POST /api/semantic-repairs/generations/{generation_id}/deploy`. It contains
  placeholders only; complete and sign the statement outside CogniGraph with
  an active root-certified `promoter` key, then submit it as that stable
  principal with the same raw Idempotency-Key whose SHA-256 address is bound
  inside the payload.

## Build before deployment

M26 build is an explicit synchronous operation. It accepts only the current
M22 or M23 promotion authority whose exact candidate has an approved M25
Semantic Repair revision. With
`COGNIGRAPH_ARTIFACT_SOURCE=local-cas`, CogniGraph hash-reads the exact prepared
`corpus.json`, reruns the pinned deterministic grounder, and requires its
sorted semantic facts to equal both original and replay derivation receipts.
It then stores one immutable complete target occurrence projection plus the
exact candidate-versus-baseline added, removed, and unchanged impact.

Building does not deploy. It writes no graph rows, and a stale promotion head,
CAS failure, derivation mismatch, timeout, or capacity error stores no partial
generation. The current Native backend is the only supported M26 backend;
ArangoDB rejects build before CAS reads, M26 collection creation or authority
writes, and graph mutation because it lacks the required atomic batch
capability.

CLI equivalent:

```sh
cognigraph semantic-repair generation build \
  --target-space acme_supply \
  --channel m26-production \
  --expected-promotion-head PROMOTION_DECISION_ID \
  --idempotency-key acme-generation-1
cognigraph semantic-repair generation show GENERATION_ID
```

## Sign one exact deployment act

Use the complete generation detail response to replace the deployment
placeholders exactly. Signers must use the NFC-normalized, lexicographically
ordered, integer-only canonical JSON and domain-separated Ed25519 protocol
provided by `cognigraph-governance`; do not sign pretty-printed bytes, an ad
hoc hash, or a bare digest. Public keys and signatures use canonical unpadded
base64url. Ordinary canonical digests use
`sha256:<64 lowercase hexadecimal characters>`.

Both nullable deployment fields are required in JSON:

- bootstrap `activate`: set `expected_deployment_head_decision_id` to explicit
  `null` and `rollback_target_generation_id` to explicit `null`;
- later `activate`: bind the current deployed decision id and keep the rollback
  target explicit `null`;
- `rollback`: first complete the separate signed promotion rollback so the
  retained prior candidate is current again, set `requested_action` to
  `rollback`, bind the current deployed decision id, and set
  `rollback_target_generation_id` to the exact retained prior generation id
  also named by `semantic_repair_generation_id` and the request path.

Omitting either field is not equivalent to `null` and is rejected. The path id,
generation and impact digests, current promotion decision/projection,
candidate, M25 revision/review, signer registration, principal, signed time,
and Idempotency-Key hash must all match. The Promoter principal must remain
distinct from the M25 author and approver and from every Artifact Attestor in
the selected promotion authority.

Deployment atomically replaces the exact target space's `chunks`, `mentions`,
and `facts` projection, inserts compatible missing shared entities, and commits
the immutable signed deployment decision plus its derived head. One space has
one deployed generation even when promotion uses multiple channels. Other
spaces remain untouched; entities are shared and are never deleted. A
rollback is another signed atomic decision, not mutation of a retained
generation.

```sh
cognigraph semantic-repair generation deploy GENERATION_ID \
  @deployment-intent.request.json \
  --idempotency-key acme-deployment-1
cognigraph semantic-repair deployment current acme_supply
```

M26 adds no automatic build or activation after promotion, drift scheduler,
durable repair job, background retry, LLM or prompt qualification, physical
per-generation query routing, generation deletion/GC, vector-index generation,
distributed writer, quorum, replication, or HA. See the
[`M26 decision`](../../docs/decisions/decision_m26_verified_semantic_repair_materialization.md)
for the complete contract and limits.
