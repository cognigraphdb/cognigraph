# CG-5: Native NFC stamping breaks construction hashes and byte spans

- Status: Resolved
- Priority: P2
- Area: Construction evidence integrity
- Found: 2026-09-08
- Reviewed revision: `bc4bff1` (workspace 2.6.0)
- Verification: Pre-fix release reproduction, 912 passing Rust tests, and corrected release ingestion/repair/snapshot/restart checks in all supported persistent Native modes
- Resolved: 2026-09-09 (committed locally as `36a4a19`)

## Problem

The construction writer hashes and measures the original chunk text, but Native stamping subsequently NFC-normalizes every string. Decomposed Unicode therefore changes after its content hash and trigger byte offsets have been calculated. Stored evidence can claim a hash that does not match stored text and an end offset beyond the stored UTF-8 buffer.

## Evidence

- `crates/cognigraph-construct/src/ingest.rs:255-263,344-351,393` — original text, hash, and spans.
- `crates/cognigraph-native/src/memory/helpers.rs:59-93` — recursive NFC normalization during document/edge stamping.
- `docs/decisions/decision_unicode_semantics.md` — intended normalization contract.

## Reproduction / failure sequence

Directed ingestion of `Cafe\u0301. Alpha supplies Beta.` stored `Café. Alpha supplies Beta.`. The input is 28 UTF-8 bytes and the stored text is 27 bytes. The stored fact retained `trigger_start:8, trigger_end:28`, and the chunk retained the hash of the 28-byte input.

## Acceptance criteria

- [x] Choose one canonical evidence representation before grounding, hashing, ID generation, and storage, or preserve exact source bytes where provenance requires them.
- [x] Verify every stored content hash against the stored representation and ensure byte spans slice the intended evidence.
- [x] Cover composed/decomposed text through ordinary and directed ingestion, re-ingestion, snapshot roundtrip, and supported Native storage modes.

## Resolution and verification

Chunk text is NFC before rule/directed grounding, hashing, and occurrence-key
generation. Directed prompts and quote gates use that same text; pure fact
derivation and materialization match ingestion. The shared writer rejects
invalid or stale custom-grounder spans before its atomic batch. Projection
validation rejects noncanonical evidence text. Raw source-byte custody remains
unchanged; stored hashes and byte offsets describe the canonical chunk text.

Formatting, Clippy, all 912 tests, and the optimized build passed. Five new
tests cover Unicode forms, occurrence stability, derivation parity, invalid
spans, and legacy repair. The saved release reproduced bad hashes/spans in six
ingestions across all three persistent configurations. The corrected release
passed 30 ingestion/reconciliation calls, six old-binary repairs, three snapshot
roundtrips, and six restarts across resident/embedded, resident/sidecar, and
paged/sidecar storage. Existing bad records require explicit re-ingestion;
snapshot import alone does not repair them. See
[the report and raw evidence](unicode-evidence-2026-09-09.md),
[Unicode decision](../decisions/decision_unicode_semantics.md), and
[original review](review-2026-09-08.md).
