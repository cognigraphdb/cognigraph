# M23 raw-document preparation authoring guide

M23 has no new signing key, artifact kind, upload API, or promotion mutation.
It changes the exact bytes covered by the existing M20 `corpus` attestation and
adds a fresh `PromotionContext` generation. This directory documents the
public authoring boundary; it contains no private key, secret seed, credential,
usable signature, or complete attestable package.

[`preparation-golden.json`](preparation-golden.json) is an unsigned regression
vector. It stores canonical `documents.json` and `corpus.json` bytes as
unpadded base64url plus their exact SHA-256 digests, so tests detect preparer or
canonical-envelope drift without regenerating expected output through the
function under test.

Use the server's exact checked plan from
[`docs/examples/m23-preparation-plan.json`](../../docs/examples/m23-preparation-plan.json).
Do not edit or independently substitute plan fields. Context v6 accepts only
that complete plan-v3 object. Its complete digest is
`sha256:8367079f692bd383b4789e69d319546b42d1b7dab794a5c142799fc2bec30a66`;
the nested preparation-plan digest is
`sha256:ef29c26eb3bb5700dd280a2b8421d2e421216ad593e0093512c2d3f822f4dd56`.

## Required M20 manifests

The corpus attestation must use artifact format
`cognigraph.reproducible-prepared-chunk-corpus.v1` with exactly two manifest
entries in this sorted order:

| Logical path | Media type | Executable |
|---|---|---:|
| `corpus.json` | `application/json` | `false` |
| `documents.json` | `application/json` | `false` |

The graph attestation remains the M22
`cognigraph.reproducible-evaluation-graph.v1` package containing canonical
`candidate.json` and `graph.json`. Oracle, scorer, and verifier retain their
M21/M22 formats and entrypoints. See the
[`fixtures/m22/` guide](../m22/) for those unchanged shapes.

Every manifest entry's `byte_length` and `blob_digest` cover the exact bytes
staged in the tenant-incarnation local CAS. Signed location observations are
never fetch authority.

## Canonical byte requirement

Both M23 corpus-package files must already be encoded as CogniGraph's integer-
only NFC canonical JSON: object keys NFC-normalized and sorted, strings NFC-
normalized, array order retained, no insignificant whitespace, and no
floating-point values. The formatted snippets below illustrate fields only;
copying them with indentation or a trailing newline will fail exact-byte
validation.

Use `cognigraph_governance::canonical_json_bytes` in an external authoring tool,
write exactly those bytes, and compute SHA-256 over that byte sequence.
Canonical parsing of each file must reproduce its exact blob address.

## Raw document package shape

`documents.json` is a closed schema-version-1 object:

```json
{
  "schema_version": 1,
  "space_type": "REPLACE_WITH_SPACE_TYPE",
  "corpus_revision_id": "REPLACE_WITH_CORPUS_REVISION",
  "preparation_plan_digest": "sha256:ef29c26eb3bb5700dd280a2b8421d2e421216ad593e0093512c2d3f822f4dd56",
  "documents": [
    {
      "id": "source-document-0001",
      "title": "Example title",
      "media_type": "text/plain; charset=utf-8",
      "byte_length": 123,
      "blob_digest": "sha256:REPLACE_WITH_EXACT_RAW_BYTE_DIGEST",
      "content_base64url": "REPLACE_WITH_CANONICAL_UNPADDED_BASE64URL"
    }
  ]
}
```

Authoring requirements:

- `space_type` and `corpus_revision_id` must match context v6 and the corpus
  attestation subject.
- Sort documents by unique `id`.
- IDs must be non-blank NFC/control-free UTF-8 of at most 1,024 bytes. Titles
  may be empty but must also be NFC/control-free and at most 1,024 bytes.
- `media_type` is exactly `text/plain; charset=utf-8`.
- The decoded payload must be non-empty strict UTF-8. Only CR, LF, and TAB are
  admitted control characters.
- `content_base64url` is canonical unpadded URL-safe base64. Decode then
  re-encode it and require exact string equality before signing.
- `byte_length` and `blob_digest` bind the decoded raw bytes, including an
  optional leading UTF-8 BOM and original CR/LF representation.

The per-document raw bytes are embedded in `documents.json`; they are not
additional manifest entries and CogniGraph does not fetch them from the
document id, a logical path, or a signed location.

The SHA-256 of the complete canonical `documents.json` bytes becomes all three
of its exact-byte digest, semantic digest, and durable
`raw_document_set_digest`. The compact server receipt does not carry a copied
document count, byte count, per-document inventory, or raw bytes.

## Reproduced prepared corpus shape

`corpus.json` retains the M22 prepared-corpus envelope, but its
`preprocessing_digest` is the exact M23 preparation-plan digest:

```json
{
  "schema_version": 1,
  "space_type": "REPLACE_WITH_SPACE_TYPE",
  "corpus_revision_id": "REPLACE_WITH_CORPUS_REVISION",
  "preprocessing_digest": "sha256:ef29c26eb3bb5700dd280a2b8421d2e421216ad593e0093512c2d3f822f4dd56",
  "chunks": [
    {
      "id": "d-REPLACE_WITH_FULL_DOCUMENT_ID_SHA256-c00000000",
      "title": "Example title",
      "text": "Prepared NFC chunk text."
    }
  ]
}
```

The authoring tool must reproduce the server's pinned Unicode-17.0.0
normalization and whitespace tables plus these preparation semantics:

1. sort raw documents by id;
2. strip one leading UTF-8 BOM;
3. reject controls other than CR, LF, and TAB;
4. map CRLF and bare CR to LF, normalize to NFC, and enforce the per-document
   normalized-byte cap before whitespace collapse;
5. trim Unicode whitespace from line edges, collapse interior whitespace runs
   in non-blank lines, join adjacent non-blank lines with one ASCII space, use
   blank lines as paragraph boundaries, reject documents with no non-blank
   paragraph, and enforce the aggregate normalized-byte cap after collapse;
6. greedily pack paragraphs to 8 KiB, splitting overlong text first at sentence
   punctuation before whitespace, then whitespace, then a UTF-8 boundary;
7. use no chunk overlap; and
8. assign `d-<full lowercase SHA-256 of NFC document-id UTF-8>-c<zero-based
   eight-digit decimal ordinal>` and sort the final chunks by that id.

CogniGraph independently rebuilds the complete object and rejects the job
unless the reconstructed object, canonical bytes, byte length, and SHA-256
address exactly equal the staged `corpus.json`. It performs the existing M22
candidate-to-graph derivation only after this check succeeds.

## Pinned limits

| Limit | Value |
|---|---:|
| `documents.json` retention | 96 MiB |
| `corpus.json` retention | 64 MiB |
| documents | 100,000 |
| raw bytes per document | 4 MiB |
| total decoded raw bytes | 64 MiB |
| post-newline/NFC bytes per document, before whitespace collapse | 8 MiB |
| total prepared-normalized bytes after whitespace/paragraph collapse | 64 MiB |
| prepared chunk text | 8 KiB |
| chunks | 100,000 |
| total prepared text | 48 MiB |
| cooperative yield interval | 16 documents |

The M21/M22 aggregate manifest-entry, unique-blob, cumulative-read, deadline,
cancellation, suspension, and shutdown limits also apply.

## Authoring sequence

1. Select immutable non-empty UTF-8 plain-text sources and assign stable NFC
   document ids, titles, one corpus revision, and the target space.
2. For each source, preserve the exact raw bytes, compute its exact length and
   SHA-256 digest, encode canonical unpadded base64url, and build sorted
   `documents.json` with the pinned preparation-plan digest.
3. Run the pinned preparer externally to author the expected chunks. Build
   canonical `corpus.json` with the same space/revision and the preparation-
   plan digest as `preprocessing_digest`.
4. Build the two-entry corpus manifest and sign the normal M20 corpus
   attestation. Stage both exact blobs in the correct tenant-incarnation CAS
   scope.
5. Author the unchanged M22 `candidate.json` and exactly derived `graph.json`,
   sign their graph attestation, and prepare the unchanged oracle, scorer, and
   verifier attestations.
6. Resolve the five active artifact bindings and freeze them, the complete
   plan-v3 object, matching candidate/configuration/revision identities, and
   `reproducibility.backend = "artifact-snapshot"` in a fresh context-v6
   target.
7. Submit candidate and baseline original/replay evaluations. Never submit a
   preparation, derivation, or consumption receipt; CogniGraph creates them
   only after exact verification, reconstruction, scoring, and final active-
   authority validation.

## Receipt and custody boundary

The M23 preparation receipt binds the signed `documents.json` address, signed
and reproduced `corpus.json` address, exact plan, read set, and preparation
material. It deliberately does not embed the raw bytes, document inventory, or
prepared chunks.

Consequently, jobs, promotion authority, recovery, and Native snapshots are
**address-durable**, not **artifact-byte-durable**. They can validate that the
authority chain names one exact raw package and prepared corpus, but they cannot
rerun preparation after the external CAS bytes are lost. Back up and replicate
the CAS separately. ArangoDB has no CogniGraph application snapshot surface.

These authoring shapes are not proof of container extraction, OCR, semantic
truth, complete persistent graph reconstruction, independent execution
attestation, deployment, replication, quorum, or high availability.
