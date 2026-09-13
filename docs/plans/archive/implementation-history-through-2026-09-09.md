# CogniGraph Rust — Implementation Plan

> Amended reading copy — 2026-09-13. Link targets have been updated for the current
> repository layout; dated claims retain their original scope.
> The exact original is preserved in the private evidence repository.
> Original SHA-256: `7384394bfffa190f66fd74959a73b4f28a61c99faf1ff16a820b44c90b2b9f3d` (118510 bytes).
> See the [navigation amendment](../archive-navigation-2026-09-13.md) for its path and provenance.

## Documentation organization — planning, 2026-09-10

The user requested documentation cleanup and confirmed that product strategy,
positioning, sales, papers and publishing material belong in the sibling
`/Users/skitsanos/FTP/Products/CogniGraph/docs` directory. Engineering contracts,
decisions, issue records and reproducible experiments remain with the code.
The [layout and migration proposal](../../plans/documentation-layout-2026-09-10.md)
defines the suggested folders, draft reconciliation, smaller README and individual
changelog records. No moves have been performed yet. Root/UI AGENTS.md, hooks,
skills and related workflow policy will be reviewed after the documentation
migration, with only necessary path corrections included in that migration.

## Review checkpoint — 2026-09-09

The completed CG-7, CG-29, CG-32, and model-baseline work was committed locally
as `41cdb8a`. Unrelated research/draft changes were excluded; nothing was pushed.
The [dynamic-query batch](../../issues/dynamic-query-2026-09-08.md) resolved CG-8 and
CG-15 together: normal execution and analysis share backend resolution, one
row counter spans all phases, and reports distinguish settled rows from
attempted retry work. Formatting, Clippy, all 901 tests, 60 release
normal/analysis comparisons, 240 budget-boundary executions, plain EXPLAIN,
and resident/paged restart checks passed. These changes were committed below.
The subsequent [document-error batch](../../issues/document-errors-2026-09-08.md)
resolved CG-16: backend failures abort DOCUMENT lookups, with 403/503/500
responses preserved through normal/analyzed HTTP and Lua execution. Missing
documents still return null. Formatting, Clippy, all 907 tests, 72 release
forbidden-lookup cases, 72 controls, 36 plain EXPLAIN checks, 12 Lua pcall checks,
and resident/paged restarts passed. Injected route tests verify connection and
storage failure status mappings. Both batches were committed as `41490db`
on 2026-09-09, excluding unrelated research/draft changes. Nothing was pushed.
The [canonical-evidence batch](../../issues/unicode-evidence-2026-09-09.md) resolved
CG-5: NFC chunk text precedes grounding, content hashes, byte spans, and fact
occurrence keys across ordinary/directed ingestion and pure materialization.
Stale custom-grounder spans are rejected before the atomic write. Formatting,
Clippy, all 912 tests, 30 release ingestion/reconciliation calls, six old-binary
repairs, three snapshot roundtrips, and six restarts passed across all supported
persistent Native configurations. Existing inconsistent rows require explicit
re-ingestion; governed generations retain their rebuild/approval boundary.
CG-5 was committed locally with CG-13 as `36a4a19`.

The [side-view lifecycle batch](../../issues/side-view-lifecycle-2026-09-09.md) resolved
CG-13. Public deletes share cascade cleanup with a tenant/incarnation-scoped
publication fence; provider calls stay outside the critical section, and stale
results cannot publish after delete/recreation. Native deletes are atomic;
collection drops clean up first and propagate retryable failures. Formatting,
Clippy, all 922 tests, 21 release delete cases, 42 provider races and retries,
six source-identity rejections, regeneration/rollback cases, and three restarts
passed across resident/embedded, resident/sidecar, and paged/sidecar modes.
CG-13 temporarily required NFC source handles; CG-33 below removes that
restriction while retaining the `/` collection-name rejection. CG-5 and CG-13
were committed locally as `36a4a19`; nothing was pushed. The registry has
36 resolved and 2 open tickets after the CG-30 work below.

The [traversal confidence batch](../../issues/traversal-confidence-2026-09-09.md)
resolved CG-19: Arango applies the inclusive threshold to every edge, including
before `min_depth`, defaults missing/nonnumeric confidence to 1.0, and preserves
depth zero. Native already follows that contract. The shared 15-case fixture,
six live ArangoDB 3.12.11 integration tests, formatting, Clippy, all 923 workspace
tests, and 240 release HTTP/Lua checks across four backend configurations and
server restarts passed. The saved release reproduced 48 Arango mismatches;
the corrected release produced none. The disposable Arango container was
removed afterward. CG-19 was committed locally with CG-20 as `fb49c78`;
nothing was pushed.

The [Arango vector candidate batch](../../issues/vector-model-filter-2026-09-09.md)
resolved CG-20. Model filtering precedes indexed candidate truncation; both
indexed and fallback queries expand candidates until enough distinct parents
are found or candidates are exhausted. Filtered indexed search requires ArangoDB
≥3.12.6; older deployments can select fallback. The 12-case fixture, eight live
ArangoDB 3.12.11 integration tests, formatting, Clippy, all 925 workspace tests,
and 88 release HTTP/Lua checks passed. The captured query plan uses the vector
index with the model filter, and duplicate-parent windows expand from 4 to 64.
The disposable Arango container was removed afterward. CG-19 and CG-20 were
committed locally as `fb49c78`, excluding unrelated research/draft changes;
nothing was pushed.

The [Native sidecar model-selection batch](../../issues/sidecar-model-filter-2026-09-09.md)
resolved CG-34. Resident/sidecar and paged/sidecar filter base and delta
candidates by model before truncation, using metadata reconstructed from redb
without a file-format migration. Formatting, strict Clippy, all 928 reported
Rust tests, and 162 release HTTP/Lua checks passed across the three Native
configurations. The checks cover model-only and batch writes, removed fields,
deletion/recreation, threshold rebuilds, and old-release stores with restarts.
Direct Native tests verify unchanged-revision file reuse. Authenticated startup
unnecessarily advances the revision through existing collection ensures;
that separate performance defect is CG-35. An initial harness assumption about
whole-server file reuse failed and was corrected, as recorded in the report.
CG-34 was committed locally as `8164f4e`; nothing was pushed.

The [neuron lifecycle batch](../../issues/neuron-lifecycle-2026-09-09.md) resolved
CG-21. Human transitions, automated publication, and snapshot import share a
tenant/incarnation boundary. Provider calls stay outside the lock; publication
requires the exact proposed source, policy, and ontology to remain current.
Both judge lanes validate the current accepted set, and stale results preserve
human decisions. Read failures and malformed accepted rows fail closed. The
response identifies skipped results and retains changed proposals in its
pending count when they still need review. Formatting, strict Clippy, all 937
reported Rust tests (including nine new regressions), and 99 release HTTP
scenarios passed, with 162 exact neuron documents preserved
after three Native restarts. The baseline reproduced 36 deterministic stale or
conflict cases and 26 forbidden human pairs. Dedicated judges ignored the configured
OpenAI base URL at that checkpoint (CG-36, resolved below); live verification used the configured fallback judge,
with A+ partner races covered through the production routers. CG-21 was committed
locally as `f74f0aa`; nothing was pushed.

The [exact-identity batch](../../issues/exact-identity-2026-09-09.md) resolves CG-33.
Generic Native writes and CGQL literals preserve exact Unicode strings, so keys,
endpoints, model identities, frozen jobs, and parent references agree. Canonically
equivalent keys remain distinct. `NORMALIZE_NFC(...)` provides explicit canonical
text comparison; CG-5 construction evidence keeps its own NFC boundary.
Side-view generation now supports non-NFC source handles through restart and
cascade deletion. The offline `references audit` and snapshot-hash-bound
`references repair` commands diagnose historical ambiguity and apply only
explicit ordinary-reference mappings to a new snapshot; protected workflows
must regenerate or resubmit. The saved release reproduced 69 mismatches; the
corrected release passed 138 HTTP/Lua checks, six restarts, and three old-binary
snapshot repairs. CG-13 lifecycle and CG-5 evidence release regressions also
passed. Formatting, strict Clippy, and all 945 reported Rust tests passed (the
eight unconfigured Arango integration entries early-returned). Full gate results
and compatibility details are recorded in the report.
CG-33 was committed with CG-22 as `9fb4933`; nothing was pushed.

The [API/operator contract batch](../../issues/api-contract-2026-09-09.md) reconciles
CG-22. OpenAPI and current references cover all four job kinds, typed job input
pairing, async draft controls, side-view defaults/clamping and provider selection,
and directed limits and replacement semantics. The five added drift tests compare
Serde wire values, parsed schemas, and production-route behavior; all 15 focused
OpenAPI checks passed. Standard OpenAPI validation also caught and corrected
ambiguous inline descriptions and a missing collection path parameter. The
release run passed 57 HTTP/CLI/schema checks using synthetic local providers;
main Luna and independent Gemini side-view routing matched configuration.
Formatting, strict Clippy, and all 950 reported Rust tests passed (eight Arango
entries early-returned without credentials). Corrected initial verification
attempts are recorded in the report.
CG-22 and CG-33 were committed locally as `9fb4933`, excluding unrelated
research/draft files and the existing `CLAUDE.md` deletion. Nothing was pushed.

The [roadmap parity batch](../../issues/roadmap-parity-2026-09-09.md) resolves CG-23.
Current roadmap and workload status now agree that D1–D12 shipped. July failure
analysis, research proposals, and timings are clearly historical, with the
original measurements and caveats preserved. The active priorities link to the
CG registry. Source comparison, all 144 query tests, 58 release HTTP/Lua checks
across resident/paged Native stores, and documentation checks passed. Two harness
setup errors were corrected; no Rust change or new CRM/model benchmark was made.
CG-23 was committed locally with CG-28 as `b9084e6`. CG-28 is completed below.

The [subquery contract batch](../../issues/subquery-contract-2026-09-09.md) resolves
CG-28. The specification, v2 amendment, and unsupported list now distinguish
supported expression positions from post-collect parse errors and unsupported
inline INTO/mutation operands. Read subqueries can feed a mutation through its
body, subject to CG-7. Eighteen new corpus cases cover accepted and rejected
forms; 144 query tests, the Native corpus, formatting, strict Clippy, all 950
reported workspace tests passed; 148 release observations matched the contract
and known failures. No Rust implementation changed. The probes found generated names colliding in nested
subqueries (CG-37) and a read-only query route returning 500 for client plan
errors (CG-38). Both are resolved below with separate verification.
Initial fixture/Lua harness errors are recorded in the report. CG-23 and CG-28
were committed locally as `b9084e6`, excluding unrelated research/draft files
and the existing `CLAUDE.md` deletion. Nothing was pushed.

The [generated-binding batch](../../issues/subquery-bindings-2026-09-09.md) resolves
CG-37. A single counter spans nested and sibling desugaring within each parsed
query, preventing generated names from shadowing outer bindings. The original
reproducer now executes; four more corpus examples and three Rust tests cover
correlation, binds, repeated parses, user shadowing, depth limits, dynamic reads,
analysis, and row budgets. Formatting, strict Clippy, all 953 reported workspace
tests, 147 query tests, and the Native corpus passed. Eight unconfigured Arango
entries early-returned. The saved release reproduced 40 collisions across five
shapes; the rebuilt release returns the correct results. Its 180 HTTP/Lua
observations include 28 analysis checks and four known CG-38 status mismatches.
Initial test-type and analyzed-fixture bind-helper errors were corrected and
the complete runs passed, as recorded in the report. CG-37 was committed locally
as `c48a3b4`, preserving unrelated research/draft files and the existing
`CLAUDE.md` deletion; nothing was pushed. CG-38 is completed below.

The [query-error classification batch](../../issues/query-error-status-2026-09-09.md)
resolves CG-38. The read-only route now classifies typed plan errors as HTTP
400, matching the read-write route and preserving identical diagnostics across
normal queries, EXPLAIN, and analysis. Forbidden 403, connection 503, and other
execution/backend status mappings are preserved. Formatting, strict Clippy,
all 955 reported workspace tests, five focused route tests, and 147 query tests
passed; eight unconfigured Arango entries early-returned. The saved release
reproduced 24 status mismatches. The corrected release passed all 288 HTTP/Lua
observations across resident/paged Native stores, with no mismatches and exact
source/destination preservation after invalid requests. CG-38 was committed
locally as `aae43ad`, excluding unrelated research/draft files and the existing
`CLAUDE.md` deletion. Nothing was pushed. CG-35 is completed below.

The [collection-ensure batch](../../issues/collection-ensure-2026-09-09.md) resolves
CG-35. Existing collections return before persistence under the creation write
lock, preserving types, data, process versions, durable generation, and revision.
Concurrent ensures commit one creation; real creation and writes still
invalidate derivatives. All 61 Native tests, formatting, strict Clippy, and
958 reported workspace tests passed; eight unconfigured Arango entries
early-returned. The release matrix passed 112 checks and nine authenticated
restarts across all three persistent Native configurations, using stores seeded
by the prior release. It preserved 18 derivatives unnecessarily rewritten by
the baseline and retained seven required rebuilds after real changes. Vector
warm loading still reconstructs key/model metadata from redb; no latency
benchmark is claimed. CG-35 was committed locally as `d98e374`, preserving
unrelated research/draft work and the existing `CLAUDE.md` deletion; nothing was
pushed. CG-36 is completed below.

The [dedicated judge endpoint batch](../../issues/judge-endpoint-2026-09-09.md) resolves
CG-36. Both judge models use the shared configuration snapshot and validated
OpenAI endpoint independently of the main provider/model. Explicit judge
configuration now fails before storage opens for missing/blank keys or invalid
selected endpoints; blank/absent models retain fallback/absent-partner behavior.
Model qualification and review policy remain unchanged. All 21 focused embedding
tests, 24 review route tests, formatting, strict Clippy, and 961 reported workspace
tests passed. Eight unconfigured Arango entries early-returned; the two existing
live embedding tests passed. The corrected release passed 22 review scenarios
and 12 startup rejections, with 58 loopback screen/quality calls and zero outbound
judge attempts. The saved release reproduced nine default-endpoint attempts
blocked by a local proxy without forwarding. No real judge qualification or
model benchmark was run. CG-36 was committed locally as `57d65af`; nothing
was pushed, and unrelated user work was preserved. CG-24 is completed below.

The [paper counting-unit correction](../../issues/paper-counting-units-2026-09-09.md)
resolves CG-24. Rev2 and its HTML/PDF exports now use 59/59 cold construction,
54/59 repaired, and 0/32 violations, counting distinct triples within each kit.
The historical adjacent-only deduplication and per-question answer units are
explicit; similar answer recall is no longer described as identical outputs.
All twelve offline release `blind_recheck` runs reproduced the fixture ledger,
with no model calls. Desktop/mobile HTTP checks, PDF text/visual checks, and
documentation validation passed. Rust source and fixture data are unchanged;
the previous CG-36 Rust gates remain the latest full workspace validation.
CG-24 was committed locally as `b90a28a`, including the corrected paper and its
existing publication build assets; no publication or push was performed.
CG-27 is completed below.

The [WebNLG status reconciliation](../../issues/webnlg-status-2026-09-09.md) resolves
CG-27. Current guidance records the July 17 deterministic/test result and the
July 20 fuzzy, expanded-corpus, live-proposal, and pruned-candidate experiments
with their dates and limits. Retained artifact links now resolve under the
construction crate. Candidate pruning and automatic disambiguation are kept
distinct from runtime governed review; the pruned set's small recall gain has
a slight precision cost. Offline corpus verification and release validation
replays use the retained artifacts without model calls or test scoring; the
frozen mined artifact reproduces byte for byte. Rust source, original research
edits, and the CG-24 publication bundle are unchanged. CG-27 was committed
locally as `44e8890`; nothing was pushed. CG-30 is completed below.

The [decision-index reconciliation](../../issues/decision-index-2026-09-09.md) resolves
CG-30. All 53 decision records have scoped statuses and amendment/successor
links, with current owner documents for 14 areas. Historical decisions and
measurements remain unchanged. A reusable checker validates complete, unique
inventory coverage and all 148 local index links, including 19 Markdown anchors;
six negative probes verify rejection of navigation defects. Registry and
preservation checks passed. No Rust or product behavior changed. CG-30 was
committed locally as `e7ea755`; nothing was pushed. CG-25 progressed below.

The [CUAD recovery](../../issues/cuad-recovery-2026-09-09.md) partially remediates
CG-25. Ten original external evaluation files now form a pinned scoring
package; a fresh source download and the retained archive both reproduce the
prepared split/gold/chunks byte for byte. The historical 0.744 / 0.635 / 0.685
replays, but recall/F1 mixed gold-entry and contract/category counts. With the
same matches and consistent gold-entry accounting, the diagnostic is
P 0.744 / R 0.370 / F1 0.494. The paper, HTML/PDF exports, and pilot one-pager
withdraw the old recall/F1 and qualify the holdout restart and causal claims.
Seven regression tests, notebook execution, source/scoring replays, and
HTTP/visual/navigation checks passed. Rust source is unchanged; no model calls were made.
At that recovery checkpoint, CG-25 remained open for the missing original provider/model/settings, raw
completion/nominations, and full accepted fact rows. This recovery was committed
locally as `3fa7db7`; nothing was pushed. CG-26 is completed below.
At that recovery checkpoint, broader model benchmarking was still deferred.

The [server modularity refactor](../../issues/server-modularity-2026-09-09.md) resolves
CG-26 for the six named hotspots. Compatible module entry points now separate
contracts, validation, transitions, recovery/storage, and tests. Of 240 files,
233 meet the 450-line soft cap; seven existing coherent methods/scenarios have
explicit budgets. The size guard also runs in manual CI. Other oversized
workspace modules remain outside this scoped pass.
All 1,087 moved declarations/methods match the baseline after mechanical
normalization. Formatting, strict Clippy, all 961 reported workspace tests,
release build, navigation/size checks, and workflow lint passed. Saved/final
releases produced identical signed authority and graph results through Native
replay/recovery and restarts in all three storage modes. Existing release API
and neuron harnesses passed 57 and 99 checks, and disposable live Arango
preserved the M26 rejection boundary. CG-26 and the completed evaluation
evidence were committed locally as `8b8cceb`; nothing was pushed. The later
user decision below retires CG-25's remaining historical recovery scope and
begins a captured Luna baseline.

**Initial Luna evaluation checkpoint (user decision, 2026-09-09):** retire CUAD from
active evaluation and close CG-25 without change to its missing original
execution evidence. At that checkpoint the registry had 37 Resolved, 1 Closed
without change, and 0 Open issues; the later directed checkpoint below closes
the two subsequent findings. Preserve the corrected historical package and qualified
paper. Establish a [captured Luna baseline](../../decisions/decision_luna_baseline.md)
using 48 fictional relation-extraction excerpts through the real server, with
full provider requests/responses, gate results and accepted occurrence rows.
Astra at medium reasoning is the bounded stronger reference; Luna at none
remains the economical default. Both configurations use the same frozen corpus,
prompt/schema, taxonomy, and scoring grain. This synthetic regression sample
has no independent annotator and cannot support general product accuracy.
The [completed results](../../research/experiments/luna-baseline-2026-09-09/results.md)
record sixteen successful real provider/server calls, no retries, Luna TP/FP/FN
27/1/5 and 26/1/6, and Astra 32/0/0 twice. All 119 accepted occurrences pass
provenance checks. Mean twelve-excerpt batch latency was 3.725 s versus 6.734 s;
estimated cost was $0.004202 versus $0.203370. Six scorer tests, loopback controls,
exact offline replay and capture-integrity checks passed. Luna stays the default,
Astra is the useful reference, and the synthetic sample supplies no general
accuracy or judge-qualification claim. The requested DeepSeek/GLM extension is
recorded below; other models and independent representative-document
qualification remain follow-ups. No new commit or push.

**Cross-provider comparison (2026-09-09):** the user requested DeepSeek V4
Pro/Flash and GLM-5.3/Flash using the existing `.env` keys. A
[separate captured comparison](../../research/experiments/cross-provider-baseline-2026-09-09/results.md)
uses JSON-object mode and a shared schema appendix for all four candidates and
a fresh Luna reference. The earlier baseline remains byte-for-byte intact.
Among 38 provider calls, four configurations completed both repetitions.
Luna scored 31/0/1 and 31/1/1 TP/FP/FN; DeepSeek Pro, GLM Flash and GLM-5.3
scored 32/0/0 twice. DeepSeek Flash's first measured batch exhausted its
4,096-token allowance, with 3,586 reasoning tokens and incomplete answer JSON;
the server rejected it and the arm stopped without retries or a full score.
GLM Flash averaged 4.165 seconds per batch against Luna's 3.710, with measured
cost $0.00123481 promotional ($0.00246962 at regular rates) versus $0.00467520.
Total estimated cost, including preflights and failure, was $0.075562071.
Six tests, 45 synthetic controls, offline replay, 255 occurrence checks,
integrity probes, frozen code/binary checks and shared-file preservation passed.
**Current candidate policy:** the user dropped DeepSeek V4 Flash entirely;
retain its historical failure, with no configuration follow-up or further calls.
Luna remains the default. A larger GLM Flash–Luna trial is complete in the
[SemEval package](../../research/experiments/glm-luna-semeval-2026-09-09/README.md):
600 distinct externally human-labelled sentences, including 110 Other pairs,
two repetitions and paired uncertainty. The pair-aware benchmark adapter gives
both models the same supplied nominals; it does not test entity discovery or
exhaustive customer-document extraction. Raw classification and frozen-vocabulary
post-gate results are separate. See the
[decision](../../decisions/decision_luna_baseline.md#independently-labelled-supplied-pair-trial--2026-09-09).
The [measured result](../../research/experiments/glm-luna-semeval-2026-09-09/results.md)
is GLM 65.75% versus Luna 42.67% pooled raw accuracy, a +23.08-point lead
with paired 95% interval [+17.67, +28.75]. Other-case false assertions remain
high (113 versus 175 across 220 repeated observations); accepted accuracy is
64.50% versus 44.58%. GLM is the stronger domain-trial candidate, while both
need representative restraint evaluation. The 202-call schedule retained four
Luna content-filter failures and two GLM failures (schema/timeout); no failed
batch committed facts. A prior five-call attempt and its capture defect remain
archived; the amended run, controls, eight official-scorer comparisons, 1,705
evidence checks and integrity/replay checks passed. Combined known token cost
was $0.119233, with two timeout calls of unknown usage. No native provider
integration, Rust behavior or judge qualification changed.

**Low-effort extension (2026-09-09, completed):** the user authorized
`gpt-5.6-terra` low and a `gpt-5.6-luna` low control on the identical 600 cases,
two repetitions, prompts and scoring. The [separate frozen protocol](../../evidence/research-terra-luna-low-semeval-2026-09-09.md)
retains the old trial intact and treats comparisons to its Luna none / GLM
Flash low captures as historical contrasts. The [results](../../research/experiments/terra-luna-low-semeval-2026-09-09/results.md)
show Terra at 72.42% and Luna low at 70.08% raw accuracy. Terra's +2.33-point
margin has paired 95% interval [+0.42, +4.25], at about 10.4 times the token
cost and 42% more HTTP latency. Luna low gains 27.42 observed points over
historical Luna none, with direction errors falling from 136 to 1. Its +4.33
points against historical GLM has interval [-1.83, +10.00], so a clear winner
between those economical candidates is not established.

All 202 new calls were captured; each arm completed 96/100 measured requests,
with content-filter failures on batches 23 and 42 in both repetitions and no
facts committed by a failed request. The new schedule's token estimate is
$1.764058–$1.766769 including preflights. Accounting now includes cache-write
premiums, bounds unreported cache counters, and supplies corrected historical
Luna estimates without modifying the old package. Ten unit tests, 202 real-server
gold controls, 4,000 cost-bound checks, exact new/historical score replay,
404 prompt comparisons, eight official-scorer comparisons, 1,772 evidence
checks and five tamper probes passed. All 545 pinned source/harness files and
the existing release binary match. No Rust source or default changed.

**Selected baseline (user decision, 2026-09-09):** stay on `gpt-5.6-luna`
with reasoning `low`. The model-selection phase is complete for now. The shared
OpenAI provider now sends `low` wherever Luna is resolved, including inherited
side-views. Other model overrides retain their provider defaults. Production
JSON schemas, prompts, timeouts, and judge qualification are unchanged.
Formatting, strict Clippy, all 962 reported tests, and the release server/CLI
build passed. The [runtime verification](../../issues/luna-low-runtime-2026-09-09.md)
passed 13 server configurations, 13 rejected configurations, three CLI cases,
and two real Luna calls with unchanged strict JSON schemas. Construction stored
the two expected directed facts and abstained on the negative excerpt; the
inherited side-view job stored one grounded pair. Disposable Native databases
were used; the existing local database was not reset.

**Git checkpoint:** `8b8cceb` preserves CG-26 and all four completed evaluation
packages before runtime adoption. Their source and binary pins describe that
checkpoint, not the newer runtime; keep the captures and historical conclusions
unchanged. No remote push.

**Next quality milestone:** independently checked representative complete
documents for Luna low, including entity discovery, exhaustive relation labels
and near-miss negatives. Freeze separate development and holdout splits before
tuning. Prioritize precision, abstention and evidence fidelity, while retaining
recall, execution-failure, latency and cost measurements. Luna low still made
120 false assertions across 220 repeated Other observations, and 101 survived
the finite gates. Improve that behavior on development documents and validate
on the untouched holdout. GLM/Terra remain optional references; DeepSeek V4
Flash remains retired.

**Document development checkpoint (2026-09-09):** the user selected general
factual documents. The [Luna document trial](../../research/experiments/luna-documents-2026-09-09/results.md)
freezes 40 development and 80 unrun holdout documents from independently
human-reviewed Re-DocRED, covering twelve predeclared relations. These are whole
annotated benchmark documents, not full Wikipedia articles or customer reports.
The model receives no entity list or supplied pairs. All forty live development
calls completed without retries, at an estimated $0.0201583 token cost.
It proposed 55 relations and stored 32; 21 raw and 16 stored relations matched
the 683 published references. Raw/stored reference recall is 3.07%/2.34%.
The source's geographic skew, inferred relations, incomplete labels and entity
aliases prevent interpreting unmatched predictions as independently proven
false assertions. Exhaustive truth/negative qualification remains unfinished.

The frozen scorer initially stopped on five invalid chunk citations. A versioned
V2 amendment retains those as unmatched predictions, without modifying model
outputs or making extra calls. Ten tests, forty real-server gold controls,
source/selection replay, forty prompt comparisons, 32 live evidence checks and
five integrity probes passed. Three additional synthetic release HTTP cases
reproduced [CG-39](../../issues/CG-39.md), endpoint substring acceptance, and
[CG-40](../../issues/CG-40.md), unconstrained completion identifiers. The original
trial was committed locally as `a0e4b24` before implementation.

**Directed contract checkpoint (2026-09-09): CG-39 and CG-40 resolved.**
[Policy v2](../../decisions/decision_directed_extraction_contracts.md) separates Unicode
endpoint boundaries from unchanged quote lookup and binds completion chunk IDs
and relations to exact request values. Formatting, strict Clippy, 967 reported
Rust tests, the release build and 34 OpenAI/Gemini HTTP cases passed. Eight Arango
integration entries returned early without the exported password; release tests
used disposable Native stores. No existing database reset was needed.

The [new frozen candidate](../../research/experiments/luna-directed-v2-2026-09-09/results.md)
replayed all 55 original proposals, removing one invalid endpoint occurrence
while retaining all 16 reference matches. Forty fresh Luna-low calls had zero
invalid chunk citations and stored 35 occurrences, 20 matching the 683 published
references (2.93% reference recall). Estimated token cost was $0.0194147.
Capture/score replay and five integrity probes passed. All historical packages
and the 80-document holdout remain unchanged; no remote push was made.

**Next:** independent reference review and explicit-evidence policy alignment on
development documents, including unlabelled near-miss candidates. Freeze the
qualification criteria before using holdout data. Published-reference agreement
does not establish truth precision or abstention quality. Keep Luna low as the
baseline; no further model comparison is required for this step.

The [mutation backend-read batch](../../issues/mutation-backend-reads-2026-09-08.md)
resolved CG-7 by rejecting `DOCUMENT()` and correlated traversal starts
throughout mutation queries before execution. This selects the issue's explicit
validation remedy; enabling these reads remains future capability work with
defined read visibility and safe write execution. Resolving the shared-budget
and error issues (CG-15/CG-16) does not lift the mutation restriction. The HTTP
endpoint now returns 400 for planning/validation errors. Formatting, Clippy,
all 893 tests, and 256 release HTTP/Lua rejection checks passed, with exact
document preservation, supported lifecycles, and restart checks in resident
and paged storage. The first live run caught the corrected 500/400 mapping
defect. This work was committed as `41cdb8a`. Its next bounded issue,
analyzed-query document resolution (CG-8), is completed above.

**Historical deferral (user decision, 2026-09-08; prerequisite now satisfied):** finish the current
issue list, CG-1 through CG-32, before reopening model selection. Until then,
`gpt-5.6-luna` remains the economical baseline. After those tickets are resolved
or explicitly closed, compare the latest available DeepSeek, GLM, and other
relevant models against Luna. Verify model identities and pricing at that time,
and measure source fidelity, retrieval utility, schema/count compliance,
latency, and cost with reproducible artifacts. Construction and judge use
require their separate qualification evaluations. This follow-up is deferred
work outside the current remediation list, not a prerequisite for closing it.

The [real side-view comparison](../../research/experiments/sideviews-model-comparison-2026-09-08/README.md)
is complete: two preflights plus 30 measured real API calls, all successful.
Luna returned 178 pairs and hit the exact count on 13/15 requests; Gemini
returned 180 and hit 15/15. Mean latency was 3.679 s versus 5.141 s; estimated
cost per 1,000 similar passages was $0.4844 versus $4.9993, using the configured
reasoning defaults. Both had source-fidelity limitations in the qualitative
review. Keep Luna as the economical baseline; no judge qualification changed.
CG-5, CG-7, CG-8, CG-13, CG-15, CG-16, CG-19, CG-20, CG-21, CG-22, CG-23,
CG-24, CG-27, CG-28, CG-30, CG-33, CG-34, CG-35, CG-36, CG-37, and CG-38 are completed above; CG-26 is also complete; CG-25 was partially remediated and then explicitly closed without completing historical recovery.
Construction/judge qualification requires separate replay and injection
evaluations, not this side-view sample.

The September 8 model-default refresh selected `gpt-5.6-luna` for OpenAI and
stable `gemini-3.8-flash` for Gemini. At that checkpoint Luna explicitly retained
the previous OpenAI default's `reasoning_effort: none`; the September 9 adoption
above replaces this with `low`. Side-view inheritance and model overrides
remain supported. Configuration references record the current setting separately
from historical benchmarks.
Formatting, Clippy, all 888 tests, and the release HTTP/harness matrix passed
again with the new defaults and synthetic loopback providers. The subsequent
real comparison is recorded above.

The [completion-provider batch](../../issues/completion-provider-2026-09-08.md)
resolved CG-29: server and harnesses share validated provider/key/model/base-URL
selection and side-view inheritance. Explicit configuration errors now fail
startup before stores open, while unconfigured lanes remain optional.
Formatting, Clippy, all 888 tests, 13 release HTTP configurations, 13 rejected
startups, and 3 benchmark runs passed using loopback providers. CG-29 and CG-32
were committed as `41cdb8a`. Its next bounded issue, `DOCUMENT()` mutation handling
(CG-7), is completed above using the accepted validation remedy.

The completed review/remediation through CG-31 was committed locally as
`291fbb1`, preserving unrelated research/draft changes. The subsequent
[semantic/graph-cache batch](../../issues/search-cache-2026-09-08.md) resolved CG-32:
versioned JSON retains optional values and collection-name boundaries, with
production key builders in fixtures. Formatting, Clippy, all 876 tests,
OpenAPI checks, and release HTTP exact/strong/assisted/restart regressions
passed across resident/paged storage and memory/persistent caches. CG-32
changes were committed as `41cdb8a`. Its next bounded quick win, completion-provider
override handling (CG-29), is completed above.

The [hybrid-cache identity batch](../../issues/hybrid-cache-2026-09-08.md) resolved
CG-31. Versioned JSON preserves all result parameters and ordered field lists;
fixtures use the production builder. Formatting, Clippy, all 872 tests, and
release HTTP exact/strong/assisted/restart probes passed with resident/paged
storage and memory/persistent caches. Result caches still start cold on
restart, while persistent embeddings survive. Cross-checks found semantic
and graph-augmented key collisions, recorded as CG-32 and now completed above.

The [request-isolation batch](../../issues/request-isolation-2026-09-08.md) resolved
CG-12. Admission pins incarnation/store/cache under the tenant lifecycle lock;
Lua carries that context across tasks, and user administration fences its
shared-control-store operations. Formatting, Clippy, all 869 tests, and
resident/paged release HTTP paused-provider, slow-body, lifecycle, and restart
checks passed. Admitted ordinary work may drain into quarantined data; it
cannot reach a replacement tenant. All reviewed P1 issues are now resolved.
Its next bounded quick win, the hybrid-cache fingerprint collision (CG-31),
is completed as recorded above.

The [tenant derivative isolation batch](../../issues/derivative-isolation-2026-09-08.md)
resolved CG-11. Native schema 2 binds derivatives to database and commit
identity; retirement includes text directories and unfinished vector builds.
Opening schema 1 upgrades metadata atomically, and older binaries reject the
upgraded store. Formatting, Clippy, all 863 tests, reused-path/restore
regressions, and resident/paged release HTTP upgrade/recreation/restart checks
passed. Its next priority, request incarnation pinning (CG-12), is completed
as recorded above.

The [paged-cache batch](../../issues/paged-cache-2026-09-08.md) resolved CG-10 and
CG-14: ordered write/cache/vector publication, protected cache fills, complete
collection-cache purge, and serialized sidecar builds. Formatting, Clippy,
all 857 tests, deterministic interleavings, and release HTTP concurrent-access,
drop/recreate, export, and restart checks passed. These checks covered paged
caching enabled/disabled and resident sidecars. Its next priority, tenant
derivative isolation (CG-11), is completed as recorded above.

The [directed deployment fence](../../issues/directed-fence-2026-09-08.md) resolved
CG-3. Directed ingestion shares tenant-health and M26 admission checks and
holds the promotion transition lock through provider execution and
reconciliation. Formatting, Clippy, all 848 tests, the concurrent signed M26
lifecycle, and resident/paged release-server HTTP checks passed, including
restart and exact snapshot preservation on rejection. Its next batch,
paged-cache validity (CG-10/CG-14), is completed as recorded above. General
request incarnation pinning is now completed in CG-12 as recorded above.

The [Lua execution-control batch](../../issues/lua-controls-2026-09-08.md) resolved
CG-1 and CG-6: verified JIT disabling, protected-call/coroutine instruction
termination, CGQL budget parity, a shared script deadline, and worker
cancellation with tenant-correct cache cleanup. Formatting, Clippy, all 845
tests, and release-server HTTP checks passed. Cancellation remains cooperative
for synchronous backend work; completed writes are not rolled back. Its next
priority, directed deployment fencing (CG-3), is completed as recorded above.

The [first remediation batch](../../issues/fixes-2026-09-08.md) resolved CG-2, CG-4,
CG-9, CG-17, and CG-18: safe derivative paths, strict directed-output validation,
complete UPSERT matching, and Native text-search field/index identity handling.
Formatting, Clippy, all 836 tests, and release-server HTTP regressions in
resident/paged modes passed. CG-31 records a separate hybrid result-cache
fingerprint collision discovered during that batch; its original next priority,
Lua execution controls, is now completed as recorded above.

The documentation and Rust implementation review is recorded in
[the product-status report](../../issues/review-2026-09-08.md), with the open defect
backlog in [the CG issue registry](../../issues/README.md). Existing milestone
completion dates below remain historical delivery records; the review found
unresolved execution, storage, tenant-lifecycle, construction, and query
correctness defects. No remediation phase is marked complete by this audit.

## Strategic Direction (July 2026): Native-First

ArangoDB's licensing changes and the v4 API churn make third-party database
backends a product risk, and maintaining multiple query languages is not worth
the effort. Decision:

- **CGQL is the public query language of the product.** Opaque backend-native
  AQL text is disabled on HTTP and Lua surfaces for every role; ArangoDB AQL is
  an internal backend implementation detail.
- **The native backend (`cognigraph-native`) is the strategic backend.** It
  will gain persistence, indexes, and full-text search, then become the
  default.
- **ArangoDB is demoted to maintenance:** it stays as a reference
  implementation for backend-contract tests until the native backend reaches
  parity; no new ArangoDB-specific features.
- **No new third-party database backends.** SurrealDB plans are dropped.
  Embedded *library* dependencies (e.g. redb for storage, tantivy for BM25)
  are acceptable — they are pinned, license-stable (MIT/Apache), and
  replaceable behind our own traits.

Roadmap milestones (each gets its own detailed plan under
`docs/superpowers/plans/`):

| Milestone | Scope | Status |
|-----------|-------|--------|
| M26 Verified Semantic Repair materialization | exact M22/M23-to-occurrence generation + candidate/baseline impact + signed atomic target-space deployment and rollback | **Done** (2026-07-19) |
| M25 Governed Semantic Repair authority | immutable signed typed-candidate revisions + independent review + exact existing-promotion-head resolution + generic mutation closure | **Done** (2026-07-19) |
| M24 Durable CAS custody and verified restoration | deterministic evidence recovery plans + closed content-addressed bundles + conflict-safe offline restoration and operation receipts | **Done** (2026-07-19) |
| M23 Reproducible raw-document-to-prepared-corpus processing | exact verified UTF-8 document bytes + pinned normalization/chunking recipe replay to the attested canonical prepared-chunk corpus | **Done** (2026-07-18) |
| M22 Reproducible corpus-to-graph derivation | exact prepared-chunk corpus + governed construction candidate replay to the attested canonical evidence-bearing evaluation-fact projection | **Done** (2026-07-18) |
| M21 Verified artifact consumption | fail-closed local-CAS byte verification; immutable graph/oracle evaluation; executable identity and durable consumption authority | **Done** (2026-07-18) |
| M20 Content-addressed external artifact attestations | signed exact-byte manifests and usage bindings for corpus, graph, oracle, scorer, and verifier inputs | **Done** (2026-07-18) |
| M19 Signed governance and separation of duties | root-certified Ed25519 principal keys; signed policy/approval/promotion intent; three-principal authority | **Done** (2026-07-18) |
| M18 Evaluation promotion gates | deterministic context-bound evaluation evidence; independent policy gates; attributed per-target promotion and rollback | **Done** (2026-07-18) |
| M17 Queue scale and governance | bounded cursor listing, queue quotas/backpressure, checkpoint-fair tenant scheduling, terminal archive, catalog reconciliation, operator status | **Done** (2026-07-17) |
| M16 Durable governed operations | persistent tenant-scoped `construct.ingest`/`construct.evaluate` jobs, idempotency, restart recovery, cancellation/retry, audit history, API/CLI, bounded metrics | **Done** (2026-07-17) |
| M15 Trustworthy foundation | authorization closure, revision-safe construction, backend/search contract parity, cache coherence, reliable readiness | **Done** (2026-07-17) |
| M1 CGQL semantics hardening | null semantics, depth-0 traversal, structured parse errors, strict bind vars, grammar fixes | **Done** (2026-07-02) |
| M2 Native-first server | `query_language()`-aware routes, explicit hybrid-search fallback, backend-contract test suite, O(E) traversal | **Done** (2026-07-02) |
| M3 Native persistence | redb storage model doc + implementation, durability/import/export tests | **Done** (2026-07-02) |
| M4 CGQL v2 + pushdown | function registry, LIMIT pushdown, native BM25 + hybrid, native default backend, CGQL corpus | **Done** (2026-07-02) |
| M5 CGQL ergonomics | LET, multi-key SORT, RETURN DISTINCT, null-falsy booleans, date functions | **Done** (2026-07-02) |
| M14 Semantic Neurons port | cognigraph-construct: governed grounding, recall+restraint evals, live proposal loop; research parity plus hostile-scale eval evidence | **Done** (2026-07-06) |
| M13 Execution budgets | per-query row cap + time budget on CGQL endpoints | **Done** (2026-07-03) |
| M12 UPSERT + batch transactions | CGQL UPSERT, atomic execute_batch, POST /api/batch | **Done** (2026-07-03) |
| M11 redb-primary reads | paged storage mode: keys + LRU resident, docs page from redb; conformance green | **Done** (2026-07-02) |
| M10 Vector sidecar + mmap | int8 mmap sidecar, search-only embeddings, 7.3× RAM reduction | **Done** (2026-07-02) |
| M9 Production hardening | rate limit, /metrics, timeouts, JSON logs, graceful shutdown, Docker | **Done** (2026-07-02) |
| M8 Auth & RBAC | cognigraph-auth crate, bearer tokens, role scopes, /users API, Lua write lift | **Done** (2026-07-02) |
| M7 CGQL mutations | INSERT/UPDATE(merge)/REPLACE/REMOVE/UPSERT + NEW/OLD, QueryMode gate, POST /api/query, atomic backend batches | **Done** (2026-07-03) |
| M6 CGQL COLLECT | grouping + WITH COUNT INTO + AGGREGATE SUM/MIN/MAX/AVG + INTO group capture | **Done** (2026-07-02) |

## M26: Verified Semantic Repair Materialization (done 2026-07-19)

M26 closes M25's selection-versus-deployment boundary for one deliberately
bounded Native-only target projection. The accepted contract is recorded in the
[M26 decision](../../decisions/decision_m26_verified_semantic_repair_materialization.md).

- [x] Add a synchronous generation build that accepts only the current M22 or
  M23 promotion selection with an exactly approved M25 revision. Reverify the
  canonical prepared-corpus CAS bytes, reuse the embedded exact candidate, and
  require the candidate occurrence facts to project exactly to both durable
  candidate derivation receipts.
- [x] Freeze materialization plan v1 at no more than 1,000 chunks, 10,000
  candidate semantic facts, 10,000 baseline semantic facts, 50,000 combined
  entity/chunk/mention/fact rows, and 16 MiB of canonical generation JSON.
  Enforce checked per-space, tenant-generation, aggregate-byte, and
  deployment-chain quotas before mutation.
- [x] Store one immutable complete target occurrence projection with exact
  source evidence, M25 revision/review, corpus, derivation, plan, per-array,
  semantic-fact, projection, request, and record digest bindings. Add bounded
  summary/detail and projection inspection surfaces without exposing the
  protected collection generically.
- [x] Compute and bind exact candidate-versus-baseline added, removed, and
  unchanged semantic fact sets plus the full candidate occurrence projection
  digest. Keep the report generation-exact without claiming that one neuron
  caused every observed difference.
- [x] Add an externally signed deployment-intent v1 for `activate` and
  `rollback`, admitted only from an active root-certified Promoter owned by the
  authenticated actor. Bind the exact generation, impact, promotion head, M25
  authority, reason, idempotency hash, required nullable expected deployed
  head, and required nullable rollback target. Accept no private key server or
  transport field.
- [x] Keep one deployed generation per `space_type`; record its source channel.
  In one Native atomic batch, validate/upsert required shared entities, replace
  every target-space chunk/mention/fact row, insert the immutable deployment
  decision, and update the derived head. Preserve byte-equivalent rows for all
  other spaces and reject conflicting shared entity identity.
- [x] Require a signed control-plane promotion rollback before an explicit
  signed deployment rollback to the exact retained one-step prior generation.
  Preserve immutable generations and monotonic deployment decision history.
- [x] Extend status, startup validation, explicit recovery, mutation fencing,
  metrics, and Native stored-plus-incoming snapshot preflight across generation,
  impact, decision, head, and active target-row integrity. Repair only derived
  head/target state from fully valid immutable authority; never overwrite a
  conflicting shared entity or corrupt immutable generation.
- [x] Fail M26 build, activation, rollback, and recovery of present M26
  authority before CAS or database mutation when the backend does not
  advertise atomic batches. General maintenance-mode ArangoDB startup/status
  remain compatible and report M26 disabled; ArangoDB gains no M26 transaction
  emulation.
- [x] Add checked fixtures, HTTP/CLI/OpenAPI contracts, strict JSON and
  adversarial regressions, then pass the exact Rust gates and authenticated
  release-binary persistent-Native A/B/rollback/recovery lifecycle plus a live
  ArangoDB zero-write capability probe.
- [x] Preserve explicit non-goals: no durable materialization job, automatic
  promotion-to-deployment hook, drift scheduler, LLM judge qualification,
  physical generation collection routing, multi-channel generic reads,
  generation GC, global exact entity union, distributed coordination, or HA.

Verification completed on 2026-07-19. The focused lifecycle passed in 81.04
seconds with collision-safe concurrent build replay, exact `+1/-1/1 unchanged`
impact, stale chunk/mention/fact removal, exact A/B collection replacement,
missing-entity insertion and preservation, promotion-first one-step rollback,
isolated signed-intent and separation-of-duties checks, tamper/recovery,
snapshot conflict, other-space preservation, and tenant-incarnation coverage.
The exact Rust gates completed; the full server suite reported 235 passing
tests. An authenticated release-binary persistent-Native probe imported the
full authority snapshot, replayed build and deploy, materialized the exact
active `DISTRIBUTES` and `SUPPLIES` facts, reported healthy state, recovered
zero heads, preserved the same state across restart, and exposed both retained
generations through the CLI. A configured live ArangoDB Enterprise 3.12.9-1
probe started normally, reported M26 disabled, returned HTTP 503 for an
authenticated generation build in 304 ms, wrote no M26 authority or graph
rows, and restored the exact 13-collection baseline after isolated cleanup.
The complete evidence and release-binary digests are in the M26 decision.

## M25: Governed Semantic Repair Authority (done 2026-07-19)

M25 closes the remaining authority gap between the mutable legacy Semantic
Neurons workflow and the signed M18-M24 candidate-selection chain. The bounded
foundation contract is recorded in the
[M25 decision](../../decisions/decision_m25_governed_semantic_repair_authority.md)
and the unsigned authoring templates under [`fixtures/m25/`](../../../fixtures/m25/).

- [x] Store the exact unchanged M22 schema-v1 typed `candidate.json` inside an
  immutable tenant/incarnation/target-bound semantic revision signed by an
  existing root-certified `PolicyAuthor`. Recompute the canonical candidate
  digest server-side and reject altered, non-canonical, or context-mismatched
  candidates.
- [x] Require one immutable signed `approve` or `reject` review from an
  independent existing `PolicyApprover`. Give approve and reject the same
  natural id so concurrent final decisions cannot fork one revision.
- [x] Keep the existing M18-M24 promotion head as the sole selection
  authority. Governed resolution succeeds only when its selected exact
  candidate digest names one valid approved M25 revision for the same target;
  approved-but-unselected and selected-but-unapproved candidates remain inert.
- [x] Protect `space_types`, `neurons`, `review_policies`, `eval_specs`,
  `entities`, `chunks`, `mentions`, and `facts` from generic mutation through
  documents, batch, CGQL, Lua, and graph routes while retaining compatible
  reads and dedicated typed construction paths.
- [x] Add immutable-conflict, signature/purpose/revocation, distinct-principal,
  target/incarnation, selection, rollback, recovery, and Native snapshot-union
  validation. Preserve every M18-M24 wire version and legacy record meaning;
  normal adoption uses a fresh channel rather than retroactive authority.
- [x] Add the bounded HTTP/CLI/operator surface, checked request fixtures, and
  current-contract documentation, then complete the exact Rust gates and
  authenticated release-binary persistent-Native plus configured live-ArangoDB
  authority lifecycles.
- [x] Close the adjacent legacy-review fail-open cases without treating them as
  M25 authority: relation-hint-only automatic acceptance, strict local parsing
  of screener/judge results, strict provider schemas when compatible, and the
  documented `0.1` default audit sample. Keep signed judge qualification open.
- [x] Preserve the explicit boundary: no M25 durable repair job, LLM
  qualification, new artifact kind, CAS delivery, automatic re-ingestion,
  materialized graph generation switch, distributed coordination, or HA.

## M24: Durable CAS Custody and Verified Restoration (done 2026-07-19)

M24 closes the explicit M21-M23 byte-recovery gap without changing evaluation
or promotion authority. The accepted contract is recorded in the
[M24 decision](../../decisions/decision_m24_durable_cas_custody_verified_restoration.md).

- [x] Add a deterministic, read-only Admin recovery-plan endpoint for one
  immutable M21-M23 evidence record. Bind the exact candidate and baseline
  artifact sets, historically valid M20 attestations, sorted unique blob
  inventory, tenant incarnation, checked counts/bytes, and canonical plan
  digest. Never include signed locations as fetch or filesystem inputs.
- [x] Bound one plan to 8,192 unique blobs and
  `COGNIGRAPH_ARTIFACT_MAX_CUSTODY_BYTES` (2 GiB by default). Reject foreign
  scope, pre-M21 evidence, conflicting digest lengths, arithmetic overflow,
  and altered evidence/attestation bindings.
- [x] Extract the tenant-scoped local-CAS verifier into a shared
  `cognigraph-artifacts` crate and preserve the current fail-closed read path.
- [x] Add offline CLI `artifact custody plan/create/verify/restore` workflows.
  Create a closed portable content-addressed bundle with a backup receipt;
  require an out-of-band expected plan digest and exact tenant/incarnation for
  verification and restore.
- [x] Keep the online server CAS read-only. Restore only an absent derived
  tenant scope through a fully verified sibling staging tree and no-replace
  publication; never merge, repair, or overwrite an existing scope. Reread the
  published CAS and emit an external restore receipt only after full success.
- [x] Preserve all M18-M23 context/job/receipt/evidence/decision/head and
  signed-intent versions byte-for-byte. M24 receipts remain unkeyed operational
  observations, not promotion authority, ongoing custody proof, or trusted
  timestamps.
- [x] Add adversarial tests and public fixtures for canonical ordering,
  deduplication, bounds, symlinks, unexpected bundle entries, missing/wrong
  bytes, cross-scope restoration, conflicting destinations, and failed
  publication cleanup.
- [x] Correct the disaster-recovery documentation boundary: Native recovery
  composes its backend snapshot/cold copy with the artifact bundle and external
  configuration; ArangoDB requires operator-native database backup plus the
  bundle. Neither path adds replication, retention policy, encryption, RPO/RTO,
  quorum, or HA.
- [x] Complete the exact Rust gates and authenticated release-binary persistent
  Native plus live-ArangoDB lifecycle, including tamper, absent-CAS failure,
  verified restore, fresh M23 reevaluation, restart, and isolated cleanup.

## M23: Reproducible Raw-Document-to-Prepared-Corpus Processing (done 2026-07-18)

M23 closes M22's explicit upstream preprocessing boundary for one narrow,
text-only input contract. It replays a pinned mechanical preparation recipe
from exact verified raw UTF-8 document bytes and accepts the signed
`corpus.json` only when its object, canonical bytes, byte length, and SHA-256
content address exactly equal the independently reproduced prepared-chunk
corpus. The exact contract is recorded in the
[M23 decision](../../decisions/decision_m23_reproducible_raw_document_prepared_corpus_processing.md),
the [checked plan](../../examples/m23-preparation-plan.json), and the
[`fixtures/m23/` authoring guide](../../../fixtures/m23/).

The M23 authority-version matrix is:

| Surface | M23 version |
|---------|-------------|
| Promotion context | 6 |
| Durable evaluation job | 4 |
| Artifact consumption plan / receipt | 3 / 3 |
| Preparation plan / receipt | 1 / 1 |
| Nested corpus-to-graph derivation receipt | 2 |
| Promotion evidence / decision | 6 / 6 |
| Selected head projection | 4 |
| Signed promotion intent domain | `cognigraph.promotion-intent.v4` |

- [x] Add the closed consumption-plan schema version 3. Retain M22's graph,
  oracle, scorer, verifier, local-CAS, and no-location-fetch contracts, while
  replacing the corpus slot with exactly two sorted, canonical,
  non-executable `application/json` entries, `corpus.json` and
  `documents.json`, under
  `cognigraph.reproducible-prepared-chunk-corpus.v1`.
- [x] Define canonical `documents.json` schema version 1. Bind it to the target
  space, corpus revision, and preparation-plan digest; require non-empty
  documents sorted by unique non-blank NFC/control-free id; require ids and
  NFC/control-free titles to be at most 1,024 UTF-8 bytes; require
  `text/plain; charset=utf-8`; and require canonical unpadded base64url content
  to match each declared byte length and lowercase SHA-256 content address.
- [x] Pin `cognigraph.utf8-document-preparer` version 1 and
  `cognigraph.raw-utf8-documents-to-prepared-chunks.v1`. Freeze Unicode 17.0.0
  for NFC normalization and whitespace classification. Reject invalid UTF-8
  and controls other than CR, LF, and TAB; strip exactly one leading UTF-8 BOM
  when present; map CRLF and bare CR to LF; normalize to NFC; and enforce the
  per-document normalized-byte cap before whitespace collapse. Trim Unicode
  whitespace from line edges, collapse each interior run to one ASCII space,
  join adjacent non-blank lines with one ASCII space, use blank lines as
  paragraph boundaries, reject a document with no non-blank paragraph, and
  enforce the aggregate normalized-byte cap after full collapse.
- [x] Pin deterministic chunking. Greedily pack paragraphs to 8 KiB; split an
  overlong paragraph after the latest fitting `.`, `!`, or `?` only when
  followed by whitespace or paragraph end, then at whitespace, then at the
  largest fitting UTF-8 boundary; never overlap chunks. Emit
  `d-<full-lowercase-sha256-of-NFC-document-id>-c<eight-digit-zero-based-ordinal>`
  ids and sort the resulting chunks canonically.
- [x] Enforce the preparation limits from the signed plan: at most 100,000
  documents, 4 MiB per raw document, 64 MiB total raw bytes, 8 MiB per
  post-newline/NFC document before whitespace collapse, 64 MiB total
  prepared-normalized bytes after full whitespace/paragraph collapse, 100,000
  chunks, 48 MiB total prepared text, a 96 MiB retained `documents.json`, a
  64 MiB retained `corpus.json`, and one cooperative scheduler yield per 16
  documents.
- [x] Reconstruct schema-v1 `corpus.json` with the preparation-plan digest as
  its preprocessing digest. Fail closed before M22 derivation unless the
  reproduced value, canonical bytes, length, and SHA-256 digest exactly equal
  the signed corpus entry.
- [x] Add compact preparation receipt schema 1 and bind the corpus manifest,
  exact `documents.json` address, raw-document-set address, preparation plan,
  reproduced `corpus.json` address, read set, and material digest. Do not copy
  raw bytes or the document inventory into durable jobs, evidence, decisions,
  or snapshots.
- [x] Carry preparation authority through derivation receipt v2, consumption
  receipt v3, job v4, context/evidence/decision v6, head v4, and the signed
  `cognigraph.promotion-intent.v4` preparation-authority digest. Require all
  four candidate/baseline original/replay runs to reproduce the same raw-set,
  preparation-plan, and prepared-corpus authority.
- [x] Extend restart recovery, reconciliation, status, archive/catalog
  validation, idempotent replay, and Native stored-plus-incoming snapshot
  preflight to the M23 generation, including unreferenced archived jobs.
  Preserve M18-M22 wire and digest meanings and require a fresh context-v6
  target for normal M23 adoption.
- [x] Preserve the custody and output boundaries. Exact raw bytes remain in the
  operator-backed local CAS, so receipts are address-durable rather than
  byte-self-contained and replay requires a corresponding CAS backup. Native
  snapshots do not contain those bytes; ArangoDB still has no CogniGraph
  application snapshot surface.
- [x] Preserve the extraction boundary. M23 accepts plain UTF-8 text bytes; it
  does not parse HTML, PDF, Office, archive, or compressed containers, perform
  OCR, fetch signed locations, prove source custody or semantic truth,
  reconstruct complete persistent graph storage, execute staged code, deploy a
  selected head, or add independent attestation, quorum, or HA.
- [x] Complete and record final acceptance evidence: the exact Rust gates and
  authenticated release-binary HTTP lifecycles on isolated persistent Native
  and configured live ArangoDB, including deterministic replay, raw/prepared
  mismatch, signed-authority, restart/recovery, snapshot, revocation, and exact
  cleanup checks. Persistent Native passed in 14.46 seconds; live ArangoDB
  Enterprise 3.12.9-1 passed in 109.22 seconds and removed all 36 probe records
  while restoring zero pre-existing records.

## M22: Reproducible Corpus-to-Graph Derivation (done 2026-07-18)

M22 closes M21's explicit corpus-provenance gap for a narrowly defined
construction output. It replays the pinned Semantic Neurons grounder from an
exact verified **prepared-chunk** corpus and exact verified construction
candidate, then accepts the signed `graph.json` only when its canonical bytes
are identical to the independently derived evidence-bearing evaluation-fact
projection. The accepted contract and its limits are recorded in
`docs/decisions/decision_m22_reproducible_corpus_graph_derivation.md`.

- [x] Add one closed consumption-plan schema version 2. Require a singleton
  canonical `cognigraph.prepared-chunk-corpus.v1` `corpus.json` and an exact
  two-entry `cognigraph.reproducible-evaluation-graph.v1` package containing
  canonical `candidate.json` and `graph.json`; retain the M21 oracle,
  scorer, verifier, tenant-scoped local-CAS, and no-location-fetch contracts.
- [x] Bind `corpus.json` to the target space, corpus revision, and preprocessing
  digest. Require non-empty chunks sorted by unique raw id, reject construction
  storage-key collisions, retain at most 64 MiB, admit at most 100,000 chunks,
  and cap each chunk text at 1 MiB of NFC text with only LF/TAB controls.
- [x] Bind canonical `candidate.json` exact bytes to the context candidate
  digest and identity. Resolve one strict base space plus sorted accepted
  alias, relation-hint, and relation-blocker neurons into the effective space
  and veto set, then require its canonical resolved digest to equal the frozen
  construction-configuration digest.
- [x] Pin the deriver identity, semantics, ABI, and limits in the plan: at most
  100,000 configuration items, a deterministic 10,000,000-unit grounding-work
  admission estimate, a 250,000-unit per-chunk ceiling, indexed candidate and
  veto compilation, one-pass clause-negation indexing, lazy literal
  trigger-template expansion, cached sentence-gate decisions, and 100,000
  derived evidence-bearing fact rows.
- [x] Derive facts without reading or mutating the live `GraphBackend`. Apply
  the existing negation, sentence-gate, direction-faithful trigger, accepted
  neuron, and veto semantics; sort and deduplicate exact
  `{source, relation, target, evidence_chunk_id}` rows.
- [x] Reconstruct the complete canonical schema-v1 evaluation-graph envelope,
  including corpus manifest/semantic, candidate, resolved configuration,
  derivation-plan, and facts digests. Fail closed unless the reconstructed
  object, canonical bytes, and SHA-256 content address exactly equal the
  attested `graph.json` bytes; score only that reproduced fact projection.
- [x] Store receipt schema version 2 with a nested derivation receipt binding
  corpus/candidate/plan/configuration/read-set/fact/claimed-graph/derived-graph
  material. Carry it through job schema v3, context/evidence/decision schema
  v5, head schema v3, and `cognigraph.promotion-intent.v3`'s signed explicit
  derivation-authority digest. Preserve the rule that an unkeyed server receipt
  is not independent attestation.
- [x] Extend restart recovery, reconciliation, idempotent replay, status, and
  Native stored-plus-incoming snapshot preflight across M22 jobs, receipts,
  evidence, signed intents, decisions, and heads, including unreferenced
  archived jobs. Keep CAS bytes outside Native snapshots and ArangoDB.
- [x] Preserve generations M18-M21 and their historical semantics. Do not
  reinterpret M21 corpus verification as derivation; normal M22 adoption
  requires a fresh context-v5 target.
- [x] Preserve the narrow output boundary. M22 does not replay raw-document
  decoding, normalization, segmentation, or chunking and does not reconstruct
  persistent chunk/entity/mention collections, indexes, trigger spans, storage
  keys, or a deployed graph. It adds no artifact delivery, code execution,
  consumer switching, remote attestation, quorum, or HA claim.
- [x] Pass `cargo fmt --all -- --check`,
  `cargo clippy --all-targets -- -D warnings`, and `cargo test --all`, including
  deterministic derivation, forged-but-score-equivalent graph, durable
  authority, recovery, compatibility, and Native snapshot regressions.
- [x] Complete authenticated release-binary HTTP lifecycles on isolated
  persistent Native and configured live ArangoDB. The M22 decision records the
  observed restart, recovery, fail-closed mismatch, revocation, and cleanup
  evidence.

## M21: Verified Artifact Consumption (done 2026-07-18)

M21 turns M20's signed exact-byte claims into fail-closed evaluation inputs.
The accepted contract, security boundaries, and verification evidence are
recorded in
`docs/decisions/decision_m21_verified_artifact_consumption.md`.

- [x] Add an explicit artifact source configuration with `disabled` as the
  default and `local-cas` as the only consuming mode. Require an existing
  absolute, normal non-symlink CAS root and positive per-evaluation byte
  budget; never create, upload, replace, or delete staged blobs.
- [x] Scope staged content by a SHA-256 tenant/incarnation identity and derive
  every blob path only from fixed directory names and a validated lowercase
  SHA-256 digest. Never use a signed location URI or manifest logical path as
  a local path or network target.
- [x] Stream every entry in all five M20 manifests and fail closed on missing,
  cross-tenant, symlink, non-file, length, digest, observed parent-chain/file
  replacement, retention-cap, or cumulative I/O-budget errors. Treat the root
  as an operator-controlled read-only boundary rather than an adversarial
  `openat2`/dirfd-anchored filesystem sandbox. Cap the five manifests at 10,000
  total entries and 4,096 unique digest+length pairs, and reuse verification
  when the cache satisfies the next retained-byte requirement. The byte budget
  is not a memory, CPU, or time cap. Revalidate active attestation authority
  after consumption.
- [x] Race the complete asynchronous consumption, parsing, scoring, and receipt
  operation against a fixed 300-second deadline, while polling durable
  cancellation, tenant suspension, and shutdown state every second. Keep this a
  cooperative process-local guard rather than claiming kernel I/O or process
  isolation.
- [x] Freeze the one supported loader/ABI contract in `PromotionContext` v4:
  complete `cognigraph.corpus.v1`, singleton
  `cognigraph.evaluation-graph.v1` `graph.json`, singleton
  `cognigraph.promotion-oracle.v1` `oracle.json`, and singleton scorer/verifier
  `cognigraph.server-executable.v1` `cognigraph-server` blobs.
- [x] Treat corpus bytes as verified provenance without claiming graph
  reconstruction. Parse the verified graph and oracle envelopes, enforce their
  context bindings and closed limits, require the oracle EvalSpec to equal the
  resolved serialized value with NFC/control-safe text, and score its actual
  fact set and EvalSpec rather than querying the live graph. Require
  `reproducibility.backend = "artifact-snapshot"` for this generation.
- [x] Require both scorer and verifier blobs plus the reproducibility identity
  to match the normal non-symlink executable-path digest pinned once through
  `current_exe()` at server startup. Do not claim mapped-code identity, and do
  not load, spawn, sandbox, or dynamically execute staged binaries or obtain a
  separate verifier verdict.
- [x] Store a canonical consumption receipt atomically in a successful
  job-schema-v2 result after scoring and final active-authority validation.
  Bind the job attempt/input/payload/EvalSpec, canonical result, five slot
  projections, and stable material digest through evidence v4, decision v4,
  head v2, and the promoter's `cognigraph.promotion-intent.v2` signed
  consumption-authority digest. Do not describe its unkeyed hash as independent
  server authentication.
- [x] Extend restart recovery, reconciliation, idempotent replay, protected
  authority validation, and Native stored-plus-incoming snapshot preflight
  across M21 jobs, receipts, evidence, signed intent, decisions, and derived
  heads. Keep staged CAS bytes outside both Native snapshots and ArangoDB.
- [x] Preserve context/evidence/decision generations 1-3 and their historical
  live-backend evaluation meaning. Do not fabricate receipts or bridge
  authority generations; normal M21 adoption requires a fresh v4 target.
- [x] Preserve the singleton, selection-only boundary: no artifact delivery,
  automatic graph construction, dynamic code deployment, consumer switching,
  distributed lease, quorum, remote attestation, or HA claim.
- [x] Pass `cargo fmt --all -- --check`,
  `cargo clippy --all-targets -- -D warnings`, and `cargo test --all`, including
  adversarial CAS, immutable-input, receipt, evidence, intent, recovery, and
  compatibility regressions.
- [x] Complete authenticated release-binary HTTP lifecycles on isolated
  persistent Native and configured live ArangoDB, including durable receipts,
  promotion authority, restart/recovery, CAS tamper, prospective revocation,
  and exact cleanup. Repository regressions separately cover missing,
  wrong-length/digest, cross-tenant, symlink, byte-budget, executable/context,
  immutable-input, and recovery failures. The observed evidence is recorded in
  the M21 decision.

## M20: Content-Addressed External Artifact Attestations (done 2026-07-18)

M20 adds signed exact-byte manifest claims for the five external inputs used by
promotion evaluation while preserving the M18/M19 gates and singleton
selection boundary. The accepted contract and its explicit trust limits are
recorded in
`docs/decisions/decision_m20_content_addressed_external_artifact_attestations.md`.

- [x] Add the tenant-local `artifact-attestor` role/scope and root-certified
  `artifact_attestor` key purpose. The role may inspect governance and
  promotion records but cannot author/approve policy, decide promotions,
  mutate tenant data, or administer host lifecycle. Keep private keys outside
  product configuration, API, CLI, storage, fixtures, and snapshots.
- [x] Define a closed canonical manifest of sorted logical paths, byte lengths,
  media types, executable flags, and SHA-256 blob digests. Sign the manifest,
  exact kind-specific usage subject, location observations, and timestamps;
  treat locations as non-authoritative audit hints and never fetch them.
- [x] Persist immutable idempotent signed records in the protected
  `_cognigraph_artifact_attestations` collection, with tenant/incarnation
  identity, active-key admission, prospective revocation, 64 MiB aggregate
  canonical-manifest capacity, bounded summary listing, API/CLI submission and
  detail inspection, and a five-slot binding resolver.
- [x] Add `PromotionContext` v3, evidence v3, and decision v3 bindings. Require
  exact corpus/graph/oracle/scorer/verifier attestations, preserve each
  original/replay pair, permit only policy-authorized graph-attestation
  differences between candidate and baseline, and bind the resulting artifact
  authority digest into the signed promoter intent.
- [x] Enforce Artifact Attestor separation from the policy author, policy
  approver, and promoter. Preserve the existing semantics-identity meaning of
  scorer/verifier hashes rather than relabeling them as executable hashes.
- [x] Extend status, recovery, reconciliation, system-collection protection,
  and Native stored-plus-incoming snapshot preflight across attestation
  records and bindings. Store signed records and manifests, never external
  bytes; ArangoDB retains no application snapshot surface.
- [x] Keep context/evidence/decision v1 and v2 readable and recoverable, reject
  mixed authority generations within one target, and require a fresh v3 target
  for normal M20 migration without retroactive attestation.
- [x] Document the exact boundary: the server validates signature, authority,
  canonical content claims, and bindings but does not prove fetch, evaluation
  consumption, availability, custody, semantic truth, or freshness. The
  evaluator still reads the live tenant graph.
- [x] Pass the exact Rust gates and release-binary HTTP lifecycles on isolated
  persistent Native storage and live ArangoDB, including tamper, cross-duty,
  revocation, restart/recovery, and exact cleanup. Cover the full v3
  job/evidence/promotion chain and Native snapshot-union preflight in stored
  integration regressions, and record both evidence layers in the M20 decision.

## M19: Signed Governance and Separation of Duties (done 2026-07-18)

M19 replaces M18's single-Admin policy trust boundary with signed,
three-principal governance while preserving the evaluation and singleton
selection contracts. The accepted foundation is recorded in
`docs/decisions/decision_m19_signed_governance_separation_of_duties.md`.

- [x] Load one externally pinned host-wide Ed25519 root public key and reject
  signed-governance mutation when it is absent or invalid. Never accept or
  persist signer private keys.
- [x] Bootstrap HostAdmin with a credential separate from the tenant Admin;
  keep HostAdmin limited to tenant lifecycle and initial-Admin provisioning,
  while tenant Admin retains trust bootstrap/recovery without `TenantAdmin`.
- [x] Add protected immutable root-certified key registrations and prospective
  revocations for exactly `policy_author`, `policy_approver`, and `promoter`,
  bound to tenant incarnation, authenticated user, stable `principal_id`, and
  canonical public-key identity.
- [x] Persist immutable signed resolved-policy revisions and require exactly
  one signed approval by a different principal for the exact target and policy
  digest. Admin authority alone must not author or approve a policy.
- [x] Add `PromotionContext` v2 with exact immutable root, key-registration,
  policy-revision, approval, target, and digest bindings. Require all four
  promotion source jobs to carry the same valid signed policy authority while
  retaining every M18 evaluation gate.
- [x] Require signed promote/reject/rollback intents from a third distinct
  promoter principal; bind evidence, gates, policy/approval, reason,
  idempotency hash, rollback target, and expected head, then persist the
  complete signed intent in the immutable decision.
- [x] Validate root, key lifecycle, policy, approval, context, intent, and
  principal separation across status, recovery, reconciliation, and the
  stored-plus-incoming snapshot union. Rebuild only derived heads and fail
  closed on invalid signed authority.
- [x] Preserve M18 v1 jobs/evidence/decisions/heads as readable historical or
  diagnostic records, but forbid new v1 evidence and promote/reject decisions.
  Permit only a signed rollback to exact prior v1 evidence already in the
  current decision chain; do not retro-sign history, and use a fresh target
  channel for normal foundation migration.
- [x] Document that external corpus/graph/oracle assertions and snapshot
  freshness remain outside M19 cryptographic proof, and preserve the
  single-process, non-HA, selection-only, no-auto-deploy boundary.
- [x] Add checked fixtures, API/CLI/operator documentation, adversarial tests,
  exact Rust gates, and release-binary native/Arango verification before
  marking M19 complete.

## M17: Queue Scale and Governance (done 2026-07-17)

M17 keeps the M16 singleton-writer boundary while making its durable queue
bounded, tenant-fair, and operable at a larger history size. The detailed
contract and acceptance evidence live in
`docs/decisions/decision_m17_queue_scale_governance.md`.

- [x] Make the default job list a filter-bound, tenant-bound cursor scan over
  a projected derived catalog; retain capped offset pagination only as an
  explicit compatibility path.
- [x] Enforce global and per-tenant active-job limits, plus the tenant
  `quotas.max_active_jobs` override capped by the host default. Reject new work
  with HTTP 429 and `Retry-After: 1`; idempotent replays do not consume a new
  slot.
- [x] Replace task-per-job execution with one tenant-aware round-robin
  dispatcher. Long ingests yield after each durable batch checkpoint; FIFO is
  preserved within a tenant. Evaluation remains one non-preemptible unit.
- [x] Move eligible terminal records copy-first into the protected immutable
  archive. Retention chooses archive eligibility; M17 deliberately provides no
  destructive purge.
- [x] Reconcile hot records, archive records, and the repairable catalog in
  bounded cursor batches; expose dry-run/operator controls, status, health,
  and fixed-cardinality metrics.
- [x] Close the exact Rust gates and release-binary native/Arango verification,
  with the observed results and cleanup recorded in the M17 decision record.

## M18: Evaluation Promotion Gates (done 2026-07-18)

M18 makes a successful evaluation job necessary but not sufficient for a
promotion. The accepted foundation contract is recorded in
`docs/decisions/decision_m18_evaluation_promotion_gates.md`.

- [x] Freeze the foundation contract: deterministic, context-bearing
  `construct.evaluate` only; legacy jobs and historical WebNLG, clinical, and
  blind/hostile results remain diagnostic rather than promotable.
- [x] Resolve and freeze a closed `PromotionContext` before execution, including
  candidate/configuration digests, corpus/graph/oracle attestations, resolved
  integer policy, case manifest, reproducibility, and the exact four-stage
  verified-status oracle contract. Exclusions are empty in v1; scorer/verifier
  hashes are pinned operator-trusted semantics identities rather than fetched
  artifact verification. Baseline and replay links are assigned only after
  jobs exist. The checked authoring fixture lives at
  `fixtures/m18/promotion-evaluation.json`.
- [x] Register one immutable evidence bundle in
  `_cognigraph_evaluation_evidence` from ordered candidate original/replay and
  baseline original/replay succeeded jobs; strictly reject malformed, empty,
  overlapping, excluded, manifest-mismatched, or below-denominator case sets.
- [x] Apply independent aggregate distinct-fact recall and restraint gates to
  candidate original and replay against the comparable baseline pair,
  including baseline-regression checks and a closed
  `allowed_candidate_differences` enum. No weighted score or force-promote
  override is permitted relative to the frozen operator-authored policy. M18
  v1 does not separate policy authoring from promotion; M19 adds the signed
  policy registry and distinct approver boundary without rewriting v1 history.
- [x] Persist attributed, idempotent Admin promote/reject/rollback decisions in
  `_cognigraph_promotion_decisions` for each `{space_type, channel}` target, and
  derive `_cognigraph_promotion_heads` decision-first with fail-closed
  reconciliation. Promote and rollback compare the applied decision id, and
  rollback also names the exact target evidence bundle. A tenant mutation fence
  follows authority degradation; `POST /api/admin/promotions/recover` performs
  full validation/repair before clearing it. Target channels are capped at
  10,000 actionable decisions.
- [x] Preflight authenticated-Admin snapshot imports for immutable
  evidence/decision conflicts and hot/archive job provenance, then rebuild
  derived heads rather than trusting imported projections. Treat snapshot
  restore as a trusted-root boundary without built-in signature verification.
- [x] Preserve the singleton/non-HA selection-only boundary and document that
  graph, corpus, oracle, scorer, and verifier trust currently comes from bound
  operator attestations/semantics identities, not portable backend revisions or
  executable verification. Full tenant recovery still materializes tenant-wide
  authority. Public opaque AQL and Lua AQL are disabled for every role after
  Unicode-escaped identifiers proved textual screening insufficient.
- [x] Pass the exact Rust gates and release-binary verification. Native v2.5.0
  completed the full lifecycle, forged-head snapshot rebuild, and restart
  persistence checks. Live ArangoDB 3.12.9-1 completed the same governed
  lifecycle, restart persistence, out-of-band head-loss detection, mutation
  fencing, and full recovery; exact probe rows were removed afterward. The
  observed evidence is recorded in the M18 decision.

## M16: Durable Governed Operations (done 2026-07-17)

M16 moves long-running governed construction from connection-bound requests to
durable, tenant-scoped jobs while preserving the native singleton-writer model.
The accepted contract is recorded in
`docs/decisions/decision_m16_durable_governed_operations.md`.

- [x] Freeze the foundation contract: only `construct.ingest` and
  `construct.evaluate`; no distributed scheduler or HA claim.
- [x] Persist immutable job input, frozen execution payload, state, progress,
  result/error, actor, and append-only transition history in one tenant-local
  protected `_cognigraph_jobs` document per job.
- [x] Require an idempotency key; replay same-key/same-input submissions and
  reject same-key/different-input submissions.
- [x] Run one fair in-process worker, checkpoint ingest at atomic batch
  boundaries, and recover queued/interrupted jobs at least once after restart.
- [x] Add cooperative cancellation and idempotent resume/restart retry, with
  tenant suspension/deletion and snapshot-import fencing.
- [x] Add tenant-scoped submit/list/status/cancel/retry HTTP and CLI surfaces,
  bounded projected list summaries, and owner-or-Admin mutation checks.
- [x] Add fixed-label aggregate job metrics and `/health/jobs`; never label by
  tenant, job, user,
  idempotency key, space, or error text.
- [x] Pass the Rust gates and release-binary persistent-store verification,
  including forced-stop/restart and graceful-interruption runs. Live Arango
  verification also caught the built-in `_jobs` collision, pinned the qualified
  collection name, proved evaluation persistence, and proved ingest fails
  before graph writes on a non-atomic backend.

## Phase 1: Foundation

- [x] Set up Cargo workspace with all crate stubs
- [x] Define core types in cognigraph-core (Document, Edge, Node, Embedding, etc.)
- [x] Define GraphBackend trait
- [x] Define EmbeddingProvider trait
- [x] Define error types with thiserror
- [x] Copy and adapt embeddings from cognigraph-chunker
- [x] Write unit tests for core types (36 tests: DocumentId, Document, Edge, Direction, defaults, error Display)

**Crates:** `cognigraph-core`, `cognigraph-embeddings`

---

## Phase 2: ArangoDB Backend

- [x] Build minimal ArangoDB HTTP client
  - [x] Connection management (base URL, auth)
  - [x] Basic auth and bearer token support
  - [x] Document CRUD (GET, POST, PUT, PATCH, DELETE on `/_api/document`)
  - [x] AQL query execution (`/_api/cursor`) with auto-pagination
  - [x] Collection management (create, drop, list, truncate)
  - [x] Index management (create, list, drop)
  - [x] Error handling and response parsing
- [x] Implement GraphBackend trait for ArangoDB
  - [x] Document operations (create, get, update, replace, delete, list)
  - [x] Edge operations (create, upsert, get by direction)
  - [x] Graph traversal (AQL-based with confidence filtering and path decay scoring)
  - [x] Vector search (APPROX_NEAR_COSINE with deduplication)
  - [x] Collection/index setup (idempotent ensure_collection/ensure_index)
- [x] Integration tests against real ArangoDB instance
- [x] Database initialization (auto-create collections + indexes)
- [x] Auth control collections use Arango system-collection create/drop flags;
  live auth-enabled startup and backend-contract verification passed on
  ArangoDB 3.12.9-1 (2026-07-17)
- [x] First document, edge, or relationship-upsert write materializes its
  missing collection on both native and Arango; the shared contract covers all
  three paths (2026-07-17)

**Crate:** `cognigraph-arango`

---

## Phase 3: HTTP API

- [x] Set up Axum server in cognigraph-server
- [x] Document routes: CRUD endpoints (create, get, update, replace, delete, list)
- [x] Search routes: vector search and parsed read-only CGQL query (public raw
  AQL removed after the Unicode-escape security supersession)
- [x] Search routes: semantic (with embedding), hybrid, graph-augmented
- [x] Graph routes: relationship upsert, get by direction, traversal
- [x] Health/metrics endpoints (health check, database connectivity)
- [x] Lua script execution endpoint
- [x] Error handling middleware (CogniGraphError → HTTP status codes)
- [x] Configuration via environment variables (.env support via dotenvy)

**Crate:** `cognigraph-server`

---

## Phase 4: Lua Query Engine

- [x] Set up mlua with LuaJIT in cognigraph-lua
- [x] Create sandboxed Lua runtime (disable os, io, debug, load, package, require)
- [x] Expose graph primitives to Lua:
  - [x] `graph.query(cgql, bind_vars)` — parsed CGQL only on a CGQL backend;
    read-only for `lua:execute`, with mutations gated by `documents:write`;
    unavailable on AQL backends
  - [x] `graph.get_document(collection, key)` — fetch single document
  - [x] `graph.find_documents(collection, opts)` — list with limit/offset
  - [x] `graph.create_document(collection, doc)` — create document
  - [x] `graph.delete_document(collection, key)` — delete document
  - [x] `graph.traverse(start_vertex, opts)` — multi-hop traversal with scoring
  - [x] `graph.neighbors(vertex_id, direction?, collection?)` — get edges
  - [x] `graph.upsert_edge(from, to, type, data?, collection?)` — create/update edge
  - [x] `graph.similarity(collection, vector, opts)` — vector search
- [x] Resource limiting (instruction count hook, 1M instruction default)
- [x] Script execution API endpoint (`/api/lua/execute`)
- [x] Tests for Lua runtime and sandboxing (18 tests: sandbox enforcement, instruction limits, pure Lua execution)

**Crate:** `cognigraph-lua`

---

## Phase 5: CogniGraph Query Language (CGQL)

- [x] Add isolated `cognigraph-query` crate to the workspace
- [x] Define CGQL AST types for collection queries, traversal queries, vector sources, expressions, projections, sorting, and limits
- [x] Implement parser with `pest`
  - [x] Collection query syntax: `FOR d IN documents ... RETURN d`
  - [x] Traversal syntax: `FOR v, e, p IN 1..3 OUTBOUND @start relationships ...`
  - [x] Vector source syntax: `FOR d IN VECTOR_SEARCH(documents, @embedding) ...`
  - [x] `FILTER`, `SORT`, `LIMIT`, and `RETURN` clauses
  - [x] Object and array projections
  - [x] Bind variables (`@name`)
  - [x] Function calls
  - [x] Unary and binary expressions with operator precedence
  - [x] Line comments (`// ...`)
  - [x] Case-insensitive keywords/functions with case-sensitive identifiers
- [x] Implement semantic validation
  - [x] Scoped identifier checks
  - [x] Duplicate traversal variable detection
  - [x] Duplicate object projection field detection
  - [x] Reserved collection/function name checks
  - [x] Limit bounds checks
  - [x] Traversal depth bounds checks
  - [x] `VECTOR_SEARCH` input validation
- [x] Parser edge-case coverage
  - [x] Malformed clauses and missing `RETURN`
  - [x] Invalid bind variables
  - [x] Repeated/out-of-order clauses
  - [x] Trailing commas
  - [x] Escaped and unterminated strings
  - [x] Nested object/array projections
- [x] Define formal CGQL v1 grammar/spec in docs (`docs/cgql-v1.md`)
- [x] Add AST serialization/snapshot tests
- [x] Add non-executing planner skeleton
- [x] Add in-memory executor for correctness testing
- [x] Add generic `GraphBackend` executor for CGQL
- [x] Add explicit HTTP CGQL entry point; after the M18 security supersession it
  is the only public query-text path
  - [x] `/api/search/query` accepts parsed read-only CGQL only; non-`cgql`
    language selection is forbidden
- [x] Keep Lua on a single `graph.query(query, bind_vars)` entry point
- [x] Add `GraphBackend::query_language()` capability and expose it as `graph.query_language`
- [x] Route `graph.query()` through parsed CGQL on CGQL backends and disable it
  on AQL backends
- [x] Semantics hardening (M1, see `docs/superpowers/plans/2026-07-02-cgql-semantics-hardening.md`)
  - [x] Missing attribute paths evaluate to `null`; `null` is falsy in `FILTER`
  - [x] Depth-0 traversal emits the start vertex (null edge, empty path)
  - [x] Parse errors carry line/column position
  - [x] Strict, eager bind-variable matching (missing and unexpected both fail)
  - [x] Keyword word-boundary guards in the grammar
  - [x] Reserved words rejected as variable names
  - [x] Non-associative comparison operators
  - [x] Full JSON string escape set
- [x] CGQL v2 (M4, see `docs/superpowers/plans/2026-07-02-m4-cgql-v2-pushdown.md`)
  - [x] Function registry (32 built-ins) with validation-time name/arity checks; lenient type semantics (wrong type → null)
  - [x] LIMIT pushdown for unfiltered/unsorted collection scans
  - [x] Filter pushdown — done 2026-07-03: `list_documents_filtered` capability (core default + native by-reference impl, contract-suite covered), AND-conjunct predicate extraction with exact CGQL semantics (`FieldPredicate`), LIMIT pushdown through fully-pushed filters, engine re-applies filters as correctness belt. Fixed a latent COLLECT+LIMIT pushdown bug found during the work (limit was pushed past grouping). 4.2x on filter+sort+limit, 340x on filter+limit full pushdown.
  - [x] EXPLAIN — done 2026-07-03: `EXPLAIN` query prefix returns the static plan + pushdown decisions (identical on both engines, corpus-pinned); never executes, safe for mutations on read-only endpoints
  - [x] File-driven CGQL corpus (`tests/corpus/*.cgql`): parse_ok / parse_err / validate_err / exec with expected results, executed on BOTH engines (in-memory executor + native backend via `query()`)
  - [x] Multilingual coverage (German, Russian, Spanish, Hebrew, mixed-script) across corpus, native BM25, and persistence
- [x] CGQL ergonomics (M5, see `docs/superpowers/plans/2026-07-02-m5-cgql-let-sort-distinct.md`)
  - [x] `LET` per-row bindings (declaration order, before filters; scope-checked)
  - [x] Multi-key `SORT` with per-key direction
  - [x] `RETURN DISTINCT` (numeric-aware dedupe, first-occurrence order)
  - [x] `null` falsy in `NOT`/`AND`/`OR` (consistent with filters)
  - [x] Date functions: `NOW`, `DATE_YEAR/MONTH/DAY/HOUR/MINUTE/SECOND`, `DATE_TIMESTAMP`, `DATE_DIFF`
- [x] COLLECT (M6): multi-key grouping with `WITH COUNT INTO`, `AGGREGATE`, `INTO` group capture, scope replacement after grouping, and first-occurrence group order
- [x] Unicode decisions (closed 2026-07-03)
  - [x] Locale collation: `SORT ... COLLATE "de"` via ICU4X; codepoint order stays the default (pin unchanged)
  - [x] Exact Unicode storage/literal identity (CG-33); explicit `NORMALIZE_NFC` for text comparisons, pinned by `i18n_exact_literals`. Supersedes the historical implicit NFC default; construction evidence retains its explicit NFC boundary.
  - [x] `COSINE_SIMILARITY(a, b)` added to the function catalog

**Crate:** `cognigraph-query`

---

## Phase 6: Native Backend

- [x] Add `cognigraph-native` crate
- [x] Implement first in-memory native backend
  - [x] Document CRUD
  - [x] Edge creation/upsert and directional edge lookup
  - [x] BFS traversal with direction, depth, confidence filtering, and path decay
  - [x] Exact cosine vector search over `embedding` arrays
  - [x] Schema collection creation/drop
  - [x] No-op index creation placeholder
- [x] Native backend reports `query_language = cgql`
- [x] Native backend routes `query()` through CGQL
- [x] Server can select native backend with `COGNIGRAPH_BACKEND=native`
- [x] Native-first server (M2, see `docs/superpowers/plans/2026-07-02-m2-native-first-server.md`)
  - [x] `GraphBackend::ping()` capability; `/health/database` no longer sends AQL
  - [x] `DocumentConflict` error (HTTP 409); duplicate `_key` create conflicts on all backends
  - [x] Update/replace on a missing collection reports `CollectionNotFound` without creating it
  - [x] Collection type enforced on native writes (native-only semantics)
  - [x] Shared conformance suite in `cognigraph_core::contract` (native always, Arango env-gated)
  - [x] Conformance verified against live ArangoDB (2026-07-02, `COGNIGRAPH_VECTOR_SEARCH_MODE=fallback`): CRUD, 409 conflict, missing-collection, depth-0 traversal, vector search all pass
  - [x] Arango `vector_search` now returns the full stored document (merged with score) and dedupes by `document_id` only when present — previously it returned an embeddings-row projection and collapsed all rows lacking `document_id` into one hit
  - Note: native-mode (`APPROX_NEAR_COSINE`) conformance needs a pre-built vector index on the test collection; the suite's ad-hoc collections exercise the fallback path
  - [x] Hybrid search: BM25 leg gated on `query_language() == Aql`, failures propagate, skip is explicit in the response
  - [x] `/api/search/query` rejects an explicit language the backend does not speak
  - [x] Native traversal is O(E + output) after a lazily built,
    generation-invalidated adjacency index; paged mode builds the same index
    from redb
- [x] Persistent storage layer (M3, redb write-through — model in `docs/native-storage-model.md`, plan in `docs/superpowers/plans/2026-07-02-m3-native-persistence.md`)
  - [x] `NativeBackend::open(path)`; `COGNIGRAPH_NATIVE_PATH` selects persistence in the server
  - [x] One committed redb transaction per write operation, before memory mutates
  - [x] Schema version guard (`meta.schema_version = 1`)
  - [x] JSON export/import (`export_json` / `import_json`) as the migration path
- [x] Durability tests (reopen, deletions, type preservation, CGQL over reloaded state), import/export roundtrip tests, conformance suite against the persistent backend
- [x] `GraphBackend::text_search` capability; native BM25 is Tantivy-indexed,
  lazily built, generation-invalidated, and persisted beside durable native
  stores for warm restart (0.09 ms on the recorded 10k-doc benchmark)
- [x] Hybrid search works on the native backend (text_search leg + RRF); native is the default `COGNIGRAPH_BACKEND`
- [x] `VectorSearchOpts.threshold` is `Option<f64>` (no `-1.0` sentinel in the shared contract)

**Crate:** `cognigraph-native`

---

## Phase 7: Additional Backends

**Dropped (July 2026, native-first strategy).** No new third-party database
backends. ArangoDB remains in maintenance mode as the backend-contract
reference implementation until native parity.

---

## Phase 8: Authentication & User Management

- [x] Backend-agnostic `AuthProvider` in `cognigraph-auth`, storing users and tokens through `GraphBackend`
- [x] User create/list/delete and argon2 password hashing
- [x] Hashed API-token create/list/revoke/rotate, optional expiry, and one-time plaintext return
- [x] Bearer-token and JWT authentication middleware
- [x] Role-based access control for admin, editor, viewer, and script-runner roles
- [x] Route-level scope policy, including read/write graph and document scopes, Lua execution, admin, and host-level tenant administration
- [x] JWT session tokens (HS256, `POST /api/auth/login`, `COGNIGRAPH_JWT_SECRET`/`COGNIGRAPH_JWT_TTL_SECS`)
- [x] Multi-tenancy identity and isolation: tenant-bearing users/tokens/JWTs, separate control store, and one routed backend/cache per tenant

**Crates:** `cognigraph-auth`, `cognigraph-server`

---

## Phase 9: Advanced Features

- [ ] RAG service (context assembly + LLM synthesis)
- [x] Hybrid search (BM25 + vector with RRF fusion)
- [x] Graph-augmented search with multi-hop traversal
- [x] Semantic query cache (cognigraph-cache crate)
  - [x] `QueryCache` trait with swappable backends
  - [x] In-memory LRU backend with similarity-aware lookup
  - [x] Embedding cache (query text → vector)
  - [x] Search result cache with cosine similarity matching (configurable threshold)
  - [x] TTL-based expiration and LRU eviction
  - [x] Cache integration in semantic, hybrid, and graph-augmented search routes
  - [x] Cache invalidation on document writes and relationship upserts
  - [x] Hit-quality tiers (strong/medium) with progressive retrieval
  - [x] Cache stats endpoint (`GET /api/cache/stats`) and clear endpoint (`POST /api/cache/clear`)
  - [x] Continuous cache influence model (cubic weight curve, replaces discrete tiers)
  - [x] Cache-assisted retrieval via RRF (cached signal + fresh vector search, weight by similarity)
  - [x] Observability: direct/assisted hit breakdown, invalidation counts, similarity reporting
  - [x] Per-document rank decay in cache-assisted fusion (top cached docs influence more)
  - [x] Above-floor continuous cache influence; at/below-floor lookups are fresh and are not reported as assisted
  - [x] redb-persistent embedding cache (result cache remains memory-only)
  - [ ] Redis cache backend (deferred until multi-instance deployment)
- [ ] Clustering algorithms
- [x] Governed relationship construction through `cognigraph-construct` and Semantic Neurons
- [ ] DailyMed 10k-document operational pilot — acquisition, oracle-scored
  gates, synthetic and real-revision drift, and pass-2 no-labels clinical
  construction/self-healing completed. Clinical calibration froze rules R1-R7 and
  exposed that the hand-picked condition vocabulary (not the reference) was the
  defect (3/24 candidates TRUE). Fixed in two parts: the vocabulary is now
  corpus-derived (4,047 condition-typed relation-specific entries = 4,017 unique strings; 30 are valid for both relations), and an isolated clinical
  assertion matcher (`clinical-matcher-v2`, enumeration/bullet/adjunctive/
  prohibition/typed-bare-list aware; `ground_chunk` and gates 1-3 untouched)
  reaches **19/19 accepted TRUE, 8/8 accepted FALSE refused, 14/14 restraint
  probes** on the project-owner showcase calibration. Grounding is scored in
  three lanes (the oracle-vocabulary lane is a diagnostic, never end-to-end), and
  the three losses are reported as a partition (vocabulary / trigger-structure /
  type noise). **None of this is a clinical recall claim.** The blinded
  two-reviewer clinical reference toolkit is implemented; expert annotation,
  adjudication, held-out recall, and clinical judge-quality measurement remain —
  they are the only things that can produce recall
  (`docs/dailymed-clinical-reference.md`).
- [x] WebNLG external-oracle pilot — **deterministic scoring complete**
  (2026-07-17): the pinned English canonical splits yield 38,872 text
  documents and 115,279 DBpedia triples in physically separated
  construction/oracle files. Acquisition and offline verification are
  reproducible. The frozen, supervised template-mining lane scored 12.4%
  recall / 76.6% precision on validation and 7.0% / 68.8% on the one-shot test;
  all lanes received oracle-provided entity surfaces. This measures relation
  template construction, not entity discovery or the full runtime Neuron
  review lifecycle. July 20 validation follow-ups are also complete: fuzzy
  matching was rejected, expanded corpus mining left recall flat, and live LLM
  proposals followed by offline pruning reached 12.6% recall / 76.3% precision
  (609 correct versus 601 baseline). Validation informed predicate selection;
  this is a small recall/precision tradeoff, not a new frozen test result.
  Hard negatives, entity-discovered scoring, and runtime governed-review
  evaluation remain follow-up work. See the [current guide](../../research/webnlg/pilot.md).
- [x] Hot JSON snapshot export/import; CSV export is not implemented

**Crates:** `cognigraph-server` (extensions), `cognigraph-cache`

---

## Phase 10: Production Readiness & Performance

- [x] Close authorization gaps before untrusted auth-enabled deployment:
  `/api/search/query` accepts parsed read-only CGQL only; public raw AQL and Lua
  AQL are disabled for every role after Unicode-escaped identifiers bypassed
  textual screening; CGQL mutations and every typed Lua mutation require
  `documents:write`; auth-disabled Lua stays read-only; and the guarded backend
  rejects system-collection access through direct, batch, edge, result, and
  runtime traversal paths.
- [x] Make construction revision- and provenance-safe: the native backend
  atomically replaces each supplied chunk plus its mention/fact occurrences;
  identical triples retain independent evidence occurrences; sanitized-key
  collisions fail closed; concurrent in-process reconciliation is serialized;
  backends without atomic batches reject before writes.
- [x] Restore backend/search contract parity: unbounded Arango scans are no
  longer capped at 100 rows; traversal scores consistently combine edge
  confidence with path decay; weak cache hits merge with fresh retrieval;
  strong cache hits keep the fresh response shape; deleted cache-only rows are
  filtered; dependency-changing managed writes invalidate result caches. The
  shared suite and an auth-enabled release binary were live-verified against
  ArangoDB on 2026-07-17.
- [x] Make operational status reliable: `/health/database` returns HTTP 503
  when the active backend probe fails, native persistence is probed instead of
  inheriting an unconditional success, and cache statistics resolve the active
  tenant rather than returning fallback zeros.
- [x] Rate limiting (per-IP fixed window, custom, `COGNIGRAPH_RATE_LIMIT_PER_MINUTE`)
- [ ] Circuit breaker / retry patterns (deferred — Arango is maintenance-mode)
- [x] Prometheus metrics (`/metrics`, hand-rolled text exposition)
- [x] Structured logging (`COGNIGRAPH_LOG_FORMAT=json`)
- [x] Request timeout (`COGNIGRAPH_REQUEST_TIMEOUT_SECS`) and graceful shutdown (SIGINT/SIGTERM)
- [x] Docker / container support (multi-stage Dockerfile, non-root, native backend + volume) — hardened 2026-07-04: /data ownership, HEALTHCHECK, LuaJIT build deps, docker-compose.yml production profile; image smoke-tested (health, CRUD, export, restart persistence)
- [x] Backup/restore (2026-07-04): `export_snapshot`/`import_snapshot` backend capability + admin routes GET /api/admin/export, POST /api/admin/import (hot JSON snapshot, admin scope, cache-clearing import); cold-copy procedure documented
- [x] OpenAPI spec (2026-07-04): hand-maintained crates/cognigraph-server/openapi.yaml, served live at GET /openapi.yaml
- [x] Operations runbook (2026-07-04): docs/operations.md — config reference, docker/systemd deployment, auth bootstrap, backup/restore, capacity notes
- [x] Concurrent-load benchmark (2026-07-03): cargo bench -p cognigraph-server --bench load; see docs/benchmarks.md
- [x] CLI tool for administration (2026-07-04): `cognigraph` binary (crates/cognigraph-cli, hand-rolled parsing, blocking reqwest) — health, export/import, CGQL query (+EXPLAIN, --write, --bind), doc CRUD, user/token management with username resolution, cache ops, JWT login; smoke-tested end-to-end against a live authed server; shipped in the Docker image
- [x] Comprehensive test suite across all crates (use the current `cargo test --all` result rather than freezing a repository-wide count here)
- [x] Benchmark harness (cargo bench -p cognigraph-native, docs/benchmarks.md); ~~Python comparison~~ dropped 2026-07-03 — the Python original was research-grade, not a meaningful baseline
- [x] Performance optimizations — reference scoring/top-k cloning,
  quantization, mmap sidecar, and Rayon delivered; see `benchmarks.md` for
  dated measurements and remaining conditional triggers
  - [x] Int8 quantization: two-stage search (i8 scan + exact re-rank), 2x on top of rayon, exact scores preserved; the 4x memory saving lands with the vector sidecar
  - [x] Zero-copy mmap reads via the vector sidecar (COGNIGRAPH_VECTOR_MODE=sidecar): 10.2 MB f64 RAM → 1.4 MB paged i8 file (7.3×), 0.85 ms search
  - [x] Parallel scoring with Rayon (vector 23.7x total, BM25 1.4x — see docs/benchmarks.md)
  - [x] redb-backed persistent cache for hot embeddings
  - [x] Batch embedding pipeline (provider-batched requests → one atomic store transaction)

**Crates:** All

---

## Management UI (prototype, July 2026)

- [x] Select the initial product direction: a familiar database-management
  console centered on collections, documents, and a contextual JSON inspector.
- [x] Scaffold a standalone `ui/` React + TypeScript application using Bun's
  native HTML development server and production bundler; no Vite or server
  integration.
- [x] Configure Biome and keep UI modules within the repository's 300–400 line
  modularity convention.
- [x] Build the first interactive collection-management slice with mock data:
  search/filter, row inspection, JSON/metadata tabs, edit/save, create, delete,
  tenant selection, and responsive desktop behavior.
- [x] Integrate the accepted prototype with functional server endpoints without
  changing `cognigraph-server`: health/database status, document CRUD, read-only
  CGQL, graph traversal, user inspection, cache operations, snapshot export,
  and the server-owned OpenAPI document.
- [x] Add session-scoped server URL and bearer-token configuration, honest
  offline/auth-disabled states, and browser-verified live API workflows.
  (Superseded 2026-07-15: a username/password login screen now exchanges
  credentials for a JWT; the server URL derives from the load origin and the
  bearer-token paste is gone — see decision_management_ui.md, addendum.)
- [x] Complete a cross-page desktop UX audit: restore JSON result gutters,
  implement sidebar collapse and document page sizing, remove false menu
  affordances, and clarify token-derived tenant and auth-disabled states.
- [x] Upgrade Graph into a canvas-first neighborhood explorer backed by the
  existing traversal API: a centered root with radial depth placement and
  compact orbital document markers, typed/confidence-labelled edges,
  visual/JSON modes, node and relationship inspection, neighborhood expansion,
  canvas controls, minimap, and a selected-path summary.
- [x] Migrate the Graph canvas from React Flow to the exact-pinned AntV G6
  5.1.1 renderer, retaining the approved circular depth language while using
  G6's native radial layout, canvas interactions, and minimap.
- [x] Replace the CGQL query textarea with a CodeMirror 6 editor aligned to the
  current `cognigraph-query` grammar and built-in function registry: syntax
  highlighting, visible line numbers, comment/string/bind handling, inline
  diagnostics, missing-bind checks, and explicit `Command+Enter` plus
  `Control+Enter` execution. Keep Rust as the
  validation authority by sending debounced, non-executing `EXPLAIN` requests
  through the existing read-only `/api/search/query` route; downgrade background
  validation of `EXPLAIN ANALYZE` so editor validation never executes a query.
- [x] Adopt exact-pinned Ant Design 6.5.1 for stateful management controls while
  preserving CogniGraph's application shell, visual tokens, CodeMirror editor,
  and G6 canvas. Migrate forms and validation, selects, dialogs and destructive
  confirmations, status feedback, tabs, tables, pagination controls, empty and
  disabled states, and loading affordances. Verify all six primary pages with
  live browser interactions and accepted screenshots; keep native document
  pagination because the current endpoint does not expose a total count.
- [x] Harden G6 lifecycle teardown for rapid Graph-to-page navigation. The
  minimap's debounced `AFTER_RENDER` work now completes before the graph model
  is destroyed, preventing the `getData` runtime overlay when leaving Graph.
- [x] Namespace the application API under `/api` (the UI owns `/`); keep
  `/health`, `/metrics`, and `/openapi.yaml` at the root; enforce the mapping
  with bidirectional OpenAPI drift tests.
- [x] Replace token paste with a real session flow: username/password login
  (`POST /api/auth/login` → JWT in sessionStorage), origin-derived server URL,
  and expired/revoked sessions dropping back to sign-in.
- [x] Add the Review workspace (Semantic Neurons management): proposal queue
  with accept/reject/retire and reviewer attribution, graduation flags, and an
  ontology-validated propose dialog.
- [x] Add user management: account list + create/delete with a self-delete
  guard, and a `/users/{username}` page with role facts plus the API-token
  lifecycle (issue with TTL presets, rotate, revoke, plaintext shown once).
- [x] Add URL routing (react-router, path URLs): every page addressable, deep
  links survive the login gate, `?doc={key}` opens a document.
- [x] Serve the built console from `cognigraph-server` (`COGNIGRAPH_UI_DIST`)
  with an SPA fallback; unknown `/api` paths stay JSON 404; the UI build emits
  absolute asset URLs and cleans `dist/`.
- [x] Add the collection catalog as an HTTP contract and restructure the
  browser around it: `GraphBackend::list_collections` (native impl),
  `GET /api/collections` (names, types, counts; system collections hidden),
  a `/collections` index page, and the document browser at
  `/collections/{collection}` with `?doc={key}` one-shot deep links.
- [x] Add search-mode tabs to the Query console: semantic, hybrid, vector, and
  graph-augmented forms backed by `/api/search/*`, with catalog-fed collection
  selects, hit tables deep-linking into the document browser, and honest
  disabled states (no embedder, no edge collections). Fixing its verification
  path exposed and fixed the query cache ignoring request parameters
  (decision_cache_backends.md, addendum).
- [x] Back the document browser's search box with the server: new
  `POST /api/search/text` (plain BM25 via `GraphBackend::text_search`, no
  embedder required); a query searches the whole collection merged with an
  exact-key lookup instead of filtering the loaded page.
- [x] Add the host-admin Tenants screen: list with status and store state,
  create, suspend/resume, delete (`/api/tenants*`); building it exposed and
  fixed the server minting sessions for suspended tenants' users.
- [ ] Add reviewed snapshot import in a later UI phase (tracked in
  `ui/TODO.md`).

---

## Next: Roadmap 2026 H2

Phases 1–10, the CGQL v2 arc, and the first Semantic Neurons arc are done
except where explicitly deferred above. Current forward work and its
remaining triggers live in [roadmap-2026-h2.md](roadmap-2026-h2.md).

## Progress Tracking

| Phase | Status | Crate(s) |
|-------|--------|----------|
| 1. Foundation | **Done** | cognigraph-core, cognigraph-embeddings |
| 2. ArangoDB Backend | **Done** | cognigraph-arango |
| 3. HTTP API | **Done** | cognigraph-server |
| 4. Lua Query Engine | **Done** | cognigraph-lua |
| 5. CGQL | **Done through v2** (joins/subqueries/positional semantics, EXPLAIN + EXPLAIN ANALYZE, filter/IN/threshold pushdown, budgets, 190+ corpus cases dual-engine) | cognigraph-query |
| 6. Native Backend | **Done** for single-node scope (in-memory + redb persistence, paged reads, BM25 index, mmap vector sidecar) | cognigraph-native |
| 7. Additional Backends | **Dropped** (native-first strategy) | — |
| 8. Auth & User Management | **Done** (argon2 users, hashed API tokens, JWT sessions, RBAC scopes) | cognigraph-auth, cognigraph-server |
| 9. Advanced Features | **Partially Done** (hybrid + graph-augmented search, persistent embedding cache, governed construction; clustering and CSV export remain out) | cognigraph-server, cognigraph-cache, cognigraph-construct |
| 10. Production Readiness | **Done** (perf arcs, load bench, Docker/compose, backup/restore, OpenAPI, ops runbook, admin CLI, CI) | All crates |
| 11. Semantic Neurons (M14+) | **Done for v1** (research parity, 4 neuron kinds, control loop, generalization evidence; Arc B continues in roadmap-2026-h2.md) | cognigraph-construct |
| M15. Trustworthy Foundation | **Done** (authorization, revision provenance, backend/cache contracts, readiness truth; native and Arango release-binary HTTP verified) | All crates |
| M16. Durable Governed Operations | **Done** (persistent tenant-scoped construction/evaluation jobs, restart recovery, lifecycle API/CLI, audit history, metrics, and native/Arango live verification) | cognigraph-server, cognigraph-auth, cognigraph-cli |
| M17. Queue Scale and Governance | **Done** (cursor catalog, queue quotas/backpressure, checkpoint-fair scheduling, terminal archive, reconciliation, operator controls, native/Arango release verification) | cognigraph-core, cognigraph-native, cognigraph-arango, cognigraph-server, cognigraph-auth, cognigraph-cli |
| M18. Evaluation Promotion Gates | **Done** (closed context/policy, four-job immutable evidence, independent gates, attributed per-target decisions, mutation-fenced recoverable heads, trusted-root snapshot provenance, and native/Arango release verification) | cognigraph-construct, cognigraph-server, cognigraph-cli |
| M19. Signed Governance and Separation of Duties | **Done** (root-certified purpose-bound keys, prospective revocation, signed policy/approval/intent, three-principal authority, full recovery validation, and native/Arango release verification) | cognigraph-governance, cognigraph-auth, cognigraph-server, cognigraph-cli |
| M20. Content-Addressed External Artifact Attestations | **Done** (signed five-kind exact-byte manifests, context/evidence/intent bindings, four-duty separation, bounded summary/storage behavior, recovery/snapshot validation, and Native/Arango release verification) | cognigraph-governance, cognigraph-auth, cognigraph-server, cognigraph-cli |
| M21. Verified Artifact Consumption | **Done** (local-CAS byte verification, immutable graph/oracle evaluation, startup-pinned executable identity, durable receipt/evidence/intent authority, adversarial regressions, and Native/Arango release verification) | cognigraph-construct, cognigraph-governance, cognigraph-server |
| M22. Reproducible Corpus-to-Graph Derivation | **Done** (prepared-chunk corpus + exact construction candidate replay, canonical evaluation-fact graph equality, durable derivation authority, recovery/snapshot validation, persistent Native/live ArangoDB verification) | cognigraph-construct, cognigraph-governance, cognigraph-server |
| M23. Reproducible Raw-Document-to-Prepared-Corpus Processing | **Done** (exact UTF-8 byte preparation, canonical prepared-corpus equality, durable preparation authority, recovery/snapshot propagation, and release-binary Native/ArangoDB verification) | cognigraph-construct, cognigraph-governance, cognigraph-server |
| M24. Durable CAS Custody and Verified Restoration | **Done** (deterministic evidence recovery plans, closed portable bundles, pinned verification, absent-scope no-replace restore, final CAS reread, and release-binary Native/ArangoDB recovery verification) | cognigraph-artifacts, cognigraph-server, cognigraph-cli |
| M25. Governed Semantic Repair Authority | **Done** (immutable signed typed-candidate revisions, independent review, existing-head exact resolution, generic mutation closure, and fail-closed recovery) | cognigraph-governance, cognigraph-server, cognigraph-cli |
| M26. Verified Semantic Repair Materialization | **Done** (CAS-verified complete occurrence generations, exact impact, independent Promoter-signed Native deployment/rollback, immutable recovery, and Native/ArangoDB boundary verification) | cognigraph-construct, cognigraph-governance, cognigraph-native, cognigraph-server, cognigraph-cli |

---

## Dependencies Summary

Key Rust crates for the project:

| Crate | Purpose | Used In |
|-------|---------|---------|
| `tokio` | Async runtime | All |
| `serde` / `serde_json` | Serialization | All |
| `pest` / `pest_derive` | CGQL parser grammar | cognigraph-query |
| `thiserror` | Error types | cognigraph-core |
| `async-trait` | Async trait support | cognigraph-core, cognigraph-embeddings |
| `reqwest` | HTTP client | cognigraph-arango, cognigraph-embeddings |
| `axum` | HTTP server | cognigraph-server |
| `tower` | Middleware | cognigraph-server |
| `mlua` | Lua scripting | cognigraph-lua |
| `tracing` | Structured logging | All |
| `dotenvy` | .env file loading | cognigraph-server |
| `ort` | ONNX inference | cognigraph-embeddings (optional) |
| `argon2` | Password hashing | cognigraph-auth |
| `ed25519-dalek` / `getrandom` | Ed25519 signing, verification, and generated key material for external governance tooling and tests | cognigraph-governance |
| `unicode-normalization` | NFC-normalized canonical signed statements and principal identities | cognigraph-governance, cognigraph-server |
| `lru` | LRU cache data structure | cognigraph-cache |
| `redb` | Native durability and persistent embedding cache | cognigraph-native, cognigraph-cache |
| `tantivy` | Native BM25 text index | cognigraph-native |
| `icu` | Locale-aware CGQL collation | cognigraph-query |
| `rayon` | Parallel vector and text scoring | cognigraph-native |
