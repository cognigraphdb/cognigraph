# Decision: M21 verified artifact consumption

**Status:** Implemented and verified (2026-07-18).

## Context

M20 authenticates purpose-bound, content-addressed manifests for the corpus,
graph, oracle, scorer, and verifier used by a promotion evaluation. It does not
read those bytes: signed locations remain audit observations and the M20
evaluator still queries the live tenant graph through `GraphBackend`.

M21 closes that consumption gap for a new promotion authority generation. An
operator stages the exact manifest blobs in a read-only local content-addressed
store scoped by tenant incarnation. CogniGraph verifies every declared entry,
reusing exact digest+length verification where safe, parses the verified graph
and oracle entrypoints, evaluates those immutable inputs instead of the live
backend, and records the execution in the terminal durable job result.

This is a local custody and verification contract, not general artifact
distribution. CogniGraph does not dereference signed locations, fetch a remote
object, accept an artifact upload, execute a staged binary, reconstruct a graph
from corpus documents, or remotely attest the host.

## Decision

### D1. Add an explicit, disabled-by-default local artifact source

M21 accepts exactly two `COGNIGRAPH_ARTIFACT_SOURCE` values:

```text
disabled
local-cas
```

`disabled` is the default and preserves M18-M20 operation. An M21 evaluation is
rejected at admission when no local CAS is configured. `local-cas` requires an
existing absolute `COGNIGRAPH_ARTIFACT_CAS_ROOT` and a positive
`COGNIGRAPH_ARTIFACT_MAX_EVALUATION_BYTES`; the default byte budget is 1 GiB.
The root and its `tenants` child must be normal non-symlink directories.
CogniGraph never creates, uploads, replaces, or deletes content below this
operator-managed boundary. The byte budget limits the cumulative declared
lengths charged for actual verification reads after safe deduplication; it is
not a peak-memory or CPU limit. Separate fixed caps permit at most 10,000
manifest entries and 4,096 unique `(digest, declared length)` pairs across the
five slots. Parsed graph objects can require substantially more memory than the
retained JSON bytes.

The blob path is derived only from tenant scope and a validated SHA-256 digest:

```text
<root>/tenants/<tenant-scope>/sha256/<first-2-hex>/<remaining-62-hex>
```

`tenant-scope` is lowercase hexadecimal SHA-256 over the tenant bytes, one NUL
separator, and the tenant-incarnation bytes. Signed logical paths and location
URIs are never filesystem inputs. Every directory in the fixed parent chain
and every blob must be an existing normal non-symlink object. The reader checks
and rechecks the parent chain and opened-file identity, checks the declared
length, and streams the complete file through SHA-256. These path-based checks
fail closed on observed replacement, truncation, growth, digest mismatch,
cross-tenant placement, and budget overflow. They are not an `openat2` or
dirfd-anchored defense against a hostile custodian racing filesystem operations;
the CAS root must remain operator-controlled and mounted read-only to the
CogniGraph process.

### D2. Pin one closed five-slot loader plan in `PromotionContext` v4

`PromotionContext` schema version 4 retains the complete M19 signed-governance
and M20 five-attestation authority, and adds `ArtifactConsumptionPlan` schema
version 1. The plan is not operator-extensible. Its resolver, loader identity,
loader semantics digest, scorer ABI digest, verifier ABI digest, formats,
entrypoints, and modes must equal the server's single supported plan:

| Kind | Required format | Entrypoint | Consumption mode |
|---|---|---|---|
| corpus | `cognigraph.corpus.v1` | none | account for and verify every manifest entry, with safe exact-blob reuse |
| graph | `cognigraph.evaluation-graph.v1` | `graph.json` | verify, retain, parse, and evaluate |
| oracle | `cognigraph.promotion-oracle.v1` | `oracle.json` | verify, retain, parse, and evaluate |
| scorer | `cognigraph.server-executable.v1` | `cognigraph-server` | verify against the executable-path digest pinned at server startup |
| verifier | `cognigraph.server-executable.v1` | `cognigraph-server` | verify against the executable-path digest pinned at server startup |

The exact current plan object, including its derived ABI and plan digests, is
checked in at
[`docs/examples/m21-consumption-plan.json`](../examples/m21-consumption-plan.json).
Changing any field is a loader-contract change and the server rejects it.

Graph and oracle manifests must each contain exactly their one non-executable
`application/json` entrypoint. Scorer and verifier manifests must each contain
exactly their one executable `application/octet-stream` entrypoint. The corpus
manifest may contain its complete sorted M20 entry set. The five signed
attestations remain subject-bound and active before consumption begins and are
checked again after the bytes have been consumed.

The five manifest entry counts are checked-summed and capped at 10,000 before
blob reads, and the verification cache accepts at most 4,096 unique `(SHA-256
digest, declared length)` pairs. Within an evaluation, a verified pair is
reused when the cache has the bytes required by the next consumer. A later
graph/oracle entrypoint that needs retained bytes may be read again if the
earlier occurrence was hash-only. Deduplication reduces I/O accounting; it does
not merge or weaken the per-manifest receipt bindings.

The worker races the complete asynchronous consumption, parsing, scoring,
final-authority check, and receipt construction against a fixed 300-second
deadline. It polls durable cancellation, tenant-suspension, and shutdown state
every second; deadline expiry fails the attempt, cancellation completes it as
canceled, and suspension or shutdown parks it for recovery. The guard is
cooperative process-local control, not per-file kill isolation and not
protection from a kernel filesystem call that itself does not return.

M21 uses `reproducibility.backend = "artifact-snapshot"`. The active Native or
ArangoDB backend remains responsible for durable job and governance records,
but it is not the source of facts scored by a version-4 evaluation.

### D3. Treat corpus verification as provenance, not reconstruction

CogniGraph streams, length-checks, and SHA-256-checks every corpus manifest
entry under the evaluation byte budget. It records the verified blob-set and
manifest identities, but it does not parse the corpus bytes, rerun
preprocessing or construction, or prove that the supplied graph was derived
from those documents.

The graph artifact itself binds its corpus manifest digest, candidate digest,
construction-configuration digest, and space type. Those references provide a
fail-closed consistency link to the frozen context; they are not a derivation
proof. Full reproducible graph reconstruction remains outside this milestone.

### D4. Score the verified graph and oracle bytes

`graph.json` uses a closed schema version 1 envelope:

```text
schema_version
space_type
corpus_manifest_digest
candidate_digest
construction_config_digest
facts[] { source, relation, target, evidence_chunk_id }
```

It must match the context, contain no more than 100,000 facts, and keep facts
sorted and unique. Every source, relation, target, and evidence-chunk string
must be non-empty NFC, control-free, and at most 1,024 bytes. M21 retains at
most 32 MiB for the graph entrypoint, parses it after byte verification, and
reduces its rows to the same distinct `{source, relation, target}` fact-set
semantics used by the existing evaluator. The live graph is not consulted for
the score.

`oracle.json` uses a closed schema version 1 envelope containing the case
manifest digest, corpus-manifest digest, and the actual `EvalSpec`. It must
match the frozen target, case manifest, corpus, effective EvalSpec digest, and
the resolved EvalSpec as an exact serialized value. Its space, question IDs,
question text, and fact strings must be non-empty NFC, control-free, and at
most 1,024 bytes, preventing a canonical-digest-equivalent Unicode substitution
from changing scoring semantics. M21 retains at most 4 MiB for this entrypoint.
The parsed `EvalSpec`, not an unverified alternate oracle, supplies the expected
and forbidden facts scored against the verified graph snapshot.

### D5. Bind scorer and verifier to the startup-pinned executable path without executing blobs

The scorer and verifier manifest blobs are fully streamed and rehashed. Each
blob digest must equal both the digest pinned once at local-CAS server startup
from the `cognigraph-server` file then reachable through
`std::env::current_exe()` and `reproducibility.executable_digest` in the
context. Startup fails unless that path resolves to a normal non-symlink file.

This is a path-content check, not proof of the process's loaded code pages. An
atomic path replacement between process image loading and startup hashing can
make the pinned file bytes diverge from already mapped code; a replacement
after hashing does not update the in-process pin. M21 does not load, spawn,
dynamically link, sandbox, or otherwise execute either staged blob. In
particular, it does not run a separate verifier program or obtain a verifier
verdict; the existing oracle-separation gate still validates its frozen
metadata attestation. M21 also does not prove boot-chain integrity, process
memory integrity, container identity, host identity, or remote attestation.

### D6. Persist an input-consumption hash receipt in the job result

A successful M21 job uses job schema version 2. Its terminal result contains an
`artifact_consumption` receipt with tenant/incarnation, job id, attempt,
recovery count, immutable input and execution-payload digests, exact EvalSpec
digest, canonical evaluation-result digest, pinned plan and context digests,
M20 artifact-set digest, five per-slot attestation/manifest/blob-set/semantic
projections, start and completion times, a timestamp-independent material
digest, and a complete receipt digest.

The receipt is finalized after distinct-fact scoring and the final check that
artifact authority remains active, then stored in the same durable terminal job
update as the evaluation result. Its `evaluation_result_digest` binds the
canonical result object without the receipt, while its job fields prevent
transplant across a
different attempt, recovery, input, or execution payload. Its unkeyed digests
alone still do not authenticate server authorship; later evidence validation
and the promoter's signed intent bind the receipt into the governed promotion
chain. It records no independent verifier-program verdict and is not signed by
an external witness, timestamp authority, TPM, HSM, or transparency log.

Candidate original/replay receipts must name identical verified material, as
must baseline original/replay receipts. Candidate and baseline material must
share corpus, oracle, scorer, and verifier; their graph material may differ
only under the existing policy-authorized graph difference.

### D7. Carry consumption authority through evidence and signed intent

Promotion evidence schema version 4 retains the four source receipt digests
and derives candidate and baseline material digests plus one aggregate
consumption-authority digest. Promotion decision schema version 4 records a
signed promoter intent in domain `cognigraph.promotion-intent.v2` that binds
that exact consumption-authority digest in addition to the M19/M20 policy and
artifact authority. The selected projection uses promotion-head schema version
2.

Recovery, reconciliation, idempotent replay, and Native stored-plus-incoming
snapshot preflight validate the M21 job/result/context, receipt, evidence,
intent, decision, and derived-head links. A receipt does not make its five M20
attestations permanently active: prospective revocation blocks a fresh M21
evaluation or new downstream authority while valid historical records remain
verifiable under their signed times and immutable chain.

### D8. Preserve M18-M20 meaning and require a fresh M21 target

Context/evidence/decision generations 1 through 3 and job schema version 1
remain readable and recoverable with their original meanings. Their evaluators
retain the historical live-backend behavior; M21 never fabricates consumption
receipts for them. Version-4 evidence cannot extend a target containing an
earlier generation, and a version-4 baseline cannot bridge that boundary.
Normal M21 adoption therefore requires a fresh `{space_type, channel}` target.

Rollback remains exact and chain-local. Compatibility does not permit
retroactive consumption claims, cross-generation promotion, or interpreting
an M20 manifest signature as proof that a pre-M21 evaluator read the bytes.

### D9. Preserve the selection-only singleton boundary

M21 changes how one governed evaluation obtains and proves its inputs. It does
not turn the selected promotion head into a deployment. CogniGraph does not
install the verified executable, rebuild or publish the graph, route traffic,
switch a consumer, manage replicas, coordinate another server, or add a
distributed lease, quorum, consensus, or high-availability guarantee.

The local CAS is an operator-staged custody boundary for one server process.
Availability, replication, retention, backup, cleanup, mount permissions, and
atomic staging remain operator responsibilities. Neither Native snapshots nor
ArangoDB records contain the staged blob bytes.

## Acceptance matrix

| Area | Acceptance contract | M21 status |
|---|---|---|
| Source | Disabled by default; explicit operator-controlled, read-only-mounted tenant-scoped local CAS with no location dereference, network fetch, or upload | Verified |
| Bytes | Cap the five manifests at 10,000 total entries and 4,096 unique digest+length pairs; stream and safely reuse verification; check tenant scope, regular-file chain, opened-file identity, declared length, SHA-256, retention caps, and cumulative I/O budget under an operator-controlled read-only root | Verified |
| Inputs | Score the parsed verified `graph.json` against the parsed verified `oracle.json`; hash-read corpus as provenance | Verified |
| Executable | Scorer/verifier blobs and reproducibility digest equal the executable-path digest pinned once at server startup; no loaded-code-page or verifier-verdict claim | Verified |
| Receipt | Terminal job result durably binds job/attempt/payload/EvalSpec identity, five consumed slots, and the canonical evaluation result; its unkeyed hash does not independently authenticate authorship | Verified |
| Runtime guard | Complete asynchronous M21 operation has a fixed 300-second deadline and one-second cancellation, tenant-suspension, and shutdown polling | Verified |
| Authority | Evidence and v2 signed promoter intent bind all four receipts and candidate/baseline material | Verified |
| Compatibility | M18-M20 remain readable with historical meaning; M21 requires a fresh v4 target | Verified |
| Recovery | Recovery, reconciliation, restart, and Native snapshot-union validation fail closed across M21 authority | Verified |
| Rust gates | Format, Clippy with warnings denied, and full workspace tests | Verified 2026-07-18 |
| Native live probe | Release binary, persistent Native storage, real HTTP lifecycle, restart, tamper/revocation, and cleanup | Verified in 5.09s |
| ArangoDB live probe | Release binary, configured live ArangoDB, real HTTP lifecycle, restart, tamper/revocation, and exact cleanup | Verified on Enterprise 3.12.9-1 in 103.09s |
| Boundary | Server hash receipt only; no derivation proof, dynamic execution, remote attestation, deployment, HA, or quorum claim | Explicitly preserved |

## Explicit non-goals and limits

- Signed M20 locations are never dereferenced. M21 has no HTTP/S3/object-store
  fetcher and no artifact upload or synchronization API.
- Corpus verification proves that the staged bytes match the signed manifest.
  It does not prove that `graph.json` was constructed from them.
- Parsing and scoring verified graph/oracle JSON does not prove that either
  artifact is semantically correct, complete, unbiased, or safe.
- Matching scorer/verifier blobs to the executable-path digest pinned through
  `current_exe()` at startup is path-content evidence, not proof of mapped code,
  dynamic execution, a verifier verdict, sandboxing, measured boot, or remote
  attestation.
- The durable receipt is generated and hashed by the same server after scoring
  and binds the canonical result, exact job execution identity, and verified
  inputs. Its unkeyed hash does not independently prove server authorship.
  There is no independent witness, trusted timestamp, transparency log,
  HSM/KMS signature, or append-only external ledger.
- Local-CAS access uses checked and rechecked filesystem paths, not an
  `openat2`/dirfd-anchored hostile-filesystem sandbox. The operator must control
  the root and expose it read-only to CogniGraph.
- The configured byte budget bounds cumulative declared lengths charged for
  verification reads after safe deduplication, not peak memory, elapsed time,
  or CPU. Separate fixed limits cap the five manifests at 10,000 total entries
  and 4,096 unique digest+length pairs, and the worker separately races
  asynchronous consumption/scoring against a 300-second deadline. These
  cooperative guards are not process or kernel I/O isolation.
- The CAS is not included in Native snapshots and ArangoDB gains no application
  snapshot or CAS storage. Operators must back up authority records and staged
  bytes under their separate custody policies.
- Full authority recovery remains tenant-wide and materializes bounded
  governance/promotion authority in memory. M21 does not add distributed
  scheduling, streaming recovery, HA, or multi-writer promotion.

## Verification evidence

The final workspace passed the release build and exact repository gates:

```sh
cargo build --release --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
```

Repository regressions cover strict request and stored-record wire shapes;
missing, wrong-length, wrong-digest, cross-tenant, symlink, byte-budget, and
executable/context failures; graph/oracle immutable-input scoring; exact
evaluation-result receipt binding; prospective revocation; historical
compatibility; recovery; and Native snapshot preflight for all incoming jobs
and divergent hot/archive duplicates. Separation checks cover the union of
candidate and baseline Artifact Attestors. An active-consumption regression
proves tenant suspension parks an M21 job and resume/recovery completes a fresh
attempt without retaining a partial receipt.

The final v2.5.0 release binary then completed authenticated real-HTTP probes:

- Persistent Native passed in 5.09 seconds. Four schema-v2 evaluation jobs
  stored consumption receipts; evidence v4, a domain-v2 signed promotion
  intent, decision v4, and head v2 survived restart/recovery. Replacing a CAS
  blob caused a fresh job to fail closed. Prospective Artifact Attestor
  revocation rejected fresh evaluation and evidence while the historical head
  remained verifiable. The isolated filesystem root was removed afterward.
- Live ArangoDB Enterprise 3.12.9-1 passed in 103.09 seconds with the same
  governed lifecycle. The database-scoped account correctly received `401` when the
  probe first attempted isolated database creation, with no writes, so the
  probe used the configured database under an exact baseline/restore protocol.
  Cleanup deleted exactly 31 probe-created records, restored zero pre-existing
  records, and reproduced normalized baselines across all 14 scoped auth, job,
  governance, and promotion collections. A separate count-only AQL check found
  zero remaining records in every scoped collection, matching the all-zero
  pre-probe baseline.

The probe harness and temporary CAS data were removed, and no credentials or
private signing material were printed or committed. This evidence verifies the
bounded local-CAS consumption contract; it does not prove corpus-to-graph
derivation, mapped process code, independent receipt authorship, remote
custody, deployment, quorum, or HA.
