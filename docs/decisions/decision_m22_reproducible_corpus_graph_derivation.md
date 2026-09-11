# Decision: M22 reproducible corpus-to-graph derivation

**Status:** Implemented and verified (2026-07-18).

## Context

M21 proves that an evaluation read exact Artifact-Attestor-claimed bytes from
the tenant-scoped local CAS and scored the verified graph and oracle rather than
the live backend. Its corpus slot is deliberately provenance-only: the server
hashes the corpus manifest bytes but does not parse them or establish that the
attested graph was constructed from them.

M22 closes that gap for one precisely bounded construction function. Given an
already prepared chunk corpus, an exact governed construction candidate, and a
pinned deterministic Semantic Neurons grounding implementation, CogniGraph can
independently reproduce the evidence-bearing fact projection used by promotion
evaluation. The claimed graph is accepted only when its complete canonical
bytes equal the independently reconstructed artifact.

The milestone shorthand does not mean raw documents become a reproducible full
database image. Chunk preparation remains upstream, and the derived output is
not the persistent graph's complete document, chunk, entity, mention, index,
trigger-span, or storage-key state.

## Decision

### D1. Add a fresh, closed M22 authority generation

M22 uses these non-retroactive versions:

| Record | Version |
|---|---:|
| `PromotionContext` | 5 |
| durable evaluation job | 3 |
| artifact-consumption plan | 2 |
| artifact-consumption receipt | 2 |
| nested derivation plan | 1 |
| nested derivation receipt | 1 |
| promotion evidence | 5 |
| promotion decision | 5 |
| selected head projection | 3 |
| signed promoter intent domain | `cognigraph.promotion-intent.v3` |

Context v5 retains the complete M19 signed policy authority, M20 five-artifact
authority, M21 local-CAS source boundary, immutable oracle input, and startup-
pinned scorer/verifier path-content checks. It requires
`reproducibility.backend = "artifact-snapshot"` and the server's single exact
plan-v2 object. Unknown or modified plans fail admission.

The checked plan, including its derived semantics, ABI, nested-plan, and outer
plan digests, is
[`docs/examples/m22-derivation-plan.json`](../examples/m22-derivation-plan.json).

### D2. Make the prepared chunk corpus one exact canonical input

The M22 corpus attestation uses format
`cognigraph.prepared-chunk-corpus.v1` and contains exactly one non-executable
`application/json` entry named `corpus.json`. The raw bytes must equal the
integer-only NFC canonical JSON encoding of this closed schema-version-1 shape:

```text
space_type
corpus_revision_id
preprocessing_digest
chunks[] { id, title, text }
```

The header must match the target space, frozen corpus revision, and frozen
preprocessing digest. The corpus must be non-empty and sorted by unique raw
chunk id. Chunk ids are also checked for collisions under the existing
construction storage-key transform. IDs are non-empty NFC/control-free text;
titles may be empty but remain NFC/control-free; chunk text is non-empty NFC
and permits only LF and TAB control characters.

The pinned plan retains at most 64 MiB of corpus JSON, admits at most 100,000
chunks, and caps one chunk's text at 1 MiB. These bytes are prepared chunks, not
raw source documents. M22 does not define or replay decoding, normalization,
segmentation, chunk overlap, chunk-id assignment, OCR, or document extraction.

### D3. Put the exact construction candidate beside the claimed graph

The M22 graph attestation uses format
`cognigraph.reproducible-evaluation-graph.v1` and contains exactly two sorted,
non-executable `application/json` entries: `candidate.json` and `graph.json`.
The candidate is therefore covered by the existing Artifact Attestor graph
signature without creating an ungoverned sixth artifact slot.

`candidate.json` must be exact canonical schema-version-1 JSON and its raw
SHA-256 digest must equal `PromotionContext.candidate.candidate_digest`; its
declared length and media type must match the context candidate artifact. It
binds the candidate kind, id, and revision and contains one base space plus
accepted neurons of exactly three construction-relevant kinds:

- `alias`
- `relation_hint`
- `relation_blocker`

Entities, relation rules, aliases, triggers, sentence gates, neurons, evidence,
and endpoints are strict, bounded, sorted/unique where required, and checked
for referential validity. No candidate confidence, ranking hint, arbitrary
extension field, or unaccepted neuron state affects this derivation.

CogniGraph applies accepted aliases and relation hints to the base space and
resolves blockers into veto rules. The canonical digest of that effective
space plus veto set must equal the context's frozen
`construction_config_digest`. This keeps the exact candidate blob identity
separate from the canonical resolved-configuration identity.

The candidate is retained up to 8 MiB. The complete configuration is capped at
100,000 counted items. A deterministic checked grounding-work estimate counts
text size, every matching-triple blocker phrase, trigger-template expansion
count and size, sentence-gate surfaces, and entity surfaces. It must not exceed
10,000,000 units overall or 250,000 units for one chunk. Template candidates
are streamed rather than materialized as a Cartesian product; authored
placeholders are substituted once so inserted surfaces remain literal.
Entity/rule/veto compilation and clause-negation lookup use indexes, and
repeated trigger occurrences share one sentence-gate decision per rule and
sentence. Derivation yields after every bounded chunk. These are admission
guards, not measurements of CPU, memory, or elapsed time.

### D4. Pin and replay one deterministic grounding implementation

The nested derivation plan freezes the deriver id/version, semantics digest,
ABI digest, retention limits, configuration/work limits, fact limit, and its
own canonical digest. The implementation invokes the existing Semantic Neurons
grounder for each prepared chunk with the resolved effective space and vetoes.
It therefore preserves the existing:

- entity and alias surface matching;
- negation handling;
- sentence-local endpoint gates;
- direction-faithful `{source}` and `{target}` trigger expansion;
- accepted relation-hint behavior; and
- relation-blocker veto behavior.

Derivation does not query or mutate the live `GraphBackend`. It emits exact
`{source, relation, target, evidence_chunk_id}` rows, sorted and deduplicated by
all four fields, with a maximum of 100,000 rows. The async implementation yields
periodically for cooperative process progress; M21's 300-second operation
deadline and cancellation, suspension, and shutdown polling still bound the
larger consume/derive/score operation.

### D5. Require exact canonical equality with the claimed graph

`graph.json` is a closed canonical schema-version-1 envelope:

```text
space_type
graph_revision_id
corpus_manifest_digest
corpus_semantic_digest
candidate_digest
construction_config_digest
derivation_plan_digest
facts_digest
facts[] { source, relation, target, evidence_chunk_id }
```

Before derivation, every binding and the canonical digest of the claimed sorted
fact rows are validated. After derivation, the server constructs a new envelope
from the actual verified read set and derived facts, canonicalizes it, and
requires all three equalities:

1. reconstructed object equals parsed claimed object;
2. reconstructed canonical bytes equal the exact staged `graph.json` bytes;
3. SHA-256 of those bytes equals the graph manifest entry's blob digest.

Any mismatch fails the evaluation before scoring. This includes an altered,
removed, duplicated, or transplanted evidence row even if reduction to the
existing distinct `{source, relation, target}` scorer would produce the same
recall/restraint result. The scorer consumes only the distinct facts projected
from the exactly reproduced evidence-bearing rows; it never falls back to the
live graph.

### D6. Keep M21's byte, oracle, executable, and runtime boundaries

M21's tenant-incarnation SHA-256 CAS scope, fixed digest-derived path layout,
regular-file and parent-chain checks, streamed length/SHA-256 verification,
operator-controlled read-only-root requirement, 10,000 aggregate manifest-entry
limit, 4,096 unique digest+length-pair limit, cumulative byte budget, and final
active-attestation check remain in force. Signed locations are still audit
observations and are never dereferenced.

Oracle input remains exact verified `cognigraph.promotion-oracle.v1`
`oracle.json`. Scorer and verifier remain exact
`cognigraph.server-executable.v1` blobs matched to the executable-path digest
pinned through `current_exe()` at startup. They are not loaded or executed and
do not prove mapped code or produce an independent verifier verdict.

The pinned M22 retention limits are 64 MiB for corpus JSON, 8 MiB for candidate
JSON, and 32 MiB for graph JSON. Parsed and derived values can occupy more
memory. Neither those limits nor the configured read budget is a precise CPU,
memory, or wall-time budget; the separate cooperative deadline remains.

### D7. Store derivation material in the terminal job receipt

A successful job-v3 result contains artifact-consumption receipt version 2.
The outer receipt retains M21's exact job, attempt, recovery, input, execution
payload, EvalSpec, context, five-slot, canonical score, timing, material, and
receipt bindings. Its nested derivation receipt additionally binds:

- deriver semantics, ABI, and plan;
- exact corpus, graph, and oracle manifest projections whose canonical digests
  reproduce the immutable signed M20 bindings;
- corpus exact blob, canonical semantic digest, and chunk count;
- candidate exact blob and length, canonical semantic digest, and resolved
  configuration, all re-bound to `candidate.json` in the graph manifest;
- claimed and independently derived graph exact/semantic digests re-bound to
  `graph.json` in that manifest;
- the exact sorted evidence-bearing fact rows, their canonical digest, and
  count, sufficient to reconstruct the signed `graph.json` address offline;
- the exact verified oracle JSON, sufficient to reconstruct its signed entry
  address and recompute the canonical score offline; and
- the canonical construction read-set and derivation-material digests.

The receipt is finalized only after exact graph equality, scoring, and the
final active-authority check, then stored atomically with the terminal job
result. Its hashes are unkeyed values created by the same server that executed
the derivation. They provide deterministic consistency bindings. The embedded
manifest projections, fact rows, and exact oracle bytes make offline recovery
reconstruct the signed graph and oracle addresses and recompute the score, so a
self-consistent rewrite of computed fact/score metadata fails even if every
unkeyed receipt hash is recomputed. Later signed promotion authority anchors
the selected evidence. The receipt is still not an independent witness: a
privileged process can lie while issuing it, and neither mechanism
independently authenticates execution, time, or host state.

### D8. Carry explicit derivation authority into signed promotion intent

Evidence v5 requires derivation receipts on candidate original/replay and
baseline original/replay. Each pair must reproduce identical derivation
material; candidate and baseline share the corpus manifest and pinned plan.
Evidence records the four derivation-material digests, candidate and baseline
derived graph digests, plan digest, and one canonical derivation-authority
digest alongside M21 consumption authority.

The promoter's `cognigraph.promotion-intent.v3` signature explicitly binds that
derivation-authority digest. Decision v5 stores the signed intent and head v3
projects the selected decision. Thus the Artifact Attestor remains accountable
for the exact input-byte claims and the promoter is accountable for selecting
evidence with the exact derivation binding. Neither signature is an independent
witness that the server executed faithfully.

### D9. Preserve history and fail closed across recovery

M18-M21 jobs and authority remain readable and recoverable under their original
semantics. M22 does not fabricate derivation receipts for M21 jobs or reinterpret
M21 corpus verification as reconstruction. A target cannot mix authority
generations, so normal M22 adoption requires a fresh context-v5 target.

Restart recovery, job catalog/archive reconciliation, idempotent replay,
promotion recovery, and Native stored-plus-incoming snapshot preflight validate
M22 contexts, terminal receipts, derivation authority, signed intents,
decisions, and heads. Native preflight validates referenced and unreferenced
hot/archive job records before applying a snapshot and rejects divergent
immutable copies. Snapshot files contain the authority and receipts, never the
CAS byte streams. ArangoDB retains no CogniGraph application snapshot surface.

### D10. Preserve the selection-only singleton boundary

M22 proves equality for a prepared-corpus-to-canonical-evaluation-facts
function. It does not materialize that output into tenant storage, rebuild the
persistent graph, publish an artifact, install an executable, deploy a release,
route traffic, switch a consumer, coordinate another server, or provide remote
attestation, distributed leases, consensus, quorum, replication, or high
availability. The selected promotion head remains a tenant-local control-plane
pointer under one process-local transition lock.

## Acceptance matrix

| Area | Acceptance contract | M22 status |
|---|---|---|
| Input formats | Exact canonical prepared `corpus.json`; exact canonical `candidate.json` + `graph.json` package; unchanged verified oracle/scorer/verifier contracts | Implemented |
| Configuration | Strict base space + accepted construction neurons resolve to the frozen effective-configuration digest under pinned limits | Implemented |
| Derivation | Backend-free replay emits deterministic sorted unique evidence-bearing fact rows under the pinned semantics and fact/work limits | Implemented |
| Equality | Reconstructed graph object, canonical bytes, facts digest, and content address equal the attested `graph.json` before scoring | Implemented |
| Receipt | Job-v3 receipt-v2 binds the complete verified read set and claimed/derived graph equality; no independent-attestation claim | Implemented |
| Authority | Evidence/decision v5 and signed intent v3 bind explicit derivation authority; fresh target required | Implemented |
| Recovery | Restart, reconciliation, idempotent replay, status, and Native stored-plus-incoming snapshot validation fail closed across M22 authority | Implemented |
| Compatibility | M18-M21 retain their historical meanings and cannot mix with M22 within one target | Implemented |
| Rust gates | `cargo fmt --all -- --check`, Clippy with warnings denied, and full workspace tests | Verified |
| Native live probe | Release binary, persistent Native, authenticated real HTTP lifecycle, restart/recovery, mismatch/revocation, and cleanup | Verified in 5.24s |
| ArangoDB live probe | Release binary, configured live ArangoDB, authenticated real HTTP lifecycle, restart/recovery, mismatch/revocation, and exact cleanup | Verified on Enterprise 3.12.9-1 in 118.63s |
| Boundary | Prepared chunks to canonical evaluation facts only; no raw preprocessing, full graph reconstruction, independent attestation, deployment, or HA claim | Explicitly preserved |

## Explicit non-goals and limits

- M22 does not define how raw documents become the prepared chunk corpus. A
  reproducible upstream extraction/chunking package would be a separate
  milestone and authority surface.
- Exact derivation does not establish that the ontology, accepted neurons,
  corpus, oracle, or resulting facts are true, complete, fair, unbiased, or
  safe. Those remain governed input and evaluation-quality questions.
- The output omits persistent chunk, document, entity, mention, index,
  trigger-span, storage-key, embedding, and transaction state. It cannot be
  treated as a complete Native or ArangoDB backup or import image.
- Candidate and graph exact bytes are signed under the graph attestation; this
  does not add independent custody or guarantee that the local CAS remains
  available after evaluation.
- The server receipt is not signed by a separate verifier, timestamp authority,
  TPM, HSM, transparency service, or external execution witness. Later promoter
  signature binds selection intent, not an independently observed execution.
- Local-CAS path checks retain M21's operator-controlled-filesystem assumption
  and are not an `openat2`/dirfd-anchored hostile-custodian sandbox.
- Native snapshots omit CAS bytes; ArangoDB has no CogniGraph application
  snapshot. Back up and replicate external artifacts separately.
- The process remains singleton for job scheduling and promotion transitions.
  M22 adds no multi-writer, distributed recovery, consensus, quorum, or HA.

## Verification evidence

The implementation includes pure derivation regressions and a full stored M22
governance lifecycle covering deterministic replay, exact evidence rows,
forged-but-score-equivalent graph rejection, offline receipt/result rewrite
rejection against signed graph/oracle addresses, receipt and signed-authority
binding, restart recovery, and Native snapshot preflight.

Final validation on 2026-07-18 passed:

```text
cargo build --release --workspace --all-targets
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
```

The authenticated release-binary persistent-Native lifecycle passed in 5.24s.
It completed four derivation-backed evaluations, evidence registration, signed
v3 promotion, restart/recovery, signed score-equivalent graph rejection, CAS
tamper rejection, prospective Artifact Attestor revocation fencing, historical
head preservation, and isolated-store cleanup.

The same release binary passed the configured live ArangoDB lifecycle on
Enterprise 3.12.9-1 in 118.63s. Cleanup removed 34 probe-created records,
restored 0 pre-existing records, and confirmed every governed collection
matched its exact pre-test baseline. The local credential check and probe did
not print or persist secret values.
