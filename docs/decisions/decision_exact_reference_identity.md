# Decision: Exact stored identity and explicit text normalization

Date: 2026-09-09 · Status: Accepted and implemented (CG-33)

## Contract

Accepted collection names, document keys, and caller-owned JSON strings retain
their exact Unicode contents. Canonically equivalent strings can identify
different documents: `docs/caf\u00e9` and `docs/cafe\u0301` are distinct handles.
Graph endpoints, parent `document_id`, frozen job collection/keys, model names,
and nested opaque values follow that same rule. Backend metadata stamping still
sets document identity, timestamps, and edge defaults as before.

Native no longer recursively normalizes values during writes. CGQL string
literals decode JSON escapes without normalization, so a literal and a bind
variable naming the same handle resolve the same document. Equality, pushed
predicates, and default sorting retain exact codepoint semantics. There is no
new query grammar or backend routing. Arango's adapter already passes caller
JSON through; backend-specific identifier grammar can reject names, but must
not silently rewrite an accepted reference.

For text comparisons, use `NORMALIZE_NFC(value)` explicitly. It takes one
argument, returns its NFC form for a string, and returns null for other values.
For example, `NORMALIZE_NFC(d.title) == NORMALIZE_NFC(@title)` compares canonical
text. Do not use this transformation on an opaque handle to select a document.
Locale-aware `SORT ... COLLATE` remains separate and unchanged.

## Compatibility and existing data

This supersedes the generic NFC-on-write and NFC-literal defaults from the
[2026-07-03 Unicode decision](decision_unicode_semantics.md). Applications that
relied on implicit canonical text equality must normalize their designated text
at the application boundary or explicitly in the query. Existing stored values
are unchanged, including on partial updates; there is no automatic key rename,
collision merge, or migration. Restoring an old snapshot preserves its values
and therefore also preserves any old wrong references.

The explicit [CG-5 construction evidence boundary](decision_unicode_semantics.md#construction-evidence-boundary-cg-5-2026-09-09)
still canonicalizes chunk text before grounding, hashes, byte spans, and
occurrence identities. Governed/custodied ingestion and signed statement
validation keep their own canonical representations. Generic storage does not
reinterpret those contracts or rewrite signed artifacts in place.

CG-13's temporary non-NFC side-view source rejection is removed. The source
collection and keys survive freezing, persistence, job restart, publication,
and cascade deletion exactly. Collection names containing `/` remain rejected
for side-view sources because full handles split at the first slash.

## Bounded diagnosis and repair

`cognigraph references audit SNAPSHOT` inspects root `_from`, `_to`, and
`document_id`, plus frozen `_execution.collection`/`keys`. It reports unresolved
references and canonically equivalent competing identities even when the
current reference resolves. That last case can already target the wrong
existing document. Missing original intent cannot be recovered from normalized
storage, and arbitrary application reference fields are outside this scan.

`cognigraph references repair SNAPSHOT PLAN --out NEW_FILE` produces a separate
snapshot using explicit mappings bound to the input's SHA-256. Each target must
exist exactly, differ from the source, and be canonically equivalent. Source
values must match; duplicate, stale, unknown-field, and oversized plans fail.
Only root reference fields in ordinary collections are eligible. System,
governed, and generated collections are forbidden. Every other JSON value and
key is preserved. Output is private and atomically published without overwrite;
there is no network operation or automatic import.

Inputs are limited to 64 MiB and 100,000 documents, handles to 4,096 bytes, and
plans to 1,000 changes. Reports retain at most 2,000 findings and 2 MiB of finding
JSON, disclose truncation and total counts, and list at most 20 candidate handles
per finding. Larger datasets need a separately bounded workflow; this command
does not stream an unlimited repair. See the [operator procedure](../operations/reference-repair.md)
for review, quiescence, protected workflows, and import requirements.

## Verification

The saved CG-21 release reproduced 69 identity mismatches in 105 HTTP/Lua checks
across resident/embedded, resident/sidecar, and paged/sidecar configurations.
The corrected server passed 138 checks, six restarts including interrupted job
recovery, and three repairs of snapshots seeded by that old binary. The repair
test preserves the source and all unrelated JSON values, refuses overwrite and
unsafe plans, then imports and traverses the repaired exact endpoint.

The CG-13 lifecycle suite and the CG-5 construction suite also passed against
the corrected release. All providers in these feature checks were synthetic
loopback servers. The [verification report](../issues/exact-identity-2026-09-09.md)
records Rust gates, test counts, runtime artifacts, and corrected initial
test/build attempts. No production data was migrated.
