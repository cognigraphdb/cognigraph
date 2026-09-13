# Artifact Custody

### M24 artifact custody and verified restoration

M24 preserves every M18-M23 authority generation. It derives a deterministic
recovery plan from one immutable M21-M23 evidence record and provides offline
bundle/restore tooling; it does not make backup state part of promotion.

Fetch or save the plan as a live tenant Admin:

```sh
cognigraph artifact custody plan EVIDENCE_ID --out custody-plan.json
```

The underlying route is
`GET /api/admin/artifact-custody/evidence/{evidence_id}`. It validates the
evidence, its exact candidate and baseline artifact sets, and the historically
valid M20 attestations. The timestamp-free plan contains only fixed authority
projections and a sorted deduplicated digest/length inventory. It never
dereferences an attested location or uses an artifact logical path as a host
path. A non-Admin, stale Admin identity, foreign tenant, pre-M21 evidence, or
altered authority fails closed.

On the storage host, create and independently verify a closed bundle:

```sh
cognigraph artifact custody create EVIDENCE_ID \
  --cas-root /var/lib/cognigraph/artifacts \
  --out /mnt/backup/cognigraph-evidence-EVIDENCE_ID

cognigraph artifact custody verify /mnt/backup/cognigraph-evidence-EVIDENCE_ID \
  --expected-plan-digest sha256:... \
  --tenant acme --incarnation INCARNATION
```

Pin the plan digest and scope outside the bundle, for example in the database
backup manifest. `create` streams exact source addresses into an absent
sibling staging directory, verifies both source and copied bytes, writes the
canonical plan and backup receipt, synchronizes the tree, and publishes
without replacing an existing path. `verify` rejects unknown entries,
symlinks, non-regular files, non-canonical metadata, wrong counts, lengths,
hashes, scope, or plan digest.

Stop the server before restoring the CAS. The target root and its normal
`tenants` directory must exist, while the derived tenant-incarnation scope must
not:

```sh
cognigraph artifact custody restore /mnt/backup/cognigraph-evidence-EVIDENCE_ID \
  --cas-root /var/lib/cognigraph/artifacts-restored \
  --expected-plan-digest sha256:... \
  --tenant acme --incarnation INCARNATION \
  --receipt /var/lib/cognigraph/restore-EVIDENCE_ID.json
```

Restore verifies the complete bundle before copying, builds a complete sibling
scope, rereads it, publishes it with no-replace semantics, then rereads the
normal CAS path before emitting the receipt. It never merges, repairs, or
overwrites an existing scope. Keep the receipt outside the CAS and run a fresh
M21-M23 evaluation after restart; also keep it outside the source bundle so
the closed bundle remains verifiable. That evaluation remains the
authoritative active-signature and end-to-end consumption check.

Before publication, a failure cleans the owned stage and leaves the requested
bundle or scope absent. After publication, a parent-directory sync or final
reread failure reports that the exact final path exists but is not verified
durable. CogniGraph does not remove or replace that path: fix the storage
fault, inspect/verify the exact path, and do not retry by overwriting it.

Bundle and scope publication currently requires the atomic directory
no-replace primitive implemented for Linux/Android and Apple platforms.
Windows and other unsupported targets fail closed before publication.

The deterministic backup/restore receipts are unkeyed operation observations
and contain no trusted or server-observed timestamp. They do
not prove that the bundle is the newest, independently stored, continuously
available, replicated, encrypted, or still intact. CogniGraph does not
schedule or prune backups, transport them, manage keys, synchronize CAS roots,
promise RPO/RTO, or add quorum/HA. See the
[M24 decision](../../decisions/decision_m24_durable_cas_custody_verified_restoration.md).
