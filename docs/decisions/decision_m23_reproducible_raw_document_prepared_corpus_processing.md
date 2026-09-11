# Decision: M23 reproducible raw-document-to-prepared-corpus processing

**Status:** Accepted and verified (2026-07-18).

## Context

M22 proves one bounded prepared-corpus-to-evaluation-facts function. It starts
from an exact canonical prepared-chunk corpus, replays the pinned Semantic
Neurons grounder, and requires the claimed canonical evaluation graph to equal
the independently derived graph before scoring. It deliberately leaves the
preceding raw-document decoding, normalization, segmentation, chunking, and
chunk-id assignment outside governed replay.

M23 closes that specific upstream gap for one deliberately narrow input class:
non-empty exact UTF-8 plain-text byte streams embedded in a canonical signed
artifact. It pins a mechanical preparation recipe, independently reconstructs
the complete canonical M22 prepared corpus, and admits the existing M22 graph
derivation only after the reconstructed corpus object, canonical bytes, length,
and SHA-256 address all equal the Artifact-Attestor-claimed `corpus.json`.

The word “document” does not broaden this contract to arbitrary document
formats. Container parsing, HTML, PDF, Office formats, archives, charset
transcoding, OCR, layout recovery, language detection, and semantic cleanup
remain outside CogniGraph.

## Decision

### D1. Add a fresh, closed M23 authority generation

M23 uses these non-retroactive versions:

| Record | Version |
|---|---:|
| `PromotionContext` | 6 |
| durable evaluation job | 4 |
| artifact-consumption plan | 3 |
| artifact-consumption receipt | 3 |
| nested preparation plan | 1 |
| nested preparation receipt | 1 |
| nested corpus-to-graph derivation plan | 1, unchanged from M22 |
| nested corpus-to-graph derivation receipt | 2 |
| promotion evidence | 6 |
| promotion decision | 6 |
| selected head projection | 4 |
| signed promoter intent domain | `cognigraph.promotion-intent.v4` |

Context v6 retains the complete M19 signed policy authority, M20 five-artifact
authority, M21 tenant-scoped local-CAS boundary, and M22 candidate, grounding,
graph-reconstruction, oracle, scorer, and verifier contracts. It requires
`reproducibility.backend = "artifact-snapshot"`, the server's one exact plan-v3
object, and an effective-configuration `preprocessing_digest` equal to the
nested preparation-plan digest. Unknown, modified, or cross-generation plans
fail admission.

M18-M22 jobs and authority keep their original meanings. A target cannot mix
authority generations, so normal M23 adoption requires a fresh context-v6
target rather than retroactively treating an M22 prepared corpus as
raw-document preparation evidence.

The checked plan is
[`docs/examples/m23-preparation-plan.json`](../examples/m23-preparation-plan.json).
Its pinned identities are:

| Identity | Digest |
|---|---|
| loader semantics | `sha256:c7b5271d7f4609a68ce1b6e9633be7cab19b8d66503279057d2fe0ea0bd975aa` |
| preparer semantics | `sha256:d7c3553b49df8313c339d82c3c2568ef50a56d6dcb0e9178c558d93851c4405c` |
| preparation ABI | `sha256:0be798c0584f4ed739283e91380485ecd537b8de91ca7117dddf6768349e90fe` |
| nested preparation plan | `sha256:ef29c26eb3bb5700dd280a2b8421d2e421216ad593e0093512c2d3f822f4dd56` |
| unchanged M22 derivation plan | `sha256:22e20e4d05fe665976c5e3201756fd22cc372f6ae635623ffb87506ea91032de` |
| complete consumption plan | `sha256:8367079f692bd383b4789e69d319546b42d1b7dab794a5c142799fc2bec30a66` |

### D2. Make raw bytes and prepared output one signed corpus package

The M23 corpus attestation uses format
`cognigraph.reproducible-prepared-chunk-corpus.v1`. Its manifest contains
exactly two sorted, non-executable `application/json` entries:

1. `corpus.json`
2. `documents.json`

Both byte streams must already be integer-only NFC canonical JSON. Their
manifest lengths and SHA-256 digests cover the exact staged bytes in the
tenant-incarnation local CAS. Signed locations remain audit observations and
are never dereferenced.

`documents.json` is a closed schema-version-1 object:

```text
schema_version
space_type
corpus_revision_id
preparation_plan_digest
documents[] {
  id,
  title,
  media_type,
  byte_length,
  blob_digest,
  content_base64url
}
```

The header must match the target space, frozen corpus revision, and exact
preparation plan. Documents are sorted by unique `id`. IDs are non-blank NFC,
control-free text of at most 1,024 UTF-8 bytes; titles may be empty but are
also NFC, control-free, and at most 1,024 bytes. `media_type` is exactly
`text/plain; charset=utf-8`.

`content_base64url` is canonical unpadded URL-safe base64. Decoding and then
re-encoding must reproduce the field exactly. The decoded non-empty byte
stream must match its declared `byte_length` and lowercase SHA-256
`blob_digest`. Per-document digests describe the embedded raw bytes; they do
not create additional manifest entries or an upload surface.

The complete canonical `documents.json` byte digest must equal its manifest
entry. Because the object is canonical, its semantic digest, exact-byte digest,
and M23 `raw_document_set_digest` are the same content address. This address,
not a derived count or copied per-document inventory, is the durable
raw-document-set authority.

`corpus.json` retains the M22 closed schema-version-1 shape:

```text
schema_version
space_type
corpus_revision_id
preprocessing_digest
chunks[] { id, title, text }
```

For M23, `preprocessing_digest` must equal the nested preparation-plan digest.

### D3. Pin one deterministic UTF-8 preparation function

The nested preparation plan identifies
`cognigraph.utf8-document-preparer` version `1` and freezes these limits:

| Limit | Value |
|---|---:|
| retained `documents.json` bytes | 100,663,296 (96 MiB) |
| retained `corpus.json` bytes | 67,108,864 (64 MiB) |
| documents | 100,000 |
| raw bytes per document | 4,194,304 (4 MiB) |
| total decoded raw bytes | 67,108,864 (64 MiB) |
| post-newline/NFC bytes per document, before whitespace collapse | 8,388,608 (8 MiB) |
| total prepared-normalized bytes after whitespace/paragraph collapse | 67,108,864 (64 MiB) |
| prepared chunk text bytes | 8,192 (8 KiB) |
| chunks | 100,000 |
| total prepared text bytes | 50,331,648 (48 MiB) |
| cooperative yield interval | 16 documents |

The preparer freezes Unicode 17.0.0 for both NFC normalization and Unicode
whitespace classification, sorts input by document id, and, for every
document:

1. requires a non-blank NFC/control-free id and an NFC/control-free title,
   each at most 1,024 UTF-8 bytes, plus non-empty strict UTF-8 content bytes;
2. rejects control characters other than CR, LF, and TAB;
3. strips one leading UTF-8 BOM when present;
4. maps CRLF and bare CR to LF;
5. normalizes the text to NFC and enforces the per-document normalized-byte
   limit before whitespace collapse;
6. trims Unicode whitespace from line edges, collapses each interior run to
   one ASCII space, joins adjacent non-blank lines with one ASCII space, uses
   blank lines as paragraph boundaries, rejects a document with no non-blank
   paragraph, and applies the aggregate normalized-byte limit to this collapsed
   text;
7. greedily packs paragraphs within the 8 KiB byte limit, splitting an
   overlong paragraph first at sentence punctuation (`.`, `!`, or `?`) before
   whitespace, then at whitespace, then at the largest valid UTF-8 boundary;
8. uses no overlap; and
9. assigns `d-<64 lowercase SHA-256 hex>-c<8 decimal digits>`, where the hash
   covers the exact NFC document-id UTF-8 bytes and the suffix is the
   zero-based per-document chunk ordinal.

Each chunk copies the document title. Final chunks are sorted by their complete
id, so input permutation cannot alter the canonical output.

These are deterministic admission and allocation bounds, not exact CPU,
peak-memory, or wall-time measurements. M21's aggregate manifest-entry,
unique-blob, cumulative-read, operation-deadline, cancellation, suspension,
and shutdown guards remain in force.

### D4. Require exact prepared-corpus equality before M22 derivation

The worker canonicalizes and validates `documents.json`, checks every embedded
raw stream, and executes only the pinned preparation function. It constructs a
new prepared-corpus envelope from the target space, frozen corpus revision,
preparation-plan digest, and actual generated chunks.

Before grounding or scoring, all four equalities must hold:

1. the reconstructed prepared-corpus object equals parsed `corpus.json`;
2. its canonical bytes equal the exact staged `corpus.json` bytes;
3. its byte length equals the corpus manifest entry length; and
4. SHA-256 of those bytes equals the corpus manifest entry digest.

Any changed raw byte, non-canonical base64url or JSON encoding, metadata
mismatch, different normalization/chunk boundary, removed or reordered chunk,
or altered prepared output fails the job. There is no fallback to the claimed
prepared corpus.

Only after this equality succeeds does the unchanged M22 plan resolve the
construction candidate, derive evidence-bearing fact rows, reproduce
`graph.json`, and score the verified graph against the verified oracle. The
live `GraphBackend` remains outside the M23 score and derivation read set.

### D5. Store a compact, address-durable preparation receipt

A successful job-v4 result contains artifact-consumption receipt version 3,
derivation receipt version 2, and nested preparation receipt version 1. The
preparation receipt binds:

- preparer identity, semantics, ABI, and exact preparation plan;
- the signed corpus manifest digest;
- exact/semantic `documents.json` digests and their identical
  `raw_document_set_digest` address;
- exact/semantic reproduced `corpus.json` digests;
- the preparation read-set digest; and
- the complete preparation-material digest.

The compact receipt deliberately contains no raw document bytes, document
count, total-byte count, per-document digest inventory, normalized text, or
prepared chunk inventory. Those values cannot be re-proven during offline
snapshot validation without the external `documents.json` bytes, so copying
them into the receipt would create unauthenticated-looking metadata rather than
durable evidence.

For the same reason, derivation receipt version 2 omits M22's historical
`chunk_count` observation. It retains the exact prepared-corpus address and the
downstream fact rows it can re-prove, but does not copy an external-corpus
count into M23 durable state.

This makes the receipt **address-durable**, not **byte-durable**. Recovery and
Native snapshot preflight can validate the signed canonical addresses and the
complete downstream authority chain without dereferencing the CAS. They cannot
rerun raw-document preparation from the receipt or snapshot alone. Re-execution
requires the exact externally retained `documents.json` and `corpus.json`
blobs. Back up, replicate, and restore the CAS separately.

The receipt is an unkeyed record created by the same server that performed the
work. Its hashes provide deterministic consistency bindings; they are not an
independent execution witness, trusted timestamp, remote attestation, or proof
of host integrity.

### D6. Carry explicit preparation authority into signed promotion intent

Evidence v6 requires preparation receipts on candidate original/replay and
baseline original/replay. All four preparation-material digests must match,
and candidate and baseline must share one raw-document-set address, prepared-
corpus address, and preparation-plan digest. Evidence stores those values in a
nested preparation-authority projection and computes its canonical authority
digest.

The promoter's `cognigraph.promotion-intent.v4` signature explicitly binds the
preparation-authority digest in addition to M20 artifact, M21 consumption, and
M22 derivation authority. Decision v6 stores that signed intent, and head v4
projects the selected decision. The Artifact Attestor remains accountable for
the exact byte claims; the promoter remains accountable for selecting the
four-run evidence. Neither signature independently observes execution.

### D7. Preserve fail-closed recovery and external custody

Restart recovery, job catalog/archive reconciliation, idempotent replay,
promotion recovery, status, and Native stored-plus-incoming snapshot preflight
validate M23 contexts, receipt nesting, preparation authority, signed intents,
decisions, and heads. Validation includes unreferenced hot and archived jobs so
a malformed receipt cannot hide outside selected evidence.

Native snapshots retain signed manifests, compact receipts, and authority
records, never the external corpus, graph, oracle, scorer, verifier, or raw
document package bytes. ArangoDB still has no CogniGraph application snapshot
surface. Local-CAS path validation retains M21's operator-controlled,
read-only-filesystem assumption and is not an `openat2`/dirfd-anchored sandbox
against a hostile concurrent custodian.

### D8. Preserve the narrow selection-only boundary

M23 proves one exact UTF-8-plain-text-to-canonical-prepared-corpus function and
then composes it with M22's prepared-corpus-to-evaluation-facts function. It
does not:

- extract text from containers, markup, PDFs, Office files, images, audio, or
  archives;
- perform OCR, charset detection/transcoding, language detection, semantic
  cleanup, learned segmentation, tokenization, or overlapping chunking;
- establish that a source, ontology, oracle, prepared chunk, or derived fact is
  true, complete, fair, unbiased, or safe;
- materialize persistent documents, chunks, entities, mentions, trigger spans,
  indexes, embeddings, storage keys, or transactions;
- publish or deploy a graph, execute staged code, switch a consumer, or route
  traffic; or
- provide artifact custody, independent execution attestation, distributed
  scheduling, leases, consensus, quorum, replication, or high availability.

The selected head remains a tenant-local control-plane pointer under one
process-local transition lock.

## Acceptance matrix

| Area | Acceptance contract | Current status |
|---|---|---|
| Input package | Exact canonical two-entry `corpus.json` + `documents.json` signed corpus package | Implemented |
| Raw bytes | Canonical base64url, strict UTF-8, exact length/digest, sorted unique document ids | Implemented |
| Preparation | Pinned normalization, paragraph packing, byte-bounded splitting, no overlap, stable hashed ids | Implemented |
| Equality | Reconstructed prepared object, canonical bytes, length, and content address equal the attested corpus before M22 | Implemented |
| Receipt | Compact receipt binds signed raw/prepared addresses without copying counts, inventories, or bytes | Implemented |
| Authority | Evidence/decision v6 and signed intent v4 bind explicit preparation authority | Implemented |
| Recovery | Job recovery, archive validation, promotion recovery, status, and Native snapshot preflight fail closed | Implemented |
| Compatibility | M18-M22 retain historical meanings; a fresh M23 target is required | Implemented |
| Rust gates | Format, Clippy with warnings denied, and full workspace tests | Verified |
| Native live probe | Authenticated release binary, persistent Native, restart/recovery, tamper paths, cleanup | Verified in 14.46 s |
| ArangoDB live probe | Authenticated release binary, configured live ArangoDB, restart/recovery, tamper paths, exact cleanup | Verified on Enterprise 3.12.9-1 in 109.22 s |
| Boundary | UTF-8 text preparation plus M22 evaluation projection only; no extraction/OCR/full graph/deployment/HA claim | Explicitly preserved |

## Implemented regression coverage

The implementation contains a static unsigned golden vector whose exact
canonical `documents.json` bytes reproduce one fixed canonical `corpus.json`
digest, plus pure preparation regressions for permutation
stability, BOM/newline/NFC/whitespace normalization, UTF-8-safe hard splits,
invalid UTF-8, duplicate ids, unsupported controls, blank normalized input,
non-NFC metadata, every configured size boundary, sentence punctuation,
decimals, dotted names, and stable chunk ids.

The stored governance lifecycle regression exercises job-v4 preparation,
backend-free evaluation, receipt nesting, four-run evidence, signed v4
promotion, derived head, restart recovery, archived unreferenced-job snapshot
preflight, self-consistently rehashed receipt and forbidden copied-count
tamper, explicit-null rejection across generation-switched plans, receipts,
evidence, and signed intents, staged CAS tamper, a signed but inconsistent
prepared corpus, a second M23 selection, signed v4 rollback, and missing-
preparation-authority rejection.
These tests are implemented evidence and were complemented by the final gates
and release-binary runs below.

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
  backend/storage: authenticated HTTP against persistent Native/redb storage
  elapsed: 14.46 seconds
  restart/recovery: four preparation-and-derivation receipts, evidence, signed
    promotion-intent.v4 promotion, and recovered head remained valid
  fail-closed checks: signed inconsistent prepared output, documents.json CAS
    tamper, and prospective Artifact Attestor revocation/history fencing
  cleanup: isolated Native probe storage removed

PASS ArangoDB release-binary lifecycle:
  server: ArangoDB Enterprise 3.12.9-1; authenticated credential check passed
  elapsed: 109.22 seconds
  restart/recovery and fail-closed checks: same M23 lifecycle as Native
  exact cleanup: removed all 36 probe records and restored 0 pre-existing records
```
