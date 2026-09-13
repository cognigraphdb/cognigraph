//! Review policy.

use super::*;

/// Authored, per-space review policy (decision_review_policy.md, D2).
/// Absent document = today's behavior: everything stays human-reviewed.
#[derive(serde::Deserialize)]
pub(super) struct ReviewPolicy {
    pub(super) auto_accept: AutoAcceptPolicy,
    #[serde(default = "default_sampling_rate")]
    pub(super) sampling_rate: f64,
    /// D5 attestation: the injection suite must have been run and passed
    /// for this policy revision before any auto-acceptance.
    #[serde(default)]
    pub(super) injection_suite_passed: bool,
}
pub(super) fn default_sampling_rate() -> f64 {
    0.1
}
#[derive(serde::Deserialize)]
pub(super) struct AutoAcceptPolicy {
    pub(super) kinds: Vec<String>,
    pub(super) min_confidence: f64,
    /// Lane A+ (decision_agreement_lane.md): concordance of two judges
    /// with measured-disjoint blind spots extends auto-accept to hints
    /// on templated/gated EXISTING rules — the direction-critical class
    /// Lane A excludes. Optional; absent = today's behavior.
    #[serde(default)]
    pub(super) agreement: Option<AgreementPolicy>,
    /// Judge models whose Lane A authority this policy attests — each
    /// must have passed BOTH harnesses (replay 0 hint false-accepts at
    /// threshold + injection zero induced accepts). An unlisted judge
    /// still runs as triage, but everything it reviews queues: the
    /// 2026-07-07 cross-family measurement proved qualification is
    /// per-model (a second family failed the hint bar at exactly the
    /// threshold), so authority cannot follow whatever provider a
    /// deployment happens to wire.
    #[serde(default)]
    pub(super) qualified_judges: Vec<String>,
}
#[derive(serde::Deserialize)]
pub(super) struct AgreementPolicy {
    /// Whitelist-validated: only `relation_hint` (the direction-critical
    /// class is a hint class; aliases are never direction-critical).
    pub(super) kinds: Vec<String>,
    /// Exactly two judge model names; the PAIR is the attested unit.
    pub(super) judges: Vec<String>,
    pub(super) min_confidence: f64,
    /// Attestation: the offline concordance simulation
    /// (`examples/concordance_sim.rs`) showed ZERO concordant
    /// false-accepts at threshold for this pair, worst-case across all
    /// recorded runs — refused when absent, like injection_suite_passed.
    #[serde(default)]
    pub(super) concordance_measured: bool,
}
#[derive(Deserialize)]
pub(super) struct ReviewRequest {
    pub(super) space_type: String,
    /// Judge at most this many pending proposals in one call (oldest
    /// key first, deterministic) — the pilot-scale slice: cron the call
    /// and the queue drains in bounded, resumable batches. Absent =
    /// the whole queue, today's behavior.
    pub(super) limit: Option<usize>,
    /// Re-judge proposals that already carry a verdict. Default false:
    /// a judged-and-queued proposal is awaiting a HUMAN, and re-judging
    /// it would burn calls and stall a limited drain on the same queue
    /// head. Set true after a POLICY_REV change or judge swap.
    #[serde(default)]
    pub(super) rejudge: bool,
}
/// Deterministic per-neuron sampling (FNV-1a of the id) — reproducible,
/// no RNG, so the same neuron is either always or never in the sample
/// for a given rate.
pub(super) fn sampled(id: &str, rate: f64) -> bool {
    let mut hash: u64 = 0xcbf29ce484222325;
    for byte in id.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    ((hash % 10_000) as f64) / 10_000.0 < rate
}
/// Lane A eligibility beyond the verdict (D1 + injection-suite finding):
/// only relation hints; a hint must target an EXISTING rule that is not
/// precision-critical (no template triggers, no sentence gates) — those
/// rules were marked by the author as leakage- or direction-sensitive
/// and their extensions always deserve human eyes. New-triple hints also
/// queue.
/// Lane classification for a proposal against the space's rules.
pub(super) enum LaneClass {
    /// Lane A: non-precision-critical — single qualified judge suffices.
    Eligible,
    /// Lane A+ candidate: hint on an EXISTING templated/gated rule —
    /// auto-acceptable only via two-judge concordance
    /// (decision_agreement_lane.md); otherwise queues.
    DirectionCritical(&'static str),
    /// Never auto-acceptable (new triple, blockers, rank hints).
    Excluded(&'static str),
}
pub(super) fn lane_a_eligible(neuron: &Neuron, space: &SpaceType) -> LaneClass {
    match neuron.kind {
        NeuronKind::Alias => LaneClass::Excluded(
            "alias auto-accept is disabled until a kind-specific judge packet is qualified",
        ),
        NeuronKind::RelationHint => {
            let Some(rule) = space.relation_rules.iter().find(|rule| {
                rule.source == neuron.source
                    && rule.relation == neuron.relation
                    && rule.target == neuron.target
            }) else {
                return LaneClass::Excluded("hint would create a new triple");
            };
            if !rule.require_in_sentence.is_empty() {
                return LaneClass::DirectionCritical("rule is sentence-gated (precision-critical)");
            }
            if rule
                .when_any
                .iter()
                .any(|p| p.contains("{source}") || p.contains("{target}"))
            {
                return LaneClass::DirectionCritical(
                    "rule uses template triggers (direction-critical)",
                );
            }
            LaneClass::Eligible
        }
        _ => LaneClass::Excluded("kind is never auto-acceptable"),
    }
}
