# M23 reproducible raw-document-to-prepared-corpus processing

- Date: 2026-07-18
- Status: Unreleased
- Kind: Change
- Date source: Git author date of the original record heading
- Source: `CHANGELOG.md:952-1007` at `dfc115803cd42fdf1f9b7026333a1bfddc139873`

## Record

- **M23 reproducible raw-document-to-prepared-corpus processing.** Added a new,
  non-retroactive context-v6 authority generation and closed consumption-plan
  v3. The corpus attestation now uses
  `cognigraph.reproducible-prepared-chunk-corpus.v1` with exactly two sorted,
  canonical, non-executable `application/json` entries: `corpus.json` and
  `documents.json`. The schema-v1 raw-document envelope binds target space,
  corpus revision, and preparation-plan digest, then requires documents sorted
  by unique non-blank NFC/control-free id with NFC/control-free titles and
  `text/plain; charset=utf-8` content represented as canonical unpadded
  base64url and matching its declared byte length and lowercase SHA-256
  address.
  The pinned `cognigraph.utf8-document-preparer` v1 rejects invalid UTF-8 and
  unsupported controls and freezes Unicode 17.0.0 for NFC normalization and
  whitespace classification. It strips exactly one leading UTF-8 BOM when
  present, converts CRLF/bare CR to LF, normalizes to NFC, and enforces the
  per-document normalized-byte cap before whitespace collapse. It then trims
  Unicode whitespace from line edges, collapses interior runs to ASCII spaces,
  joins adjacent non-blank lines, uses blank lines as paragraph boundaries,
  rejects documents with no non-blank paragraph, and enforces the aggregate
  normalized-byte cap after the full collapse. Deterministic packing uses no
  overlap; an overlong paragraph splits after `.`, `!`, or `?` only when
  followed by whitespace or paragraph end, then at whitespace, then at a valid
  UTF-8 boundary. Chunk ids bind the full lowercase SHA-256 of the NFC document
  id and an eight-digit zero-based ordinal. The frozen plan caps inputs at
  100,000 documents, 4 MiB per raw document, 64 MiB total raw bytes, 8 MiB per
  post-newline/NFC document before whitespace collapse, 64 MiB total
  prepared-normalized bytes after full collapse, 8 KiB per chunk, 100,000
  chunks, and 48 MiB total prepared text; retained `documents.json` and
  `corpus.json` are capped at 96 MiB and 64 MiB respectively.
  Before the existing M22 graph derivation runs, the reproduced schema-v1
  `corpus.json` object, canonical bytes, byte length, and SHA-256 digest must
  exactly equal the signed entry. A compact preparation receipt v1 binds the
  raw-document-set, plan, reproduced corpus, read set, and material digests
  without embedding raw bytes or a document inventory. It is nested in
  derivation receipt v2 and consumption receipt v3, then carried through job
  v4, evidence/decision v6, head v4, and the promoter's signed
  `cognigraph.promotion-intent.v4` preparation-authority digest. Restart,
  archive/catalog, recovery, status, idempotent replay, and Native snapshot
  preflight recognize the new generation while preserving M18-M22 wire and
  digest meanings; generation-switched plan, receipt, evidence, and signed-
  intent fields reject explicit JSON `null` rather than normalizing it into
  omission during verification or snapshot import.
  This is a text-only, address-durable preparation contract. Exact source bytes
  remain in the operator-managed local CAS and are not copied into receipts or
  Native snapshots; replay requires the corresponding CAS backup. It does not
  parse document containers, HTML, PDF, Office, archives, or compressed input,
  perform OCR, fetch signed locations, prove custody or semantic truth,
  reconstruct the complete persistent graph, execute staged code, deploy a
  selected head, or add independent attestation, quorum, or HA. Authenticated
  release-binary verification passed on persistent Native in 14.46 seconds and
  live ArangoDB Enterprise 3.12.9-1 in 109.22 seconds. Both exercised four
  preparation/derivation receipts, signed v4 promotion, restart/recovery,
  signed inconsistent prepared-output rejection, `documents.json` CAS-tamper
  rejection, prospective Artifact Attestor revocation/history fencing, and
  isolated cleanup. The Arango probe removed all 36 records it created and
  restored zero pre-existing records.
