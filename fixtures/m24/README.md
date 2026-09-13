# M24 artifact recovery fixture guide

M24 fixtures describe unsigned operational recovery metadata. They contain no
private key, signature authority, raw production document, or valid production
backup.

The server derives `cognigraph.artifact-recovery-plan.v1` from an existing
immutable M21-M23 evidence record. Clients do not author or submit a plan. The
formatted reference objects are:

- [`recovery-plan.json`](recovery-plan.json), modeling shared
  corpus/oracle/scorer/verifier authority plus distinct candidate/baseline
  graph attestations and binding two five-byte fixture blobs (`hello` and
  `world`);
- [`backup-receipt.json`](backup-receipt.json), bound to that plan; and
- [`restore-receipt.json`](restore-receipt.json), bound to the plan and backup
  receipt.

Their typed digests are valid and covered by the Rust test suite. The files
are formatted for review; bundle metadata is the same object serialized as
compact canonical NFC JSON with no trailing whitespace. They are examples,
not server-authoring inputs or evidence of a completed backup.

A valid plan has these properties:

1. `tenant`, `tenant_incarnation`, and `tenant_scope_digest` identify exactly
   one scope. The scope is SHA-256 over the tenant UTF-8 bytes, one NUL byte,
   and the incarnation UTF-8 bytes.
2. `evidence_id`, `evidence_digest`, `evidence_schema_version`,
   `artifact_authority_digest`, and both artifact-set digests reproduce stored
   M21-M23 authority.
3. `attestations` is sorted by artifact kind and id and contains each distinct
   candidate/baseline attestation once.
4. `blobs` is sorted by lowercase `sha256:<64 hex>` digest and contains each
   digest/length address once. The same digest with another length is invalid.
5. `manifest_entry_count`, `blob_count`, and `total_bytes` are checked exact
   counts/sums. One plan admits at most 20,000 manifest entries, 8,192 unique
   blobs, and the configured custody-byte bound.
6. `plan_digest` is the SHA-256 content address of the canonical object with
   the `plan_digest` field omitted.

The portable bundle is a closed tree:

```text
BUNDLE/
  custody.json
  backup-receipt.json
  blobs/
    sha256/
      ab/
        <remaining-62-lowercase-hex>
```

No other entry is accepted. Filesystem paths come only from validated scope
and content digests—not manifest logical paths or signed location URIs.
Symlinks, non-regular blobs, non-canonical metadata, missing or extra bytes,
wrong lengths, and wrong digests fail verification.

Restoration is intentionally offline and whole-scope. The target CAS root and
normal `tenants` directory already exist; the derived tenant-incarnation scope
must be absent. The CLI validates the complete bundle against an independently
pinned plan digest, tenant, and incarnation, builds and rereads a sibling
staging scope, publishes without replacing an existing target, rereads the
normal CAS path, and then emits a restore receipt outside both the CAS and the
closed source bundle.

A pre-publication failure leaves the final path absent. If parent sync or the
final reread fails after the atomic rename, the command says that the path was
published but is not verified durable. The path is deliberately retained and
must be inspected or verified; no retry may overwrite it.

Atomic directory publication is currently implemented for Linux/Android and
Apple platforms. Windows and other unsupported targets fail closed before
publication.

Backup and restore receipts are deterministic unkeyed operation observations
with no trusted timestamp. They do not prove continuing custody, freshness,
ownership, independent media, replication, encryption, availability, RPO/RTO,
quorum, or HA. A fresh M21-M23 evaluation after restore remains the end-to-end
signed-authority and consumption check.

See the
[M24 decision](../../docs/decisions/decision_m24_durable_cas_custody_verified_restoration.md)
and [operations runbook](../../docs/operations.md#m24-artifact-custody-and-verified-restoration).
