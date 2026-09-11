# Decision: M24 durable CAS custody and verified restoration

**Status:** Implemented and verified (2026-07-19).

## Context

M20 authenticates exact artifact manifests and their content addresses. M21
proves that one evaluation read bytes matching those addresses. M22 and M23
reproduce the prepared-corpus-to-graph and raw-document-to-prepared-corpus
functions. None of those milestones retains the artifact bytes in durable job,
promotion, or database-snapshot records.

That boundary is deliberate but leaves a recovery gap. A Native snapshot can
restore the signed authority and compact receipts while the tenant-incarnation
CAS bytes are absent. ArangoDB has no CogniGraph application-snapshot surface
at all. In either case, a fresh M21-M23 evaluation cannot run until an operator
restores every byte named by its artifact authority.

M24 closes this operational gap without claiming that a point-in-time copy is
continuing custody, current availability, independent replication, or backup
freshness.

## Decision

### D1. Preserve every M18-M23 promotion authority generation

M24 adds no promotion context, job, evidence, decision, head, consumption,
derivation, preparation, or signed-intent generation. The current M23 matrix
remains context/evidence/decision version 6, evaluation-job version 4,
consumption plan/receipt version 3, derivation receipt version 2, preparation
receipt version 1, head version 4, and `cognigraph.promotion-intent.v4`.

Custody and restore observations are operational evidence. They do not change
evaluation quality or authorize selection, rejection, rollback, or deployment.
All historical M18-M23 JSON bytes, record digests, and signatures therefore
retain their exact meanings.

### D2. Derive one deterministic recovery plan from immutable evidence

An authenticated tenant Admin may request:

```text
GET /api/admin/artifact-custody/evidence/{evidence_id}
```

The server accepts only M21-M23 evidence because earlier generations do not
bind verified-consumption artifact sets. It revalidates the immutable evidence
and every historically valid M20 artifact attestation referenced by the
candidate and baseline sets. A later prospective Artifact-Attestor revocation
does not erase valid historical authority merely because an operator needs to
preserve its bytes.

The returned `cognigraph.artifact-recovery-plan.v1` object binds:

- tenant and tenant incarnation plus the SHA-256 tenant-scope address;
- evidence id, evidence digest, and evidence schema generation;
- artifact-authority, candidate-set, and baseline-set digests;
- sorted distinct attestation projections containing kind, id, attestation
  digest, and manifest digest;
- a sorted, deduplicated `{blob_digest, byte_length}` inventory;
- checked manifest-entry, unique-blob, and total-byte counts; and
- a canonical plan digest over every field except the digest itself.

The plan is timestamp-free and actor-free, so two reads of unchanged authority
produce identical canonical bytes and the same digest. Manifest logical paths
and signed location observations never become filesystem inputs.

### D3. Bound one recovery unit

One plan admits at most 8,192 unique blobs: the closed union of the existing
4,096-blob candidate and baseline consumption bounds. Total distinct bytes may
not exceed `COGNIGRAPH_ARTIFACT_MAX_CUSTODY_BYTES` (2 GiB by default). Counts
and sums use checked arithmetic. The plan rejects a digest observed with two
different declared lengths rather than guessing which claim to preserve.

These are admission and I/O bounds, not CPU, wall-time, memory, RPO, or RTO
guarantees.

### D4. Keep the online server CAS read-only

The HTTP API returns metadata only. It does not upload, copy, delete, restore,
or repair artifact bytes. Bundle creation and restoration are explicit local
operator CLI workflows on the storage host:

```text
cognigraph artifact custody create EVIDENCE --cas-root CAS --out BUNDLE
cognigraph artifact custody verify BUNDLE \
  --expected-plan-digest DIGEST --tenant TENANT --incarnation INCARNATION
cognigraph artifact custody restore BUNDLE --cas-root CAS \
  --expected-plan-digest DIGEST --tenant TENANT --incarnation INCARNATION \
  --receipt RESTORE.json
```

`create` fetches the authenticated plan, then reads only the tenant-scoped
content addresses in that plan. `verify` and `restore` require an out-of-band
pinned plan digest, tenant, and incarnation. A bundle cannot remap one tenant
incarnation into another.

### D5. Use one closed, portable content-addressed bundle

A bundle contains canonical `custody.json`, canonical
`backup-receipt.json`, and exact blobs under:

```text
blobs/sha256/<first-two-lowercase-hex>/<remaining-62-lowercase-hex>
```

Unknown top-level entries, symlinks, non-regular blobs, unsafe path
components, non-canonical JSON, wrong counts, wrong lengths, wrong digests,
and bytes outside the plan fail verification. Logical artifact paths,
location URIs, object-store metadata, and executable flags never determine a
host path or file mode.

Bundle creation uses an absent sibling staging directory, streams and hashes
each source copy with bounded buffers, rereads every destination blob, writes
the receipt and plan only after the data is complete, synchronizes files and
directories, and publishes without replacing an existing destination. A
failure before publication removes the owned partial stage and leaves the
requested path absent. After the no-replace rename succeeds, a parent-sync or
final-reread failure is instead reported explicitly as published but not
verified durable. The final path remains present and is never removed or
replaced automatically; the operator must inspect or verify that exact path
and must not blindly retry by overwriting it.

Directory publication is enabled only where the implementation has a genuine
atomic no-replace rename primitive (currently Linux/Android and Apple
platforms). Windows and other unsupported targets fail closed before
publication rather than using replacement-capable rename semantics.

### D6. Restore an absent scope as a complete unit

Restore first verifies the complete closed bundle and its pinned identity. The
target CAS root and normal non-symlink `tenants` directory must already exist,
but the derived tenant-incarnation scope must be absent. M24 never merges into,
repairs, or overwrites an existing scope.

The CLI reconstructs the complete `sha256` tree in an absent sibling staging
directory, rehashes that staged tree, synchronizes it, and publishes the scope
with no-replace semantics. It then rereads every blob through the normal CAS
verifier. This commit-last design prevents a normal validation or copy failure
from exposing a partial final scope. It is idempotent only in the safe sense:
an existing final scope is rejected and may be verified explicitly; it is
never silently replaced.

A failure before scope publication leaves the final scope absent. A failure
to sync the parent or reread the normal CAS path after publication reports
that the scope exists but is not verified durable. It is not rolled back or
merged; resolve the storage fault and inspect the exact scope before any
further recovery action.

### D7. Emit unkeyed operation receipts

The backup receipt binds the plan digest, exact scope, verified counts and
bytes, and its own canonical digest. Restore emits a separate receipt only
after the published target passes a complete reread. Both receipts are
deterministic and intentionally contain no mutable timestamp claim; the
operator must preserve and, when required, externally timestamp/sign the
restore receipt outside both the restored CAS and source bundle. Writing it
inside the closed bundle would invalidate later bundle verification.

These server/CLI observations are not signatures, independent witnesses,
trusted timestamps, or remote attestations. They record only that the operation
returned after successfully reading the named bytes. They do not
prove who originally supplied a backup, whether another custodian holds it,
whether it is the newest backup, or whether the bytes remain available later.

### D8. Compose artifact recovery with database recovery

M24 does not replace backend backup:

- Native recovery combines a hot JSON snapshot or tested cold redb copy, the
  matching artifact bundle, and separately retained configuration, trust
  anchor, and secrets.
- ArangoDB recovery combines operator-managed `arangodump`/`arangorestore` or
  platform backup, the matching artifact bundle, and the same external
  configuration material. CogniGraph `/api/admin/export` and `/import` are not
  presented as an ArangoDB backup path.

After restoration, a fresh M21-M23 evaluation remains the authoritative
end-to-end check: it revalidates active signed authority and rehashes every
consumed byte before emitting a new receipt. Restoring a bundle does not mint
promotion evidence or change a selected head.

### D9. Keep the non-goals explicit

M24 does not fetch signed locations, provide an upload API, synchronize CAS
roots, schedule backups, choose retention, prune bundles, encrypt transport or
media, manage encryption keys, sign receipts, provide object-store adapters,
prove extraction or semantic truth, deploy a selected graph, guarantee
freshness or availability, replicate bytes, coordinate multiple writers, or
add quorum, consensus, or high availability.

The online server remains single-writer scoped. Operators are responsible for
placing bundles on independent durable media and testing them at the frequency
their recovery objectives require.

## Acceptance evidence

Final acceptance requires the exact Rust formatting, Clippy, and workspace
test gates plus authenticated release-binary lifecycles on isolated persistent
Native storage and the configured live ArangoDB. Each lifecycle must prove:

1. one real M23 evidence record yields the same recovery plan twice;
2. a non-Admin cannot read the plan;
3. bundle creation and verification bind the expected digest and scope;
4. tampered bytes and a wrong tenant/incarnation fail before publication;
5. authority restored without CAS bytes cannot rerun the evaluation;
6. the bundle restores the exact absent scope and emits a verified receipt;
7. a fresh M23 evaluation succeeds from unchanged signed authority; and
8. cleanup removes only isolated probe records and filesystem paths.

## Acceptance matrix

| Area | Acceptance contract | Current status |
|---|---|---|
| Recovery plan | Historical M21-M23 authority, exact candidate/baseline union, deterministic bounded metadata, no location dereference | Implemented |
| Bundle | Closed content-addressed tree, copied-byte rehash, final reread, deterministic backup receipt | Implemented |
| Verification | Independently pinned plan digest, tenant, incarnation, strict canonical metadata and closed filesystem tree | Implemented |
| Restoration | Absent scope only, complete sibling stage, no-replace publication, final normal-CAS reread, external receipt | Implemented |
| Compatibility | Every M18-M23 signed and durable wire retains its historical generation and meaning | Verified |
| Rust gates | Format, Clippy with warnings denied, and full workspace tests | Verified |
| Native live probe | Authenticated release binary, persistent redb authority, missing-CAS failure, restore, reevaluation, restart, cleanup | Verified |
| ArangoDB live probe | Authenticated release binary, live Enterprise 3.12.9-1 authority, missing-CAS failure, restore, reevaluation, restart, exact cleanup | Verified |
| Boundary | Operational point-in-time custody only; no online writes, remote fetch, ongoing proof, replication, RPO/RTO, or HA | Explicitly preserved |

## Verification evidence

The final repository validation completed successfully:

```text
PASS: cargo build --release --workspace --all-targets
PASS: cargo fmt --all -- --check
PASS: cargo clippy --all-targets -- -D warnings
PASS: cargo test --all
```

```text
PASS Native release-binary lifecycle:
  backend/storage: authenticated HTTP against persistent Native/redb authority
  fixture setup: 9 seconds
  full observed operator wall time: 462 seconds, including checkpoint pauses
    and correction of a harness-only base-URL input
  evidence: 1bcfd893fe7dae1f5c93441562ada2edbb6771217352aeb6ea0a625f49b7107b
  plan: sha256:57a66ccf5583301ec39d725d67037abb209db26092bb708b0b287d1a83488b97
  inventory: 6 attestations, 9 manifest entries, 8 unique blobs,
    31,256,466 distinct bytes
  auth/determinism: 401 without authentication; 403 as Viewer; repeated HTTP
    and CLI plan bytes identical
  backup receipt:
    sha256:89393f3fb6308991b88973fffcf35eabb647f1c2984b35417fdfdb41c7160662
  fail-closed: tampered bundle, wrong tenant, wrong incarnation, missing CAS,
    and wrong-scope restore; the wrong-scope restore published zero files
  restore receipt:
    sha256:66359302abf031e2211eda0f29986af13fde896eca4532698f87cdff67bf04c1
  post-restore: all 8 blobs recovered; fresh M23 job succeeded with job v4,
    consumption receipt v3, preparation receipt v1, recall 1, 0 violations;
    final restart returned the byte-identical plan and succeeded job
  cleanup: server stopped; ports released; all isolated Native paths removed

PASS ArangoDB release-binary lifecycle:
  server: ArangoDB Enterprise 3.12.9-1; configured credentials valid
  isolation boundary: disposable database creation was denied with 401, so the
    configured zero-row database was used with exact keyed cleanup
  fixture setup: 59 seconds
  full observed operator wall time: 752 seconds, including checkpoint pauses
  evidence: 0a92f7978f48906c3ab92daf615b61e9279d3f867d491a18c09fc7a3d8536b71
  plan: sha256:d30a31f906c50d5d4f4553d5f38e8efa72f8988089286fb3fe7be5aad85bdd58
  inventory/auth/determinism: same 6/9/8/31,256,466 inventory; 401 without
    authentication; 403 as Viewer; repeated HTTP and CLI plan bytes identical
  backup receipt:
    sha256:6b513cf5f466995e1d71b95f64e8e5fe32dc5409c654482d663c32e37e799013
  fail-closed: tampered bundle, wrong tenant, wrong incarnation, missing CAS,
    and wrong-scope restore; the wrong-scope restore published zero files
  restore receipt:
    sha256:71928c12482d28c3028d6b3a407475c017e3a244c00c838f3b3bf229b01a1ecd
  post-restore: all 8 blobs recovered; fresh M23 job succeeded with job v4,
    consumption receipt v3, preparation receipt v1, recall 1, 0 violations;
    final restart returned the byte-identical plan and succeeded job
  exact cleanup: deleted only 28 probe keys; touched 0 pre-existing records;
    all 27 baseline collections retained their exact names with 0 rows; server,
    ports, CAS paths, bundle paths, and temporary probe source removed
```

The observed wall times include deliberate interactive checkpoints and are not
throughput, RPO, or RTO benchmarks.
