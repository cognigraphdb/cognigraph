# Decision: Codepoint ordering, no normalization — deliberate, pinned, revisitable

Date: 2026-07-02 · Status: Historical; generic identity contract superseded by CG-33, 2026-09-09

Current contract: [exact stored identity and explicit text normalization](decision_exact_reference_identity.md).
The 2026-07-03 implicit NFC default below is historical. Locale collation and
explicit construction-evidence canonicalization remain in place.

## Context
Multilingual testing (German/Russian/Spanish/Hebrew/mixed) surfaced two
semantics that are correct-but-surprising.

## Decision
Char-based LENGTH/SUBSTRING and full Unicode case mapping (ß→SS) are
guaranteed. SORT orders strings by codepoint (not locale collation);
equality is codepoint-based (composed vs decomposed é differ). Both are
pinned by corpus files (i18n_sort_codepoint_order, i18n_no_normalization)
so any future change is visible and deliberate.

## Historical successors (decided 2026-07-03)
- **Collation delivered**: `SORT ... COLLATE "de"` (ICU4X — collation is
  library territory per decision_custom_vs_library.md); codepoint order
  remains the default, still pinned by i18n_sort_codepoint_order.
- **NFC delivered as the default**: literals normalize at parse, stored
  strings at write. The old i18n_no_normalization pin was replaced by
  i18n_nfc_literals asserting the new equality, plus a write-time
  roundtrip test (decomposed stored, composed query literal matches).

## Construction evidence boundary (CG-5, 2026-09-09)

Construction has an explicit normalization boundary before deriving evidence,
independent of generic storage defaults. Chunk text is normalized to NFC before rule matching, directed prompts and quote
gates, content hashing, and fact occurrence-key generation. `content_hash`
describes the exact stored NFC UTF-8 bytes; trigger start/end positions index
those bytes. No whitespace, case, or compatibility normalization is added by
this boundary. Original source-byte custody remains separate and unchanged.

Ordinary ingestion, directed ingestion, pure fact derivation, and materialized
projection construction share the canonical chunk representation. Directed
quotes, endpoints, and restraint phrases compare in NFC. The shared writer
checks callback evidence identities and trigger spans against that text before
the atomic batch; materialized projections reject non-NFC chunk text. CG-33
subsequently removed generic Native write-time normalization; this explicit
construction boundary remains. Already-NFC input keeps the same content hashes
and occurrence keys.

Existing inconsistent chunks require explicit re-ingestion to replace their
hashes, mentions, and fact occurrences. A snapshot faithfully restores its
contents and does not repair legacy inconsistencies by itself. Governed
generations must be re-derived and approved through the existing deployment
flow; previously signed artifacts are not rewritten in place.

Verification: formatting, Clippy, all 912 tests, and the release build passed.
The saved binary reproduced bad hashes and spans in six ordinary/directed
ingestions across resident/embedded, resident/sidecar, and paged/sidecar storage,
including restart. The corrected release passed 30 ingestion/reconciliation
calls, six repairs seeded by the old binary, three snapshot roundtrips, and
six restarts. Every checked hash matched stored text and every span selected
the intended evidence. Directed calls used a synthetic loopback provider only.
No final Rust gate or live check failed. See
[the report and raw evidence](../issues/unicode-evidence-2026-09-09.md).
