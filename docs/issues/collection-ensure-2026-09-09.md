# CG-35 collection ensure and startup reuse verification — 2026-09-09

Native collection ensures now leave existing collections unchanged, including
process write versions and durable generation/revision metadata. Authenticated
startup can reuse current vector files and Tantivy indexes. The corrected
release preserved 18 derivatives that the saved release unnecessarily rewrote;
112 checks across nine authenticated restarts also verified the rebuilds still
required by actual writes.

CG-38 was committed locally as `aae43ad`, excluding unrelated research/draft
files and the existing `CLAUDE.md` deletion. CG-35 was committed locally as
`d98e374`; nothing was pushed.

## Cause, scope, and compatibility

The [Native collection ensure](../../crates/cognigraph-native/src/memory/backend.rs)
previously persisted `PutCollection` on every call, even when the collection
already existed. `persist` advanced the process write version; the redb
transaction advanced `data_generation` and assigned a new `data_revision`.
Auth initialization ensures its three control collections at each startup,
which made persisted derivatives stale without changing application data.

The existing exclusive state lock now covers an early catalog-existence check.
An existing name returns successfully before persistence; an absent name still
commits before publication. Concurrent ensures cannot both observe an absent
name, and collection deletion uses the same lock. Original collection types
are preserved even when a later ensure requests a different type, matching
the previous Native behavior. Arango's existing collection-conflict handling
also returns success; its implementation and the shared API are unchanged.

Only the Native ensure method changes production behavior. No storage format,
dependency, query routing, or new locking mechanism is introduced. Existing
files produced by the prior release reopen directly when their revision is
current. Newly created collections and actual mutations still invalidate the
appropriate derivatives. See the [storage contract](../architecture/native-storage.md#collection-ensure-idempotency-cg-35).

Vector warm loading still scans redb to reconstruct key/model metadata before
validating the mmap file. This fix prevents unnecessary file rebuilding; it
does not remove that scan or claim a measured startup-latency improvement.
Tantivy's existing warm-open path can reuse its persisted index directly.

## Rust verification

All three new [regression tests](../../crates/cognigraph-native/src/memory/collection_tests.rs)
failed against the original ensure method. They now cover memory-only,
resident/embedded, resident/sidecar, and paged/sidecar configurations:

- Repeated same/opposite-type ensures preserve exact snapshots, original
  types, process versions, and durable generation/revision. Real creation,
  drop/recreation, and implicit document-collection creation retain their
  behavior.
- Sixteen simultaneous mixed-type ensures create one collection and commit
  exactly once. Persistent reopen preserves the resulting catalog, snapshot,
  and revision.
- Loaded text indexes are reused after no-op ensures. Reopened sidecar stores
  perform zero vector rebuilds when the revision is unchanged; creating a real
  collection still requires a rebuild.

Final checks:

- `cargo test -p cognigraph-native`: **61 passed**, zero failed/ignored.
- `cargo fmt --all -- --check`: passed.
- `cargo clippy --all-targets -- -D warnings`: passed.
- `cargo test --all`: **958 reported passes** in 72 result groups, zero
  failed/ignored. Eight unconfigured Arango integration entries early-returned;
  this does not provide new live Arango coverage.
- `cargo build --release -p cognigraph-server -p cognigraph-cli`: passed.

The standard formatting, Clippy, and full-test gates run in that order. The
[validation artifact](../evidence/engineering-historical-checks.md#artifact-1f7113a94674f37ec51f)
records logs, hashes, documentation checks, and unrelated-file preservation.

## Release HTTP, Lua, and file evidence

The [harness](../evidence/engineering-historical-checks.md#artifact-2a4e1cdbef2f955eb019) starts disposable authenticated
servers outside the checkout with explicit configuration and no providers.
It covers resident/embedded, resident/sidecar, and paged/sidecar stores. Each
mode has a seed process and three subsequent authenticated restarts. The
corrected run seeds stores using the saved pre-fix release, then opens and
modifies them using the rebuilt release. All processes stop after their run.

The saved server has SHA-256
`9bf65bcdae28e09dca921dae0025a2f0fe78984f0219eff4634731223472a903`.
The corrected server has SHA-256
`eed0bdac848e4928e730881f12a7e443e9ccb2c236a4d6ec67e394f2695fb806`.
The artifacts record HEAD during verification; binary hashes distinguish the
saved executable from the build with uncommitted CG-35 changes.

Each release run records **112 checks**:

| Observation | Count across three modes |
|---|---:|
| Exact projected query rows over seven lifecycle phases | 21 |
| Vector winner and exact score through HTTP and Lua | 42 |
| Text-search winner | 21 |
| Catalog types after 16 repeated/mixed-type ensures per mode | 3 |
| Derivative revision, content, and filesystem identity comparisons | 25 |

The lifecycle covers seeded files, initial restart, explicit no-op ensures,
real collection creation, changed text/vectors, a restart with unflushed vector
deltas, and a final unchanged restart. After mutations, the expected vector
and text winner changes from document `a` to `b`; old results cannot satisfy
the controls.

File checks include full vector SHA-256, payload SHA-256, generation, revision,
mtime, and inode. Text checks include the persisted revision, identity hash,
`meta.json` SHA-256, mtime, and inode. These establish actual file reuse; result
equality alone would not distinguish a correct rebuild from a warm open.

The [baseline artifact](../evidence/engineering-historical-checks.md#artifact-53943152ccf73f48f086)
uses `--expect-rebuilds` and reproduces **18 unnecessary rewrites**: six vector
files and twelve text indexes across no-op phases. Its other seven file
rewrites are expected after real changes. The
[corrected artifact](../evidence/engineering-historical-checks.md#artifact-9389ab6b8ad776bb70b5)
preserves those 18 files exactly and still observes **seven required rebuilds**:
five derivatives after actual collection creation and two vector files after
reopening stores with unflushed deltas. The latest text indexes survive that
data restart because their preceding search already persisted the current
revision. All 84 result checks and three catalog checks pass in both runs.

## Reproduction and limits

```bash
cargo test -p cognigraph-native
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo build --release -p cognigraph-server -p cognigraph-cli
python3 docs/issues/evidence/collection-ensure-http.py --output /tmp/cg35-http.json
```

While the saved binary is available, reproduce the old behavior and verify
compatibility separately:

```bash
python3 docs/issues/evidence/collection-ensure-http.py \
  --binary /tmp/cognigraph-pre-cg35-server --expect-rebuilds \
  --output /tmp/cg35-baseline-http.json
python3 docs/issues/evidence/collection-ensure-http.py \
  --seed-binary /tmp/cognigraph-pre-cg35-server \
  --output /tmp/cg35-fixed-http.json
```

The pre-fix failures of all three new Rust tests were intentional reproductions.
The final focused suite and both live matrices completed without failed
verification attempts. Baseline expected rewrites are historical defect
observations, not desired-behavior passes. No live Arango, performance, partner
workload, or model benchmark is claimed; concurrent creation is exercised by
Rust tests, not concurrent HTTP requests. Original research/draft work, `ui/`,
secrets, and local environment files remain untouched.

Local links, registry counts/statuses, binary and harness hashes, and all 24
pre-existing user-owned paths were checked. CG-35 is resolved; at this checkpoint
the registry had **32 Resolved and 6 Open** issues. The next bounded fix,
CG-36 dedicated judge endpoint configuration, was subsequently completed in
its [verification report](judge-endpoint-2026-09-09.md). Model benchmarking
remains deferred with Luna as the economical baseline.
