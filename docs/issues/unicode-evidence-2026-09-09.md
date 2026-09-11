# CG-5 canonical construction evidence — 2026-09-09

Current Unicode storage contract: [CG-33 exact reference identity](exact-identity-2026-09-09.md)
supersedes the generic NFC write/literal default and the temporary non-NFC
side-view source rejection. The measurements below describe this earlier batch.

The completed CG-8, CG-15, and CG-16 work was committed locally as `41490db`.
Unrelated research/draft changes and the user-deleted CLAUDE.md were excluded.
Nothing was pushed. This subsequent batch addresses construction evidence text
that Native storage changed after its hashes and offsets had been calculated.

## Representation and implementation

Construction uses **NFC chunk text** before evidence grounding, content hashing,
and fact occurrence-key generation. `content_hash` is SHA-256 of the exact
stored NFC UTF-8 bytes. Trigger start/end positions refer to those bytes, not
to a decomposed upload's byte positions. No whitespace, case, or compatibility
normalization is added. Caller-owned chunk/space identifiers are not rewritten
by this evidence-text boundary.

The shared writer supplies canonical text to both ordinary rule grounding and
custom grounding callbacks. It validates every returned fact's evidence chunk
identity and non-empty UTF-8 range against its trigger before submitting the
atomic replacement batch. A stale precomputed span cannot replace valid stored
evidence. Trigger comparison retains the existing case-insensitive semantics.

Directed ingestion canonicalizes excerpts before calling the provider. Its
pure gate compares canonically equivalent source/target/evidence strings and
restraint phrases in NFC, then returns a trigger and span from the canonical
chunk. Taxonomy membership, endpoint presence, affirmation, and reconciliation
remain enforced. The live regression uses a synthetic loopback provider;
it does not evaluate Luna or make real model calls.

Pure fact derivation and materialization use the same chunk representation,
so their facts, occurrence keys, hashes, and spans agree with ingestion.
Materialized projection validation explicitly rejects non-NFC chunk text.
Already-canonical chunk sets are borrowed without copying their text. Native's
write-time normalization and raw source-byte custody are unchanged; the
construction boundary now precedes the byte-sensitive work.

## Existing data and compatibility

NFC inputs retain their existing representation and occurrence keys. Previously
ingested decomposed text can have a bad stored hash, shifted/out-of-bounds
spans, and old occurrence keys. **Explicit re-ingestion** replaces that chunk's
text/hash/mentions/facts together and removes the stale occurrences. It can
also recover facts that could not match the stored rule vocabulary before
normalization. There is no automatic migration on startup or read.

Snapshots preserve the bytes they contain. Importing an old inconsistent
snapshot alone does not repair its evidence; re-ingest the affected chunks.
For governed materialized generations, retain the deployment fence and rebuild
through the governed derivation/approval flow. Do not edit previously signed
artifacts in place. The unit and HTTP repair regressions use undeployed spaces.

## Verification

Five new tests cover decomposed accents before and inside a trigger, Hangul
composition after an emoji, normal/directed ingestion, canonical-equivalent
re-ingestion, exact hash checks, valid byte slices, stable occurrence keys,
pure derivation/materialization parity, noncanonical projection rejection,
stale callback rejection, and repair of legacy inconsistent records.
Persistent tests use resident/embedded, resident/sidecar, and paged/sidecar
storage, plus fresh snapshot imports and reopened source/restored stores.

The saved pre-fix release reproduced the defect through both ingestion routes
in all three supported persistent configurations: six ingestions and three
restarts. For `Cafe\u0301. Alpha supplies Beta.`, storage kept the 27-byte NFC
text but the hash of the 28-byte input. The ordinary trigger span selected
`upplies ` instead of `supplies`; the directed whole-chunk quote ended at byte
28, past the stored text. The raw evidence records both hashes, slices, and
occurrence keys.

The initial test helper used a hex-format trait that this SHA-256 dependency
does not implement; it was corrected to format digest bytes individually.
All five focused tests and **184 construction-crate tests** passed. Formatting,
Clippy with warnings denied, and **all 912 workspace tests** passed, with zero
failures or ignored tests across 69 result summaries. The optimized release
server built successfully.

The corrected release passed **30 ingestion/reconciliation calls** through
ordinary and directed ingestion across resident/embedded, resident/sidecar,
and paged/sidecar storage. The checks independently recomputed hashes and
sliced stored UTF-8 bytes, compared complete logical chunk/fact/mention rows
across NFD/NFC/NFD re-ingestion, withdrew one chunk's facts without removing
the others, and restored its original occurrences. For the original example,
the ordinary span is now 13..21 (`supplies`) and the directed whole-chunk span
is 0..27; both hashes match the stored 27-byte NFC text.

The same run seeded **six inconsistent projections with the saved old binary**
(both ingestion paths in each storage configuration), then repaired them with
the corrected binary. All old occurrence keys were removed, and no duplicate
facts survived. **Three snapshot roundtrips and six restarts** preserved the
verified projections exactly. Eighteen synthetic loopback completion calls
included the three old-binary seed calls; every corrected-binary prompt used
canonical NFC excerpts. These are regression calls, not a model benchmark.
No final Rust gate or live check failed.

CG-5 is resolved and committed locally as `36a4a19`; nothing was
pushed. The registry now has 20 resolved and 12 open issues. Next is CG-13,
side-view cleanup across alternate deletion paths and generation races.

```bash
cargo test -p cognigraph-construct
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo build --release -p cognigraph-server --bin cognigraph-server
python3 docs/issues/evidence/unicode-evidence-http.py \
  --seed-legacy-binary /path/to/saved-pre-cg5-server
```

The HTTP script also runs without a legacy binary, checking new ingestion,
reconciliation, snapshot roundtrips, and restarts. `--binary ...
--expect-vulnerable` reproduces the original corruption on a saved release.
All stores, credentials, records, and provider responses are synthetic and
local. No environment file is edited.

- [Reproducible release-server regression](evidence/unicode-evidence-http.py)
- [Pre-fix observations and binary hash](evidence/unicode-evidence-baseline-http-2026-09-09.json)
- [Corrected release, legacy repair, snapshot, and restart evidence](evidence/unicode-evidence-http-2026-09-09.json)
