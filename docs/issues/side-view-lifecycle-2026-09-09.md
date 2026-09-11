# CG-13 side-view lifecycle — 2026-09-09

Current Unicode storage contract: [CG-33 exact reference identity](exact-identity-2026-09-09.md)
supersedes the generic NFC write/literal default and the temporary non-NFC
side-view source rejection. The measurements below describe this earlier batch.

This batch follows the CG-5 fix and the locally committed CG-8,
CG-15, and CG-16 batch (`41490db`). It changes the server lifecycle for generated
retrieval data. It does not change model defaults or run external model APIs.

## Shared deletion and publication boundary

`side_views.rs` owns the lifecycle shared by `GuardedBackend`, direct document
DELETE, and the generation job. Public policy checks remain in force. Ordinary
HTTP batches, CGQL mutation execution, Lua document/batch/query calls, and
collection drops all use the same cascade logic. Matching uses the exact stored
`document_id` handle and an exact collection prefix, preserving other parents.

Native batches commit parent deletion and generated-row cleanup together.
Appended cleanup operations are removed from the returned batch results so the
caller's result count and order are unchanged. Direct DELETE retains its count
envelope and can remove legacy orphans when the parent is already absent.

Generation captures a revocable token with its source. Tenant/incarnation-scoped
locks serialize source capture, deletion, and publication; providers run outside
the lock. Deleting and recreating the same key does not make the old token valid.
Publication checks the token and current source under the lock, rereads existing
side-views, and holds the lock through the atomic write. Concurrent default
publication skips already-published rows; regeneration replaces the current set.

Invalidated generation fails with an explicit source-conflict diagnostic. Resume
retry reads the current source or skips it if missing. Failed delete attempts
that reached the write phase also conservatively invalidate captured generation.
Weak registrations do not retain completed work or accumulate durable tombstones;
restarted jobs capture fresh sources because provider futures do not survive exit.

Collection DDL is separate from document batches, so cleanup runs **before** the
drop. Cleanup errors preserve the source. A later drop error can leave a source
without retrieval aids; retry the drop or regenerate them. Non-atomic backends
cannot generate side-views, but imported rows can be cleaned sequentially before
a direct delete/drop. Errors stop before removing the parent, making partial
cleanup retryable. Error paths invalidate search results where cleanup may have
committed. Each successful generation write also invalidates results before the
next source can fail.

## Identity restriction and remaining scope

Submission rejects non-NFC collection names/source keys and collection names
containing the document-handle separator `/` before provider calls. Native otherwise normalizes stored job payloads and generated references
while retaining exact primary identifiers. [CG-33](CG-33.md) records the separate
release-confirmed defect affecting opaque references in jobs and graph edges;
CG-13's guard is an interim capability restriction. CG-5 concerns evidence text,
not general identifier repair.

Ordinary parent updates retain the accepted manual-regeneration policy. There is
no automatic historical orphan sweep. Admin snapshot restore retains its separate
job-quiescence boundary. Public opaque query passthrough remains disabled; the
in-process fence does not coordinate independent direct database writers.

## Regression evidence

The [saved pre-fix release](evidence/side-view-lifecycle-baseline-http-2026-09-09.json)
reproduced **18 bypassed cascades** (six alternate paths across three storage
modes) and **42 stale publications** (seven delete paths, two provider waits,
three modes). It used 63 synthetic completion and 63 synthetic embedding calls.

Ten new Rust lifecycle tests cover all shared deletion entry points, batch
result shape, delete/reinsert and collection recreation, transactional rollback,
read/malformed-row/write/drop failures and retries, non-transactional partial
cleanup, overlapping generation, regeneration failure, legacy orphans, publication
commit versus deletion, and tenant/incarnation isolation. The source identity
guard has its own composed/decomposed check.

The [fixed release HTTP artifact](evidence/side-view-lifecycle-http-2026-09-09.json)
passes the following matrix in resident/embedded, resident/sidecar, and
paged/sidecar modes:

| Check | Result |
|---|---|
| Direct, batch, CGQL, Lua delete/batch/query, and collection drop | 21/21 cascades; no remaining generated rows |
| Completion/embedding response paused while each deletion completes | 42/42 stale publications rejected, including 21 same-key recreations |
| Resume retry after those conflicts | 42/42 succeeded; 21 fresh-source writes and 21 missing-source skips |
| Default generation replay | 3/3 skips with no provider calls |
| Ordinary update followed by explicit regeneration | 3/3 retain old rows until regeneration |
| Injected provider failure then retry | 3/3 preserve old rows, then replace them once |
| Failed user batch and successful delete/reinsert batch | 3/3 rollback checks and 3/3 result-shape/cleanup checks |
| Non-NFC source-key and ambiguous collection-name submissions | 6/6 HTTP 400 before provider calls |
| Restart after all operations | 3/3 retain only the expected row and its embedding; zero orphans |

This fixed matrix used **96 synthetic completion calls and 93 synthetic embedding
calls**. Three deliberate completion failures account for the difference.
Vector persistence is checked through snapshot export because sidecar collection
listings intentionally omit vector payloads. The recorded binary SHA-256 matches
the tested release executable.

Final validation on the completed implementation passed:

- `cargo fmt --all -- --check` — PASS.
- `cargo clippy --all-targets -- -D warnings` — PASS.
- `cargo test --all` — PASS: **922 tests**, zero failures/ignored tests, 69 summaries.
- `cargo build --release -p cognigraph-server` — PASS.
- Release HTTP matrix above — PASS.

The new production module is 350 lines; tests follow the file-modularity decision
in separate lifecycle, fault-backend, and tenant-isolation files (each below the
soft cap). The final gates and release HTTP matrix were rerun after the last
source-name guard. CG-5 and CG-13 were committed locally as `36a4a19`; no push was made.

### Corrections encountered during verification

- The initial unit query used a read-only helper, then used a document object
  where REMOVE requires a string key. The first baseline HTTP run likewise
  rejected that query. Both tests now use the supported read-write executor and
  `REMOVE d._key`; the complete baseline matrix then passed its vulnerability
  assertions.
- Clippy initially rejected the nested registry type as too complex. A named
  registry type resolved it; subsequent strict Clippy runs passed.
- The first fixed HTTP run passed resident/embedded and stopped at a sidecar
  vector assertion on a collection listing. The harness now reads snapshot
  export for stored embeddings; all three modes then passed.
- A Unicode investigation expected one generated row but observed zero. This
  exposed CG-33 rather than a model failure. The snapshot probe was corrected to
  inspect the stored `_execution` field; it verifies the changed key and edge
  reference. Side-view submission now rejects the unsupported identifier.

Storage read/write/drop failures are injected in Rust tests against a delegating
Native backend, including a non-atomic capability simulation. They are not a
physical disk-failure or live Arango experiment. The HTTP regression exercises
real Native persistence and job/provider interleavings. General Arango parity
remains in its separate tickets.

## Reproduction

From the repository root:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo build --release -p cognigraph-server
python3 docs/issues/evidence/side-view-lifecycle-http.py
```

The harness runs disposable authenticated servers with synthetic providers on
loopback, pauses completion/embedding responses to control the races, and removes
only its own processes. To reproduce the old failures, pass
`--binary /tmp/cognigraph-pre-cg13-server --expect-vulnerable` while that saved
binary remains available. Artifacts include binary SHA-256 and storage modes;
server logs remain in the temporary directory named by each JSON artifact.
