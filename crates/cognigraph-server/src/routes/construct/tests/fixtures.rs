//! Fixtures.

use super::*;

pub(super) async fn seeded_state() -> AppState {
    let backend = NativeBackend::new();
    backend
        .create_document(
            "space_types",
            json!({
                "_key": "acme",
                "id": "acme",
                "entities": [
                    {"name": "Nimbus", "type": "vendor", "aliases": []},
                    {"name": "DataCloud", "type": "platform", "aliases": []}
                ],
                "relation_rules": [
                    {"source": "Nimbus", "relation": "SUPPLIES", "target": "DataCloud",
                     "when_any": ["datacloud runs on nimbus"]},
                    {"source": "Nimbus", "relation": "HOSTS", "target": "DataCloud",
                     "when_any": []}
                ]
            }),
        )
        .await
        .unwrap();
    AppState::new(backend)
}
pub(super) fn chunks(text: &str) -> Vec<Chunk> {
    vec![serde_json::from_value(json!({"id": "c1", "text": text})).unwrap()]
}
