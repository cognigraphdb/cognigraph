# Raw Documents

### M23 raw-document preparation

M23 does not alter or reinterpret M22. It adds a fresh context generation that
reproduces M22's prepared-corpus input from one closed kind of upstream input:
exact, already extracted UTF-8 plain-text bytes.

1. Use context schema version 6 and the exact server-supported consumption plan
   schema version 3 from
   [`docs/examples/m23-preparation-plan.json`](../../examples/m23-preparation-plan.json).
   Its nested preparation plan is schema version 1; the unchanged M22
   derivation plan remains schema version 1. Set
   `effective_configuration.preprocessing_digest` to the preparation plan's
   `plan_digest`, retain the M19 governance and M20 five-attestation bindings,
   and keep `reproducibility.backend = "artifact-snapshot"`. Job schema version
   4 is selected automatically.
2. Attest the corpus as
   `cognigraph.reproducible-prepared-chunk-corpus.v1`. The manifest contains
   exactly two sorted, non-executable `application/json` entries:
   `corpus.json` and `documents.json`. Both must already be integer-only,
   NFC-normalized canonical JSON byte streams; alternate whitespace, key order,
   or non-canonical Unicode is rejected even if it parses to an equivalent
   value.
3. `documents.json` is one closed schema-version-1 object with exactly
   `schema_version`, `space_type`, `corpus_revision_id`,
   `preparation_plan_digest`, and `documents`. It must match the context and
   pinned plan. `documents` is non-empty and sorted by unique document `id`.
   Every row contains exactly `id`, `title`,
   `media_type: "text/plain; charset=utf-8"`, `byte_length`, `blob_digest`, and
   `content_base64url`. Id/title are NFC and control-free, at most 1,024 bytes,
   and id is non-blank. The payload uses canonical unpadded URL-safe base64;
   decoding must yield non-empty bytes whose exact length and SHA-256 match the
   row. Padded, alternate-alphabet, malformed, length-mismatched, or
   digest-mismatched payloads fail before preparation.
4. The pinned `cognigraph.utf8-document-preparer` schema/version-1 function
   freezes Unicode 17.0.0, sorts documents by id, strips one leading UTF-8 BOM, rejects invalid UTF-8
   and controls other than CR/LF/TAB, maps CRLF and bare CR to LF, and
   normalizes content to NFC. The per-document normalized-byte cap applies at
   that point. Blank lines delimit paragraphs; line-edge whitespace is
   trimmed, interior runs collapse to one ASCII space, and documents with no
   non-blank paragraph fail. The aggregate normalized-byte cap applies to that
   collapsed text. Other line and paragraph joins use one ASCII space.
   The preparer greedily packs without overlap; an overlong paragraph splits at
   the latest fitting `.`, `!`, or `?` followed by whitespace/end, otherwise at
   whitespace, otherwise at the largest fitting UTF-8 boundary. A chunk id is
   exactly
   `d-<full lowercase sha256 of NFC document-id UTF-8>-c<zero-based eight-digit ordinal>`.
   Final chunk rows are sorted.
5. Preparation admits at most 100,000 documents, 4 MiB per raw document,
   64 MiB total raw bytes, 8 MiB per post-newline/NFC document before collapse,
   64 MiB total collapsed prepared-normalized text, 8 KiB per chunk, 100,000
   chunks, and 48 MiB total prepared
   text. `documents.json` retention is capped at 96 MiB and canonical
   `corpus.json` retains M22's 64 MiB cap. These are deterministic admission
   guards, not peak-memory, exact CPU, or elapsed-time measurements; the shared
   M21 manifest, unique-blob, byte-budget, deadline, cancellation, suspension,
   and shutdown guards also apply.
6. The server constructs `corpus.json` schema version 1 with the context space,
   corpus revision, preparation-plan digest, and prepared chunks. It rejects
   the job unless this object, canonical bytes, byte length, and SHA-256 digest
   exactly equal the attested `corpus.json`. Only then does it run M22 candidate
   resolution, grounding, graph reconstruction, exact `graph.json` comparison,
   and scoring. A normalized-equivalent raw byte stream is still a different
   signed `documents.json` address; exact input custody is not collapsed into
   normalized text equivalence.
7. A successful result uses artifact-consumption receipt schema version 3,
   corpus-to-graph derivation receipt schema version 2, and nested preparation
   receipt schema version 1. The compact preparation receipt binds the signed
   corpus manifest digest, exact and semantic `documents.json` address, exact
   and semantic prepared-corpus address, pinned preparer/ABI/plan, read set, and
   preparation material. Evidence and decision use schema version 6, the
   selected head uses version 4, and the promoter signs the explicit
   preparation-authority digest in domain
   `cognigraph.promotion-intent.v4`. Candidate/baseline original/replay evidence
   must carry one identical raw-document preparation authority in addition to
   M22's derivation constraints.

Do not submit a receipt or derived authority. The server creates them only
after raw-to-prepared equality, prepared-to-graph equality, scoring, and final
active-authority validation. Their hashes provide deterministic internal
consistency and later promoter accountability, not an independent execution
witness, remote attestation, trusted timestamp, or proof of host integrity.

M23 is address-durable rather than byte-self-contained. Its jobs, evidence, and
Native snapshots retain signed manifest projections and content addresses but
not a second copy of `documents.json`, `corpus.json`, or their decoded raw text.
Recovery and snapshot preflight can validate the authority/address chain;
re-execution requires the external tenant-incarnation CAS. Back up, replicate,
and restore that CAS separately from the Native database.

“Raw document” here means exact bytes already declared as strict UTF-8 plain
text. M23 does not parse PDF, HTML, office, archive, or compressed containers;
sniff MIME types; fetch remote locations; run OCR; or attest extraction
fidelity. Perform and govern extraction/OCR outside CogniGraph, then attest the
resulting exact text bytes. The prepared output is an evaluation artifact, not
materialized tenant documents, chunks, embeddings, entities, mentions,
indexes, trigger spans, storage keys, or a complete operational graph. M23 does
not publish/deploy the graph, execute staged code, route traffic, switch a
consumer, distribute the scheduler, or add replication, quorum, consensus, or
HA. The [M23 decision](../../decisions/decision_m23_reproducible_raw_document_prepared_corpus_processing.md)
retains the dated release-probe measurements and cleanup evidence.
