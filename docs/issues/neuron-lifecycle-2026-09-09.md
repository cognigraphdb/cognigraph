# CG-21 neuron lifecycle verification — 2026-09-09

CG-21 serializes human transitions and automated review publication within the
active tenant incarnation. A hint and blocker for the same triple cannot both
validate against an old accepted set and then commit. Late judge results cannot
overwrite a newer human decision or another review result.

## Implementation and contract

- `neuron_lifecycle.rs` provides an application-shared, tenant/incarnation-scoped
  async mutex. Weak registrations are pruned; a recreated tenant has a separate
  scope. The lock covers the current reads, validation, write, and cache
  invalidation. Human accept/reject/retire all use it, and snapshot import shares
  it so an import cannot interleave with the validation/write boundary.
- Both human acceptance and Lane A/A+ use `validate_acceptance`, which reads the
  current accepted set and ontology. Accepted-set backend errors propagate and
  malformed accepted rows fail closed instead of being silently omitted. An
  absent neurons collection still represents an empty set for new spaces.
- Provider screening, primary judgment, and partner judgment run outside the
  lock. On return, review compares the complete stored proposal to its captured
  source, requires `proposed`, and checks that the captured policy and parsed
  ontology still match. A changed source discards the entire stale result,
  including triage metadata, without mutating the current neuron.
- A newly accepted conflicting peer queues an otherwise eligible proposal with
  the symbolic validation reason and judge verdict. Backend failures abort with
  their error status; they do not grant acceptance or get downgraded to triage.
- The review response adds `skipped: [{key, reason}]`. `reviewed` counts published
  auto-accepts and triage results. `pending_remaining` removes skipped proposals
  that have already been decided/judged, but retains changed proposals that still
  need review. Existing human response shapes, attribution, and lane eligibility
  are preserved. Review-note truncation now respects UTF-8 boundaries.

This is the repository's existing single-writer server contract, applying to
both backend implementations through the shared route layer. It does not add
coordination between multiple processes writing the same Arango database or
prohibit privileged snapshot restoration of historical data. Imported invalid
accepted sets are not silently repaired. Judge prompts, qualification policy,
model defaults, and the deferred model benchmark remain unchanged.

## Deterministic Rust regressions

Nine new tests exercise production routers or the shared scope boundary:

- Two human acceptances are interleaved after the first request captures its
  accepted set. Both hint-first and blocker-first orders allow one acceptance
  and reject the other with HTTP 400.
- Human rejection, retirement, and acceptance survive late auto-accept and
  triage responses with the complete human-authored document preserved.
- A blocker accepted during provider work makes the hint queue; two overlapping
  reviews publish once, including when both verdicts are triage.
- Proposal edits, ontology edits, and policy edits discard the stale result;
  unchanged `proposed` status alone is insufficient for freshness. These rows
  remain counted as pending.
- Lane A+ tests pause the partner after the primary returns, then exercise a
  clean acceptance, human rejection, and acceptance of a conflicting blocker.
- Current accepted-set outages produce HTTP 503; malformed rows produce HTTP
  500. Human and automated paths preserve the proposal and release the lock.
- Equal tenant/incarnation scopes share the lock; other tenants and replacement
  incarnations proceed independently. Scope registrations are pruned.

The controlled backend captures a scan before pausing it, and the controlled
judge signals after entering the provider call. These are deterministic
interleavings; the HTTP human-pair stress below is additional scheduling coverage.

## Release HTTP verification

The [reusable harness](evidence/neuron-lifecycle-http.py) launches authenticated
release servers with disposable Native stores and a gated loopback OpenAI-shaped
completion service. It forwards no real provider credentials and makes no
external model calls. Config/policy fixtures enter through admin snapshot import;
proposal authoring, human transitions, review, and document reads use real HTTP.
All server processes are terminated, while synthetic stores/logs remain in the
reported temporary directories.

The saved `8164f4e` release is the baseline. The corrected release repeats the
same scenarios in resident/embedded, resident/sidecar, and paged/sidecar modes.
Per mode the harness covers six human-decision/review races, a blocker accepted
during review, overlapping acceptance and triage reviews, three source edits,
a clean Lane A control, and 20 simultaneous human pairs. It restarts each server
and compares all final neuron documents exactly.

At this checkpoint, dedicated judge and partner startup wiring ignored
`OPENAI_BASE_URL`. That independent defect, [CG-36](CG-36.md), was subsequently
resolved in the [dedicated endpoint batch](judge-endpoint-2026-09-09.md).
This CG-21 harness uses the
configured fallback judge. A+ partner publication is covered by deterministic
production-router tests; it was not exercised with a dedicated partner in the
release process, and no external provider qualification run was performed.

## Validation

- `cargo test -p cognigraph-server neuron_lifecycle`: PASS, nine new tests.
- `cargo fmt --all -- --check`: PASS.
- `cargo clippy --all-targets -- -D warnings`: PASS.
- `cargo test --all`: PASS, 937 reported tests, zero failures or ignored tests.
  Arango was not configured for this run; eight existing integration entries
  returned without live database calls. The existing OpenAI and Gemini embedding
  smoke tests used local configuration and passed. Those unchanged smoke tests
  are separate from this task's local-only judge regression, and are not a new
  model benchmark.
- `cargo build --release -p cognigraph-server`: PASS.
- Final release: PASS, 99 HTTP scenarios, including 60 simultaneous human pairs
  with zero forbidden pairs; 162 exact document comparisons after three restarts.
  Ninety provider HTTP calls were sent to the local synthetic judge service.
- Baseline: all 36 forced stale/conflict cases reproduced the old defect; 26 of
  60 simultaneous human pairs also committed both conflicting neurons. The final
  baseline and corrected harness both completed, with expected failures asserted
  only for the saved old binary.

[Baseline observations](evidence/neuron-lifecycle-baseline-http-2026-09-09.json),
[corrected HTTP and restart observations](evidence/neuron-lifecycle-http-2026-09-09.json),
and [validation totals, source hashes, and executable hashes](evidence/neuron-lifecycle-validation-2026-09-09.json)
identify the tested artifacts. No final Rust gate or corrected release check failed.

## Corrected attempts

The first baseline harness failed while authoring `needs_human` cases: the
underscore in the generated neuron ID violated the existing kebab-case rule.
The harness now uses hyphens in IDs while preserving the provider's
`needs_human` verdict. This was a fixture error, not an application regression.

The first corrected release passed 90 scenarios and 153 document comparisons
across restarts. Final inspection identified the pending-count edge case for a
changed-but-still-proposed source. After correcting that accounting and preserving
human transition parsing, the focused Rust tests passed again. The final harness
adds three source-edit scenarios per mode and is rerun on the final release;
the complete Rust gates are repeated for the final source state.

## Reproduction

```bash
cargo test -p cognigraph-server neuron_lifecycle
cargo fmt --all -- --check
cargo clippy --all-targets -- -D warnings
cargo test --all
cargo build --release -p cognigraph-server
python3 docs/issues/evidence/neuron-lifecycle-http.py \
  --output /tmp/cg21-http.json
```

Use `--binary /path/to/saved-server --expect-vulnerable` to reproduce the prior
release's failures. The harness's synthetic fixture policy is solely for
exercising publication logic, not evidence of any model's quality or qualification.

## Completion

CG-34 was committed locally as `8164f4e` before this work. CG-21 was subsequently
committed locally as `f74f0aa`. Nothing was pushed; unrelated research drafts
and the existing `CLAUDE.md` deletion were preserved. The registry contains
25 resolved and 11 open issues after adding CG-36. Next is CG-33, the remaining
opaque-reference correctness defect, before the public-contract documentation
batch. Model benchmarking stays deferred, with Luna as the economical baseline.
