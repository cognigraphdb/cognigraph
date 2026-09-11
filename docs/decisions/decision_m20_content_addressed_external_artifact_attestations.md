# Decision: M20 content-addressed external artifact attestations

**Status:** Implemented and verified (2026-07-18).

**Successor:** [M21 verified artifact consumption](decision_m21_verified_artifact_consumption.md)
adds an opt-in local-CAS consumption contract for a fresh context-v4 target.
This record remains the exact historical contract for M20 context-v3
authority; M21 does not rewrite or retroactively strengthen it.

## Context

M18 freezes candidate, corpus, graph, oracle, scorer, and verifier identities in
promotion evaluation contexts. M19 authenticates the policy author, an
independent approver, and the promoter. Those records make the internal
promotion history attributable and consistent, but their external artifact
fields are still operator assertions. In particular, the M18 scorer and
verifier artifact digests identify fixed semantics strings; they are not hashes
of fetched binaries.

M20 adds a portable, signed claim about the exact bytes that an external
artifact attestor observed for the corpus, graph, oracle, scorer, and verifier
inputs. It does not move artifact custody or byte transfer into CogniGraph. The
server validates canonical manifests, signatures, key authority, and exact
promotion bindings, but never dereferences a signed location or independently
rehashes the remote object.

This distinction is essential because the evaluator still reads the tenant's
live graph through `GraphBackend`. A valid M20 record proves that an authorized
attestor signed a manifest and that promotion authority names that manifest. It
does not prove that the evaluator consumed those bytes, that a location remains
available, that an artifact is semantically correct, or that a complete valid
history is fresh.

## Decision

### D1. Attest exactly five external promotion input classes

M20 defines a closed `ArtifactKind` enum:

```text
corpus
graph
oracle
scorer
verifier
```

One immutable attestation covers exactly one kind. A promotable
`PromotionContext` schema version 3 contains one exact binding for each of the
five kinds. Candidate artifacts, case manifests, and reproducibility records
retain their existing M18 identities; M20 does not silently expand its trust
claim to every URI in a context.

### D2. Content-address a canonical manifest of exact byte streams

The attestor constructs the manifest outside CogniGraph after hashing the
bytes. Manifest schema version 1 contains:

```text
schema_version = 1
artifact_kind
artifact_format
entries[] {
  logical_path
  media_type
  byte_length
  blob_digest
  executable
}
entry_count
total_bytes
```

Each `blob_digest` is `sha256:<64 lowercase hexadecimal characters>`. Entries
are sorted by unique NFC-normalized, relative, slash-separated logical path.
Absolute paths, traversal segments, backslashes, control characters, duplicate
paths, and case-only aliases are rejected. Declared counts and the checked sum
of byte lengths must match. The manifest itself is encoded with the existing
integer-only, NFC-normalized, lexicographically ordered canonical JSON rules;
its SHA-256 digest is the content address carried through the authority chain.

The foundation bounds a manifest to 100,000 entries, 1 TiB of declared bytes,
and 16 MiB of canonical JSON. A tenant incarnation may retain at most 64 MiB of
canonical manifest JSON in aggregate. These are validation, admission, and
recovery limits, not an artifact-upload service. CogniGraph stores the manifest
records and digests, not the referenced byte streams. List reads return at most
50 compact summaries and load each selected full record only long enough to
validate it; the single-record route exposes the complete signed record.

### D3. Treat signed locations as observations, not authority

An attestation contains one to eight location observations. Each observation
binds an absolute URI, observation timestamp, and optional object version,
ETag, last-modified value, declared length, and content encoding. URIs with
userinfo, query strings, fragments, whitespace, or embedded credentials are
rejected.

Locations remain signed audit hints. The manifest digest, not a URI, names the
claimed bytes. CogniGraph does not fetch a location during attestation,
evaluation, evidence registration, promotion, recovery, or snapshot import.
Location availability, authorization, retention, replication, and transport
integrity remain external operational responsibilities.

### D4. Authenticate claims with a separate Artifact Attestor duty

M20 adds the tenant-local `artifact-attestor` role, `artifact-attest` scope,
and root-certified `artifact_attestor` key purpose. An Admin may submit the
externally root-signed public-key registration, but only the matching live
Artifact Attestor user may submit an attestation signed by that key. Product
HTTP and CLI surfaces accept public registrations and pre-signed JSON only;
they do not accept, generate, escrow, or recover private keys.

Artifact Attestor registrations use the domain
`cognigraph.key-registration.v2`. The attestation itself uses the
`cognigraph.artifact-attestation.v1` domain and binds the tenant, tenant
incarnation, manifest, exact usage subject, signed location observations,
attestor registration and stable principal, hash-completion time, and signing
time. The natural attestation id is derived from tenant/incarnation, artifact
kind, manifest digest, subject digest, and attestor registration id.

An exact idempotent replay returns the existing immutable record. Reuse of an
identity or idempotency key with different authority conflicts. The protected
`_cognigraph_artifact_attestations` collection is capped at 10,000 records per
tenant incarnation and 64 MiB of aggregate canonical manifest JSON.
Prospective key revocation prevents new use while preserving valid
pre-revocation history and exact idempotent replays.

### D5. Bind each byte claim to its exact promotion use

A manifest alone could be replayed into an unrelated evaluation. M20 therefore
signs a closed, kind-specific usage subject:

- `corpus` binds the exact corpus revision and preprocessing digest;
- `graph` binds the exact graph revision, candidate digest, construction
  configuration digest, corpus manifest digest, and preprocessing digest;
- `oracle` binds the exact oracle revision, EvalSpec digest, case-manifest
  digest, and corpus manifest digest;
- `scorer` binds scorer id/version, the existing semantics identity digest,
  metric-semantics version, and reproducibility executable digest;
- `verifier` binds verifier name/version and the existing semantics identity
  digest.

The scorer and verifier semantics identities retain their M18 meaning. M20
does not reinterpret them as binary hashes. Their signed usage subjects point
to separate content-addressed manifests so the authority chain preserves both
the declared semantics identity and the newly attested exact-byte claim.

### D6. Freeze an exact five-slot set in `PromotionContext` v3

The binding resolver loads five active attestations, checks that every id is in
the correct kind slot, and returns the complete immutable bindings plus a
canonical `set_digest`. `PromotionContext` version 3 requires both the M19
signed policy/approval binding and this exact artifact-attestation set. Job
submission verifies all records, signatures, active key lifecycles, binding
digests, and kind-specific usage subjects before freezing the context.

Candidate original and replay must carry the same complete set. Baseline
original and replay must carry another identical complete set. Candidate and
baseline must share corpus, oracle, scorer, and verifier bindings. Their graph
bindings may differ only when the frozen policy explicitly allows
`graph_revision`, and then the graph usage subjects must differ. All existing
M18 replay, comparability, denominator, recall, restraint, regression, and
oracle-separation gates remain independent and mandatory.

Evidence schema version 3 copies both candidate and baseline sets and computes
one artifact-authority digest. The signed promote, reject, or rollback intent
binds that exact digest. Artifact-attestor principals must be distinct from the
policy author, policy approver, and promoter. A single Artifact Attestor may
attest more than one of the five inputs, but one stable principal cannot cross
governance purposes through key rotation.

### D7. Keep authority immutable, protected, and recoverable

Status, reconciliation, full recovery, and Native snapshot preflight validate
the stored-plus-incoming union of public key registrations, revocations,
policies, approvals, artifact attestations, promotion contexts/evidence,
signed intents, decisions, and derived heads. Missing records, divergent
immutable content, invalid signatures, cross-purpose principals, inactive keys,
or mismatched subject/binding digests fail closed and fence new mutations.

Native snapshot export/import includes the signed attestation records and
canonical manifests. It never includes the external corpus, graph, oracle,
scorer, or verifier bytes. The externally configured root remains outside the
snapshot, and derived heads are rebuilt from validated decisions. The ArangoDB
maintenance backend uses the same protected-record semantics but has no
CogniGraph application snapshot support.

### D8. Preserve M18/M19 history and require a fresh M20 target

Promotion context/evidence/decision schema versions 1 and 2 remain readable
and recoverable with their original meanings; M20 never fabricates artifact
attestations for them or rewrites their signed bytes. A target channel may
contain exactly one authority generation. Version-3 evidence cannot extend a
target that already contains version-1 or version-2 evidence, and a version-3
baseline cannot point across that generation boundary. Operators must start a
fresh `{space_type, channel}` target for normal M20 adoption.

Existing rollback rules remain exact and chain-local. Compatibility does not
permit retroactive attestation, cross-generation promotion, or treating old
metadata-only evidence as if it carried M20 byte claims.

### D9. Preserve the selection-only, singleton execution boundary

M20 changes authority records, not evaluator execution. `construct.evaluate`
still queries the live tenant graph; CogniGraph does not materialize a graph
from the attested graph manifest or compare a portable graph revision at read
time. A passing decision still changes only the tenant-local control-plane
head under the process-local transition lock.

M20 does not deploy binaries, rebuild graphs, route traffic, switch consumers,
provide a distributed lease, add quorum, or add high availability. A separate
deployment system must deliberately consume the selected head and independently
enforce any runtime artifact-loading policy.

## Acceptance matrix

| Area | Acceptance contract | M20 status |
|---|---|---|
| Manifest | Closed canonical manifest with exact entry digests, counts, byte lengths, path safety, and bounded size | Verified |
| Identity | Natural tenant/incarnation/kind/manifest/subject/attestor identity with immutable idempotency | Verified |
| Signature | Active root-certified Artifact Attestor signs the exact domain-separated claim; product surfaces accept no private key | Verified |
| Context | Version 3 requires exact policy authority and all five exact subject-bound artifact attestations | Verified |
| Evidence | Original/replay and candidate/baseline comparison rules are enforced; evidence and signed intent bind the artifact-authority digest | Verified |
| Separation | Artifact-attestor principals cannot author/approve policy or promote the evidence they attest | Verified |
| Recovery | Full authority validation and Native stored-plus-incoming snapshot preflight include attestation records and bindings | Verified |
| Compatibility | V1/v2 history remains readable; v3 uses a fresh target and never retro-attests old evidence | Verified |
| Boundary | No fetch, byte storage, consumption proof, availability, semantic truth, custody, freshness, deployment, HA, or quorum claim | Explicitly preserved |

## Explicit non-goals and limits

- CogniGraph does not download, upload, mirror, execute, or independently hash
  the external artifacts named by an attestation.
- A valid signature proves who made a byte-manifest claim; it does not prove
  that the claim is truthful or that the bytes are safe, complete, useful, or
  semantically correct.
- M20 does not prove that `construct.evaluate` consumed the attested graph or
  other inputs. The evaluator still reads the live tenant graph.
- Signed location observations do not prove current availability, freshness,
  retention, ownership, or custody.
- Native snapshots contain authority records, manifests, and digests, not the
  external artifact bytes. They are not signed freshness checkpoints.
- ArangoDB gains no application snapshot, transaction, deployment, or
  backend-specific artifact semantics.
- There is no transparency log, witness quorum, remote attestation, HSM/KMS
  integration, automatic artifact synchronization, or runtime loader in this
  foundation.

## Verification evidence

The final workspace state passed the exact repository gates on 2026-07-18:

```sh
cargo build --release --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
```

The automated full-lifecycle regression stores root-certified M19/M20
authority, six signed
attestations (distinct candidate and baseline graph manifests), four real
evaluation jobs, v3 evidence, a signed v3 promotion, the derived head, and
recovery. It additionally proves that an exact historical decision replay
survives later Artifact Attestor revocation while a fresh reject using that
evidence is forbidden. Admission rejects a signature timestamp before its key
window, URI and manifest adversarial tests fail closed, list pagination returns
summary-only keyset pages, and Native snapshot preflight validates the complete
stored-plus-incoming authority union.

Authenticated release-binary HTTP probes exercised the public artifact
authority lifecycle against both supported storage modes:

- Persistent Native storage accepted a v2 root-certified Artifact Attestor key
  and one signed attestation for each of corpus, graph, oracle, scorer, and
  verifier. Exact five-slot resolution and status/recovery were healthy;
  wrong-slot resolution, Admin submission, private-key-shaped input, protected
  collection access, and invalid authority were rejected. All five records
  survived restart. Revocation then blocked fresh resolution/use, preserved an
  exact historical replay, and remained healthy through another recovery.
- The configured live ArangoDB 3.12.9-1 endpoint authenticated successfully.
  The same five-kind lifecycle returned the expected HTTP outcomes: registration
  `201`, exact replay `200`, tampered registration `403`, immutable conflict
  `409`, exact resolution `200`, wrong slot `400`, Admin submission `403`,
  private field `422`, protected access `403`, and recovery `200`. A padded
  3 MiB replay succeeded while a request above the route-local 17 MiB bound
  returned `413`. Five records and the same set digest survived restart;
  post-revocation historical replay remained `200`, fresh resolution was
  `403`, and recovery stayed healthy after a second restart.
- The Arango probe stopped the server before deleting only its exact recorded
  keys. Cleanup removed five attestations, one revocation, one key, and two
  probe users; every relevant collection returned to its zero-row baseline and
  the probe port was released. No collection was truncated or dropped.

The real-HTTP probes validate the shipped API, persistence, restart, lifecycle,
and backend behavior. The stored full-v3 integration regression validates the
job/evidence/promotion chain. Neither layer fetches external artifacts or turns
the attestor's signed byte claim into proof of evaluator consumption, custody,
availability, semantic truth, or freshness.
