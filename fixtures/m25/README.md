# M25 governed Semantic Repair request templates

These files document the closed request shapes for the implemented M25
authority. They are authoring templates, **not valid signed authority** or
release evidence, and do not prove that another deployment has passed the
required live lifecycle.

- `semantic-revision.request.json` embeds one exact unchanged M22 schema-v1
  typed `candidate.json` in a `PolicyAuthor`-signed statement.
- `semantic-review.request.json` independently approves that exact revision
  and candidate digest with a `PolicyApprover` signature. To reject instead,
  change `decision` and `reason` before computing any digest or signature.

Every `REPLACE_WITH_...` value is deliberately nonfunctional. Epoch-
millisecond values shown as `1` are placeholders. No private key, secret seed,
credential, or production signature belongs in this directory, `.env`, an
HTTP request, a snapshot, or server configuration.

The candidate nested in the revision is the existing M22
`ConstructionCandidateArtifact` schema version 1. Do not add an M25-only field
to it. Its entities, relation rules, aliases, triggers, evidence strings, and
accepted neurons must satisfy the existing M22 bounds and canonical ordering.
Only alias, relation-hint, and relation-blocker neurons are admitted by that
construction contract.

The server recomputes the candidate's canonical SHA-256 digest. Signers must
use the NFC-normalized, lexicographically ordered, integer-only canonical JSON
and domain-separated Ed25519 signing protocol supplied by
`cognigraph-governance`; do not sign an ad hoc pretty-printed JSON string or a
bare digest. Verification keys and signatures use canonical unpadded
base64url. Ordinary canonical digests use
`sha256:<64 lowercase hexadecimal characters>`.

Both statements require `base_promotion_head_decision_id`. Use explicit JSON
`null` for a fresh target with no head; for an existing target, use the exact
64-character lowercase id returned as its current applied promotion decision.
Omitting the field is not equivalent to `null` and is rejected. The review must
copy the revision's exact base field.

Natural ids are lowercase SHA-256 hex without the `sha256:` prefix:

- `semantic_repair_revision_id`: the canonical digest of the integer-only JSON
  identity `{tenant, tenant_incarnation, target,
  base_promotion_head_decision_id, candidate_digest}`. Because the base is in
  the identity, the same candidate authored from another head has another id;
- `semantic_repair_review_id`: SHA-256 of
  `tenant NUL tenant_incarnation NUL semantic-repair-review NUL
  semantic_repair_revision_id`.

Approve and reject share the same review natural id. A revision therefore has
one immutable final decision; reconsideration means authoring a new revision,
not overwriting a review.

Authoring flow:

1. Start from exact canonical M22 candidate bytes that can already be placed
   beside `graph.json` in the M22/M23 graph artifact.
2. Obtain the exact tenant incarnation, active root-certified `policy_author`
   registration, and current promotion-head decision id for the target. Use
   explicit `null` only when the target has no head.
3. Recompute the candidate digest and revision natural id, replace every
   placeholder in `semantic-revision.request.json`, and sign its exact
   `cognigraph.semantic-repair-revision.v1` statement outside CogniGraph.
4. Submit the revision as the authenticated `policy-author` subject.
5. Use the accepted immutable revision record's
   `semantic_repair_revision_digest`, copy its exact target/base/candidate and
   author bindings, and use an independent `policy_approver` registration to complete and sign
   `semantic-review.request.json` under
   `cognigraph.semantic-repair-review.v1`.
6. Submit the review as the authenticated `policy-approver` subject, with the
   path id exactly equal to the signed `semantic_repair_revision_id`.
7. Run the unchanged M23 evaluation/promotion workflow. Approval alone is
   inert: governed resolution succeeds only when the existing promotion head
   selects the exact approved candidate digest for the same target.

M25 adds no separate semantic head, durable repair job, LLM qualification,
artifact kind, CAS delivery path, or graph-generation deployment switch. See
[`docs/decisions/decision_m25_governed_semantic_repair_authority.md`](../../docs/decisions/decision_m25_governed_semantic_repair_authority.md)
for the complete implemented and verified boundary.
