//! Review fixtures.

use super::*;

pub(super) async fn store_policy(state: &AppState, extra: Value) {
    let mut doc = json!({
        "_key": "acme",
        "auto_accept": { "kinds": ["relation_hint"], "min_confidence": 0.9,
                         "qualified_judges": ["scripted"] },
        "sampling_rate": 1.0,
        "injection_suite_passed": true,
    });
    if let (Some(base), Some(patch)) = (doc.as_object_mut(), extra.as_object()) {
        for (k, v) in patch {
            base.insert(k.clone(), v.clone());
        }
    }
    state
        .managed_backend
        .create_document(REVIEW_POLICIES, doc)
        .await
        .unwrap();
}
pub(super) async fn store_proposed(state: &AppState, id: &str, relation: &str) {
    state
        .managed_backend
        .create_document(
            NEURONS,
            json!({
                "_key": id, "id": id, "type": "relation_hint", "status": "proposed",
                "space_type": "acme", "confidence": 0.8,
                "evidence": ["chunk text"], "rationale": "test",
                "source": "Nimbus", "relation": relation, "target": "DataCloud",
                "triggers": ["nimbus hosts the datacloud platform"],
            }),
        )
        .await
        .unwrap();
}
