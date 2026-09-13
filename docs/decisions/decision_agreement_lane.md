# Decision: the agreement lane (Lane A+, D1–D6)

**Status:** Decided 2026-07-07 (user approved all six); landed same day.

## Context

Lane A's exclusions exist because the qualified judge is direction-blind
(gpt-5.4-mini accepts both possessive-appositive traps at 0.97–0.99), so
every hint on a templated or gated rule queues for a human. The model
matrix then measured two judges that CATCH direction (gpt-5.4 and
gemini-3.5-flash reject both traps at 0.99–1.00) but fail the negation
class at or above the authority threshold — disjoint blind spots. The
offline concordance simulation over every preserved verdict run
(`examples/concordance_sim.rs`): **mini+gpt-5.4 → 0 concordant
false-accepts at threshold, worst-case across all four runs (43/65 good
hints concordantly accepted); mini+gemini → 0 concordant FA (30/65).**
On the direction traps the partners reject what mini accepts, so
concordance queues them: the pair covers both measured weakness classes.

## Decisions (owner: user, 2026-07-07 design session)

**D1 — Extension, not replacement.** Lane A stays as-is (single
qualified judge on the non-precision-critical class, 0/67-at-threshold
measured). Lane A+ ADDS auto-accept for a currently-excluded class via
two-judge concordance — converting the measured disjointness into new
autonomy instead of paying double for held territory.

**D2 — Scope: hints on EXISTING templated/gated rules only.** Exactly
the class the direction-blind exclusion covers and the partner judge is
measured to catch. New-triple hints (vocabulary-adjacent) and all
blockers stay out — mini's own worst-case threshold errors are
blockers, so concordance is not clean cover there. *Honest caveat:* the
autonomy gain on this class is projected from general-hint concordance;
the replay corpus predates gates. First real traffic plus audit
sampling is the measurement.

**D3 — Concordance semantics.** Both judges run the full two-stage v2
pipeline independently. Auto-accept iff BOTH verdict=accept AND both
conf ≥ the agreement threshold; any screener taint escalates; any
disagreement queues with BOTH verdicts attached (richer triage).
Verdict-instability compounds down: a threshold-crossing flip must now
happen in both models on the same case.

**D4 — Schema and qualification, the attestation pattern again.**
`auto_accept.agreement: { kinds (relation_hint only,
whitelist-validated), judges (exactly two distinct model names — the
PAIR is the attested unit), min_confidence, concordance_measured }`.
Refused without `concordance_measured: true` (the concordance_sim
instrument must show zero concordant FA at threshold for the pair,
worst-case across runs). POLICY_REV rule applies twice: a prompt change
re-runs both harnesses for both members.

**D5 — Shipped default pairing: mini + gpt-5.4.** Same provider (no
cross-provider dependency; consistent with the standing Gemini stance),
native schema enforcement, 43/65 vs 30/65 concordant autonomy.
mini+gemini stays a measured, documented alternative for when
cross-FAMILY diversity is demanded — pairing is config, not code
(`COGNIGRAPH_JUDGE_PARTNER_MODEL`).

**D6 — Failure handling and attribution.** Partner unavailable or model
mismatch vs the attested pair → Lane A+ degrades to QUEUE (never to
single-judge for this class; response says `lane_a_plus: withheld`);
Lane A proper unaffected. A+ acceptances stamp
`reviewed_by: judges:<a>+<b>@POLICY_REV` with both verdicts stored on
the neuron; audit sampling applies at the policy rate. Cost: 4 LLM
calls per A+ proposal — replacing a human review each.

## Boundaries kept

Blockers never auto-accept; judges never reject; new-triple never
auto-accepts; the symbolic floor is unwaivable; everything attributed
and reversible.

## Outcome

Landed 2026-07-07: `AgreementPolicy` schema + validation (attestation,
pair-of-two, kind whitelist), `LaneClass` classification
(Eligible / DirectionCritical / Excluded), the A+ concordance path with
pair attribution and both-verdict recording,
`COGNIGRAPH_JUDGE_PARTNER_MODEL` wiring, degrade-to-queue on mismatch,
`concordance_sim` as the permanent attestation instrument (verified:
mini+gpt-5.4 = 0 concordant FA over 74 shared cases, worst-case across
four runs). Tests pin concordance-accept with pair attribution,
disagreement-queue with both verdicts, pair-mismatch withholding, and
attestation refusal.

The production server currently constructs the agreement partner through the
OpenAI completion provider. The mini+Gemini pair remains measured evidence and
a library-level alternative, but it is not selectable through server
configuration without additional provider wiring; D5's “pairing is config”
statement is therefore only fully true for models served by the wired provider.

The [CG-36 endpoint correction](../issues/judge-endpoint-2026-09-09.md) keeps
that provider boundary: both dedicated models now share the validated
`OPENAI_BASE_URL` and key instead of bypassing the configured endpoint. Pair
attestation, model matching, the two independent review pipelines, and queueing
on disagreement remain unchanged. An absent partner still withholds A+;
an explicit partner model without a nonempty OpenAI key now fails startup.
