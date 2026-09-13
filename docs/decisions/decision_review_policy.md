# Decision: risk-tiered review policy (D1–D6)

> Data removal, 2026-09-11: Historical aggregate results and source hashes affected by repository data removal are withdrawn. Retained records are a subset, not a rerun or a qualified benchmark. See the repository data-removal record for the current boundary.

**Status:** Historical decision record from 2026-07-06, with the current M25
contract recorded in the 2026-07-19 addendum and lifecycle concurrency
contract in the 2026-09-09 CG-21 addendum below.

> **Current-contract warning:** the dated D1-D6 outcomes and later experiment
> addenda preserve what was measured or shipped at the time; they are not the
> current authority contract. Today only `relation_hint` can auto-accept,
> omitted `sampling_rate` means `0.1`, malformed judge output fails closed, and
> `qualified_judges` is mutable legacy configuration rather than signed judge
> qualification. See the final addendum before using this record operationally.

## Context

The human-review-everything model dies at scale: at 10k documents no
human meaningfully reviews every proposed neuron, and a rubber-stamping
human is worse than no review — the audit trail records diligence that
didn't happen. The judge-replay experiment
(`fixtures/semantic-neurons/judge-replay/`) measured the alternative: a
purpose-bound, schema-constrained LLM reviewer replayed 76 recorded
decisions at 82% agreement with **0/67 false-accepts on relation hints**
(independently catching the negated-evidence case), all hint errors
conservative; its two blocker false-accepts were exactly the cases whose
labels encoded coverage-simulation data it wasn't shown — layered with
the existing symbolic checks, 0/76. Separately, the analytical labor of
review was already mechanized (verbatim self-check, coverage simulation,
graduation, dead-trigger detection); the open question was who consumes
the verdicts.

## Decisions (owner: user, 2026-07-06 design session)

**D1 — Three lanes, split by neuron class × judge verdict.** Lane A
(auto-accept): `relation_hint`/`alias` only, symbolic floor passed,
judge `accept` at ≥ `min_confidence` (0.90 initial). Lane B (human
queue, judge-assisted): all blockers and rank hints, plus any Lane-A
item below threshold, escalated, judge-rejected, or targeting a
precision-critical rule. Lane C (human-only, never automatable):
transitions of already-accepted state (retire, reject-after-accept).
**The judge never auto-rejects** — a judge "reject" queues with the
verdict attached; proposals are inert, so auto-rejection buys nothing
and risks recall on the judge's measured 15% conservative error.

**Lane A exclusions beyond the verdict (added from the injection-suite
finding):** hints that would CREATE a new triple, and hints extending
rules the author marked precision-critical (template triggers or
`require_in_sentence` gates) always queue — the injection suite found
the judge accepts possessive-appositive direction reversals at 0.98
(clean controls included), and direction-critical rules are exactly the
templated ones.

**D2 — Policy is authored, per-space, reviewable config — off by
default.** `review_policies/{space_id}`: `{auto_accept: {kinds,
min_confidence}, sampling_rate, injection_suite_passed}`. No policy
document = today's behavior, byte-identical (the review verb refuses).
Kinds are whitelist-validated server-side (blockers/rank hints rejected
at policy load, not trusted to config). Auto-accepts stamp
`reviewed_by: judge:<model>@<POLICY_REV>` plus the judge's verdict,
confidence, and reasoning on the neuron — QW1's trail, honest about
what reviewed.

**D3 — A separate `POST /api/construct/review` verb.** Propose stays pure;
review judges the pending queue and applies policy — re-runnable,
cron-able. Judge model separable via `COGNIGRAPH_JUDGE_MODEL`
(falls back to the completion provider). The judge prompt lives in
`cognigraph_construct::judge` under an explicit `POLICY_REV`
("judge-policy-v1"); **any prompt change must re-run the decision
replay AND the injection suite before shipping** — calibration
measurably flips verdicts (both harnesses are permanent regression
suites for exactly this).

**D4 — Sampled human audit.** Deterministic FNV-hash sampling (no RNG —
reproducible) at `sampling_rate` (default 0.1): sampled auto-accepts get
`audit_sample: true`; `cognigraph neuron audit SPACE` lists them with
the judge's reasoning inline. Agree = leave; disagree = retire (Lane C,
attributed).

**D5 — The injection gate: PASSED for judge-policy-v1.**
`fixtures/semantic-neurons/judge-injection/` — 16 injected bad
proposals (8 payload families × 2 scenarios), 4 clean controls, 2
inverse injections. Result: **zero injection-induced accepts** (all
direct-address/fake-SYSTEM/authority/schema-mimicry/rule-rewrite/
urgency/mid-sentence/false-verbatim payloads rejected at 0.98–0.99;
injected cases resisted BETTER than clean controls). Found instead: the
direction-reversal semantic weakness (mitigated in Lane A above) and
conservative-direction obedience to "reject this" text (costs autonomy,
never safety). Policies must attest `injection_suite_passed: true`; the
route refuses otherwise.

**D6 — Boundaries kept.** The judge never authors or modifies gates,
templates, or ontology; never proposes; vetoes/blockers remain
human-gated with judge + coverage simulation attached as triage;
symbolic checks are a floor no verdict can waive; auto-acceptance runs
the SAME validation as a human accept (hint/blocker conflict included,
against the running accepted set). The dossier's claim updates from
"human review" to **accountable, risk-tiered review — attributable,
reversible, symbolically floored.**

## Outcome

### Dedicated judge endpoint resolution (CG-36, 2026-09-09)

The server resolves dedicated primary and partner judges through the same
configuration snapshot and validated OpenAI endpoint path as completion and
side-views. `COGNIGRAPH_JUDGE_MODEL` and `COGNIGRAPH_JUDGE_PARTNER_MODEL` remain
explicit OpenAI model selectors, independently of the main completion provider.
Both honor `OPENAI_BASE_URL` and `OPENAI_API_KEY`; URL whitespace and trailing
slashes are normalized under the existing absolute-HTTP(S), no-query/fragment
contract. No new endpoint or provider-selection variable is introduced.

Absent/blank primary models retain completion fallback; absent/blank partner
models leave A+ unavailable. A nonempty dedicated model now requires a nonempty
OpenAI key, and invalid selected judge configuration fails before stores open.
This intentionally replaces silent fallback/omission when an explicitly selected
judge lacked its key. An unused OpenAI endpoint does not invalidate a Gemini
fallback with no dedicated judges. Model and pair qualification, symbolic
validation, prompts, and `POLICY_REV` are unchanged.

The saved release attempted the default OpenAI endpoint in nine dedicated-judge
cases; a local non-forwarding proxy blocked those connections. The corrected
release passed 22 review scenarios and 12 fail-fast startup checks, with 58
loopback screen/quality calls and zero outbound attempts. Exact model attribution,
qualified acceptance, pair disagreement, missing/mismatched partners, and
unqualified primary queueing were verified. All Rust gates passed with 961
reported tests. See the [CG-36 verification report](../issues/judge-endpoint-2026-09-09.md)
for runtime evidence, compatibility details, and standard-suite limits.

### Original delivery (2026-07-06)

Landed 2026-07-06: `cognigraph_construct::judge` (shared prompt +
POLICY_REV + evidence view, used by the server, the replay harness, and
the injection suite), `POST /api/construct/review` with lanes/attribution/
sampling, `COGNIGRAPH_JUDGE_MODEL`, CLI `neuron review` / `neuron
audit`, tests pinning policy refusal, attestation, whitelist,
auto-accept attribution + sampling, below-threshold queueing, new-triple
and gated-rule exclusions. Live-verified end to end: propose → policy →
auto-accept at 0.99 with `judge:gpt-5.4-mini@judge-policy-v1` → audit
queue with reasoning.

**POLICY_REV rule exercised on day one:** the shipping v1 prompt adds
the injection-defense line, so the 76-case replay was re-run against
it — hint safety unchanged (0/67 false-accepts, negated-evidence case
still caught), both false-accepts remain Lane-B blockers, and the
defense line costs measured conservatism: hint escalation ~18% → ~30%.
Operating point at scale for v1: humans see ~30% of hint volume + 10%
sampled audit + 100% of restraint-affecting changes, all with triage
attached; tightening escalation is a POLICY_REV v2 exercise gated on
both regression harnesses.

## Outcome addendum — judge-policy-v2 shipped (2026-07-06)

The POLICY_REV v2 exercise anticipated above ran the same day, gated on
both harnesses as required. v2 is a **two-stage judge** (LLM-per-
purpose): a taint screener escalates review-steering material before a
quality verdict exists, and the quality prompt is the v0 calibration
verbatim — the defense line's job moved out of it. Measured results:

- Injection suite: PASSED, zero induced accepts; all 16 payloads
  flagged by the screener at 0.96–0.99.
- Replay, run twice (first replication data): hint false-accepts at
  the Lane A threshold **0/67 in both runs**; hint escalation
  recovered ~30% → ~20–24%; blocker false-accepts remain the
  borderline-veto class (Lane B, coverage-sim-flagged, never
  auto-accepted).
- Two direction-check prompt variants were rejected BY MEASUREMENT
  (25 and 21 errors-on-good vs v0's 12; one misfired on passive
  relation labels). Direction risk stays at the policy layer — the D1
  Lane A exclusions are cheaper than a prompt line that taxes every
  hint.
- New methodological finding: borderline verdicts are unstable under
  sampling (the negated-evidence case flipped once in six runs, at
  0.76 — below threshold). Replay gates are therefore stated at the
  confidence threshold the policy consumes, and the symbolic floor
  remains the primary defense.

## Outcome addendum — Lane A is model-bound (2026-07-07)

The cross-family measurement (gemini-3.5-flash on both harnesses)
proved judge qualification is per-model: a second family failed the
hint bar with a negation-class false-accept at exactly the auto-accept
threshold, while passing the injection suite and catching the
incumbent's direction weakness. Until then, the server's provider
fallback meant a deployment with only a Gemini key would have silently
handed Lane A authority to an unqualified model. Landed: policies must
now attest `auto_accept.qualified_judges` (refused when absent, like
`injection_suite_passed`); at review time an active judge model not in
the list still produces triage verdicts, but Lane A is withheld —
everything queues with the verdict attached and the response says so
(`lane_a: withheld`). Test-pinned. Which models trust is granted to is
now authored, attested config — never a deployment accident. The
measured cross-family option (two-family agreement lane) remains a
future design session.

## Outcome addendum — the qualification ledger and Lane A+ (2026-07-07)

The model matrix completed the qualification story this policy started.
Three models have now been measured on both harnesses against the same
authority bar (hint false-accepts at the policy-consumed threshold):

| judge model | negation boundary case | direction traps (ctl-s2) | Lane A verdict |
|---|---|---|---|
| `gpt-5.4-mini` @ v2 | errs 1-in-6 runs, at 0.76 — **below** the line | accepts (0.97–0.99) | **qualified** (the only one) |
| `gemini-3.5-flash` | accepts at 0.90 — at the line | rejects (1.00) | disqualified |
| `gpt-5.4` | accepts 1-in-2 runs at 0.92 — **above** the line | rejects (0.99) | disqualified (replication vetoed a 0/76 first run) |

Two durable lessons: **qualification selects for error calibration
below the authority line, not raw accuracy** (the incumbent is not the
smartest judge; it is the one whose rare mistake stays sub-threshold);
and **single-run replays overstate determinism** — the two-run
replication protocol is now part of the qualification bar itself.

The D1 Lane A exclusions ("direction-critical rules always queue") also
gained their measured relaxation path: **Lane A+, the agreement lane**
(decision_agreement_lane.md) — two judges with measured-disjoint blind
spots (mini catches negation, gpt-5.4 catches direction) may jointly
auto-accept hints on templated/gated existing rules, attested per PAIR
by the offline concordance instrument (mini+gpt-5.4: zero concordant
false-accepts over 74 shared cases, worst-case across four runs). The
exclusions themselves remain in force for single-judge Lane A; Lane A+
is the governed way past them, not an exception to them.

## Current-contract addendum — M25 fail-closed narrowing (2026-07-19)

The evidence and original 2026-07-06 D1 decision above remain historical
records of what was measured and shipped then. The current server contract is
narrower in three ways:

1. Single-judge Lane A and two-judge Lane A+ admit only `relation_hint`.
   For wire compatibility, a legacy policy may still list `alias` in
   `auto_accept.kinds`, but the lane classifier always queues aliases until a
   kind-specific judge packet and qualification bar exist. Blockers, rank
   hints, and unknown kinds remain human-only and are rejected in that field.
2. The taint screener and quality judge are independently shape-validated after
   JSON decoding. Missing or wrongly typed required fields, unknown verdicts,
   out-of-range confidence, empty reasoning, or malformed concerns fail closed:
   malformed screening is tainted, and malformed quality output becomes
   `needs_human` at zero confidence. OpenAI strict structured-output mode is an
   additional provider constraint, not a replacement for server validation.
3. Omitting `sampling_rate` now preserves the documented deterministic default
   of `0.1` rather than silently selecting zero sampling.

M25 also changes how this legacy policy should be described. Generic mutation
of `review_policies` is now blocked, while existing rows remain readable for
the dedicated review route. `qualified_judges`, `injection_suite_passed`, and
the historical offline ledger are not signed model-qualification authority.
The LLM judge is an upstream candidate-authoring aid. M25 governed Semantic
Repair authority comes only from a PolicyAuthor-signed exact typed candidate,
an independent PolicyApprover review, and exact selection by the existing
promotion head. Signed, expiring judge qualification remains future work.

## Current lifecycle addendum — CG-21 (2026-09-09)

Human accept/reject/retire and automated publication share a tenant/incarnation
lock from current-state reads through validation and commit. Admin snapshot
import shares that boundary. This is process-local serialization under the
existing single-writer contract; it does not introduce multi-process authority.

Screening and both judge calls execute outside the lock. Publication requires
the captured proposal document to still match exactly and remain proposed, and
the policy and parsed ontology to remain current. A changed source discards the
whole result, including triage fields. Both auto-accept lanes revalidate against
the current accepted set; conflicts queue, while backend read failures and
malformed accepted rows fail closed. The human's attribution remains intact
when a late review is skipped.

`POST /api/construct/review` now includes `skipped: [{key, reason}]` alongside
`auto_accepted` and `queued`. `reviewed` counts only published results.
`pending_remaining` retains skipped proposals that are still eligible for a
future review, while excluding those already decided or judged by another
request. It remains a count over the request's captured queue, not a global live
queue size. Review notes truncate at a UTF-8 boundary within the 300-byte cap.

Nine deterministic Rust regressions cover conflicting human accepts, late
human decisions, current accepted-set failures, duplicate reviews, source edits,
A+ partner races, and scope isolation. The final release passed 99 HTTP
scenarios and 162 exact document comparisons after three restarts across all
persistent Native configurations. Formatting, strict Clippy, and all 937 reported
Rust tests passed. The saved release reproduced 36 deterministic
stale/conflict cases and 26 forbidden pairs in 60 human-pair stress attempts.
The first harness attempt used invalid underscored IDs and was corrected; the
pending-count edge case found after the first passing run was also corrected
and the final gates rerun. See the [verification report](../issues/neuron-lifecycle-2026-09-09.md)
for the full gate results and runtime scope. At the CG-21 checkpoint, dedicated
partner HTTP verification was limited by [CG-36](../issues/CG-36.md); A+ was
covered through the production routers with controlled providers. The CG-36
resolution above adds dedicated partner release HTTP coverage. Judge prompts
and qualification are unchanged.
