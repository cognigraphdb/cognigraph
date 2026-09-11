# M19 signed-governance request templates

These JSON files document the closed request shapes for M19. They are
authoring templates, **not valid signed authority**:

- every `REPLACE_WITH_...` value is deliberately nonfunctional;
- every epoch-millisecond value shown as `1` is also a placeholder and must be
  replaced with the intended current/valid time;
- no private key, secret seed, credential, or production signature is present;
- do not add private material here, to `.env`, to an HTTP request, or to a
  snapshot;
- signers must obtain the exact tenant incarnation, compute the server-defined
  natural ids and canonical digests, and sign the unmodified statement outside
  CogniGraph.

The canonical format is NFC-normalized, lexicographically ordered,
integer-only JSON. Signatures are Ed25519 over the M19 domain-separated bytes,
not over an ad hoc JSON rendering or bare digest. Verification keys and
signatures use canonical unpadded base64url. Key ids use
`ed25519:sha256:<64 lowercase hex>`; ordinary canonical digests use
`sha256:<64 lowercase hex>`.

Natural ids are deterministic lowercase SHA-256 hex without the `sha256:`
prefix:

- registration: SHA-256 of the UTF-8 byte sequence
  `tenant NUL tenant_incarnation NUL key-registration NUL key_id`;
- revocation: SHA-256 of
  `tenant NUL tenant_incarnation NUL key-revocation NUL registration_id`;
- approval: SHA-256 of
  `tenant NUL tenant_incarnation NUL policy-approval NUL policy_revision_id`;
- policy revision: the canonical digest of
  `{tenant, tenant_incarnation, target, policy_id, policy_revision}`, using the
  values from the resolved policy, with its `sha256:` prefix removed.

The statement digest is the canonical statement digest, while the Ed25519
signature is produced over the governance protocol's prefixed signing bytes.
Use `cognigraph-governance` rather than duplicating that protocol. The
promotion intent's `idempotency_key_hash` is the ordinary SHA-256 digest of the
exact `Idempotency-Key` header bytes. Policy/registration/approval record
digests needed by later statements come from the accepted server records; do
not predict server-enriched record digests.

All `signed_at_ms` values must be positive and no more than five minutes ahead
of server time. Registration `not_before_ms` has the same future-skew bound,
and `not_after_ms`, when present, must be later than both `not_before_ms` and
server admission time. Revocation `effective_at_ms` must be within five minutes
of server time; the foundation does not accept a backdated revocation that
reinterprets committed history.

Suggested flow:

1. Configure only the external root **public** key as
   `COGNIGRAPH_GOVERNANCE_ROOT_PUBLIC_KEY`.
2. Create separate tenant users with roles `policy-author`,
   `policy-approver`, and `promoter`.
3. Copy `key-registration.request.json` for each user/purpose. The external
   root and the subject key both sign the exact registration statement. An
   Admin submits the resulting public request.
4. The policy author submits `policy-revision.request.json`; the independent
   approver submits `policy-approval.request.json`.
5. Resolve the accepted binding with
   `GET /api/governance/bindings/{approval_id}`. Put that exact object in the
   `governance` field shown by `promotion-context-v2.binding.json`, alongside
   the unchanged M18 context fields.
6. After four successful v2 evaluation jobs and evidence registration, the
   promoter submits `promotion-intent.request.json` with an idempotency header
   whose SHA-256 hash exactly matches the signed payload.
7. Use `key-revocation.request.json` for a prospective root-signed revocation.
   Audit accepted revocations through `GET /api/governance/revocations` and
   `GET /api/governance/revocations/{revocation_id}`; revocation history is
   immutable and readable, never a write-only control.

See `docs/operations.md` and
`docs/decisions/decision_m19_signed_governance_separation_of_duties.md` for the
role, lifecycle, compatibility, and non-deployment boundaries.
