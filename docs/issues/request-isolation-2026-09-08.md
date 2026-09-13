# CG-12 request isolation — 2026-09-08

CG-12 is resolved in the local worktree. No commit or remote publication was
performed. The React UI was outside this batch.

## Change

Authentication now holds the tenant lifecycle lock from credential validation
through tenant-status validation and capture of the immutable incarnation,
concrete backend, and cache. It releases the lock before body extraction and
handler execution. Routed operations use those captured handles even after
retirement; they never resolve the old name to a replacement registry entry.

`TenantContext` transfers that identity into Lua's blocking backend adapter
and its async cleanup supervisor. Job/governance request helpers retain the
captured incarnation. Independently scheduled durable work continues to use
its existing lifecycle checks, checkpoints, and incarnation fences.

User/token administration operates on the shared control store. Its common
live-administrator check now returns a lifecycle guard retained through every
operation, preventing deletion/recreation between authorization and access.
Host administration does not pin or open a default data store. Auth-disabled
and single-store routing retain their existing behavior.

## Rust verification

All required gates passed:

```bash
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo build --release -p cognigraph-server
```

The workspace suite passed **869 tests**, with zero failures or ignored tests
across 66 result summaries; the server passed 260 tests. Six new regressions
cover deletion before the first backend call, recreation with a same-key
document, suspension during embedding, admission waiting for lifecycle
completion, already-admitted user administration waiting for lifecycle
completion, and Lua/store/cache/incarnation preservation across task transfers.
Existing durable queue recovery, deletion, governance, and signed M26 lifecycle
tests also passed. `git diff --check` passed.

An initial compile needed an explicit error type for the fallible handle-capture
closure; the corrected code passed all gates. The first baseline HTTP probe
incorrectly expected sidecar vectors inline in a document response. It was
corrected to verify them through real vector search; both baseline and fixed
probes then completed successfully.

## Real release HTTP verification

The pre-fix executable was preserved before rebuilding. Both executables ran
against disposable, authenticated, loopback-only multitenant stores, with a
local HTTP embedding provider paused until the tenant lifecycle operation
completed. No external provider or production data was used.

| Interleaving | Pre-fix release, both modes | Fixed release, both modes |
|---|---|---|
| Delete while embedding waits; resume before recreation | Old request opens a live database for the absent name; new tenant inherits its document | No live database is reopened; new tenant reads 404 |
| Delete and recreate while embedding waits; seed the same document key | Old request overwrites replacement text and adds its vector | Entire replacement document stays equal; old vector search finds zero rows |
| Suspend while embedding waits | New requests receive 403; admitted write finishes | Same declared drain behavior; activation exposes the completed write |
| Restart after deletion/recreation checks | Contamination survives restart | Isolation survives restart |

The fixed release additionally admitted a user-management request, paused its
JSON body upload, deleted/recreated its tenant, then completed the upload.
The request returned 401 and the replacement user list stayed unchanged.
Fresh user creation, token creation/listing, user deletion, and refusal of the
deleted user's token also passed in both resident and paged modes. Old tenant
credentials returned 401 in both lifecycle probes.

Evidence includes executable SHA-256 hashes:

- [Pre-fix observations](../evidence/engineering-historical-checks.md#artifact-c7111c26d7efc235815d)
- [Fixed observations](../evidence/engineering-historical-checks.md#artifact-4859dacdbe396807c921)
- [Reproduction script](../evidence/engineering-historical-checks.md#artifact-cc2a98d243e9f2557967)

Run the checked script from the repository after building the release binary:

```bash
python3 docs/issues/evidence/request-isolation-http.py
```

`--binary PATH` selects another executable; `--expect-vulnerable` verifies
the pre-fix behavior. Each run creates disposable databases in the OS temporary
directory, reports its results path, and stops all test servers.

## Contract and tradeoffs

Suspension/deletion closes admission; it does not cancel every ordinary
request already admitted. Those requests may finish against the old store and
cache. Quarantined redb files can therefore still receive writes until the
last admitted worker drains. Operators should wait for that work or stop the
server cleanly before recovery/purge. This implements the existing T4 drain
contract rather than introducing rollback or request cancellation.

The first authenticated tenant request now opens its store during admission,
even when its handler has not yet touched data. Admission and control-store
user administration share the existing global lifecycle mutex; cold store
opens or lifecycle recovery can delay other admissions. Provider execution
does not hold that mutex. This batch makes no new throughput claim.

The registry now has **12 Resolved and 19 Open issues**: no open P1, 18 P2,
and one P3. The next bounded quick win is CG-31's hybrid cache fingerprint.
