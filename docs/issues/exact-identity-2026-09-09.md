# CG-33 exact reference identity verification — 2026-09-09

CG-33 gives generic storage and CGQL literals the same exact Unicode identity
contract. Canonically equivalent collection names and keys remain distinct;
references, nested payloads, model names, and durable job inputs retain the
caller's representation. `NORMALIZE_NFC` supplies explicit canonical text
comparison. Construction evidence and governed canonicalization keep their
existing explicit boundaries.

CG-21 was committed locally as `f74f0aa` before this work. This report describes
the CG-33 changes on top of that revision, committed with CG-22 as `9fb4933`. No push or
production migration was performed. Unrelated research/drafts and the existing
`CLAUDE.md` deletion were preserved.

## Implementation and compatibility

- Removed recursive Native normalization from document and edge stamping;
  metadata stamping and exact primary keys remain unchanged.
- Removed NFC from CGQL literal decoding. Literal and bind-variable document
  lookups now agree. Exact equality and codepoint sorting are unchanged; callers
  explicitly normalize designated text with `NORMALIZE_NFC(value)` when desired.
- Removed CG-13's temporary non-NFC source rejection. The collection `/`
  delimiter restriction remains; no query grammar/backend routing was added.
- Added bounded offline `references audit` and explicit snapshot-hash-bound
  `references repair` commands. Only approved ordinary root references can
  change; keys, system/governed/generated records, and unrelated JSON values
  remain untouched. Output uses the existing private atomic no-overwrite writer.

This intentionally changes the historical implicit NFC text behavior. Existing
data is not automatically rewritten; a resolved canonical reference may already
point at the wrong existing document. See the [decision](../decisions/decision_exact_reference_identity.md)
and [operator procedure](../operations/reference-repair.md) for scope, limits, ambiguity,
reviewed repair plans, protected workflows, and quiescent import requirements.

## Rust validation

The focused query/core/CLI/Arango/Native run passed. New coverage includes three
Native lifecycle regressions across resident/embedded, resident/sidecar, and
paged/sidecar: CRUD/batch writes, exact edge upserts, model filters, literal/bind
lookups, dynamic resolution, pushed equality, snapshot restore, and persistent
reopen. Five CLI tests cover diagnosis, protected/unsafe/stale mappings, report
truncation, required arguments, and no-overwrite output. The query corpus pins
exact literal/bind equality, explicit normalization, type handling, and arity.
The server route regression now accepts exact non-NFC identities and still
rejects ambiguous collection delimiters.

Final workspace validation and counts are recorded in
[the gate artifact](../evidence/engineering-historical-checks.md#artifact-ae8238270f2656206dba).
`cargo fmt --all -- --check`, `cargo clippy --all-targets -- -D warnings`, and
`cargo test --all` passed. The complete test run reported 945 passed, zero failed,
and zero ignored across 71 test/doc-test result groups. Eight reported passes
were the unconfigured Arango integration entries described below; the new
Native/CLI regressions executed. The focused run reported 335 passes. Both
existing provider embedding smoke entries reported success as part of the
workspace run; their local configuration and defaults were unchanged.

Arango's unchanged adapter was inspected: accepted JSON values pass through
without recursive normalization. The Arango integration entries early-return
when `ARANGO_PASSWORD` is absent; this batch did not run a live Arango instance or
claim Unicode identifier acceptance beyond that database's own grammar.

## Release HTTP and repair evidence

The saved pre-fix binary contains CG-21 at `f74f0aa`. Every feature run used
disposable synthetic stores and local loopback completion/embedding servers.
No real provider or model qualification was exercised by these harnesses.

| Suite | Verified outcome |
|---|---|
| [Old-binary identity reproduction](../evidence/engineering-historical-checks.md#artifact-3f36ba05a7d3efafe7d9) | 105 checks, 69 desired-contract mismatches, three restarts |
| [Corrected exact identity and repair](../evidence/engineering-historical-checks.md#artifact-edff67199edb01f43b5f) | 138 checks, zero mismatches, six restarts, three repaired old-binary snapshots |
| [Side-view lifecycle regression](../evidence/engineering-historical-checks.md#artifact-24081eeffb4759a463f8) | 21 delete cases, 42 controlled provider races/retries, three accepted non-NFC sources, three rejected `/` collections, regeneration/rollback checks, three restarts |
| [Construction evidence regression](../evidence/engineering-historical-checks.md#artifact-e9c2831ed3f5986c50f3) | 30 ingestion/reconciliation calls, three snapshot roundtrips, six restarts; all checked hashes/spans valid |

The identity suite creates four distinct collection/key combinations from
composed and decomposed spellings. It checks exact HTTP storage, CGQL literal
and bind resolution, Lua lookup, edge endpoints, traversal, model filtering,
batch payloads, and mutation literals. The old binary silently changes values
and literal targets while exact binds still resolve the intended document.

On the corrected binary, the suite freezes a side-view job over two exact source
keys, blocks completion, inspects the persisted `_execution`, kills the server,
and restarts the same store. Recovery writes exactly two rows to the correct
parents. Deleting the decomposed parent removes only its row, preserves its
composed peer, and remains correct after another restart. The job remains
terminal. These cases run in all three persistent Native configurations.

The repair fixture uses the old binary to write an edge intended for a decomposed
key while a composed peer already exists. Audit reports that ambiguity. An
explicit `_from` mapping produces a separate snapshot; the test compares the
entire parsed document against the original with only that one approved field
changed. It verifies original bytes are unchanged, protected synthetic opaque
values survive, existing output is not overwritten, unsafe/protected/stale-hash
plans produce no output, and import into the corrected server restores exact
traversal. All three repaired output files have mode `0600` on this host.
The protected fixture is synthetic, not proof of a real signature qualification.

Identity tests made nine local completion requests and six embedding requests;
the blocked pre-kill requests account for three completion attempts. The
side-view suite made 99 local completion and 96 embedding requests. Construction
made 15 local completion requests and preserved the NFC evidence boundary.
These are regression observations, not model benchmarks. Luna settings remain
unchanged and the broader model comparison remains deferred.

## Corrected attempts and limits

- Initial Native tests used nonexistent option defaults, an incorrect traversal
  field, a read-only mutation entry point, an extra bind, and unsupported AQL
  `@@collection` syntax. They were corrected to the existing explicit options,
  read/write executor, strict binds, and supported CGQL sources. No syntax was
  added to accommodate the fixture.
- Initial CLI compilation exposed the SHA-256 output formatting API and a test
  borrow conflict; formatting now handles digest bytes explicitly. A formatting
  attempt during temporary module wiring also failed before the file existed.
- The first release build exhausted local disk space. Only rebuildable Cargo
  `target/debug/incremental` cache was removed; sources and data stores were
  untouched. The release server and CLI then built successfully.
- The first HTTP fixture used JSON's `\uXXXX` escapes as Lua source and the wrong
  Lua getter name. The corrected fixture passes Unicode directly and uses the
  existing `graph.get_document`. Baseline and corrected runs then completed.
- No final release regression failed. Existing malformed production references
  were not repaired, and the scanner cannot reconstruct lost source intent or
  infer arbitrary application-defined reference/signature fields.

## Reproduce

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo build --release -p cognigraph-server -p cognigraph-cli
python3 docs/issues/evidence/exact-identity-http.py \
  --output /tmp/cg33-identity.json
python3 docs/issues/evidence/side-view-lifecycle-http.py \
  --output /tmp/cg33-lifecycle.json
python3 docs/issues/evidence/unicode-evidence-http.py \
  --output /tmp/cg33-construction.json
```

Pass `--seed-binary /path/to/pre-CG33-server` to the identity harness to include
offline repair. Use `--binary /path/to/pre-CG33-server --expect-vulnerable` for
the baseline reproduction. The lifecycle harness's `--expect-nfc-rejection`
option retains the previous CG-13 boundary when testing that historical binary.
Artifact hashes, gate totals, and the tested binary identities accompany the
validation artifact. Next is CG-22, reconciling the public API/operator references.
