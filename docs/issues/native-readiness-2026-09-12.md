# Native-only local readiness

- Date: 2026-09-12
- Issue: [CG-68](CG-68.md)
- Baseline: `4fdaa47`, workspace 2.7.1, plus CG-68 documentation and verification changes
- Status: Locally qualified for deployment preparation; no publication or deployment

## Scope

Reconcile active runtime guides with Native-only storage, preserve dated
engineering evidence and qualify both editions for subsequent deployment
preparation. The [first-deployment guide](../operations/first-deployment.md)
owns setup and operational prerequisites. Optional dump import remains deferred
under CG-64/CG-66. No hosted models or research holdouts are part of this gate.

## Qualification

All local qualification checks passed. The
[manifest](../evidence/engineering-native-readiness-2026-09-12.md#artifact-b3200f40deb2ef60a45d) binds source,
protocol/fixture hashes, release binaries, images and captures to this revision.
Rust, Cargo and Docker inputs are unchanged from `4fdaa47`; the local changes
add documentation and the readiness harness. Both release binaries were rebuilt
for CG-67 from those same source bytes and exercised afresh here. Docker rebuilt
both editions with `--pull`, Linux/amd64 and revision labels for `4fdaa47`.

| Qualification | Result |
|---|---|
| `python3 scripts/verify.py --suite ci` | Passed the exact current shared workflow |
| Rust formatting and strict Clippy, both editions | Passed |
| Community / Enterprise Rust tests | 722 / 913 reported passed; zero failed; two ignored per suite |
| UI frozen install, Biome, TypeScript, unit tests, production build | Passed; 161 tests |
| Real API Chromium journeys | 4 Community + 5 Enterprise passed, no retries |
| Script tests / edition dependency checks | 64 tests / four dependency trees passed |
| Documentation, decisions, issues and server modularity | Passed |
| Native release-binary protocol | 257 assertions per edition across four modes; 514 total |
| Startup configuration rejection | 5 Community + 7 Enterprise cases failed before creating storage |
| Cross-edition HTTP/CLI probe | File/snapshot compatibility, ordinary operations, edition refusal and tenant isolation passed |
| Documented fresh-start example | Community identity, persistent file and anonymous HTTP 401 passed |
| Linux/amd64 Community + Enterprise Docker images | Built and passed hardened runtime checks |
| Both Helm live backup checks | Rendering/rejections, environment, probes, authentication, seeded document and packaged backup passed |

The [Native captures](../evidence/engineering-native-readiness-2026-09-12.md#artifact-655fcef3ecb676aece66) and
[Enterprise captures](../evidence/engineering-native-readiness-2026-09-12.md#artifact-eba4980d725a2315b53c) cover
memory, resident/embedded, resident/sidecar and paged/sidecar stores. Each tests
CRUD persistence, CGQL/Lua writes and read permissions, protected collections,
query languages and row budgets, atomic rollback, traversal, BM25/vector/hybrid
search, cache reuse, snapshots and restart. A deterministic loopback embedding
stub supplies the search vector. The 12 startup cases cover bootstrap/auth/JWT,
listen address, Community edition refusal and Enterprise completion/judge/CAS
configuration. Completion/judge validation is Enterprise-only; it is not an
expected Community rejection surface.

The [browser record](../evidence/ui-2026-09-12-native-readiness.md#artifact-c9962a91f863ef6fac4a) binds
fresh production-asset journeys to the Rust debug binaries. These scoped
regressions do not cover every console feature or viewport. The
[image identities](../evidence/engineering-native-readiness-2026-09-12.md#artifact-393355e38234cec5160b) identify
the two locally built images. Runtime checks verify non-root execution,
read-only root filesystem, authentication, edition API, CLI, packaged licences,
restart and persisted CGQL. Helm checks run isolated Docker containers and
back up synthetic data; they are not a Kubernetes deployment or restore drill.

The Rust totals include two construction live-loop cases per suite that return
early under `COGNIGRAPH_LIVE_LLM=0`; two embedding-provider cases per suite are
ignored. No hosted provider, model benchmark, research holdout or external
database was run. Owned temporary stores/containers were removed. Existing local
environment files and user databases were not changed.

## Documentation boundary

Active operator, architecture, data preparation and API guides describe Native
directly. Sixteen earlier decisions receive a current-storage amendment; their
original measured outcomes remain unchanged. The native-first successor and
decision inventory now point to implemented Native-only behavior. The current
configuration guide also describes the actual tenant-directory precedence;
operators should set only the intended storage root.

The [reference inventory](../evidence/engineering-native-readiness-2026-09-12.md#artifact-927d9de815af0bd592ba)
classifies remaining tracked Arango/AQL references. They are dated decisions,
research, captures and batch history; external migration/language comparisons;
or explicit dependency guards, isolation fixtures and query-language rejection
tests. None selects an active runtime adapter. Earlier mutation design notes
are marked historical. Historical issue identities, goldens, fixture bytes and
sealed measurement packages are preserved.

## Separate product and website follow-up

The owner of the following work is the corresponding sibling repository. Paths
were inspected read-only; no edit or publication is claimed.

| Owner / path | Required alignment |
|---|---|
| Product docs: `../docs/product/overview.md` | Remove the maintained adapter, backend choice and obsolete crate/architecture entries |
| Product docs: `../docs/sales/DECK-NOTES.md` | Replace two-backend sales claims and adapter-specific capability language |
| Product docs: `../docs/research/semantic-neurons/README.md` | Update its current runtime boundary; preserve dated research evidence |
| Website: `../website/src/pages/AqlPage.tsx` | Remove the conformance-backend claim and stale CogniGraph 2.6 scope; keep external migration teaching |
| Website: `../website/design/language-briefs.md` | Align current migration scope; preserve sealed example-query evidence |
| A9 teaching module | No separate A9 source was located in the inspected code, product-docs or website-docs files; identify its owner/path before editing. The existing external AQL guide remains available |

The website positioning, homepage migration hook and product website brief can
retain AQL familiarity as external migration education. That does not promise
complete AQL compatibility or a shipped dump importer. CG-64/CG-66 stay deferred.

## Publication and deployment boundary

CG-68 closes local readiness only. No remote CI run, incoming-PR integration,
version increment, push, tag, image publication or deployment was initiated.
Workspace version 2.7.1 identifies this local checkpoint, not a new published
Native-only release. Before an authorized push, inspect incoming work, apply
the normal version increment, rebuild/requalify the final versioned candidate
and recheck the target head and PRs. Any changed runtime/build inputs require
fresh applicable checks.

Native-only code is ready for deployment preparation. The chosen live target
still needs its own persistent-volume, network/TLS/auth, monitoring and restore
validation under the [first-deployment guide](../operations/first-deployment.md).
Passing local checks or uploading an image does not complete those operations.
