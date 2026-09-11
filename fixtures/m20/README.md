# M20 artifact-attestation templates

These files document M20's closed public request shapes. They are authoring
templates, **not valid signed authority**. Every `REPLACE_WITH_...` value and
timestamp `1` must be replaced before signing. They contain no private key,
secret seed, credential, or usable signature.

An external tool must hash exact raw file octets, build a sorted canonical
manifest, and sign the complete
`cognigraph.artifact-attestation.v1` statement with a root-certified
`artifact_attestor` key. CogniGraph validates the manifest, signature,
tenant/incarnation, key lifecycle, natural identity, and later promotion
bindings. It never fetches a signed location and never accepts private key
material.

Manifest rules:

- use `sha256:<64 lowercase hex>` for each `blob_digest`, hashing the exact
  raw octets of that blob;
- use relative, NFC-normalized, slash-separated paths sorted ascending;
- reject duplicate, case-only alias, absolute, empty-segment, `.`/`..`, and
  backslash paths;
- set `entry_count` and `total_bytes` to the exact checked sums;
- compute `manifest_digest` over the NFC canonical JSON encoding of the
  complete manifest, not the original JSON formatting, a URI, an archive
  expansion, or a decompressed representation;
- locations are signed audit hints without userinfo, query strings, fragments,
  credentials, or authority over content identity.

The attestation id is the canonical digest, without the `sha256:` prefix, of:

```json
{
  "tenant": "...",
  "tenant_incarnation": "...",
  "artifact_kind": "corpus|graph|oracle|scorer|verifier",
  "manifest_digest": "sha256:...",
  "subject_digest": "sha256:...",
  "attestor_registration_id": "..."
}
```

The subject digest covers the exact closed kind-specific `subject`. Scorer and
verifier `semantics_identity_digest` values retain M18/M19's meaning; they are
not executable hashes. The adjacent M20 manifest is the new content address.

Suggested flow:

1. Create an `artifact-attestor` tenant user.
2. Register its `artifact_attestor` key using
   `cognigraph.key-registration.v2`; the external root and subject key both
   sign the exact statement.
3. Produce and submit one signed attestation for each of corpus, graph, oracle,
   scorer, and verifier with a unique `Idempotency-Key`.
4. Submit `artifact-bindings.resolve.json` to
   `POST /api/governance/artifact-bindings/resolve`.
5. Copy the returned set, unchanged, into `artifact_attestations` in a
   `PromotionContext` v3. Use a fresh target channel; v1/v2 authority is
   historical and is never retro-attested.

The current evaluator still reads the live tenant graph and obtains oracle
facts from the resolved `EvalSpec`. M20 authenticates an attestor's exact-byte
claim; it does not prove retrieval, availability, custody, freshness, semantic
truth, or that those bytes were consumed.
