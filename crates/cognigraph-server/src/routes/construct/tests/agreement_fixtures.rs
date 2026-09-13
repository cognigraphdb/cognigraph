//! Agreement fixtures.

use super::*;

/// A space with one sentence-GATED rule (the Lane A+ class) plus its
/// policy carrying an attested agreement pair.
pub(super) async fn agreement_state(partner_says: Value) -> AppState {
    let backend = NativeBackend::new();
    backend
        .create_document(
            "space_types",
            json!({
                "_key": "pact", "id": "pact",
                "entities": [
                    {"name": "Nimbus", "type": "vendor", "aliases": []},
                    {"name": "DataCloud", "type": "platform", "aliases": []}
                ],
                "relation_rules": [
                    {"source": "Nimbus", "relation": "SUPPLIES", "target": "DataCloud",
                     "when_any": ["nimbus supplies datacloud"],
                     "require_in_sentence": ["source"]}
                ]
            }),
        )
        .await
        .unwrap();
    backend
        .create_document(
            REVIEW_POLICIES,
            json!({
                "_key": "pact",
                "auto_accept": {
                    "kinds": ["relation_hint"], "min_confidence": 0.9,
                    "qualified_judges": ["primary"],
                    "agreement": { "kinds": ["relation_hint"],
                                   "judges": ["primary", "partner"],
                                   "min_confidence": 0.9,
                                   "concordance_measured": true }
                },
                "sampling_rate": 1.0,
                "injection_suite_passed": true,
            }),
        )
        .await
        .unwrap();
    backend
        .create_document(
            NEURONS,
            json!({
                "_key": "h-gated", "id": "h-gated", "type": "relation_hint",
                "status": "proposed", "space_type": "pact", "confidence": 0.8,
                "evidence": ["chunk text"], "rationale": "test",
                "source": "Nimbus", "relation": "SUPPLIES", "target": "DataCloud",
                "triggers": ["datacloud ships from nimbus"],
            }),
        )
        .await
        .unwrap();
    AppState::new(backend)
        .with_judge(NamedScripted(
            json!({ "verdict": "accept", "confidence": 0.97, "reasoning": "verbatim, affirmed", "concerns": [] }),
            "primary",
        ))
        .with_judge_partner(NamedScripted(partner_says, "partner"))
}
